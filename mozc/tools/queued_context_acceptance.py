"""周囲文付きの確定待ち・後続入力を隔離SessionHandlerと実モデルで確認する。実TSFではない。"""
import argparse
import json
import os
from pathlib import Path
import sys
import tempfile
import time

from first_display_compare import Session, romanize, sha256
from large_input_campaign import parse_output


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    for name in ('exe', 'identity', 'install', 'proto', 'romanji-table', 'out'):
        ap.add_argument('--' + name, type=Path, required=True)
    ap.add_argument('--require-device', default='', help='実測statusのdeviceに必要な文字列')
    args = ap.parse_args()
    for name in ('exe', 'identity', 'install', 'proto', 'romanji_table', 'out'):
        setattr(args, name, getattr(args, name).resolve())
    identity = json.loads(args.identity.read_text(encoding='utf-8'))
    if sha256(args.exe) != identity['session_handler_sha256']:
        raise ValueError('固定source receiptとSessionHandlerのSHAが一致しません')
    sys.path.insert(0, str(args.proto))
    from google.protobuf import text_format
    from protocol.commands_pb2 import Output
    args.out.mkdir(parents=True, exist_ok=False)
    identities = {str(p): sha256(p) for p in
                  [args.exe, args.identity, args.romanji_table, Path(__file__)] +
                  sorted(args.install.glob('*.gguf')) + sorted(args.install.glob('*.dll'))}
    env = {k: v for k, v in os.environ.items() if not k.startswith('KOTORI_')}
    env.update(KOTORI_MODEL_DIR=str(args.install), KOTORI_RUNTIME_DIR=str(args.install),
               KOTORI_LM_DEBUG='1')
    profile = Path(tempfile.mkdtemp(prefix='asteria-queued-context-'))
    stderr = args.out / 'engine.stderr.txt'
    session = Session(args.exe, env, profile, stderr)
    events, failures = [], []
    preceding, following = '会議の日程を相談しています。', 'あとで連絡します。'

    def send(command):
        value = session.send(command)
        event = parse_output(value['output'], Output, text_format)
        event.update(command=command, ms=value['response_ms'], raw=value['output'])
        events.append(event)
        return event

    def key_with_context(key):
        # ホストがまだ最初の確定を反映していない時点の周囲文を渡す。
        return send('SEND_KEY_WITH_OPTION\t' + key +
                    '\tcontext.preceding_text=' + json.dumps(preceding, ensure_ascii=False) +
                    '\tcontext.following_text=' + json.dumps(following, ensure_ascii=False))

    def check(condition, reason):
        if not condition:
            failures.append(reason)

    def wait(event, commit=False):
        deadline = time.monotonic() + 90
        while event['callback_ms'] and not event['result']:
            check(not event['candidates'], 'candidates_visible_while_pending')
            if time.monotonic() > deadline:
                raise TimeoutError('AI result not delivered')
            time.sleep(.1)
            event = send('KOTORI_REFRESH')
        if commit:
            check(bool(event['result']), 'queued_commit_missing')
        return event

    failed = True
    try:
        send('SEND_KEY\tON\nSWITCH_COMPOSITION_MODE\tHIRAGANA')
        first, second = 'あしたのかいぎはじゅうじからです', 'そのかいぎはとうきょうです'
        roman = romanize(first, args.romanji_table)
        key_with_context(roman[0])
        send('SEND_KEYS\t' + roman[1:])
        pending = key_with_context('Space')
        check(bool(pending['callback_ms']) and not pending['candidates'], 'first_space_did_not_wait')
        enter = key_with_context('Enter')
        check(not enter['result'], 'enter_committed_before_ai')
        roman = romanize(second, args.romanji_table)
        key_with_context(roman[0])
        buffered = send('SEND_KEYS\t' + roman[1:])
        check(buffered['value'] == first + second, 'buffered_input_not_visible')
        committed = wait(buffered, commit=True)
        check(committed['value'] == second, 'following_composition_lost')
        check(bool(committed['result']), 'first_result_empty')
        converted = wait(send('SEND_KEY\tSpace'))
        check(bool(converted['candidates']) and converted['value'] != second, 'second_ai_conversion_missing')
        final = send('SEND_KEY\tEnter')
        check(final['result'] == converted['value'], 'second_display_and_commit_disagree')
        check(all(x['ms'] < 500 for x in events), 'key_or_callback_over_500ms')
        failed = False
    except Exception as exc:
        failures.append(type(exc).__name__ + ': ' + str(exc))
    finally:
        session.close(failed=failed)
        status = profile / 'session/kotori_ai_status.txt'
        status_text = status.read_text(encoding='utf-8') if status.exists() else ''
        if args.require_device:
            check(args.require_device in status_text, 'requested_device_not_observed')
        # 最後のstatusの計算時間が0でも、実際の候補採点をstderrで確認する。
        score_lines = [line for line in stderr.read_text(encoding='utf-8', errors='replace').splitlines()
                       if line.startswith('[kotori] ') and
                       all(name in line for name in ('zenz=', 'llm=', 'score='))]
        check(bool(score_lines), 'real_model_candidate_scoring_not_observed')
        summary = dict(source_commit=identity['source_commit'], passed=not failures, failures=failures,
                       scope='isolated native SessionHandler with synthetic surrounding context; not installed TSF',
                       context_request_exactness='use the C++ Session regression for exact request context assertions',
                       first_commit=next((x['result'] for x in events if x['result']), ''),
                       last_commit=events[-1]['result'] if events else '', events=len(events),
                       max_response_ms=max((x['ms'] for x in events), default=0),
                       model_score_lines=len(score_lines), status=status_text, identities=identities)
        (args.out / 'events.json').write_text(json.dumps(events, ensure_ascii=False, indent=2), encoding='utf-8')
        (args.out / 'summary.json').write_text(json.dumps(summary, ensure_ascii=False, indent=2), encoding='utf-8')
        print(json.dumps({k: v for k, v in summary.items() if k != 'identities'}, ensure_ascii=False), flush=True)
    return 1 if failures else 0


if __name__ == '__main__':
    raise SystemExit(main())
