"""固定Wikipedia区間7MiBだけを取得し、初期連語の同梱候補を作る。"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import urllib.request

ROOT = Path(__file__).resolve().parents[2]
AJIMEE_URL = ('https://raw.githubusercontent.com/azooKey/AJIMEE-Bench/'
              '401666cd56d1a570c2021798b64b6da4396bfd45/JWTD_v2/v1/evaluation_items.json')
AJIMEE_SHA = 'e9eb668fd6aa14b1e26436f429b5550108af0a1dfd443b8cea0bcb3ab3028fca'


def digest(data):
    return hashlib.sha256(data).hexdigest()


def save_fixed(path, data):
    """既存の別成果物を上書きしない。"""
    path.parent.mkdir(parents=True, exist_ok=True)
    if path.exists():
        if path.read_bytes() != data:
            raise ValueError(f'既存の成果物と一致しません: {path}')
    else:
        with path.open('xb') as stream:
            stream.write(data)


def fetch_range(url, offset, length, file_size):
    request = urllib.request.Request(url, headers={
        'Range': f'bytes={offset}-{offset + length - 1}',
        'Accept-Encoding': 'identity', 'User-Agent': 'Asteria-collocations/1'})
    with urllib.request.urlopen(request, timeout=45) as response:
        expected = f'bytes {offset}-{offset + length - 1}/{file_size}'
        if response.status != 206 or response.headers.get('Content-Range') != expected:
            raise ValueError('固定した範囲だけの取得をサーバーが受理しませんでした')
        data = response.read(length + 1)
        if len(data) != length:
            raise ValueError('取得した範囲の長さが一致しません')
    return data


def load_chunk(config, chunk, out, cache=None):
    suffix = chunk['group'].split(':')[-1]
    filename = chunk['file'] + '.' + suffix + '.raw'
    existing = [out / filename] + ([cache / filename] if cache else [])
    for path in existing:
        if path.exists():
            data = path.read_bytes()
            if len(data) != chunk['bytes'] or digest(data) != chunk['sha256']:
                raise ValueError(f'既存の範囲データSHAが一致しません: {path}')
            return data
    url = (f"https://huggingface.co/datasets/{config['source']}/resolve/"
           f"{config['revision']}/{chunk['file']}?sample_offset={chunk['offset']}")
    data = fetch_range(url, chunk['offset'], chunk['bytes'], config['source_file_size'])
    if digest(data) != chunk['sha256']:
        raise ValueError('取得した範囲のSHAが一致しません')
    save_fixed(out / filename, data)
    return data


def canonical_sample(config, out, cache=None):
    groups = [f'train_wikipedia.jsonl:{i:02}' for i in range(1, 8)]
    if ([c['group'] for c in config['chunks']] != groups or
            any(c['file'] != 'train_wikipedia.jsonl' or c['license'] != 'CC-BY-SA-4.0'
                or c['bytes'] != 1048576 for c in config['chunks'])):
        raise ValueError('固定Wikipedia学習区間以外が指定されました')
    rows = []
    for chunk in config['chunks']:
        data = load_chunk(config, chunk, out, cache)
        lines = data.split(b'\n')[1:-1]
        if len(lines) != chunk['records']:
            raise ValueError('範囲中の完全レコード数が一致しません')
        for line in lines:
            item = json.loads(line)
            if not isinstance(item.get('input'), str) or not isinstance(item.get('output'), str):
                raise ValueError('コーパスの形式が一致しません')
            rows.append(dict(input=item['input'], output=item['output'],
                             source_group=chunk['group'], split='train'))
        print(chunk['group'], len(lines), 'SHA verified', flush=True)
    raw = ''.join(json.dumps(row, ensure_ascii=False, sort_keys=True, separators=(',', ':'))
                  + '\n' for row in rows).encode('utf-8')
    if digest(raw) != config['sample_sha256'] or len(rows) != config['records']:
        raise ValueError('正規化した学習区間のSHAが一致しません')
    sample = out / 'sample.jsonl'
    save_fixed(sample, raw)
    manifest = {k: config[k] for k in ('source', 'revision', 'chunks', 'records', 'sample_sha256')}
    metadata = out / 'sample.manifest.json'
    save_fixed(metadata, (json.dumps(manifest, indent=2) + '\n').encode('utf-8'))
    return sample, metadata


def ajimee(path, out):
    if path:
        raw = path.read_bytes()
    else:
        path = out / 'ajimee-bench.json'
        if path.exists():
            raw = path.read_bytes()
        else:
            with urllib.request.urlopen(AJIMEE_URL, timeout=45) as response:
                raw = response.read(2097153)
    if digest(raw) != AJIMEE_SHA:
        raise ValueError('除外に使う固定AJIMEEのSHAが一致しません')
    save_fixed(path, raw)
    return path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--baseline', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    parser.add_argument('--cache-dir', type=Path, help='既存取得原票の読み取り専用の再利用')
    parser.add_argument('--ajimee', type=Path)
    args = parser.parse_args()
    config_path = Path(__file__).with_name('sources.json')
    config = json.loads(config_path.read_text(encoding='utf-8'))
    if digest(args.baseline.read_bytes().replace(b'\r\n', b'\n')) != config['baseline_sha256']:
        raise ValueError('元のMozc連語データSHAが一致しません')
    source = args.out / 'source'
    sample, manifest = canonical_sample(config, source, args.cache_dir)
    evaluation = ajimee(args.ajimee, source)
    generated = args.out / 'generated'
    command = [sys.executable, '-X', 'utf8', str(ROOT / 'mozc/tools/build_fast_collocations.py'),
               '--corpus', str(sample), '--corpus-manifest', str(manifest),
               '--baseline', str(args.baseline), '--out', str(generated)]
    exclusions = sorted((ROOT / 'eval/sets').glob('*.json')) + [evaluation]
    for path in exclusions:
        command += ['--exclude', str(path)]
    subprocess.run(command, check=True)
    data = (generated / 'fast_collocations.txt').read_bytes()
    if digest(data) != config['result_sha256']:
        raise ValueError('比較済みの連語データSHAを再現できません')
    metadata = json.loads((generated / 'fast_collocations.json').read_text(encoding='utf-8'))
    metadata['exclusion_files'] = {
        (str(path.relative_to(ROOT)).replace('\\', '/') if path.is_relative_to(ROOT)
         else 'AJIMEE-Bench/evaluation_items.json'): digest(path.read_bytes()) for path in exclusions}
    metadata.update(source_manifest_sha256=digest(config_path.read_bytes()),
                    filename='fast_collocations.txt', distribution_status='generated_from_pinned_public_source')
    notice = f'''Asteriaの初期変換用連語データ

追加データ: CC BY-SA 4.0 https://creativecommons.org/licenses/by-sa/4.0/
出典: Wikipedia日本語版の執筆者、Keita Miwa / zenz-v2.5-dataset
https://huggingface.co/datasets/{config['source']}/tree/{config['revision']}
Wikipedia由来の学習区間01〜07だけを使用。dev区間とCommon Crawl部分は使用していません。
変更: 複数の文・取得区間に共通する短い日本語の連語を抽出し、評価読みを除外しました。
追加連語もCC BY-SA 4.0で提供します。元のMozc40件はGoogle Inc.のBSD-3-Clauseデータです。

結果SHA-256: {config['result_sha256']}
実行時はコンパイル済みデータを使い、モデルやネットワークへのアクセスは追加しません。
fast_collocations.txtは再構築と出典確認用です。生成規則と取得原票は以下を参照。
https://github.com/aruiki/Asteria-Japanese-IME/tree/main/training/collocations
'''
    save_fixed(args.out / 'fast_collocations.txt', data)
    save_fixed(args.out / 'fast_collocations.json', (json.dumps(metadata, ensure_ascii=False, indent=2) + '\n').encode('utf-8'))
    save_fixed(args.out / 'NOTICE-fast-collocations.txt', notice.encode('utf-8'))
    print('fixed collocations reproduced', config['result_sha256'], flush=True)


if __name__ == '__main__':
    main()
