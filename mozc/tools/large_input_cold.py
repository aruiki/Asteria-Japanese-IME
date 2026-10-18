"""初回/準備後の実Mozcセッションで1打鍵ずつ入力し即Space・確定を確認する。

各phaseは別の新規プロファイル/プロセス。導入済みIMEへ接続せず、学習を
phase間で使い回さない。CPU強制は実CPUでの確認であり低性能実機・TSFの証明ではない。
"""
import argparse
import json
import os
from pathlib import Path
import sys
import tempfile
import time
from first_display_compare import Session
from large_input_campaign import assess, parse_output, sha


def status_text(path):
    try:
        return path.read_text(encoding='utf-8')
    except FileNotFoundError:
        return ''


def ready_status(text, pid, cpu=False):
    fields = dict(line.split('=', 1) for line in text.splitlines() if '=' in line)
    return (fields.get('pid') == str(pid) and fields.get('state') in ('llm', 'zenz')
            and (not cpu or fields.get('device') == 'CPU'))


def capture_phase(args, cases, phase, env, output_type, text_format, rows):
    # 各phaseで新しいプロセスにする。確定学習で準備後の精度を上げて見せない。
    profile = Path(tempfile.mkdtemp(prefix='asteria-cold-' + phase + '-')).resolve()
    if not str(profile).isascii():
        raise ValueError('Native argv requires an ASCII temporary profile path')
    session = Session(args.exe, env, profile, args.out / (phase + '.stderr.txt'))
    # 途中失敗でも既に採取した行は呼び出し側の集計に残す。
    failed = True
    status_path = profile / 'session/kotori_ai_status.txt'

    def state():
        return status_text(status_path)

    def send(command):
        event = session.send(command)
        parsed = parse_output(event['output'], output_type, text_format)
        parsed.update(ms=event['response_ms'], raw=event['output'], command=command)
        return parsed

    started = time.monotonic()
    try:
        send('SET_CONFIG\tkotori_lm_enabled\ttrue\nSET_CONFIG\tkotori_lm_quality\tKOTORI_LM_STANDARD')
        if phase == 'loaded':
            deadline = time.monotonic() + args.ready_timeout
            while not ready_status(state(), session.proc.pid, args.cpu):
                if session.proc.poll() is not None:
                    raise RuntimeError('Session exited during model preparation')
                if time.monotonic() > deadline:
                    raise TimeoutError('Readiness for this process was not observed: ' + state())
                time.sleep(.05)
        for case in cases:
            send('SEND_KEY\tESC\nSEND_KEY\tESC\nRESET_CONTEXT\nSEND_KEY\tON\nSWITCH_COMPOSITION_MODE\tHIRAGANA')
            before, key_events = state(), []
            for index, key in enumerate(case['romaji']):
                command = 'SEND_KEYS\t' + key
                if index == 0 and case.get('preceding'):
                    command = 'SEND_KEY_WITH_OPTION\t' + key + '\tcontext.preceding_text=' + json.dumps(case['preceding'], ensure_ascii=False)
                key_events.append(send(command))
                if args.key_interval:
                    time.sleep(args.key_interval)
            typed = key_events[-1]
            space = send('SEND_KEY\tSpace')
            commit = send('SEND_KEY\tEnter')
            events = dict(typed=typed, space=space, commit=commit)
            defects, reviews = assess(case, events)
            if max(event['ms'] for event in key_events) > args.max_key_ms:
                defects.append('key_over_limit')
            if space['ms'] > args.max_space_ms:
                defects.append('space_over_limit')
            row = dict(case_id=case['id'], phase=phase, reading=case['reading'],
                       events=events, key_events=key_events, defects=defects, reviews=reviews,
                       profile=str(profile), pid=session.proc.pid, status_before=before,
                       status_after=state(), seconds_since_start=time.monotonic() - started)
            rows.append(row)
            with (args.out / 'results.jsonl').open('a', encoding='utf-8') as stream:
                stream.write(json.dumps(row, ensure_ascii=False) + '\n')
            print(phase, case['reading'], space['value'], round(space['ms'], 1), defects, reviews, flush=True)
        failed = False
    finally:
        session.close(failed)


