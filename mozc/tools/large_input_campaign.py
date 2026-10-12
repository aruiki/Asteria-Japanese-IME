"""実辞書・モデルの隔離セッションで万単位の入力を実行する。

Windows TSFの画面操作とは別。実打鍵はSessionHandlerに通し、原応答と
途中結果をJSONL/gzipへ逐次保存する。正解のない読みは意味の採点をしない。
"""
import argparse
import collections
import gzip
import hashlib
import json
import os
from pathlib import Path
import random
import sys
import tempfile
import time
import unicodedata
from first_display_compare import Session


def sha(path):
    with Path(path).open('rb') as f: return hashlib.file_digest(f,'sha256').hexdigest()


def norm(text):
    return unicodedata.normalize('NFKC',text)


def parse_output(text, output_type, text_format):
    text=text.strip()
    if text.split('\n',1)[0] in {'goo.gle/debugproto','goo.gle/debugonly','goo.gle/debugstr'}:
        text=text.split('\n',1)[1]
    message=output_type()
    text_format.Parse(text,message)
    if message.error_code:
        raise RuntimeError('native session error_code='+str(message.error_code))
    values=[c.value for c in message.all_candidate_words.candidates]
    if not values: values=[c.value for c in message.candidate_window.candidate]
    return {'key':''.join(s.key for s in message.preedit.segment),
            'value':''.join(s.value for s in message.preedit.segment),
            'segments':[{'key':s.key,'value':s.value,'annotation':s.DESCRIPTOR.fields_by_name['annotation'].enum_type.values_by_number[s.annotation].name} for s in message.preedit.segment],
            'result':message.result.value,'candidates':values,
            'callback_ms':message.callback.delay_millisec if message.HasField('callback') else None,
            'consumed':message.consumed,
            'candidate_window_size':message.candidate_window.size,
            'candidate_window':[{'value':c.value,'id':c.id,'index':c.index} for c in message.candidate_window.candidate]}


def assess(case, events, check_commit=True):
    defects=[]; reviews=[]; typed=events['typed']; converted=events['space']; commit=events.get('commit')
    if check_commit and commit is None: raise ValueError('commit event missing')
    if norm(typed['key']) != norm(case['reading']):
        defects.append('typed_reading_mismatch')
    if not converted['value'] and not converted['result']:
        defects.append('space_empty')
    if check_commit and converted['value'] and norm(commit['result']) != norm(converted['value']):
        defects.append('commit_different_from_preedit')
    if events['space']['ms']>3000: defects.append('space_over_3000ms')
    value=converted['value'] or converted['result']
    if case['expected'] and norm(value) not in {norm(v) for v in case['expected']}:
        reviews.append('expected_surface_mismatch')
    for p in case['predicates']:
        if p['command']=='Conversion Match' and norm(p['value']) not in norm(value):
            reviews.append('upstream_match_missing')
        elif p['command']=='Conversion Not Match' and norm(p['value']) in norm(value):
            reviews.append('upstream_forbidden_surface')
    # 候補順位の原典は文節をまたぐ順位を含む。UIの一文節だけを見て誤りと断定しない。
    if case.get('surname_targets'):
        visible=set(events['typed']['candidates'])|set(events['space']['candidates'])
        if not any(v in visible for v in case['surname_targets']):
            reviews.append('surname_not_observed_in_exposed_candidates')
        elif set(case['surname_targets']) - visible:
            reviews.append('surname_some_variants_not_observed')
    return defects,sorted(set(reviews))


def compact(event):
    return {k:v for k,v in event.items() if k!='raw'}


