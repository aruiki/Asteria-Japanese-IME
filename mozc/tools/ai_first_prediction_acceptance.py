"""隔離SessionHandlerでTabの非同期AI候補・保留Enter・取消を確認する。実TSFではない。"""
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
    for name in ('exe', 'install', 'proto', 'romanji-table', 'out'):
        ap.add_argument('--' + name, type=Path, required=True)
    ap.add_argument('--cold-reading', default='あいであきょく')
    ap.add_argument('--expected-prefix', default='アイデア曲')
    args = ap.parse_args()
    for name in ('exe', 'install', 'proto', 'romanji_table', 'out'):
        setattr(args, name, getattr(args, name).resolve())
    args.out.mkdir(parents=True, exist_ok=False)
    sys.path.insert(0, str(args.proto))
    from google.protobuf import text_format
    from protocol.commands_pb2 import Output
    env = {k: v for k, v in os.environ.items() if not k.startswith('KOTORI_')}
    env.update(KOTORI_MODEL_DIR=str(args.install), KOTORI_RUNTIME_DIR=str(args.install),
               KOTORI_LM_DEBUG='1')
    profile = Path(tempfile.mkdtemp(prefix='asteria-ai-first-tab-'))
    identity = {str(p): sha256(p) for p in [args.exe, args.romanji_table] +
                sorted(args.install.glob('*.gguf')) + sorted(args.install.glob('*.dll'))}
    session = Session(args.exe, env, profile, args.out / 'engine.stderr.txt')
    events, failures = [], []
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

    def type_reading(reading):
        return send('SEND_KEYS\t' + romanize(reading, args.romanji_table))

    def wait(event, committed=False):
        deadline = time.monotonic() + 90
        while event['callback_ms'] and not event['result']:
            check(not event['candidates'], 'prediction_published_while_pending')
            if time.monotonic() >= deadline:
                raise TimeoutError('Tab result was not delivered')
            time.sleep(.1)
            event = send('KOTORI_REFRESH')
        if committed:
            check(bool(event['result']), 'queued_tab_enter_result_missing')
        return event

    failed = True
    try:
        reset()
        reading = args.cold_reading
        type_reading(reading)
        tab = send('SEND_KEY\tTab')
        check(tab['value'] == reading and bool(tab['callback_ms']), 'cold_tab_not_async')
        check(not tab['candidates'], 'cold_tab_published_before_ai')
        enter = send('SEND_KEY\tEnter')
        check(not enter['result'], 'tab_enter_committed_before_ai')
        following = send('SEND_KEYS\tsa')
        check(following['value'] == reading + 'さ', 'tab_following_input_not_visible')
        committed = wait(following, committed=True) if not enter['result'] else enter
        check(committed['result'].startswith(args.expected_prefix), 'cold_tab_prediction_reference_mismatch')
        check(committed['value'] == 'さ', 'tab_following_input_lost')
        reset()
        reading = 'あしたのかいぎはじゅうじからです'
        type_reading(reading)
        space = wait(send('SEND_KEY\tSpace'))
        body = space['value']
        check(bool(body) and body != reading, 'space_ai_body_missing')
        send('SEND_KEY\tESC')
        predicted = wait(send('SEND_KEY\tTab'))
        check(bool(predicted['candidates']), 'tab_candidates_missing_after_ready')
        check(predicted['value'].startswith(body), 'tab_changed_evaluated_body')
        committed = send('SEND_KEY\tEnter')
        check(committed['result'] == predicted['value'], 'tab_candidate_commit_disagree')
        reset()
        reading = 'あしをいためた'
        type_reading(reading)
        pending = send('SEND_KEY\tTab')
        check(bool(pending['callback_ms']), 'cancel_case_not_pending')
        send('SEND_KEY\tESC')
        stale = send('KOTORI_REFRESH')
        check(not stale['result'] and not stale['value'], 'cancelled_tab_result_applied')
        reset()
        reading = 'かれはこうえんでこうえんをした'
        type_reading(reading)
        pending = send('SEND_KEY\tTab')
        check(bool(pending['callback_ms']), 'edit_case_not_pending')
        edited = send('SEND_KEYS\tsa')
        check(edited['key'] == reading + 'さ' and not edited['result'], 'tab_edit_lost_or_committed_old_text')
        reset()
        send('SET_CONFIG\tkotori_lm_enabled\tfalse')
        type_reading('にほんご')
        disabled = send('SEND_KEY\tTab')
        check(bool(disabled['candidates']) and not disabled['callback_ms'], 'explicit_ai_off_waited_for_ai')
        check(all(e['ms'] < 500 for e in events), 'key_or_callback_blocked_over_500ms')
        failed = False
    except Exception as exc:
        failures.append(type(exc).__name__ + ': ' + str(exc))
    finally:
        session.close(failed=failed)
        (args.out / 'events.json').write_text(json.dumps(events, ensure_ascii=False, indent=2), encoding='utf-8')
        status = profile / 'session/kotori_ai_status.txt'
        summary = dict(scope='isolated native Mozc SessionHandler; not installed TSF',
                       passed=not failures, failures=failures, identities=identity,
                       cold_reading=args.cold_reading, expected_prefix=args.expected_prefix,
                       profile=str(profile), events=len(events),
                       status=status.read_text(encoding='utf-8') if status.exists() else '',
                       max_response_ms=max((e['ms'] for e in events), default=0))
        (args.out / 'summary.json').write_text(json.dumps(summary, ensure_ascii=False, indent=2), encoding='utf-8')
        print(json.dumps({k: v for k, v in summary.items() if k != 'identities'}, ensure_ascii=False), flush=True)
    return 1 if failures else 0


if __name__ == '__main__':
    raise SystemExit(main())
