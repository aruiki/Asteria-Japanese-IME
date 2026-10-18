"""実OSS辞書・AIなしの隔離セッションで通常変換と編集操作を確認する。

Windows TSF/画面/導入検証ではない。利用者の辞書/履歴に接続しない。
正解表記は機能の開発例であり、独立精度評価として使わない。
"""
import argparse
import json
import os
from pathlib import Path
import sys
import tempfile
import traceback
import unicodedata

from first_display_compare import Session, romanize, sha256
from large_input_campaign import parse_output


def normalize(value):
    return unicodedata.normalize('NFKC', value)


def run(args):
    from protocol.commands_pb2 import Output
    from protocol.user_dictionary_storage_pb2 import UserDictionary, UserDictionaryStorage
    from google.protobuf import text_format

    document = json.loads(args.cases.read_text(encoding='utf-8'))
    cases = document['cases']
    if not cases or len({c['id'] for c in cases}) != len(cases):
        raise ValueError('空または重複したcase')
    identity_files = [args.exe, args.cases, args.romanji_table]
    identity_files += sorted(args.proto.glob('protocol/*_pb2.py'))
    identities = {str(p): sha256(p) for p in identity_files}
    env = {k: v for k, v in os.environ.items() if not k.startswith('KOTORI_')}
    missing = args.out / 'empty-runtime'
    missing.mkdir()
    env.update(KOTORI_MODEL_DIR=str(missing), KOTORI_RUNTIME_DIR=str(missing),
               KOTORI_LM_DEBUG='1', KOTORI_LM_DEVICE='cpu')
    rows = []

    def scenario(label, action, dictionary=False):
        profile = Path(tempfile.mkdtemp(prefix='asteria-normal-')).resolve()
        if not str(profile).isascii():
            raise ValueError('Native argv profile requires ASCII')
        if dictionary:
            db = UserDictionaryStorage()
            d = db.dictionaries.add(id=101, name='isolated acceptance')
            d.entries.add(key='あすてりあけんしょう', value='星詠検証株式会社', pos=UserDictionary.PROPER_NOUN)
            user_dir = profile / 'session'
            user_dir.mkdir()
            (user_dir / 'user_dictionary.db').write_bytes(db.SerializeToString())
        row = dict(id=label, profile=str(profile), events=[], failures=[])
        session = Session(args.exe, env, profile, args.out / (label + '.stderr.txt'))
        row['pid'] = session.proc.pid
        failed = True

        def send(command):
            event = session.send(command)
            parsed = parse_output(event['output'], Output, text_format)
            parsed.update(command=command, ms=event['response_ms'], raw=event['output'])
            row['events'].append(parsed)
            if parsed['ms'] > args.max_response_ms:
                row['failures'].append('response_over_limit')
            return parsed

        def check(condition, detail):
            if not condition:
                row['failures'].append(detail)

        def type_reading(reading, keys=None):
            send('SEND_KEY\tESC\nSEND_KEY\tESC\nRESET_CONTEXT\nSEND_KEY\tON\nSWITCH_COMPOSITION_MODE\tHIRAGANA')
            for key in keys or romanize(reading, args.romanji_table):
                typed = send('SEND_KEYS\t' + key)
            check(normalize(typed['key']) == normalize(reading), 'typed_reading_mismatch')
            return typed

        def commit(converted):
            event = send('SEND_KEY\tEnter')
            check(bool(converted['value']) and event['result'] == converted['value'], 'commit_different_from_preedit')
            return event

        try:
            send('SET_CONFIG\tkotori_lm_enabled\tfalse')
            action(send, check, type_reading, commit, row)
            failed = False
        except Exception:
            row['error'] = traceback.format_exc()
            row['failures'].append('execution_error')
        finally:
            try:
                session.close(failed)
            except Exception:
                row['error'] = traceback.format_exc()
                row['failures'].append('close_error')
            row['exit_code'] = session.proc.returncode
            status = profile / 'session/kotori_ai_status.txt'
            row['ai_status'] = status.read_text(encoding='utf-8') if status.exists() else None
            rows.append(row)
            with (args.out / 'results.jsonl').open('a', encoding='utf-8') as stream:
                stream.write(json.dumps(row, ensure_ascii=False) + '\n')
            print(label, row['failures'], flush=True)

    for case in cases:
        def conversion(send, check, type_reading, commit, row, case=case):
            type_reading(case['reading'], case.get('romaji'))
            converted = send('SEND_KEY\tSpace')
            check(normalize(converted['value']) in {normalize(s) for s in case['expected']}, 'reference_surface_mismatch')
            commit(converted)
            row.update(area=case['area'], expected=case['expected'], actual=converted['value'])
        scenario(case['id'], conversion)

    def candidate(send, check, type_reading, commit, row):
        type_reading('はし')
        first = send('SEND_KEY\tSpace')
        second = send('SEND_KEY\tSpace')
        check(first['value'] != second['value'], 'candidate_cycle_unchanged')
        selected = send('SELECT_CANDIDATE_BY_VALUE\t箸')
        check(selected['value'] == '箸', 'candidate_selection_ignored')
        commit(selected)
        type_reading('はし')
        learned = send('SEND_KEY\tSpace')
        check(learned['value'] == '箸', 'confirmed_choice_not_reused')
        commit(learned)
    scenario('candidate-and-confirmed-history', candidate)

    def resize(send, check, type_reading, commit, row):
        type_reading('きょうはいいてんきです')
        original = send('SEND_KEY\tSpace')
        expanded = send('SEND_KEY\tShift Right')
        check(original['key'] == expanded['key'], 'resize_lost_reading')
        check(original['segments'] and expanded['segments'] and
              len(original['segments'][0]['key']) < len(expanded['segments'][0]['key']), 'segment_not_expanded')
        shrunk = send('SEND_KEY\tShift Left')
        check(original['key'] == shrunk['key'], 'shrink_lost_reading')
        check(expanded['segments'] and shrunk['segments'] and
              len(expanded['segments'][0]['key']) > len(shrunk['segments'][0]['key']), 'segment_not_shrunk')
        send('SEND_KEY\tRight')
        commit(send('SEND_KEY\tLeft'))
    scenario('segments-resize-and-move', resize)

    def cancel(send, check, type_reading, commit, row):
        type_reading('とうきょう')
        send('SEND_KEY\tSpace')
        reverted = send('SEND_KEY\tESC')
        check(reverted['value'] == 'とうきょう', 'conversion_cancel_lost_reading')
        erased = send('SEND_KEY\tBackspace')
        check(erased['value'] == 'とうきょ', 'backspace_did_not_edit_composition')
        empty = send('SEND_KEY\tESC')
        check(not empty['value'] and not empty['result'], 'cancel_did_not_clear_composition')
    scenario('cancel-and-backspace', cancel)

    for label, reading, keys, key, expected in (
        ('katakana', 'すーぱー', None, 'F7', 'スーパー'),
        ('halfwidth-katakana', 'すーぱー', None, 'F8', 'ｽｰﾊﾟｰ'),
        ('hiragana', 'とうきょう', None, 'F6', 'とうきょう'),
        ('ascii', 'もｚｃ2026', 'mozc2026', 'F10', 'mozc2026'),
        ('fullwidth-ascii', 'もｚｃ2026', 'mozc2026', 'F9', 'ｍｏｚｃ２０２６')):
        def mode(send, check, type_reading, commit, row, reading=reading, keys=keys, key=key, expected=expected):
            type_reading(reading, keys)
            converted = send('SEND_KEY\t' + key)
            check(converted['value'] == expected, 'mode_conversion_mismatch')
            commit(converted)
        scenario(label, mode)

    def user_dictionary(send, check, type_reading, commit, row):
        type_reading('あすてりあけんしょう')
        converted = send('SEND_KEY\tSpace')
        check('星詠検証株式会社' in converted['candidates'], 'registered_word_not_in_candidates')
        selected = send('SELECT_CANDIDATE_BY_VALUE\t星詠検証株式会社')
        check(selected['value'] == '星詠検証株式会社', 'registered_word_not_selected')
        commit(selected)
    scenario('isolated-user-dictionary', user_dictionary, dictionary=True)

    changed = [str(p) for p in identity_files if sha256(p) != identities[str(p)]]
    report = dict(source_commit=args.source_commit, identities=identities, native_tsf=False,
                  ai_enabled=False, embedded_oss_dictionary=True, case_file_scope=document['scope'],
                  scenarios=len(rows), expected_scenarios=len(cases) + 9,
                  failed_scenarios=[r['id'] for r in rows if r['failures']], artifacts_changed=changed,
                  max_response_ms=max(e['ms'] for r in rows for e in r['events']),
                  response_limit_ms=args.max_response_ms, rows=rows)
    report['passed'] = (report['scenarios'] == report['expected_scenarios'] and
                        not report['failed_scenarios'] and not changed)
    (args.out / 'summary.json').write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding='utf-8')
    print(json.dumps({k: v for k, v in report.items() if k not in ('rows', 'identities')}, ensure_ascii=False), flush=True)
    return 0 if report['passed'] else 2


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    for name in ('exe', 'proto', 'romanji-table', 'out'):
        ap.add_argument('--' + name, type=Path, required=True)
    ap.add_argument('--cases', type=Path, default=Path('eval/normal-conversion-basics.json'))
    ap.add_argument('--source-commit', required=True)
    ap.add_argument('--max-response-ms', type=float, default=3000)
    args = ap.parse_args()
    if not 0 < args.max_response_ms <= 10000:
        ap.error('response limit must be positive and bounded')
    for key in ('exe', 'proto', 'romanji_table', 'cases', 'out'):
        setattr(args, key, getattr(args, key).resolve())
    args.out.mkdir(parents=True, exist_ok=False)
    sys.path.insert(0, str(args.proto))
    return run(args)


if __name__ == '__main__':
    raise SystemExit(main())