def main():
    ap=argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--exe',type=Path,required=True); ap.add_argument('--cases',type=Path,required=True)
    ap.add_argument('--proto',type=Path,required=True); ap.add_argument('--install',type=Path,default=Path('C:/Program Files (x86)/Kotori'))
    ap.add_argument('--out',type=Path,required=True); ap.add_argument('--limit',type=int)
    ap.add_argument('--interaction-samples',type=int,default=400); ap.add_argument('--settle',type=float,default=2.4)
    ap.add_argument('--key-interval',type=float,default=.025); ap.add_argument('--cpu',action='store_true')
    ap.add_argument('--resume',action='store_true'); ap.add_argument('--warm',type=float,default=12)
    args=ap.parse_args(); args.exe=args.exe.resolve(); args.install=args.install.resolve(); args.out=args.out.resolve(); args.cases=args.cases.resolve()
    sys.path.insert(0,str(args.proto.resolve()))
    from protocol.commands_pb2 import Output
    from google.protobuf import text_format
    cases=[json.loads(l) for l in args.cases.read_text(encoding='utf-8').splitlines() if l]
    if args.limit is not None: cases=cases[:args.limit]
    if not cases: ap.error('empty cases')
    args.out.mkdir(parents=True,exist_ok=True)
    manifest_path=args.out/'manifest.json'; rows_path=args.out/'results.jsonl'
    identity={'exe_sha256':sha(args.exe),'cases_sha256':sha(args.cases),'cpu':args.cpu,
              'settle_seconds':args.settle,'key_interval':args.key_interval,
              'interaction_samples':args.interaction_samples,'limit':args.limit}
    done=set(); counts=collections.Counter(); families=collections.Counter(); review_counts=collections.Counter(); defect_counts=collections.Counter()
    if rows_path.exists():
        if not args.resume: ap.error('results already exist; use --resume or a fresh directory')
        previous=json.loads(manifest_path.read_text(encoding='utf-8'))
        if previous['identity'] != identity: raise RuntimeError('resume identity differs')
        for line in rows_path.read_text(encoding='utf-8').splitlines():
            if not line: continue
            r=json.loads(line); done.add(r['scenario_id']); counts[r['mode']]+=1
            families[r['family']]+=1; defect_counts.update(r['defects']); review_counts.update(r['reviews'])
    env={k:v for k,v in os.environ.items() if not k.startswith('KOTORI_')}
    env.update(KOTORI_MODEL_DIR=str(args.install),KOTORI_RUNTIME_DIR=str(args.install),KOTORI_LM_TIME='1')
    if args.cpu: env['KOTORI_LM_DEVICE']='cpu'
    model_hash={p.name:sha(p) for p in args.install.glob('*.gguf')}
    manifest={'identity':identity,'native_tsf':False,'scope':'isolated native Mozc SessionHandler, real romaji key events and models',
              'source_commit':os.popen('git rev-parse HEAD').read().strip(),'models':model_hash,
              'unique_inputs_requested':len(cases),'families_requested':dict(collections.Counter(c['family'] for c in cases)),
              'expected_surface_is_strict_and_needs_review':True}
    if args.resume and previous['models']!=model_hash: raise RuntimeError('resume model identity differs')
    manifest_path.write_text(json.dumps(manifest,ensure_ascii=False,indent=2),encoding='utf-8')
    # WindowsのテストCLIのargvはANSIなので、プロファイル引数だけASCIIの一時領域に隔離する。
    profile=Path(tempfile.gettempdir())/('asteria-campaign-'+hashlib.sha256(str(args.out).encode()).hexdigest()[:20])
    profile.mkdir(exist_ok=True)
    owner=profile/'owner.json'
    ownership={'output':str(args.out),'exe_sha256':identity['exe_sha256'],'cases_sha256':identity['cases_sha256']}
    if owner.exists() and json.loads(owner.read_text(encoding='utf-8'))!=ownership:
        raise RuntimeError('profile ownership differs')
    owner.write_text(json.dumps(ownership),encoding='utf-8')
    manifest['profile']=str(profile)
    session=Session(args.exe,env,profile,args.out/'engine.stderr.txt')
    raw=gzip.open(args.out/'raw-events.jsonl.gz','at',encoding='utf-8'); results=rows_path.open('a',encoding='utf-8')
    started=time.monotonic(); command_count=0; physical_keys=0; case_started=0
    def send(command):
        nonlocal command_count
        command_count+=1
        e=session.send(command); p=parse_output(e['output'],Output,text_format)
        p.update(ms=e['response_ms'],raw=e['output'],command=command)
        return p
    def reset():
        send('SEND_KEY\tESC\nSEND_KEY\tESC\nRESET_CONTEXT\nSEND_KEY\tON\nSWITCH_COMPOSITION_MODE\tHIRAGANA')
    def type_case(case, slow=False):
        nonlocal physical_keys
        keys=case['romaji']; physical_keys+=len(keys)
        if case.get('preceding'):
            first=send('SEND_KEY_WITH_OPTION\t'+keys[0]+'\tcontext.preceding_text='+json.dumps(case['preceding'],ensure_ascii=False))
            keys=keys[1:]
        if not slow and keys: return send('SEND_KEYS\t'+keys)
        for key in keys:
            first=send('SEND_KEYS\t'+key)
            if args.key_interval: time.sleep(args.key_interval)
        return first
    def save(case,mode,events,defects,reviews):
        scenario_id=case['id']+':'+mode
        row={'scenario_id':scenario_id,'case_id':case['id'],'family':case['family'],'reading':case['reading'],
             'mode':mode,'expected':case['expected'],'defects':defects,'reviews':reviews,
             'events':{k:compact(v) for k,v in events.items()}}
        results.write(json.dumps(row,ensure_ascii=False)+'\n'); results.flush()
        raw.write(json.dumps({'scenario_id':scenario_id,'events':events},ensure_ascii=False)+'\n'); raw.flush()
        done.add(scenario_id); counts[mode]+=1; families[case['family']]+=1
        defect_counts.update(defects); review_counts.update(reviews)
    failed=True
    try:
        send('SET_CONFIG\tkotori_lm_quality\tKOTORI_LM_STANDARD\nSET_CONFIG\tkotori_lm_model\tKOTORI_LM_MODEL_LLM\nSEND_KEY\tON\nSEND_KEYS\tai')
        time.sleep(args.warm); reset()
        status=profile/'session/kotori_ai_status.txt'
        state=status.read_text(encoding='utf-8') if status.exists() else ''
        manifest['initial_status']=state
        manifest_path.write_text(json.dumps(manifest,ensure_ascii=False,indent=2),encoding='utf-8')
        if 'state=llm' not in state and 'state=zenz' not in state:
            raise RuntimeError('AI not loaded: '+state)
        print('AI READY',state.replace('\n',' '),flush=True)
        for index,case in enumerate(cases):
            if case['id']+':immediate' in done: continue
            case_started=index
            reset(); typed=type_case(case); space=send('SEND_KEY\tSpace'); commit=send('SEND_KEY\tEnter')
            events={'typed':typed,'space':space,'commit':commit}
            defects,reviews=assess(case,events); save(case,'immediate',events,defects,reviews)
            if (index+1)%100==0 or index+1==len(cases):
                if sha(args.exe)!=identity['exe_sha256']: raise RuntimeError('executable changed during campaign')
                summary={'phase':'immediate','done':counts['immediate'],'total':len(cases),'seconds':round(time.monotonic()-started),
                         'defects':dict(defect_counts),'reviews':dict(review_counts),'commands':command_count,'romaji_key_events':physical_keys}
                (args.out/'progress.json').write_text(json.dumps(summary,ensure_ascii=False,indent=2),encoding='utf-8')
                print(json.dumps(summary,ensure_ascii=False),flush=True)
        # 各分野からサンプルを選び、遅い打鍵/非同期更新/Tab/取消/文字種を操作する。
        grouped=collections.defaultdict(list)
        for case in cases: grouped[case['family']].append(case)
        rng=random.Random(20261012); samples=[]
        per=max(1,args.interaction_samples//len(grouped)) if args.interaction_samples else 0
        for group in grouped.values():
            rng.shuffle(group); samples.extend(group[:per])
        samples=samples[:args.interaction_samples]
        for index,case in enumerate(samples):
            for mode in ['settled','tab-cancel','f6-reconvert']:
                if case['id']+':'+mode in done: continue
                reset(); typed=type_case(case,slow=(mode=='settled')); events={'typed':typed}; defects=[]; reviews=[]
                if mode=='settled':
                    current=typed; deadline=time.monotonic()+args.settle
                    while current['callback_ms'] is not None and time.monotonic()<deadline:
                        delay=current['callback_ms']/1000
                        time.sleep(min(delay,max(0,deadline-time.monotonic())))
                        current=send('KOTORI_REFRESH'); events['settled']=current
                    events['space']=send('SEND_KEY\tSpace'); events['commit']=send('SEND_KEY\tEnter')
                    defects,reviews=assess(case,events)
                    # 継続する更新予約は候補の失敗ではない。callbackは観測値として残す。
                elif mode=='tab-cancel':
                    events['tab']=send('SEND_KEY\tTab'); events['cancel']=send('SEND_KEY\tESC')
                    if norm(events['cancel']['key'])!=norm(case['reading']): reviews.append('tab_cancel_reading_changed')
                    events['space']=send('SEND_KEY\tSpace'); events['commit']=send('SEND_KEY\tEnter')
                    defects,reviews2=assess(case,events); reviews.extend(reviews2)
                else:
                    events['space']=send('SEND_KEY\tSpace'); events['space_before_f6']=events['space']
                    events['f6']=send('SEND_KEY\tF6')
                    before_segments=events['space']['segments']; after_segments=events['f6']['segments']
                    if [s['key'] for s in before_segments]==[s['key'] for s in after_segments]:
                        for before,after in zip(before_segments,after_segments):
                            if before['annotation']=='HIGHLIGHT' and norm(after['value'])!=norm(before['key']):
                                reviews.append('f6_focused_segment_not_reading')
                    else:
                        reviews.append('f6_segmentation_changed_needs_review')
                    events['cancel']=send('SEND_KEY\tESC'); events['space']=send('SEND_KEY\tSpace')
                    events['commit']=send('SEND_KEY\tEnter'); defects,reviews2=assess(case,events); reviews.extend(reviews2)
                save(case,mode,events,defects,reviews)
            if (index+1)%10==0 or index+1==len(samples):
                progress={'phase':'interactions','samples_done':index+1,'total':len(samples),'counts':dict(counts),'seconds':round(time.monotonic()-started)}
                (args.out/'progress.json').write_text(json.dumps(progress,ensure_ascii=False,indent=2),encoding='utf-8')
                print(json.dumps(progress,ensure_ascii=False),flush=True)
        failed=False
    except BaseException as exc:
        (args.out/'fatal.json').write_text(json.dumps({'case_index':case_started,'error':repr(exc),'counts':dict(counts)},ensure_ascii=False,indent=2),encoding='utf-8')
        raise
    finally:
        results.close(); raw.close(); session.close(failed)
        summary={'complete':not failed,'native_tsf':False,'counts':dict(counts),'families_executed':dict(families),
                 'defects':dict(defect_counts),'reviews':dict(review_counts),'seconds_this_run':round(time.monotonic()-started),
                 'commands_this_run':command_count,'romaji_key_events_this_run':physical_keys,'exe_unchanged':sha(args.exe)==identity['exe_sha256']}
        (args.out/'summary.json').write_text(json.dumps(summary,ensure_ascii=False,indent=2),encoding='utf-8')
        print('FINAL',json.dumps(summary,ensure_ascii=False),flush=True)


if __name__=='__main__': main()