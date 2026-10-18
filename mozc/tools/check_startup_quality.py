"""同じ読みをAI準備前後で比較する。隔離converter試験であり実TSFの証明ではない。"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import queue
import re
import subprocess
import tempfile
import threading
import time


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--converter', type=Path, required=True)
    ap.add_argument('--model-dir', type=Path, required=True)
    ap.add_argument('--runtime-dir', type=Path, required=True)
    ap.add_argument('--out', type=Path, required=True)
    ap.add_argument('--reading', required=True)
    ap.add_argument('--expected', required=True)
    ap.add_argument('--ready-timeout', type=float, default=60)
    args = ap.parse_args()
    if not args.reading or any(ord(c) < 32 or ord(c) == 127 for c in args.reading):
        ap.error('reading must be a single command argument without control characters')
    if not 0 < args.ready_timeout <= 120:
        ap.error('ready-timeout must be between 0 and 120 seconds')
    exe = args.converter.resolve()
    with exe.open('rb') as source:
        digest = hashlib.file_digest(source, 'sha256').hexdigest()
    out = args.out.resolve()
    out.mkdir(parents=True, exist_ok=False)
    # Native Windows argv can interpret non-ASCII paths using the active codepage.
    # Retain this isolated profile for diagnosis; do not touch the installed IME.
    profile = Path(tempfile.mkdtemp(prefix='asteria-startup-quality-'))
    if not str(profile).isascii():
        raise ValueError('Use an ASCII temporary-directory path for native argv')
    env = {k: v for k, v in os.environ.items() if not k.startswith('KOTORI_')}
    env.update(KOTORI_MODEL_DIR=str(args.model_dir.resolve()),
               KOTORI_RUNTIME_DIR=str(args.runtime_dir.resolve()),
               KOTORI_LM_TIME='1', KOTORI_LM_DEBUG='1')
    receipt = dict(scope='isolated native converter, not installed TSF',
                   reading=args.reading, expected=args.expected, profile=str(profile),
                   converter=str(exe), converter_sha256=digest, environment={
                       k: v for k, v in env.items() if k.startswith('KOTORI_')})
    (out / 'conditions.json').write_text(json.dumps(receipt, ensure_ascii=False, indent=2), encoding='utf-8')
    stdout, stderr, q = [], [], queue.Queue()
    p = subprocess.Popen([str(exe), '--evaluation_trace=true',
                          '--user_profile_dir=' + str(profile)],
                         cwd=exe.parent / (exe.name + '.runfiles') / '_main', env=env,
                         stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE)

    def reader(stream, dest, notify=False):
        for line in iter(stream.readline, b''):
            dest.append(line)
            if notify:
                q.put(line)

    threads = [threading.Thread(target=reader, args=(p.stdout, stdout, True), daemon=True),
               threading.Thread(target=reader, args=(p.stderr, stderr), daemon=True)]
    for thread in threads:
        thread.start()
    status = profile / 'kotori_ai_status.txt'

    def state():
        return status.read_text(encoding='utf-8', errors='replace') if status.exists() else ''

    def convert(label):
        before, started, lines = state(), time.monotonic(), []
        p.stdin.write(f'start {args.reading}\nreset\nkotori_sep\n'.encode('utf-8'))
        p.stdin.flush()
        deadline = started + 30
        while True:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise TimeoutError('converter did not finish the request')
            line = q.get(timeout=remaining)
            lines.append(line)
            if line.startswith(b'ExecCommand() return false'):
                break
        raw = b''.join(lines)
        (out / (label + '.stdout')).write_bytes(raw)
        top = ''.join(re.findall(r'^  0/\d+ (.*)$', raw.decode('utf-8').replace('\r', ''), re.M))
        return dict(label=label, status_before=before, top1=top, ok=top == args.expected,
                    elapsed_ms=(time.monotonic() - started) * 1000)

    try:
        first = convert('immediate')
        deadline = time.monotonic() + args.ready_timeout
        while not any(line in ('state=llm', 'state=zenz') for line in state().splitlines()):
            if p.poll() is not None or 'state=error' in state() or 'state=off' in state():
                raise RuntimeError('AI readiness failed: ' + state())
            if time.monotonic() > deadline:
                raise TimeoutError('AI readiness not observed: ' + state())
            time.sleep(.05)
        warm = convert('ready')
        p.stdin.close()
        if p.wait(timeout=60):
            raise RuntimeError('converter failed')
        receipt['attempts'] = [first, warm]
        receipt['output_changed'] = first['top1'] != warm['top1']
        (out / 'receipt.json').write_text(json.dumps(receipt, ensure_ascii=False, indent=2), encoding='utf-8')
        print(json.dumps(receipt, ensure_ascii=False, indent=2), flush=True)
        return 0 if first['ok'] and warm['ok'] else 2
    finally:
        if p.poll() is None:
            p.kill()
            p.wait()
        for thread in threads:
            thread.join(timeout=5)
        (out / 'all.stdout').write_bytes(b''.join(stdout))
        (out / 'all.stderr').write_bytes(b''.join(stderr))


if __name__ == '__main__':
    raise SystemExit(main())
