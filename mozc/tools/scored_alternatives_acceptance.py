"""隔離SessionHandlerで採点済み代替候補の選択・表示・確定を確認する。実TSFではない。"""
import argparse
import json
import os
import re
from pathlib import Path
import sys
import tempfile
from first_display_compare import Session, romanize, sha256
from large_input_campaign import parse_output


def validate_cases(cases):
    if not isinstance(cases,list) or not cases:
        raise ValueError('empty cases')
    seen=set()
    for case in cases:
        if not isinstance(case,dict) or any(not isinstance(case.get(k),str) or not case[k]
                                           for k in ('index','reading','alternative','expected')):
            raise ValueError('incomplete cases')
        if not re.fullmatch('[A-Za-z0-9_-]+',case['index']) or case['index'] in seen:
            raise ValueError('invalid or repeated case index')
        if any(any(c in case[k] for c in '\r\n\t') for k in ('reading','alternative','expected')):
            raise ValueError('control character in case')
        seen.add(case['index'])


def main():
    ap=argparse.ArgumentParser(description=__doc__)
    for name in ('exe','identity','cases','install','proto','romanji-table','out'):
        ap.add_argument('--'+name,type=Path,required=True)
    ap.add_argument('--require-device',default='')
    args=ap.parse_args()
    for name in ('exe','identity','cases','install','proto','romanji_table','out'):
        setattr(args,name,getattr(args,name).resolve())
    identity=json.loads(args.identity.read_text('utf-8'))
    if sha256(args.exe)!=identity['session_handler_sha256']:
        raise ValueError('source identity and EXE SHA differ')
    if not re.fullmatch('[0-9a-f]{40}',identity['source_commit']):
        raise ValueError('source commit must be a fixed full SHA')
    env={k:v for k,v in os.environ.items() if not k.startswith('KOTORI_')}
    env.update(KOTORI_MODEL_DIR=str(args.install),KOTORI_RUNTIME_DIR=str(args.install),KOTORI_LM_DEBUG='1')
    cases=json.loads(args.cases.read_text('utf-8'))
    validate_cases(cases)
    sys.path.insert(0,str(args.proto))
    from google.protobuf import text_format
    from protocol.commands_pb2 import Output
    identities={str(p):sha256(p) for p in [args.exe,args.identity,args.cases,args.romanji_table,Path(__file__)]+sorted(args.install.glob('*.gguf'))+sorted(args.install.glob('*.dll'))}
    args.out.mkdir(parents=True,exist_ok=False)

    rows=[]
    for case in cases:
        index,reading,alternative,expected=(case[k] for k in ('index','reading','alternative','expected'))
        row=dict(index=index,failures=[],events=[])
        profile=Path(tempfile.mkdtemp(prefix='asteria-scored-alternative-'))
        session=Session(args.exe,env,profile,args.out/(index+'.stderr'))
        def send(command,conversion=False):
            event=session.send_conversion(command) if conversion else session.send(command)
            parsed=parse_output(event['output'],Output,text_format)
            parsed.update(command=command,ms=event['response_ms'],raw=event['output'])
            if conversion:
                parsed.update(operation_ms=event['operation_ms'],initial_event=event['initial_event'],callbacks=event['callback_events'])
                for pending in [event['initial_event'],*event['callback_events'][:-1]]:
                    item=parse_output(pending['output'],Output,text_format)
                    if item['callback_ms'] is not None and item['candidates']:
                        row['failures'].append('unscored_candidates_visible_while_pending')
            row['events'].append(parsed)
            return parsed
        failed=True
        try:
            send('SET_CONFIG\tkotori_lm_quality\tKOTORI_LM_STANDARD\nSET_CONFIG\tkotori_lm_model\tKOTORI_LM_MODEL_LLM\nSEND_KEY\tON\nSWITCH_COMPOSITION_MODE\tHIRAGANA')
            send('SEND_KEYS\t'+romanize(reading,args.romanji_table))
            converted=send('SEND_KEY\tSpace',True)
            row['first_display']=converted['value']
            focused=converted
            for _ in range(len(converted['segments'])-1):
                focused=send('SEND_KEY\tRight')
            row['available']=alternative in focused['candidates']
            if not row['available']:
                row['failures'].append('scored_alternative_missing')
            else:
                chosen=send('SELECT_CANDIDATE_BY_VALUE\t'+alternative)
                row['selected']=chosen['value']
                if chosen['value']!=expected: row['failures'].append('selection_display_mismatch')
                committed=send('SEND_KEY\tEnter')
                row['committed']=committed['result']
                if committed['result']!=expected: row['failures'].append('commit_differs_from_selection')
            failed=False
        except Exception as exc:
            row['failures'].append(type(exc).__name__+': '+str(exc))
        finally:
            session.close(failed)
            row['passed']=not row['failures']
            status=profile/'session/kotori_ai_status.txt'
            row['status']=status.read_text('utf-8') if status.exists() else ''
            if args.require_device and args.require_device not in row['status']:
                row['failures'].append('requested_device_not_observed')
                row['passed']=False
            text=(args.out/(index+'.stderr')).read_text('utf-8',errors='replace')
            row['model_score_lines']=sum(line.startswith('[kotori] ') and all(t in line for t in ['zenz=','llm=','score=']) for line in text.splitlines())
            if not row['model_score_lines']:
                row['failures'].append('real_model_scoring_not_observed');row['passed']=False
            rows.append(row)
            print(json.dumps({k:v for k,v in row.items() if k not in ['events','status']},ensure_ascii=False),flush=True)
    if any(sha256(Path(path))!=digest for path,digest in identities.items()):
        raise ValueError('binary/model/runtime/case/tool changed during verification')

    summary=dict(source_commit=identity['source_commit'],passed=all(r['passed'] for r in rows),scope='isolated native SessionHandler with real models; not installed TSF',
                 cases=rows,identities=identities)
    (args.out/'summary.json').write_text(json.dumps(summary,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
    return 0 if summary['passed'] else 1


if __name__=='__main__':
    sys.exit(main())
