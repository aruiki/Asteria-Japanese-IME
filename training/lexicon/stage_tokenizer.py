"""固定SudachiPyを私有の学習用ディレクトリへ展開する。IMEには追加しない。"""
import argparse
import hashlib
import json
from pathlib import Path, PurePosixPath
import stat
import zipfile

from fetch_sources import load_sources, sha


def member_path(root, info):
    name = PurePosixPath(info.filename)
    if (name.is_absolute() or '..' in name.parts or '\\' in info.filename or
            ':' in info.filename or stat.S_ISLNK(info.external_attr >> 16)):
        raise ValueError('展開先の外へ出るZIP要素を拒否します')
    target = root.joinpath(*name.parts)
    if not target.resolve().is_relative_to(root.resolve()):
        raise ValueError('展開先のリンクが指定範囲の外を指しています')
    return target


def extract_verified(archive_path, root):
    files = {}
    with zipfile.ZipFile(archive_path) as archive:
        if sum(info.file_size for info in archive.infolist()) > 1024 ** 3:
            raise ValueError('展開サイズが上限を超えています')
        for info in archive.infolist():
            target = member_path(root, info)
            if info.is_dir():
                continue
            with archive.open(info) as stream:
                expected = hashlib.file_digest(stream, 'sha256').hexdigest()
            if target.exists():
                if target.stat().st_size != info.file_size or sha(target) != expected:
                    raise ValueError('既存の解析用ファイルを上書きしません: ' + info.filename)
            else:
                target.parent.mkdir(parents=True, exist_ok=True)
                with archive.open(info) as stream, target.open('xb') as output:
                    while block := stream.read(1048576):
                        output.write(block)
                if sha(target) != expected:
                    raise ValueError('展開後のSHAが一致しません')
            files[str(target.relative_to(root)).replace('\\', '/')] = expected
    return files


def stage(out, cache=None):
    config_path = Path(__file__).with_name('tokenizer_sources.json')
    config = load_sources(out / 'source', cache=cache, config_path=config_path)
    files = {}
    for spec in config['files']:
        subdir = 'python' if spec['name'].endswith('.whl') else 'dictionary'
        files[subdir] = extract_verified(out / 'source' / spec['name'], out / subdir)
    dictionaries = sorted((out / 'dictionary').rglob('system_full.dic'))
    if len(dictionaries) != 1:
        raise ValueError('full辞書の展開結果を一意に決められません')
    result = dict(version=config['version'], config_sha256=sha(config_path),
                  python_root=str((out / 'python').resolve()),
                  dictionary=str(dictionaries[0].resolve()), files=files,
                  dictionary_sha256=sha(dictionaries[0]), training_only=True)
    (out / 'runtime.json').write_text(json.dumps(result, indent=2) + '\n', encoding='utf-8')
    return result


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--out', type=Path, required=True)
    parser.add_argument('--cache', type=Path)
    args = parser.parse_args()
    print(json.dumps(stage(args.out, args.cache), indent=2))
