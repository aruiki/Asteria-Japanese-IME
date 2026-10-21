"""固定Wikipedia全量の語彙・連語頻度を、重複/評価を除外し上限付きで集計する。"""
import argparse
from collections import Counter, defaultdict
from concurrent.futures import ProcessPoolExecutor
from functools import lru_cache
import hashlib
import heapq
import json
import math
from pathlib import Path
import re
import sqlite3
import sys
import time
import unicodedata

from build_lexicon import POS_MAPPING, READING
from fetch_sources import sha, verify
from stage_tokenizer import stage

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'mozc/tools'))
from conversion_training import evaluation_readings, grams, reading_key

KANJI = re.compile(r'[\u3400-\u4dbf\u4e00-\u9fff]')


def encode(value):
    return json.dumps(value, ensure_ascii=False, separators=(',', ':'))


class Exclusions:
    """三文字集合のJaccard>=0.8を、上限付きキャッシュとアンカーで照合する。"""
    def __init__(self, excluded, threshold=.8):
        self.exact = set(excluded)
        self.threshold = threshold
        self.values = [grams(key) for key in sorted(self.exact) if grams(key)]
        frequency = Counter(g for values in self.values for g in values)
        self.anchors = defaultdict(list)
        for index, values in enumerate(self.values):
            # >=80%一致なら、20%+1個の異なるアンカーの全てを欠くことはない。
            count = math.floor((1 - threshold + 1e-9) * len(values)) + 1
            for gram in sorted(values, key=lambda g: (frequency[g], g))[:count]:
                self.anchors[gram].append(index)

    @lru_cache(maxsize=16384)
    def overlaps(self, key):
        if key in self.exact:
            return True
        values = grams(key)
        possible = {i for gram in values for i in self.anchors.get(gram, ())}
        for index in possible:
            other = self.values[index]
            if min(len(values), len(other)) / max(len(values), len(other)) < self.threshold:
                continue
            common = len(values & other)
            if common / (len(values) + len(other) - common) >= self.threshold:
                return True
        return False


def reserved(start, end, policy):
    return any(start < item['offset'] + item['bytes'] and end > item['offset']
               for item in policy['reserved_byte_ranges'])


def shard_corpus(corpus, out, shards, policy, limit_bytes):
    """同じ正規化本文は必ず同じshardへ送り、shard間の二重計数を防ぐ。"""
    audit = Counter()
    source_digest = hashlib.sha256()
    handles = [(out / f'input-{i:02}.jsonl').open('xb') for i in range(shards)]
    try:
        with corpus.open('rb') as stream:
            position = 0
            for line in stream:
                source_digest.update(line)
                start = position
                position += len(line)
                if limit_bytes and start >= limit_bytes:
                    break
                audit['records_read'] += 1
                if reserved(start, position, policy):
                    audit['reserved_interval'] += 1
                    continue
                item = json.loads(line)
                if not isinstance(item.get('input'), str) or not isinstance(item.get('output'), str):
                    raise ValueError('固定コーパスの形式が一致しません')
                text = unicodedata.normalize('NFKC', item['output'])
                if not policy['min_text_length'] <= len(text) <= policy['max_text_length']:
                    audit['invalid_length'] += 1
                    continue
                identity = hashlib.sha256(text.encode('utf-8')).digest()
                bucket = int.from_bytes(identity[:4], 'little') % shards
                handles[bucket].write((encode([item['input'], text, identity.hex()]) + '\n').encode('utf-8'))
                audit['sharded_records'] += 1
                if audit['records_read'] % 500000 == 0:
                    print('sharding', position, 'bytes', audit['records_read'], 'records', flush=True)
    finally:
        for handle in handles:
            handle.close()
    audit['source_bytes_scanned'] = min(position, corpus.stat().st_size)
    result = dict(audit)
    result['full_source_sha256' if not limit_bytes else 'scanned_prefix_sha256'] = source_digest.hexdigest()
    return result


def token_features(tokenizer, text, policy):
    words, surfaces, tokens = Counter(), Counter(), []
    for morpheme in tokenizer.tokenize(text):
        surface = morpheme.surface()
        pos = morpheme.part_of_speech()
        valid = bool(surface) and all(unicodedata.category(c)[0] in 'LMN' or c == '・' for c in surface)
        tokens.append((surface, pos[0], valid))
        if valid:
            surfaces[surface] += 1
        mapped = POS_MAPPING.get(tuple(pos[1:4])) if pos[0] == '名詞' else None
        reading = reading_key(morpheme.reading_form())
        if mapped and valid and READING.fullmatch(reading) and len(surface) <= 32:
            words[encode([reading, surface, mapped])] += 1
    phrases = Counter()
    content = set(policy['content_pos'])
    for start in range(len(tokens)):
        for size in policy['phrase_tokens']:
            window = tokens[start:start + size]
            if (len(window) != size or not all(t[2] for t in window) or
                    window[0][1] not in content or window[-1][1] not in content):
                continue
            value = ''.join(token[0] for token in window)
            if policy['min_phrase_length'] <= len(value) <= policy['max_phrase_length'] and KANJI.search(value):
                phrases[value] += 1
    return words, phrases, surfaces


