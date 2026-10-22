"""比較済み語彙の固定3ファイルをビルド時に配置する。実行時通信は増やさない。"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import shutil
import urllib.request

MANIFEST = Path(__file__).with_name('package.json')
NAMES = ('short_lexicon.tsv', 'short_lexicon.json', 'NOTICE-short-lexicon.txt')


def digest(path):
    with Path(path).open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def identity(path=MANIFEST):
    spec = json.loads(Path(path).read_text('utf-8'))
    if spec['schema'] != 1 or tuple(f['name'] for f in spec['files']) != NAMES:
        raise ValueError('語彙配布の固定ファイル構成が一致しません')
    prefix = 'https://github.com/aruiki/Asteria-Japanese-IME/releases/download/'
    if not re.fullmatch(r'data-short-lexicon-[0-9]{8}', spec['tag']):
        raise ValueError('製品の版と分離したデータタグが必要です')
    for f in spec['files']:
        if (not re.fullmatch(r'[0-9a-f]{64}', f['sha256']) or
                not 0 < f['bytes'] <= 16 * 1024 * 1024 or
                f['url'] != prefix + spec['tag'] + '/' + f['name']):
            raise ValueError('語彙配布のURL/サイズ/SHAが不正です')
    return spec


def verify_file(path, spec):
    if path.stat().st_size != spec['bytes'] or digest(path) != spec['sha256']:
        raise ValueError('語彙ファイルのサイズ/SHA不一致: ' + spec['name'])


def verify(directory, spec):
    directory = Path(directory)
    for f in spec['files']:
        verify_file(directory / f['name'], f)
    metadata = json.loads((directory / NAMES[1]).read_text('utf-8'))
    data = (directory / NAMES[0]).read_text('utf-8').splitlines()
    if (len(data) != spec['entries'] or metadata['output_entries'] != spec['entries'] or
            metadata['sha256'] != spec['files'][0]['sha256'] or
            metadata['evaluation_labels_used'] is not False or
            metadata['source_commit'] != spec['sudachi_commit'] or
            metadata['frequency_data_sha256'] != spec['frequency_data_sha256']):
        raise ValueError('語彙manifestの出典/件数/参照未使用が不一致です')
    previous = None
    for line in data:
        fields = line.split('\t')
        if (len(fields) != 5 or not fields[0] or not fields[4] or
                fields[1] != fields[2] or not fields[1].isdigit() or
                not fields[3].isdigit() or not 0 <= int(fields[3]) <= 32767):
            raise ValueError('語彙TSVの形式/接続ID/コストが不正です')
        key = fields[0], fields[4], int(fields[1])
        if previous is not None and key < previous:
            raise ValueError('語彙TSVの固定順序が不正です')
        previous = key
    return metadata


def stage(out, source=None, manifest=MANIFEST):
    spec = identity(manifest)
    if source is not None:
        verify(source, spec)
    out = Path(out)
    out.mkdir(parents=True, exist_ok=True)
    # 全既存ファイルを先に検査し、不一致を正しいファイルで上書きしない。
    for f in spec['files']:
        target = out / f['name']
        if target.exists():
            verify_file(target, f)
    for f in spec['files']:
        target = out / f['name']
        if target.exists():
            continue
        partial = target.with_name(target.name + '.partial')
        with partial.open('xb') as stream:
            if source is not None:
                with (Path(source) / f['name']).open('rb') as src:
                    shutil.copyfileobj(src, stream)
            else:
                with urllib.request.urlopen(f['url'], timeout=120) as response:
                    if response.status != 200:
                        raise ValueError('語彙データを全体取得できません')
                    total = 0
                    while block := response.read(min(1048576, f['bytes'] - total + 1)):
                        total += len(block)
                        if total > f['bytes']:
                            raise ValueError('語彙データの固定サイズを超えました')
                        stream.write(block)
        verify_file(partial, f)
        partial.rename(target)
    verify(out, spec)
    return spec


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--out', type=Path, required=True)
    parser.add_argument('--source', type=Path, help='公開前のローカル固定3ファイル')
    args = parser.parse_args()
    spec = stage(args.out, args.source)
    print(f"語彙{spec['entries']:,}件の全3ファイル/SHA/出典を確認")


if __name__ == '__main__':
    main()