def summarize(rows, cases, completed_phases, error=None):
    # 空の結果・途中失敗・読みや確定の欠落を成功扱いしない。
    complete = completed_phases == ['no-warmup', 'loaded'] and len(rows) == 2 * len(cases) and bool(cases) and error is None
    defects = sum(len(row['defects']) for row in rows)
    mismatches = sum('expected_surface_mismatch' in row['reviews'] for row in rows)
    return dict(complete=complete, input_path_checks_passed=complete and defects == 0,
                defects=defects, reference_mismatches=mismatches,
                reference_matches=len(rows) - mismatches, scenarios=len(rows), error=error,
                max_space_ms=max((r['events']['space']['ms'] for r in rows), default=None),
                max_key_ms=max((event['ms'] for r in rows for event in r['key_events']), default=None))


def identities(args):
    payload = args.install
    files = sorted(payload.glob('*.gguf')) + sorted(payload.glob('*.dll'))
    files += sorted(args.proto.glob('protocol/*_pb2.py'))
    # SessionHandlerはOSS辞書を実行ファイルへ埋め込む。別ファイルを要求しない。
    files += [args.exe, args.cases]
    return {str(path.resolve()): sha(path) for path in files}


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    for name in ('exe', 'cases', 'proto', 'out'):
        ap.add_argument('--' + name, type=Path, required=True)
    ap.add_argument('--install', type=Path, default=Path('C:/Program Files (x86)/Kotori'))
    ap.add_argument('--source-commit', default='unverified')
    ap.add_argument('--cpu', action='store_true')
    ap.add_argument('--threads', type=int)
    ap.add_argument('--count', type=int, default=20)
    ap.add_argument('--key-interval', type=float, default=0)
    ap.add_argument('--ready-timeout', type=float, default=30)
    ap.add_argument('--max-key-ms', type=float, default=3000)
    ap.add_argument('--max-space-ms', type=float, default=3000)
    ap.add_argument('--require-exact', action='store_true')
    args = ap.parse_args()
    if args.count <= 0 or not 0 <= args.key_interval <= 1 or not 0 < args.ready_timeout <= 120:
        ap.error('count/interval/readiness timeout out of range')
    if args.max_key_ms <= 0 or args.max_space_ms <= 0 or (args.threads is not None and not 1 <= args.threads <= 64):
        ap.error('latency limits and thread count must be positive and bounded')
    args.exe, args.install, args.proto, args.out = (p.resolve() for p in (args.exe, args.install, args.proto, args.out))
    args.out.mkdir(parents=True, exist_ok=False)
    cases = [json.loads(line) for line in args.cases.read_text(encoding='utf-8').splitlines() if line][:args.count]
    if not cases or any(not c.get('romaji') or not c.get('reading') or not c.get('expected') for c in cases):
        raise ValueError('Cases must contain nonempty reading, romaji, and fixed references')
    if any(any(ch in c['romaji'] or ch in c['reading'] for ch in '\t\r\n') for c in cases):
        raise ValueError('Control characters in command arguments')
    sys.path.insert(0, str(args.proto))
    from protocol.commands_pb2 import Output
    from google.protobuf import text_format
    before = identities(args)
    env = {k: v for k, v in os.environ.items() if not k.startswith('KOTORI_')}
    env.update(KOTORI_MODEL_DIR=str(args.install), KOTORI_RUNTIME_DIR=str(args.install), KOTORI_LM_TIME='1')
    if args.cpu:
        env['KOTORI_LM_DEVICE'] = 'cpu'
    if args.threads is not None:
        env['KOTORI_LM_THREADS'] = str(args.threads)
    rows, completed, error = [], [], None
    try:
        for phase in ('no-warmup', 'loaded'):
            capture_phase(args, cases, phase, env, Output, text_format, rows)
            completed.append(phase)
    except Exception as exc:
        error = repr(exc)
    if identities(args) != before:
        error = 'Measurement artifacts changed during execution'
    report = summarize(rows, cases, completed, error)
    report.update(native_tsf=False, low_end_hardware_verified=False, cpu=args.cpu,
                  source_commit=args.source_commit, identities=before,
                  dictionary_identity='embedded_oss_data_covered_by_executable_sha256',
                  environment={k: v for k, v in env.items() if k.startswith('KOTORI_')},
                  key_interval=args.key_interval, max_key_limit_ms=args.max_key_ms,
                  max_space_limit_ms=args.max_space_ms, profiles_are_new_per_phase=True)
    (args.out / 'summary.json').write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding='utf-8')
    print(json.dumps(report, ensure_ascii=False, indent=2), flush=True)
    if not report['input_path_checks_passed'] or (args.require_exact and report['reference_mismatches']):
        raise SystemExit(1)


if __name__ == '__main__':
    main()