def flush(connection, counts):
    connection.executemany('INSERT INTO counts VALUES (?,?,?,?,?) '
        'ON CONFLICT(kind,key) DO UPDATE SET occurrences=occurrences+excluded.occurrences, '
        'documents=documents+excluded.documents, buckets=buckets|excluded.buckets',
        ((kind, key, *values) for (kind, key), values in counts.items()))
    connection.commit()
    counts.clear()


def process_shard(job):
    shard, out, runtime, policy, excluded = job
    out = Path(out)
    sys.path.insert(0, runtime['python_root'])
    import sudachipy
    dictionary = sudachipy.Dictionary(dict=runtime['dictionary'])
    tokenizer = dictionary.tokenizer(mode='C')
    overlaps = Exclusions(excluded, policy['near_duplicate_jaccard'])
    db = out / f'counts-{shard:02}.sqlite'
    if db.exists():
        raise FileExistsError('途中の集計DBを上書きしません')
    connection = sqlite3.connect(db)
    connection.execute('PRAGMA journal_mode=OFF')
    connection.execute('PRAGMA synchronous=OFF')
    connection.execute('PRAGMA cache_size=-16384')
    connection.execute('CREATE TABLE counts (kind INTEGER, key TEXT, occurrences INTEGER, '
        'documents INTEGER, buckets INTEGER, PRIMARY KEY(kind,key)) WITHOUT ROWID')
    seen, counts, audit = set(), {}, Counter()
    started = time.monotonic()
    with (out / f'input-{shard:02}.jsonl').open(encoding='utf-8') as stream:
        for line in stream:
            reading, text, identity = json.loads(line)
            audit['records'] += 1
            if overlaps.overlaps(reading_key(reading)):
                audit['evaluation_reading_or_near_duplicate'] += 1
                continue
            identity = bytes.fromhex(identity)
            if identity in seen:
                audit['duplicate_text'] += 1
                continue
            seen.add(identity)
            if len(seen) > policy['max_unique_records_per_shard']:
                raise ValueError('重複除去のメモリ上限に到達しました')
            bucket = 1 << (identity[8] % policy['bucket_count'])
            for kind, features in enumerate(token_features(tokenizer, text, policy)):
                for key, occurrences in features.items():
                    previous = counts.setdefault((kind, key), [0, 0, 0])
                    previous[0] += occurrences
                    previous[1] += 1
                    previous[2] |= bucket
            audit['unique_accepted_texts'] += 1
            if len(counts) >= policy['counter_flush_keys']:
                flush(connection, counts)
            if audit['records'] % 100000 == 0:
                print('shard', shard, dict(audit), round(time.monotonic() - started, 1), 'seconds', flush=True)
    flush(connection, counts)
    audit['database_keys'] = connection.execute('SELECT COUNT(*) FROM counts').fetchone()[0]
    connection.close()
    dictionary.close()
    result = dict(shard=shard, audit=dict(audit), seconds=time.monotonic() - started,
                  database_sha256=sha(db), input_sha256=sha(out / f'input-{shard:02}.jsonl'))
    (out / f'shard-{shard:02}.json').write_text(json.dumps(result, indent=2) + '\n', encoding='utf-8')
    return result


