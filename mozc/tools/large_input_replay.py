"""大量実行の疑義を新規隔離プロファイルで再現し、AI OFF/ONを比較する。"""
import argparse
import json
import os
from pathlib import Path
import sys
import tempfile
import time
from first_display_compare import Session
from large_input_campaign import parse_output, assess, sha


def selected_cases(cases, wanted):
    selected = [case for case in cases if case['id'] in wanted]
    if not wanted or {case['id'] for case in selected} != wanted:
        raise ValueError('empty or unknown selected IDs')
    if len(selected) != len(wanted):
        raise ValueError('duplicate selected IDs in corpus')
    return selected


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    for name in ['exe', 'cases', 'proto', 'out']:
        ap.add_argument('--' + name, type=Path, required=True)
    selection = ap.add_mutually_exclusive_group(required=True)
    selection.add_argument('--ids', help='case IDs separated by comma')
    selection.add_argument('--selected', type=Path)
    ap.add_argument('--install', type=Path, default=Path(('C:/Program Files (x86)/Asteria' if os.path.isdir('C:/Program Files (x86)/Asteria') else 'C:/Program Files (x86)/Kotori')))
    ap.add_argument('--key-interval', type=float, default=.15)
    ap.add_argument('--settle', type=float, default=2.4)
    ap.add_argument('--cpu', action='store_true')
    ap.add_argument('--warm', type=float, default=10)
    ap.add_argument('--candidate-only', action='store_true', help='surname Tab/Space paging; no slow typing')
    args = ap.parse_args()
    args.exe = args.exe.resolve(); args.out = args.out.resolve()
    if args.out.exists() and any(args.out.iterdir()):
        ap.error('output must be empty; never append an unrelated replay')
    args.out.mkdir(parents=True, exist_ok=True)
    sys.path.insert(0, str(args.proto.resolve()))
    from protocol.commands_pb2 import Output
    from google.protobuf import text_format
    all_cases = [json.loads(line) for line in args.cases.read_text(encoding='utf-8').splitlines() if line]
    wanted = set(args.ids.split(',')) if args.ids else set(json.loads(args.selected.read_text(encoding='utf-8')))
    cases = selected_cases(all_cases, wanted)
    exe_identity = sha(args.exe)
    runtime_files = list(args.install.glob('*.gguf')) + list(args.install.glob('*.dll'))
    runtime_identity = {p.name: sha(p) for p in runtime_files}
    manifest = {'exe_sha256': exe_identity, 'cases_sha256': sha(args.cases), 'runtime_sha256': runtime_identity,
                'unique_inputs': len(cases), 'conditions_per_ai_mode': 1 if args.candidate_only else 3,
                'key_interval_seconds': args.key_interval, 'settle_seconds': args.settle,
                'cpu': args.cpu, 'native_tsf': False, 'commit_tested': False,
                'selected_ids': [c['id'] for c in cases], 'candidate_only': args.candidate_only,
                'fresh_profile_per_ai_mode_and_condition': True}
    (args.out/'manifest.json').write_text(json.dumps(manifest, ensure_ascii=False, indent=2), encoding='utf-8')
    collected = {}
    conditions = ['burst'] if args.candidate_only else ['burst', 'settled', 'human-typing']
    for enabled, condition in [(enabled, condition) for enabled in [False, True] for condition in conditions]:
        mode = 'on' if enabled else 'off'
        label = mode + '-' + condition
        env = {k: v for k, v in os.environ.items() if not k.startswith('KOTORI_')}
        env.update(KOTORI_MODEL_DIR=str(args.install), KOTORI_RUNTIME_DIR=str(args.install),
                   KOTORI_LM_TIME='1', KOTORI_LM_DEBUG='1')
        if args.cpu: env['KOTORI_LM_DEVICE'] = 'cpu'
        profile = Path(tempfile.mkdtemp(prefix='asteria-replay-' + label + '-'))
        s = Session(args.exe, env, profile, args.out/(label + '.stderr.txt'))
        rows = []; failed = True; state = ''; config = None
        status = profile/'session/kotori_ai_status.txt'

        def send(command):
            e = s.send(command)
            p = parse_output(e['output'], Output, text_format)
            p.update(ms=e['response_ms'], raw=e['output'], command=command)
            return p

        def reset():
            send('SEND_KEY\tESC\nSEND_KEY\tESC\nRESET_CONTEXT\nSEND_KEY\tON\nSWITCH_COMPOSITION_MODE\tHIRAGANA')

        def type_case(case, slow):
            keys = case['romaji']; event = None
            if case.get('preceding'):
                event = send('SEND_KEY_WITH_OPTION\t' + keys[0] + '\tcontext.preceding_text=' + json.dumps(case['preceding'], ensure_ascii=False))
                keys = keys[1:]
            if not slow and keys: return send('SEND_KEYS\t' + keys)
            for key in keys:
                event = send('SEND_KEYS\t' + key)
                if args.key_interval: time.sleep(args.key_interval)
            return event

        def collect_pages(first):
            pages = [first]
            # 最初の4ページ。全候補の応答も保持するが、複数文節の全文順位とは呼ばない。
            for _ in range(3): pages.append(send('SEND_KEY\tPageDown'))
            visible = sorted({v for page in pages for v in page['candidates']})
            window = sorted({candidate['value'] for page in pages for candidate in page['candidate_window']})
            return {'pages': pages, 'observed_values': visible, 'window_values': window}

        try:
            config = send('SET_CONFIG\tkotori_lm_enabled\t' + ('true' if enabled else 'false') +
                          '\nSET_CONFIG\tkotori_lm_quality\tKOTORI_LM_STANDARD\nSEND_KEY\tON\nSEND_KEYS\tai')
            # コマンド拒否はSession.send/parse_outputで失敗する。OFF/ON設定の原応答を保存する。
            time.sleep(args.warm); reset()
            state = status.read_text(encoding='utf-8') if status.exists() else ''
            if enabled and 'state=llm' not in state and 'state=zenz' not in state:
                raise RuntimeError('AI not ready')
            for index, case in enumerate(cases):
                reset(); typed = type_case(case, slow=(condition == 'human-typing'))
                events = {'typed': typed}
                if condition != 'burst':
                    current = typed; deadline = time.monotonic() + args.settle
                    while current['callback_ms'] is not None and time.monotonic() < deadline:
                        time.sleep(min(current['callback_ms']/1000, max(0, deadline-time.monotonic())))
                        current = send('KOTORI_REFRESH'); events['settled'] = current
                events['space'] = send('SEND_KEY\tSpace')
                # 意味の検査では確定しない。誤変換を学習して次の条件へ持ち越さない。
                defects, reviews = assess(case, events, check_commit=False)
                row = {'id': case['id'], 'reading': case['reading'], 'family': case['family'], 'condition': condition,
                       'ai_enabled': enabled, 'commit_tested': False, 'expected': case['expected'],
                       'predicates': case['predicates'], 'events': events, 'defects': defects, 'reviews': reviews}
                if case.get('surname_targets'):
                    def selection(collection):
                        available = sorted(set(collection['pages'][-1]['candidates']).intersection(case['surname_targets']))
                        if not available: return None
                        target = available[0]
                        event = send('SELECT_CANDIDATE_BY_VALUE\t' + target)
                        return {'target': target, 'event': event, 'full_preedit_matches_target': event['value'] == target}
                    conversion = collect_pages(events['space'])
                    conversion['selection'] = selection(conversion)
                    reset(); type_case(case, slow=False)
                    prediction = collect_pages(send('SEND_KEY\tTab'))
                    prediction['selection'] = selection(prediction)
                    observed = set(conversion['observed_values']) | set(prediction['observed_values'])
                    window = set(conversion['window_values']) | set(prediction['window_values'])
                    row['candidate_access'] = {'conversion': conversion, 'prediction': prediction,
                        'targets': case['surname_targets'], 'matched_targets': sorted(observed.intersection(case['surname_targets'])),
                        'matched_window_targets': sorted(window.intersection(case['surname_targets']))}
                rows.append(row)
                with (args.out/(mode + '.jsonl')).open('a', encoding='utf-8') as f:
                    f.write(json.dumps(row, ensure_ascii=False) + '\n')
                print(label, index + 1, '/', len(cases), case['reading'], rows[-1]['events']['space']['value'], flush=True)
            failed = False
        finally:
            s.close(failed)
            (args.out/(label + '.status.json')).write_text(json.dumps({'profile': str(profile),
                'initial_status': state, 'config_event': config, 'final_status': status.read_text(encoding='utf-8') if status.exists() else '',
                'complete': not failed}, ensure_ascii=False, indent=2), encoding='utf-8')
        collected.setdefault(mode, []).extend(rows)
    if sha(args.exe) != exe_identity or any(sha(p) != runtime_identity[p.name] for p in runtime_files):
        raise RuntimeError('executable or runtime changed')
    if len(collected['off']) != len(collected['on']): raise RuntimeError('pair count differs')
    differences = []
    for a, b in zip(collected['off'], collected['on']):
        if (a['id'], a['condition']) != (b['id'], b['condition']): raise RuntimeError('pair identity differs')
        if a['events']['space']['value'] != b['events']['space']['value']:
            differences.append({'id': a['id'], 'condition': a['condition'], 'reading': a['reading'],
                                'off': a['events']['space']['value'], 'on': b['events']['space']['value'],
                                'off_reviews': a['reviews'], 'on_reviews': b['reviews']})
    (args.out/'comparison.json').write_text(json.dumps(dict(manifest, differences=differences, complete=True),
                                                        ensure_ascii=False, indent=2), encoding='utf-8')
    print('ON/OFF differences', len(differences), flush=True)


if __name__ == '__main__': main()
