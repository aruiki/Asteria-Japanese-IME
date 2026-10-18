"""固定した追加生成モデルをビルド時に用意する。実行時の取得には使わない。"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import shutil
import tempfile
import urllib.request

MANIFEST = Path(__file__).with_name('native_generator.json')


def digest(path):
    with Path(path).open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def stage(destination, source=None, manifest=MANIFEST):
    identity = json.loads(Path(manifest).read_text(encoding='utf-8'))
    name, expected = identity['filename'], identity['sha256']
    if not re.fullmatch(r'[a-zA-Z0-9_.-]+\.gguf', name) or not re.fullmatch(r'[0-9a-f]{64}', expected):
        raise ValueError('Invalid model identity')
    destination = Path(destination)
    destination.mkdir(parents=True, exist_ok=True)
    target = destination / name
    if target.exists():
        if digest(target) != expected:
            raise ValueError('Existing model differs from pinned identity')
    else:
        with tempfile.TemporaryDirectory(prefix='generator-', dir=destination) as scratch:
            temporary = Path(scratch) / name
            if source is not None:
                shutil.copyfile(source, temporary)
            else:
                url = identity['download_url']
                if not url.startswith('https://github.com/aruiki/Asteria-Japanese-IME/releases/download/'):
                    raise ValueError('Unexpected model download origin')
                with urllib.request.urlopen(url, timeout=120) as response, temporary.open('wb') as stream:
                    shutil.copyfileobj(response, stream)
            if digest(temporary) != expected:
                raise ValueError('Model SHA-256 differs from pinned identity')
            temporary.replace(target)
    (destination / 'native_generator.json').write_text(
        json.dumps(identity, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    return target


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--out', type=Path, required=True)
    parser.add_argument('--source', type=Path, help='公開前のローカル固定モデルを使う')
    args = parser.parse_args()
    target = stage(args.out, args.source)
    print(f'{target.name}: {digest(target)}')


if __name__ == '__main__':
    main()