def merge_counts(out, shards, policy):
    connections = [sqlite3.connect(out / f'counts-{i:02}.sqlite') for i in range(shards)]
    summaries = []
    try:
        for kind, name in enumerate(('word_frequency.tsv', 'phrase_frequency.tsv', 'surface_frequency.tsv')):
            # 入力は固定個数のソート済みカーソル。全語彙をメモリへ展開しない。
            cursors = [connection.execute('SELECT key,occurrences,documents,buckets FROM counts '
                                         'WHERE kind=? ORDER BY key', (kind,)) for connection in connections]
            merged = heapq.merge(*cursors)
            previous, totals, entries = None, [0, 0, 0], 0
            with (out / name).open('w', encoding='utf-8', newline='\n') as stream:
                def write(key, values):
                    if key is None or (kind == 1 and values[1] < policy['min_output_phrase_documents']):
                        return 0
                    fields = json.loads(key) if kind == 0 else [key]
                    stream.write('\t'.join(map(str, [*fields, *values])) + '\n')
                    return 1
                for key, occurrences, documents, buckets in merged:
                    if key != previous:
                        entries += write(previous, totals)
                        previous, totals = key, [0, 0, 0]
                    totals[0] += occurrences
                    totals[1] += documents
                    totals[2] |= buckets
                entries += write(previous, totals)
            summaries.append(dict(file=name, entries=entries, bytes=(out / name).stat().st_size,
                                  sha256=sha(out / name)))
    finally:
        for connection in connections:
            connection.close()
    return summaries


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source', type=Path, required=True)
    parser.add_argument('--runtime', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    parser.add_argument('--exclude', type=Path, action='append', required=True)
    parser.add_argument('--shards', type=int, default=8)
    parser.add_argument('--limit-bytes', type=int, default=0, help='取得SHAを変えない処理量の試験。採用候補には使わない')
    args = parser.parse_args()
    if not 1 <= args.shards <= 16 or args.limit_bytes < 0:
        raise ValueError('shard数または試験サイズが範囲外です')
    config_path = Path(__file__).with_name('corpus_sources.json')
    policy_path = Path(__file__).with_name('frequency_policy.json')
    config = json.loads(config_path.read_text('utf-8'))
    policy = json.loads(policy_path.read_text('utf-8'))
    for spec in config['files']:
        verify(args.source / spec['name'], spec)
    corpus = args.source / 'train_wikipedia.jsonl'
    reserved_readings = set()
    with corpus.open('rb') as stream:
        for item in policy['reserved_byte_ranges']:
            stream.seek(item['offset'])
            data = stream.read(item['bytes'])
            if hashlib.sha256(data).hexdigest() != item['sha256']:
                raise ValueError('予約済み開発区間のSHAが一致しません')
            # 区間の完全レコードだけを使い、同じ/近い読みが他の区間にもある場合も除く。
            for line in data.split(b'\n')[1:-1]:
                reserved_readings.add(reading_key(json.loads(line)['input']))
    excluded = set(reserved_readings)
    for path in args.exclude:
        excluded.update(evaluation_readings(json.loads(path.read_text('utf-8'))))
    if not excluded:
        raise ValueError('評価読みの除外集合が空です')
    runtime = stage(args.runtime)
    args.out.mkdir(parents=True, exist_ok=False)
    started = time.monotonic()
    identity = dict(corpus=config, corpus_config_sha256=sha(config_path), policy=policy,
                    policy_sha256=sha(policy_path), generator_sha256=sha(__file__),
                    code_dependencies={name: sha(Path(__file__).with_name(name)) for name in
                                       ('build_lexicon.py', 'fetch_sources.py', 'stage_tokenizer.py')},
                    exclusion_files={str(path.resolve()): sha(path) for path in args.exclude},
                    exclusion_readings=len(excluded), reserved_interval_readings=len(reserved_readings),
                    runtime=runtime, shards=args.shards,
                    limit_bytes=args.limit_bytes, evaluation_labels_used=False, product_adopted=False)
    (args.out / 'plan.json').write_text(json.dumps(identity, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    print('fixed plan', flush=True)
    identity['sharding'] = shard_corpus(corpus, args.out, args.shards, policy, args.limit_bytes)
    if not args.limit_bytes and identity['sharding']['full_source_sha256'] != config['files'][0]['sha256']:
        raise ValueError('集計中にコーパスのSHAが変わりました')
    print('sharded', identity['sharding'], flush=True)
    jobs = [(i, str(args.out), runtime, policy, excluded) for i in range(args.shards)]
    with ProcessPoolExecutor(max_workers=args.shards) as pool:
        identity['shard_results'] = list(pool.map(process_shard, jobs))
    identity['outputs'] = merge_counts(args.out, args.shards, policy)
    if sha(__file__) != identity['generator_sha256'] or sha(policy_path) != identity['policy_sha256']:
        raise ValueError('処理中に生成規則が変更されました')
    identity['seconds'] = time.monotonic() - started
    notice = ('Asteriaの語彙・連語頻度候補 / 製品採用前\n'
              'CC BY-SA 4.0 https://creativecommons.org/licenses/by-sa/4.0/\n'
              '出典: Wikipedia日本語版の執筆者、Keita Miwa / zenz-v2.5-dataset\n'
              'https://huggingface.co/datasets/' + config['repository'] + '/tree/' + config['commit'] + '\n'
              '変更: NFKC本文を重複除去し、評価入力/近似読み/開発区間を除外。\n'
              '固定Sudachiで名詞と2〜3形態素の連語を数えた頻度データ。CC BY-SA 4.0で提供。\n'
              '読みはSudachiの優先読みであり、実コーパスの発音を照合した頻度ではない。\n'
              '本文hashの16区分は独立サイト/記事を表さない。\n')
    (args.out / 'NOTICE-frequency.txt').write_text(notice, encoding='utf-8', newline='\n')
    (args.out / 'frequency.json').write_text(json.dumps(identity, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    print('COMPLETE', identity['outputs'], identity['seconds'], flush=True)


if __name__ == '__main__':
    main()
