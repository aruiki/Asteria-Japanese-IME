"""通常ローマ字/未完のn/編集/文字種/取消/候補選択を実セッションで検査する。"""
import argparse
import json
import os
from pathlib import Path
import sys
import tempfile
from first_display_compare import Session
from large_input_campaign import parse_output, norm, sha


def operation_cases():
    cases=[]
    def add(label, commands, reading=None, display=None):
        cases.append(dict(label=label,commands=commands,reading=reading,display=display))
    for keys,reading in [('shinbun','しんぶん'),('nihon','にほん'),('shashin','しゃしん'),
                         ('chotto','ちょっと'),('gakkou','がっこう'),('kitte','きって'),
                         ('shinnyuu','しんゆう'),('shinnnyuu','しんにゅう'),("shin'nyuu",'しんにゅう'),("shin'you",'しんよう'),('shinnbun','しんぶん')]:
        # 末尾nはSpaceが確定するので、入力中にんにならないことを欠落とは呼ばない。
        add('ordinary-romaji:'+keys,['SEND_KEYS\t'+keys,'SEND_KEY\tSpace'],reading)
    for label,commands in [
        ('incomplete-romaji-backspace',['SEND_KEYS\tsus','SEND_KEY\tBackSpace','SEND_KEYS\tshi']),
        ('kana-backspace',['SEND_KEYS\tsushi','SEND_KEY\tBackSpace','SEND_KEYS\tshi']),
        ('cursor-edit',['SEND_KEYS\tsushi','SEND_KEY\tLeft','SEND_KEY\tBackSpace','SEND_KEYS\tsu']),
        ('cancel-conversion',['SEND_KEYS\tsushi','SEND_KEY\tSpace','SEND_KEY\tESC']),
    ]: add(label,commands,reading='すし')
    for keys,reading,katakana in [('sushi','すし','スシ'),('gakkou','がっこう','ガッコウ'),('chotto','ちょっと','チョット'),
                                ("shin'you",'しんよう','シンヨウ'),('asteria','あsてりあ',None)]:
        # F10は生キー列の保持を確認。ラテン文字を含む入力も実際に通す。
        add('f10:'+keys,['SEND_KEYS\t'+keys,'SEND_KEY\tF10'],display=keys)
        if katakana: add('f7:'+keys,['SEND_KEYS\t'+keys,'SEND_KEY\tF7'],display=katakana)
    for keys in ['https://github.com','name@example.com','Asteria2026','C:/Users/test/file.txt','1.5GB','abc_123-xyz']:
        add('f10-mixed:'+keys,['SEND_KEYS\t'+keys,'SEND_KEY\tF10'],display=keys)
    for keys in ['sushi','chotto','shinbun','12345']:
        add('escape-clear:'+keys,['SEND_KEYS\t'+keys,'SEND_KEY\tESC'],reading='',display='')
    return cases


def main():
    ap=argparse.ArgumentParser(description=__doc__)
    for key in ['exe','proto','out']: ap.add_argument('--'+key,type=Path,required=True)
    args=ap.parse_args(); args.exe=args.exe.resolve(); args.out=args.out.resolve()
    if args.out.exists() and any(args.out.iterdir()): ap.error('use a fresh output directory')
    args.out.mkdir(parents=True,exist_ok=True)
    sys.path.insert(0,str(args.proto.resolve()))
    from protocol.commands_pb2 import Output
    from google.protobuf import text_format
    identity=sha(args.exe)
    env={k:v for k,v in os.environ.items() if not k.startswith('KOTORI_')}
    profile=Path(tempfile.mkdtemp(prefix='asteria-operations-'))
    # 操作の辞書対照。AI ONのTab/F6/確定は大量campaignが別途検査する。
    session=Session(args.exe,env,profile,args.out/'engine.stderr.txt')
    failed=True; rows=[]
    def send(command):
        e=session.send(command); p=parse_output(e['output'],Output,text_format)
        p.update(command=command,ms=e['response_ms'],raw=e['output']); return p
    try:
        send('SET_CONFIG\tkotori_lm_enabled\tfalse')
        for case in operation_cases():
            send('SEND_KEY\tESC\nSEND_KEY\tESC\nRESET_CONTEXT\nSEND_KEY\tON\nSWITCH_COMPOSITION_MODE\tHIRAGANA')
            events=[send(c) for c in case['commands']]; last=events[-1]; defects=[]
            if case['reading'] is not None and norm(last['key'])!=norm(case['reading']): defects.append('reading_mismatch')
            if case['display'] is not None and norm(last['value'])!=norm(case['display']): defects.append('display_mismatch')
            # Enterの結果を記録して、選択/文字種変更後の表示が確定で落ちないか調べる。
            commit=send('SEND_KEY\tEnter'); events.append(commit)
            if last['value'] and norm(commit['result'])!=norm(last['value']): defects.append('commit_mismatch')
            rows.append(dict(case,events=events,defects=defects))
            print(case['label'],repr(last['key']),repr(last['value']),defects,flush=True)
        failed=False
    finally: session.close(failed)
    if sha(args.exe)!=identity: raise RuntimeError('executable changed')
    (args.out/'results.json').write_text(json.dumps({'exe_sha256':identity,'native_tsf':False,'ai_enabled':False,
        'profile':str(profile),'complete':not failed,'scenarios':len(rows),'rows':rows},ensure_ascii=False,indent=2),encoding='utf-8')


if __name__=='__main__': main()
