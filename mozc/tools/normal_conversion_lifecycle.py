"""実辞書の隔離プロセスで履歴・非学習・かな・再変換・確定取消を確認する。

利用者の辞書/履歴へ接続しない。Windows TSFの画面や実アプリの文字削除を
確認したことにはしない。確定取消はエンジンの削除要求と再編集状態を確認する。
"""
import argparse
import json
import os
from pathlib import Path
import sys
import tempfile
import traceback

from first_display_compare import Session, sha256
from large_input_campaign import parse_output


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    for name in ('exe', 'proto', 'out'):
        ap.add_argument('--' + name, type=Path, required=True)
    ap.add_argument('--source-commit', required=True)
    args = ap.parse_args()
    args.exe, args.proto, args.out = (p.resolve() for p in (args.exe, args.proto, args.out))
    args.out.mkdir(parents=True, exist_ok=False)
    sys.path.insert(0, str(args.proto))
    from protocol.commands_pb2 import Output
    from google.protobuf import text_format
    files = [args.exe, Path(__file__)] + sorted(args.proto.glob('protocol/*_pb2.py'))
    identity = {str(p): sha256(p) for p in files}
    empty = args.out / 'empty-runtime'; empty.mkdir()
    env = {k: v for k, v in os.environ.items() if not k.startswith('KOTORI_')}
    env.update(KOTORI_MODEL_DIR=str(empty), KOTORI_RUNTIME_DIR=str(empty), KOTORI_LM_DEVICE='cpu')
    rows = []

    def scenario(label, action):
        profile = Path(tempfile.mkdtemp(prefix='asteria-normal-lifecycle-')).resolve()
        if not str(profile).isascii(): raise ValueError('Native profile requires ASCII')
        row = dict(id=label, profile=str(profile), phases=[], events=[], failures=[])

        def check(condition, message):
            if not condition: row['failures'].append(message)

        def phase(name, function, preserve=True):
            session = Session(args.exe, env, profile, args.out/(label+'-'+name+'.stderr.txt'), preserve_profile=preserve)
            row['phases'].append(dict(name=name, pid=session.proc.pid, preserve_profile=preserve))
            failed = True

            def send(command):
                event = session.send(command)
                parsed = parse_output(event['output'], Output, text_format)
                message = Output()
                raw = event['output'].strip()
                if raw.split('\n', 1)[0] in ('goo.gle/debugproto', 'goo.gle/debugonly', 'goo.gle/debugstr'):
                    raw = raw.split('\n', 1)[1]
                text_format.Parse(raw, message)
                if message.HasField('deletion_range'):
                    parsed['deletion_range'] = dict(offset=message.deletion_range.offset, length=message.deletion_range.length)
                parsed.update(phase=name, command=command, ms=event['response_ms'], raw=event['output'])
                row['events'].append(parsed)
                check(parsed['ms'] <= 3000, 'response_over_3000ms')
                return parsed

            try:
                send('SET_CONFIG\tkotori_lm_enabled\tfalse\nSET_CONFIG\tincognito_mode\tfalse\nSEND_KEY\tON')
                function(send, check)
                failed = False
            finally:
                session.close(failed)
                row['phases'][-1]['exit_code'] = session.proc.returncode

        try:
            action(phase, check, row)
        except Exception:
            row['error'] = traceback.format_exc()
            row['failures'].append('execution_error')
        finally:
            rows.append(row)
            with (args.out/'results.jsonl').open('a', encoding='utf-8') as stream:
                stream.write(json.dumps(row, ensure_ascii=False)+'\n')
            print(label, row['failures'], flush=True)

    def hashi(send, check):
        send('SEND_KEY\tESC\nSEND_KEY\tESC\nRESET_CONTEXT\nSEND_KEY\tON\nSEND_KEYS\thashi')
        event = send('SEND_KEY\tSpace')
        check(event['key'] == 'はし', 'wrong_hashi_reading')
        return event

    def learning(mode):
        def action(phase, check, row):
            def first(send, check):
                original = hashi(send, check)
                alternate = next(v for v in ('箸', '橋', '端') if v != original['value'] and v in original['candidates'])
                row.update(original=original['value'], selected=alternate)
                if mode == 'incognito':
                    send('SEND_KEY\tESC\nSEND_KEY\tESC\nSET_CONFIG\tincognito_mode\ttrue')
                    hashi(send, check)
                selected = send('SELECT_CANDIDATE_BY_VALUE\t'+alternate)
                check(selected['value'] == alternate, 'selection_not_applied')
                if mode == 'cancel':
                    canceled = send('SEND_KEY\tESC\nSEND_KEY\tESC')
                    check(not canceled['result'] and not canceled['value'], 'cancel_did_not_clear_input')
                else:
                    confirmed = send('SEND_KEY\tEnter')
                    check(confirmed['result'] == alternate, 'confirm_different_from_selection')
            phase('choose', first, preserve=mode != 'default-cleanup')

            def reopened(send, check):
                actual = hashi(send, check)['value']
                expected = row['selected'] if mode == 'confirmed' else row['original']
                row['reopened'] = actual
                check(actual == expected, 'reopened_choice_mismatch')
            phase('reopened', reopened)
            check(len({p['pid'] for p in row['phases']}) == 2, 'process_was_not_restarted')
        return action

    scenario('confirmed-history-after-restart', learning('confirmed'))
    scenario('canceled-selection-not-learned', learning('cancel'))
    scenario('incognito-confirmation-not-learned', learning('incognito'))
    scenario('default-cli-cleanup-control', learning('default-cleanup'))

    def kana(phase, check, row):
        def action(send, check):
            send('SET_CONFIG\tpreedit_method\tKANA')
            typed = send('SEND_KANA_KEYS\ti-yb@\tにほんこ゛')
            check(typed['key'] == 'にほんご', 'jis_kana_reading_mismatch')
            converted = send('SEND_KEY\tSpace')
            check(converted['value'] == '日本語', 'jis_kana_conversion_mismatch')
            check(send('SEND_KEY\tEnter')['result'] == '日本語', 'jis_kana_commit_mismatch')
        phase('kana', action)
    scenario('jis-kana-and-voicing', kana)

    def reconvert(phase, check, row):
        def action(send, check):
            send('SET_SELECTION_TEXT\t日本語')
            converted = send('SEND_KEY\tHenkan')
            check(converted['key'] == 'にほんご' and converted['value'] == '日本語', 'selected_text_not_reconverted')
            check(send('SEND_KEY\tEnter')['result'] == '日本語', 'reconversion_commit_mismatch')
        phase('reconvert', action)
    scenario('selected-text-reconversion', reconvert)

    def undo(phase, check, row):
        def action(send, check):
            send('SEND_KEYS\tneko')
            converted = send('SEND_KEY\tSpace')
            committed = send('SEND_KEY\tEnter')
            check(committed['result'] == converted['value'] and bool(committed['result']), 'initial_commit_mismatch')
            restored = send('SEND_KEY\tCtrl Backspace')
            check(restored['key'] == 'ねこ' and bool(restored['value']), 'undo_did_not_restore_composition')
            check(restored.get('deletion_range') == dict(offset=-len(committed['result']), length=len(committed['result'])), 'undo_deletion_request_mismatch')
            check(send('SEND_KEY\tEnter')['result'] == restored['value'], 'undo_recommit_mismatch')
        phase('undo', action)
    scenario('commit-undo-and-recommit', undo)

    report = dict(source_commit=args.source_commit, identities=identity, native_tsf=False,
                  ai_enabled=False, scenarios=len(rows), failed=[r['id'] for r in rows if r['failures']],
                  artifacts_changed=identity != {str(p): sha256(p) for p in files}, rows=rows,
                  max_response_ms=max((e['ms'] for r in rows for e in r['events']), default=None))
    report['passed'] = report['scenarios'] == 7 and not report['failed'] and not report['artifacts_changed']
    (args.out/'summary.json').write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding='utf-8')
    print(json.dumps({k: v for k, v in report.items() if k not in ('rows', 'identities')}, ensure_ascii=False), flush=True)
    return 0 if report['passed'] else 2


if __name__ == '__main__': raise SystemExit(main())
