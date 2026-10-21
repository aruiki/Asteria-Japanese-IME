"""全量コーパスの実使用を確認した語だけで、頻度制約付き辞書候補を作る。"""
import argparse
from collections import Counter
import json
import math
from pathlib import Path
import time
import unicodedata

from fetch_sources import sha


def select(cost, reading, pos, frequency, policy):
    if frequency is None:
        return None, 'unobserved'
    occurrences, documents, buckets = frequency
    minimum = policy['min_person_documents'] if ',人名,' in pos else policy['min_documents']
    if documents < minimum:
        return None, 'insufficient_distinct_texts'
    if buckets.bit_count() < policy['min_content_buckets']:
        return None, 'insufficient_content_buckets'
    if not 0 < buckets < 65536 or not 0 < documents <= occurrences:
        raise ValueError('頻度集計値が不正です')
    rarity = max(0, round(policy['rarity_margin'] - policy['cost_reduction_per_frequency_doubling'] *
                          math.log2(documents / minimum)))
    length = max(0, len(reading) - policy['reading_length_margin_start']) * policy['cost_per_additional_reading_character']
    result = cost + rarity + length
    if result > policy['max_cost']:
        return None, 'cost_overflow'
    return result, ''


def build(candidate, frequency, mozc, out):
    started = time.monotonic()
    policy_path = Path(__file__).with_name('frequency_lexicon_policy.json')
    policy = json.loads(policy_path.read_text('utf-8'))
    lexical = json.loads((candidate / 'short_lexicon.json').read_text('utf-8'))
    if sha(candidate / 'short_lexicon.tsv') != lexical['sha256']:
        raise ValueError('元の語彙候補SHAが一致しません')
    counts = json.loads((frequency / 'frequency.json').read_text('utf-8'))
    corpus_config = json.loads(Path(__file__).with_name('corpus_sources.json').read_text('utf-8'))
    if (counts['limit_bytes'] != 0 or counts['corpus'] != corpus_config or
            counts['sharding']['full_source_sha256'] != corpus_config['files'][0]['sha256'] or
            not counts['exclusion_files'] or not counts['reserved_interval_readings']):
        raise ValueError('固定した全量・評価除外済みの頻度集計が必要です')
    data_file = frequency / 'word_frequency.tsv'
    expected = next(s for s in counts['outputs'] if s['file'] == data_file.name)
    if data_file.stat().st_size != expected['bytes'] or sha(data_file) != expected['sha256']:
        raise ValueError('頻度TSVのSHAが一致しません')
    directory = mozc / 'src/data/dictionary_oss'
    for name, expected_sha in lexical['mozc_source_inputs'].items():
        if sha(directory / name) != expected_sha:
            raise ValueError('比較基準のMozc辞書が変更されています: ' + name)
    ids = {int(index): text for index, text in (line.split(' ', 1) for line in
           (directory / 'id.def').read_text('utf-8').splitlines())}
    frequencies = {}
    with data_file.open(encoding='utf-8') as stream:
        for line in stream:
            reading, surface, pos, occurrences, documents, buckets = line.rstrip('\n').split('\t')
            key = reading, surface, pos
            if key in frequencies:
                raise ValueError('頻度TSVのキーが重複しています')
            frequencies[key] = tuple(map(int, (occurrences, documents, buckets)))
    out.mkdir(parents=True, exist_ok=False)
    output = out / 'short_lexicon.tsv'
    audit = Counter()
    with (candidate / 'short_lexicon.tsv').open(encoding='utf-8') as source, output.open('w', encoding='utf-8', newline='\n') as target:
        for line in source:
            reading, left, right, cost, surface = line.rstrip('\n').split('\t')
            if left != right or int(left) not in ids:
                raise ValueError('元の語彙候補品詞が一致しません')
            pos = ids[int(left)]
            key = reading, unicodedata.normalize('NFKC', surface), pos
            mapped, reason = select(int(cost), reading, pos, frequencies.get(key), policy)
            audit['input_entries'] += 1
            if mapped is None:
                audit[reason] += 1
                continue
            target.write(f'{reading}\t{left}\t{right}\t{mapped}\t{surface}\n')
            audit['selected_entries'] += 1
    summary = dict(lexical, parent_candidate_sha256=lexical['sha256'], policy=policy,
                   policy_sha256=sha(policy_path), generator_sha256=sha(__file__),
                   frequency_manifest_sha256=sha(frequency / 'frequency.json'),
                   frequency_data_sha256=sha(data_file), selection_audit=dict(audit),
                   output_entries=audit['selected_entries'], bytes=output.stat().st_size, sha256=sha(output),
                   seconds=time.monotonic() - started, evaluation_labels_used=False, product_adopted=False,
                   license='CC-BY-SA-4.0 frequency-derived selection; SudachiDict notices retained')
    (out / 'short_lexicon.json').write_text(json.dumps(summary, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    notice = (candidate / 'NOTICE-short-lexicon.txt').read_text('utf-8')
    notice += '\n\n' + (frequency / 'NOTICE-frequency.txt').read_text('utf-8')
    notice += '\n追加変更: 全量頻度の最小本文数で選択し、希少性と読みの長さでコストを加算。頻度由来の加工データはCC BY-SA 4.0で提供。\n'
    (out / 'NOTICE-short-lexicon.txt').write_text(notice, encoding='utf-8', newline='\n')
    return summary


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--candidate', type=Path, required=True)
    parser.add_argument('--frequency', type=Path, required=True)
    parser.add_argument('--mozc', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    print(json.dumps(build(args.candidate, args.frequency, args.mozc, args.out), ensure_ascii=False, indent=2))
