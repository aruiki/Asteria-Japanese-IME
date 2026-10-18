"""検査CLIの入力ファイル失敗が空実行PASSにならないことを確認する。

隔離profileだけを使う。実TSFや製品IMEの受入れ検査ではない。
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    for name in ('exe', 'out'):
        ap.add_argument('--' + name, type=Path, required=True)
    ap.add_argument('--source-commit', required=True)
    args = ap.parse_args()
    exe = args.exe.resolve()
    out = args.out.resolve()
    out.mkdir(parents=True, exist_ok=False)
    env = {k: v for k, v in os.environ.items() if not k.startswith('KOTORI_')}
    env['KOTORI_LM_PRELOAD'] = '0'
    rows = []
    with tempfile.TemporaryDirectory(prefix='asteria-cli-input-') as directory:
        base = Path(directory)
        assert str(base).isascii()
        for name, body, wanted in (
            ('missing', None, 1),
            ('valid', 'SEND_KEY\tON\nSEND_KEYS\tnihongo\nEXPECT_PREEDIT\tにほんご\n', 0),
            ('invalid-command', 'NOT_A_REAL_COMMAND\n', 1),
        ):
            source = base / (name + '.txt')
            if body is not None:
                source.write_text(body, encoding='utf-8')
            profile = base / (name + '-profile')
            process = subprocess.run(
                [str(exe), '--test', '--dictionary=oss', '--input=' + str(source),
                 '--profile=' + str(profile)], input=b'', capture_output=True,
                timeout=60, env=env)
            (out / (name + '.stdout.txt')).write_bytes(process.stdout)
            (out / (name + '.stderr.txt')).write_bytes(process.stderr)
            row = dict(id=name, exit_code=process.returncode,
                       expected_exit=wanted, profile_created=profile.exists())
            row['passed'] = process.returncode == wanted
            if name == 'missing':
                row['passed'] &= (not profile.exists()
                                  and b'Failed to open input file:' in process.stdout
                                  and b'[ PASSED ]' not in process.stdout)
            if name == 'valid':
                row['passed'] &= b'[ PASSED ]' in process.stdout
            rows.append(row)
    receipt = dict(source_commit=args.source_commit,
                   executable=str(exe), executable_sha256=hashlib.sha256(exe.read_bytes()).hexdigest(),
                   native_tsf=False, rows=rows, passed=all(r['passed'] for r in rows))
    (out / 'summary.json').write_text(json.dumps(receipt, ensure_ascii=False, indent=2), encoding='utf-8')
    print(json.dumps(receipt, ensure_ascii=False))
    return 0 if receipt['passed'] else 1


if __name__ == '__main__':
    raise SystemExit(main())
