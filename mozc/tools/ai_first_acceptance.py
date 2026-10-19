"""隔離した実Mozcセッションで、AI前の候補非公開とSpace/Enterの配送を確認する。

実TSFの試験ではない。利用者の履歴・導入済みIMEには接続しない。
"""
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
    ap.add_argument('--exe', type=Path, required=True)
    ap.add_argument('--install', type=Path, required=True)
    ap.add_argument('--proto', type=Path, required=True)
    ap.add_argument('--romanji-table', type=Path, required=True)
    ap.add_argument('--out', type=Path, required=True)
    ap.add_argument('--cpu', action='store_true')
    ap.add_argument('--type-ahead', action='store_true')
    args = ap.parse_args()
    sys.path.insert(0, str(args.proto.resolve()))
    from google.protobuf import text_format
    from protocol.commands_pb2 import Output
    args.exe = args.exe.resolve()
    args.install = args.install.resolve()
    args.out.mkdir(parents=True, exist_ok=False)
    identities = {str(p): sha256(p) for p in [args.exe, args.romanji_table] +
                  sorted(args.install.glob('*.gguf')) + sorted(args.install.glob('*.dll'))}
    env = {k: v for k, v in os.environ.items() if not k.startswith('KOTORI_')}
    env.update(KOTORI_MODEL_DIR=str(args.install), KOTORI_RUNTIME_DIR=str(args.install),
               KOTORI_LM_DEBUG='1')
    if args.cpu:
        env['KOTORI_LM_DEVICE'] = 'cpu'
    profile = Path(tempfile.mkdtemp(prefix='asteria-ai-first-'))
    session = Session(args.exe, env, profile, args.out / 'engine.stderr.txt')
    events = []
    failures = []
    started = time.monotonic()

    def send(command):
        event = session.send(command)
        parsed = parse_output(event['output'], Output, text_format)
        parsed.update(command=command, ms=event['response_ms'], raw=event['output'],
                      elapsed_ms=(time.monotonic() - started) * 1000)
        events.append(parsed)
        return parsed

    def check(condition, reason):
        if not condition:
            failures.append(reason)

    def reset():
        send('SEND_KEY\tESC\nSEND_KEY\tESC\nRESET_CONTEXT\nSEND_KEY\tON\nSWITCH_COMPOSITION_MODE\tHIRAGANA')

    def wait_for_result():
        deadline = time.monotonic() + 90
        while time.monotonic() < deadline:
            time.sleep(0.1)
            event = send('KOTORI_REFRESH')
            if event['result']:
                return event
            check(not event['candidates'], 'candidate_published_before_queued_commit')
            check(bool(event['callback_ms']), 'pending_conversion_lost_callback')
        raise TimeoutError('AI result not delivered within 90 seconds')

    failed = True
    try:
        reset()
        reading = 'あいであきょく'
        typed = send('SEND_KEYS\t' + romanize(reading, args.romanji_table))
        check(typed['value'] == reading, 'typed_preedit_changed')
        check(not typed['candidates'], 'unscored_initial_candidates_visible')
        space = send('SEND_KEY\tSpace')
        pending = bool(space['callback_ms']) and space['value'] == reading
        check(pending, 'cold_immediate_space_did_not_enter_async_wait')
        check(not space['candidates'], 'unscored_space_candidates_visible')
        enter = send('SEND_KEY\tEnter')
        check(not enter['result'], 'enter_committed_before_ai_finished')
        if args.type_ahead:
            next_text = send('SEND_KEYS\tka')
            check(next_text['value'] == reading + 'か', 'following_typing_not_visible')
            send('SEND_KEY\tBackspace')
            next_text = send('SEND_KEYS\tsa')
            check(next_text['value'] == reading + 'さ', 'following_edit_not_visible')
        committed = enter if enter['result'] else wait_for_result()
        check(committed['result'] == 'アイデア曲', 'ai_first_conversion_reference_mismatch')
        if args.type_ahead:
            check(committed['value'] == 'さ', 'following_typing_lost_after_commit')
        else:
            check(not committed['callback_ms'], 'committed_result_kept_pending_timer')
        reset()
        reading = 'あしたのかいぎはじゅうじからです'
        typed = send('SEND_KEYS\t' + romanize(reading, args.romanji_table))
        check(not typed['candidates'], 'live_suggestion_published_before_ai')
        deadline = time.monotonic() + 90
        while time.monotonic() < deadline:
            time.sleep(0.1)
            live = send('KOTORI_REFRESH')
            if live['candidates']:
                break
            check(bool(live['callback_ms']), 'live_ai_wait_lost_timer')
        else:
            raise TimeoutError('live AI candidate not delivered')
        top = live['candidates'][0]
        space = send('SEND_KEY\tSpace')
        check(space['value'] == top, 'live_candidate_and_space_disagree')
        committed = send('SEND_KEY\tEnter')
        check(committed['result'] == top, 'space_and_commit_disagree')
        check(all(e['ms'] < 500 for e in events), 'key_or_callback_blocked_over_500ms')
        failed = False
    except Exception as exc:
        failures.append(type(exc).__name__ + ': ' + str(exc))
    finally:
        session.close(failed=failed)
        (args.out / 'events.json').write_text(json.dumps(events, ensure_ascii=False, indent=2), encoding='utf-8')
        status = profile / 'session/kotori_ai_status.txt'
        summary = dict(scope='isolated native Mozc SessionHandler; not installed TSF',
                       passed=not failures, failures=failures, identities=identities,
                       profile=str(profile), cpu=args.cpu, event_count=len(events),
                       status=status.read_text(encoding='utf-8') if status.exists() else '',
                       max_response_ms=max((e['ms'] for e in events), default=0))
        (args.out / 'summary.json').write_text(json.dumps(summary, ensure_ascii=False, indent=2), encoding='utf-8')
        print(json.dumps({k: v for k, v in summary.items() if k != 'identities'}, ensure_ascii=False), flush=True)
    return 1 if failures else 0


if __name__ == '__main__':
    raise SystemExit(main())
