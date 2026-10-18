"""Create a local model bundle with fixed identities; preserve existing output."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil


def digest(path):
    h = hashlib.sha256()
    with Path(path).open('rb') as source:
        for block in iter(lambda: source.read(1024 * 1024), b''):
            h.update(block)
    return h.hexdigest()


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--generator', type=Path, required=True)
    ap.add_argument('--base-dir', type=Path, required=True)
    ap.add_argument('--out', type=Path, required=True)
    args = ap.parse_args()
    if args.out.exists():
        raise FileExistsError('Existing bundles are preserved')
    manifest = json.loads(Path(__file__).with_name('native_generator.json').read_text(encoding='utf8'))
    sources = [(args.base_dir / name, name, sha) for name, sha in manifest['base_models'].items()]
    sources.append((args.generator, manifest['filename'], manifest['sha256']))
    for source, _, expected in sources:
        if digest(source) != expected:
            raise ValueError(f'Model identity mismatch: {source.name}')
    args.out.mkdir(parents=True)
    receipt = {'models': {}, 'generator_manifest': manifest, 'installed_system_modified': False}
    for source, name, expected in sources:
        dest = args.out / name
        shutil.copy2(source, dest)
        if digest(dest) != expected:
            raise ValueError(f'Staged model identity mismatch: {name}')
        receipt['models'][name] = expected
    (args.out / 'bundle.json').write_text(json.dumps(receipt, indent=2), encoding='utf8')
    print(json.dumps(receipt['models']), flush=True)


if __name__ == '__main__':
    main()
