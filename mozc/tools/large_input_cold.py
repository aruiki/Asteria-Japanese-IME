"""モデル読み込み前の即Spaceと読み込み後を新規隔離セッションで観測する。"""
import argparse
import json
import os
from pathlib import Path
import sys
import tempfile
import time
from first_display_compare import Session
from large_input_campaign import parse_output,assess,sha


def main():
    ap=argparse.ArgumentParser(description=__doc__)
    for name in ['exe','cases','proto','out']: ap.add_argument('--'+name,type=Path,required=True)
    ap.add_argument('--install',type=Path,default=Path('C:/Program Files (x86)/Kotori'))
    ap.add_argument('--cpu',action='store_true'); ap.add_argument('--count',type=int,default=20)
    args=ap.parse_args(); args.exe=args.exe.resolve(); args.out=args.out.resolve()
    if args.out.exists() and any(args.out.iterdir()): ap.error('use fresh output')
    args.out.mkdir(parents=True,exist_ok=True)
    sys.path.insert(0,str(args.proto.resolve()))
    from protocol.commands_pb2 import Output
    from google.protobuf import text_format
    cases=[json.loads(l) for l in args.cases.read_text(encoding='utf-8').splitlines() if l][:args.count]
    identity=sha(args.exe); profile=Path(tempfile.mkdtemp(prefix='asteria-cold-'))
    env={k:v for k,v in os.environ.items() if not k.startswith('KOTORI_')}
    env.update(KOTORI_MODEL_DIR=str(args.install),KOTORI_RUNTIME_DIR=str(args.install),KOTORI_LM_TIME='1')
    if args.cpu: env['KOTORI_LM_DEVICE']='cpu'
    session=Session(args.exe,env,profile,args.out/'engine.stderr.txt'); failed=True; rows=[]
    status=profile/'session/kotori_ai_status.txt'
    def state(): return status.read_text(encoding='utf-8') if status.exists() else ''
    def send(command):
        event=session.send(command); parsed=parse_output(event['output'],Output,text_format)
        parsed.update(ms=event['response_ms'],raw=event['output'],command=command); return parsed
    started=time.monotonic()
    try:
        send('SET_CONFIG\tkotori_lm_enabled\ttrue\nSET_CONFIG\tkotori_lm_quality\tKOTORI_LM_STANDARD')
        for phase in ['no-warmup','loaded']:
            if phase=='loaded':
                deadline=time.monotonic()+30
                while 'state=llm' not in state() and 'state=zenz' not in state():
                    if time.monotonic()>deadline: raise RuntimeError('AI failed to load')
                    time.sleep(.1)
            for case in cases:
                send('SEND_KEY\tESC\nSEND_KEY\tESC\nRESET_CONTEXT\nSEND_KEY\tON\nSWITCH_COMPOSITION_MODE\tHIRAGANA')
                before=state(); typed=send('SEND_KEYS\t'+case['romaji']); space=send('SEND_KEY\tSpace'); commit=send('SEND_KEY\tEnter')
                events={'typed':typed,'space':space,'commit':commit}; defects,reviews=assess(case,events)
                row=dict(case_id=case['id'],phase=phase,reading=case['reading'],events=events,defects=defects,reviews=reviews,
                         status_before=before,status_after=state(),seconds_since_start=time.monotonic()-started)
                rows.append(row)
                with (args.out/'results.jsonl').open('a',encoding='utf-8') as f: f.write(json.dumps(row,ensure_ascii=False)+'\n')
                print(phase,case['reading'],space['value'],round(space['ms'],1),defects,flush=True)
        failed=False
    finally: session.close(failed)
    if sha(args.exe)!=identity: raise RuntimeError('executable changed')
    (args.out/'summary.json').write_text(json.dumps(dict(complete=not failed,exe_sha256=identity,cases_sha256=sha(args.cases),
        native_tsf=False,cpu=args.cpu,profile=str(profile),scenarios=len(rows),max_space_ms=max(r['events']['space']['ms'] for r in rows),
        no_warmup_statuses=[r['status_before'] for r in rows if r['phase']=='no-warmup']),ensure_ascii=False,indent=2),encoding='utf-8')


if __name__=='__main__': main()
