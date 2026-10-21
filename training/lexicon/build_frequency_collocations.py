"""固定全量頻度から、偶然の並びを抑えた連語を既存データへ追加する候補実験。"""
import argparse
from collections import Counter
import heapq
import json
import math
from pathlib import Path
import re
import sys
import time

from fetch_sources import sha
from stage_tokenizer import stage

JAPANESE = re.compile(r'[\u3400-\u4dbf\u4e00-\u9fff々ぁ-ゖァ-ヺー]+')
BASELINE_SHA = 'c615bb88d82db2bdb19f3cc401b785e252d4efebdbd71cf9403538eb6457acc0'


def association(joint, first, last, total, policy):
    if (joint < policy['min_documents'] or joint > min(first, last) or
            min(first, last) <= 0 or joint >= total):
        return None
    pmi = math.log2(joint * total / (first * last))
    npmi = pmi / -math.log2(joint / total)
    if pmi < policy['min_pmi_bits'] or npmi < policy['min_npmi']:
        return None
    return npmi * math.log1p(joint)


def endpoints(tokens, policy):
    if len(tokens) not in (2, 3):
        return None
    allowed = set(policy['content_pos'])
    if any(pos[0] not in allowed or pos[1] == '数詞' or pos[2] == '人名'
           for _, pos in (tokens[0], tokens[-1])):
        return None
    if len(tokens) == 3:
        middle, pos = tokens[1]
        if pos[0] == '助詞':
            if middle not in policy['allowed_middle_particles']:
                return None
        elif pos[0] not in allowed:
            return None
    return tokens[0][0], tokens[-1][0]


def build(frequency, baseline, runtime_path, out):
    started = time.monotonic()
    policy_path = Path(__file__).with_name('collocation_policy.json')
    policy = json.loads(policy_path.read_text('utf-8'))
    metadata = json.loads((frequency / 'frequency.json').read_text('utf-8'))
    config = json.loads(Path(__file__).with_name('corpus_sources.json').read_text('utf-8'))
    if metadata['limit_bytes'] or metadata['corpus'] != config or metadata['sharding']['full_source_sha256'] != config['files'][0]['sha256']:
        raise ValueError('固定した全量コーパスの集計が必要です')
    if sha(baseline) != BASELINE_SHA:
        raise ValueError('公開v2.0.0の連語基準SHAが一致しません')
    for name in ('phrase_frequency.tsv', 'surface_frequency.tsv'):
        item = next(item for item in metadata['outputs'] if item['file'] == name)
        if sha(frequency / name) != item['sha256']:
            raise ValueError('集計済み頻度のSHAが一致しません')
    runtime = stage(runtime_path)
    sys.path.insert(0, runtime['python_root'])
    import sudachipy
    dictionary = sudachipy.Dictionary(dict=runtime['dictionary'])
    tokenizer = dictionary.tokenizer(mode='C')
    total = sum(item['audit']['unique_accepted_texts'] for item in metadata['shard_results'])
    surfaces = {}
    with (frequency / 'surface_frequency.tsv').open(encoding='utf-8') as stream:
        for line in stream:
            surface, occurrences, documents, buckets = line.rstrip('\n').split('\t')
            surfaces[surface] = int(documents)
    existing = set(baseline.read_text('utf-8').splitlines()) - {''}
    heap, audit = [], Counter()
    with (frequency / 'phrase_frequency.tsv').open(encoding='utf-8') as stream:
        for line in stream:
            text, occurrences, documents, buckets = line.rstrip('\n').split('\t')
            documents, buckets = int(documents), int(buckets)
            audit['input_phrases'] += 1
            if text in existing:
                audit['already_in_baseline'] += 1
                continue
            if (not policy['min_length'] <= len(text) <= policy['max_length'] or
                    not JAPANESE.fullmatch(text) or documents < policy['min_documents'] or
                    buckets.bit_count() < policy['min_content_buckets']):
                audit['insufficient_or_invalid'] += 1
                continue
            parts = endpoints([(m.surface(), m.part_of_speech()) for m in tokenizer.tokenize(text)], policy)
            if parts is None:
                audit['unsupported_token_structure'] += 1
                continue
            score = association(documents, surfaces.get(parts[0], 0), surfaces.get(parts[1], 0), total, policy)
            if score is None:
                audit['insufficient_association_or_boundary_mismatch'] += 1
                continue
            audit['eligible_phrases'] += 1
            item = score, documents, text
            if len(heap) < policy['max_new_phrases']:
                heapq.heappush(heap, item)
            elif item > heap[0]:
                heapq.heapreplace(heap, item)
    dictionary.close()
    combined = sorted(existing | {text for _, _, text in heap})
    if not heap or len(combined) > policy['max_combined_phrases']:
        raise ValueError('追加連語が空または上限を超えています')
    out.mkdir(parents=True, exist_ok=False)
    output = out / 'fast_collocations.txt'
    output.write_text(''.join(value + '\n' for value in combined), encoding='utf-8', newline='\n')
    old = json.loads(baseline.with_suffix('.json').read_text('utf-8'))
    receipt = dict(old, baseline_sha256=BASELINE_SHA, baseline_entries=len(existing),
                   source='Miwa-Keita/zenz-v2.5-dataset Wikipedia full; existing source notices retained',
                   frequency_manifest_sha256=sha(frequency / 'frequency.json'), policy=policy,
                   policy_sha256=sha(policy_path), generator_sha256=sha(__file__), runtime=runtime,
                   added_entries=len(combined) - len(existing), combined_entries=len(combined),
                   total_unique_texts=total, selection_audit=dict(audit), seconds=time.monotonic() - started,
                   result_sha256=sha(output), bytes=output.stat().st_size,
                   evaluation_labels_used=False, product_adopted=False,
                   distribution_status='candidate_not_adopted', license='CC-BY-SA-4.0 additional full-corpus phrases; previous notices retained')
    (out / 'fast_collocations.json').write_text(json.dumps(receipt, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    old_notice = baseline.with_name('NOTICE-fast-collocations.txt').read_text('utf-8')
    notice = (old_notice + '\n\n追加する全量コーパス連語候補（製品採用前）\n' +
              (frequency / 'NOTICE-frequency.txt').read_text('utf-8') +
              '\n変更: 文書頻度とPMI/NPMI、形態素境界で連語を選択。既存の連語を保持。\n' +
              '今回の結合結果SHA-256: ' + sha(output) + '\n')
    (out / 'NOTICE-fast-collocations.txt').write_text(notice, encoding='utf-8', newline='\n')
    return receipt


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('frequency', 'baseline', 'runtime', 'out'):
        parser.add_argument('--' + name, type=Path, required=True)
    args = parser.parse_args()
    print(json.dumps(build(args.frequency, args.baseline, args.runtime, args.out), ensure_ascii=False, indent=2))
