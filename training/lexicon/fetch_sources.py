"""公式の版固定語彙と帰属文書をSHA検証して取得する。本文はGitへ入れない。"""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import urllib.request
import zipfile


def sha(path):
    with Path(path).open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def verify(path, spec):
    if path.stat().st_size != spec['bytes'] or sha(path) != spec['sha256']:
        raise ValueError('固定したサイズ/SHAと一致しません: ' + spec['name'])
    if 'members' in spec:
        with zipfile.ZipFile(path) as archive:
            actual = [(item.filename, item.file_size) for item in archive.infolist()]
            expected = [(item['name'], item['bytes']) for item in spec['members']]
            if actual != expected:
                raise ValueError('固定したZIP内容と一致しません: ' + spec['name'])


def fetch(spec, out, cache=None):
    name = spec['name']
    if Path(name).name != name or '/' in name or '\\' in name:
        raise ValueError('取得名は単一のファイル名に限ります')
    target = out / name
    source = cache / name if cache else None
    if target.exists():
        verify(target, spec)
        return target
    if source and source.exists():
        verify(source, spec)
        shutil.copyfile(source, target)
        return target
    if not spec['url'].startswith('https://'):
        raise ValueError('公式HTTPS配布URLが必要です')
    partial = out / (name + '.partial')
    with urllib.request.urlopen(spec['url'], timeout=60) as response, partial.open('xb') as stream:
        if response.status != 200:
            raise ValueError('全体応答ではありません')
        total = 0
        while block := response.read(min(1048576, spec['bytes'] - total + 1)):
            total += len(block)
            if total > spec['bytes']:
                raise ValueError('固定した取得サイズを超えました')
            stream.write(block)
    verify(partial, spec)
    partial.rename(target)
    return target


def load_sources(out, cache=None, config_path=None):
    config_path = config_path or Path(__file__).with_name('sources.json')
    config = json.loads(config_path.read_text('utf-8'))
    out.mkdir(parents=True, exist_ok=True)
    for spec in config['files']:
        fetch(spec, out, cache)
    receipt = dict(source_commit=config['commit'], version=config['version'],
                   config_sha256=sha(config_path), files={s['name']: s['sha256'] for s in config['files']})
    (out / 'verified.json').write_text(json.dumps(receipt, indent=2) + '\n', encoding='utf-8')
    return config


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--out', type=Path, required=True)
    parser.add_argument('--cache', type=Path)
    parser.add_argument('--config', type=Path, help='別の版固定コーパスの取得設定')
    args = parser.parse_args()
    print(json.dumps(load_sources(args.out, args.cache, args.config), ensure_ascii=False, indent=2))
