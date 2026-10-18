"""AIなしの実converterから全文ラティス・元コストを原票付きで収集する。"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'mozc'))
sys.path.insert(0, str(ROOT / 'mozc/tools'))
from eval_baseline import first_candidates, kata_to_hira
from conversion_training import reading_key


def digest(path):
    with Path(path).open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def native_reading(text):
    # Composerは直前の英数/記号に応じて読みに含む句読点をASCIIへ揃える。
    # 参照表記の採点は変えず、かな・英字・数字の差も許容しない。
    return reading_key(text).replace('、', ',').replace('。', '.')


def command_reading(text):
    # 既存製品評価と同じ変換。NFKCは「…」等も別の入力にしてしまう。
    return kata_to_hira(text)


def parse_blocks(raw, readings):
    blocks, lines, body = [], [], False
    for line in raw.splitlines():
        if line.startswith(b'ExecCommand() return false'):
            blocks.append(lines); lines = []; body = False
        elif line.startswith(b'---------- Segment '):
            body = True
            lines.append(line.decode('utf-8'))
        elif body:
            lines.append(line.decode('utf-8'))
    if len(blocks) != len(readings):
        raise ValueError('Native result count mismatch')
    rows = []
    for lines, reading in zip(blocks, readings):
        candidates = []
        for line in lines:
            if line.startswith('KOTORI_LATTICE\t'):
                fields = line.split('\t')
                if len(fields) != 6 or int(fields[1]) != len(candidates):
                    raise ValueError('Malformed or duplicated lattice trace')
                key = bytes.fromhex(fields[4]).decode('utf-8')
                text = bytes.fromhex(fields[5]).decode('utf-8')
                if native_reading(key) != native_reading(reading) or not text:
                    raise ValueError('Lattice reading differs from request')
                candidates.append(dict(rank=int(fields[1]), cost=int(fields[2]), attributes=int(fields[3]), text=text))
        top = first_candidates(lines)
        if not top or not candidates:
            raise ValueError('Empty current output or lattice')
        rows.append(dict(reading=reading, top1=top, candidates=candidates))
    return rows


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    for name in ('exe', 'data', 'out'):
        ap.add_argument('--' + name, type=Path, required=True)
    ap.add_argument('--source-commit', required=True)
    args = ap.parse_args()
    args.exe, args.data, args.out = (p.resolve() for p in (args.exe, args.data, args.out))
    if args.data.suffix == '.jsonl':
        items = [dict(index=f'calibration-{i}', input=r['input'], expected_output=[r['output']])
                 for i, r in enumerate(json.loads(line) for line in args.data.read_text(encoding='utf-8').splitlines())]
    else:
        items = json.loads(args.data.read_text(encoding='utf-8'))
    readings = [command_reading(item['input']) for item in items]
    if not readings or any(not r or any(c.isspace() or ord(c) < 32 for c in r) for r in readings):
        raise ValueError('Empty/unsafe native argument')
    args.out.mkdir(parents=True, exist_ok=False)
    empty = args.out / 'empty-runtime'; empty.mkdir()
    env = {k: v for k, v in os.environ.items() if not k.startswith('KOTORI_')}
    env.update(KOTORI_MODEL_DIR=str(empty), KOTORI_RUNTIME_DIR=str(empty), KOTORI_LM_PRELOAD='0')
    fixed = {str(p): digest(p) for p in [args.exe, args.data, Path(__file__)]}
    with tempfile.TemporaryDirectory(prefix='asteria-initial-lattice-') as temp:
        if not temp.isascii(): raise ValueError('Native profile path requires ASCII')
        command = [str(args.exe), '--kotori_lattice_trace=true', '--user_profile_dir=' + temp]
        script = ''.join('start ' + r + '\nreset\ninitial_stat_separator\n' for r in readings)
        proc = subprocess.run(command, input=script.encode('utf-8'), env=env,
                              cwd=Path(str(args.exe) + '.runfiles')/'_main',
                              capture_output=True, timeout=120)
    (args.out / 'native.stdout').write_bytes(proc.stdout)
    (args.out / 'native.stderr').write_bytes(proc.stderr)
    if proc.returncode:
        raise RuntimeError(f'Native converter exit {proc.returncode}')
    rows = parse_blocks(proc.stdout, readings)
    for row, item in zip(rows, items):
        refs = item['expected_output']
        row.update(index=str(item['index']), references=[refs] if isinstance(refs, str) else refs)
    path = args.out / 'observations.jsonl'
    path.write_text(''.join(json.dumps(row, ensure_ascii=False)+'\n' for row in rows), encoding='utf-8', newline='\n')
    if fixed != {str(p): digest(Path(p)) for p in fixed}:
        raise RuntimeError('Measurement artifacts changed')
    manifest = dict(source_commit=args.source_commit, identities=fixed, observations_sha256=digest(path),
                    cases=len(rows), correct=sum(r['top1'] in r['references'] for r in rows),
                    original_lattice_candidate_hit=sum(any(c['text'] in r['references'] for c in r['candidates']) for r in rows),
                    scope='offline native converter, no preceding context, no AI; not TSF')
    (args.out / 'manifest.json').write_text(json.dumps(manifest, indent=2), encoding='utf-8')
    print(json.dumps(manifest), flush=True)


if __name__ == '__main__':
    main()
