"""固定Wikipedia学習区間から、Mozcの既存連語フィルター用データを作る。

本文や辞書はGitへ入れず、取得原票・生成規則・SHAを保存する。
評価入力は読みだけで除外し、参照表記を抽出や採用へ渡さない。
"""
import argparse
from collections import Counter, defaultdict
import hashlib
import json
from pathlib import Path
import re
import sys
import unicodedata

from conversion_training import evaluation_readings, reading_key

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'training/contrastive'))
from native_pairs import exclusion_filter


REVISION = '7d9c9ea36347b638a627c1d264da48ebc7eb38aa'
CORPUS_SHA = '3d47371a7704fa359d8baeed03b25f4d820e68b299fb75f3fe348bb583878552'
CANONICAL_CORPUS_SHA = 'f886d411a7565171733edc9a3e6aa5f659affcc030eae277e5a6213165564255'
TRAIN_GROUPS = {f'train_wikipedia.jsonl:{i:02}' for i in range(1, 8)}
JAPANESE = re.compile(r'[\u3400-\u4dbf\u4e00-\u9fff々ぁ-ゖァ-ヺー]+')
KANJI = re.compile(r'[\u3400-\u4dbf\u4e00-\u9fff]')
MIN_LENGTH, MAX_LENGTH = 3, 14
MIN_SENTENCES, MIN_GROUPS = 5, 2


def sha(path):
    with Path(path).open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def fragments(text):
    """文内で重複カウントせず、実文に現れた短い日本語の組合せだけを使う。"""
    result = set()
    for match in JAPANESE.finditer(unicodedata.normalize('NFKC', text)):
        run = match.group()
        for start in range(len(run)):
            # 文節の先頭に来ない助詞断片だけの登録を増やさない。
            if 'ぁ' <= run[start] <= 'ゖ':
                continue
            for length in range(MIN_LENGTH, min(MAX_LENGTH, len(run) - start) + 1):
                value = run[start:start + length]
                if KANJI.search(value):
                    result.add(value)
    return result


def collect(rows, excluded):
    overlaps = exclusion_filter(excluded)
    counts, groups, audit = Counter(), defaultdict(set), Counter()
    seen = set()
    for row in rows:
        group = row.get('source_group', '')
        if row.get('split') != 'train' or group not in TRAIN_GROUPS:
            audit['not_wikipedia_train'] += 1
            continue
        key = reading_key(row['input'])
        if overlaps(key):
            audit['evaluation_reading_or_near_duplicate'] += 1
            continue
        text = row['output']
        if not isinstance(text, str) or not 3 <= len(text) <= 512:
            audit['invalid_text'] += 1
            continue
        identity = unicodedata.normalize('NFKC', text)
        if identity in seen:
            audit['duplicate_sentence'] += 1
            continue
        seen.add(identity)
        audit['accepted_sentences'] += 1
        for value in fragments(text):
            counts[value] += 1
            groups[value].add(group)
    phrases = sorted(value for value, count in counts.items()
                     if count >= MIN_SENTENCES and len(groups[value]) >= MIN_GROUPS)
    audit.update(unique_fragments=len(counts), selected_phrases=len(phrases))
    return phrases, dict(audit)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--corpus', type=Path, required=True)
    parser.add_argument('--corpus-manifest', type=Path, required=True)
    parser.add_argument('--baseline', type=Path, required=True)
    parser.add_argument('--exclude', type=Path, action='append', required=True)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    manifest = json.loads(args.corpus_manifest.read_text(encoding='utf-8'))
    corpus_sha = sha(args.corpus)
    if (manifest.get('source') != 'Miwa-Keita/zenz-v2.5-dataset' or
            manifest.get('revision') != REVISION or
            corpus_sha not in {CORPUS_SHA, CANONICAL_CORPUS_SHA} or
            manifest.get('sample_sha256') != corpus_sha):
        raise ValueError('固定コーパスの識別またはSHAが一致しません')
    chunks = {chunk['group']: chunk for chunk in manifest['chunks']}
    if any(chunks.get(group, {}).get('license') != 'CC-BY-SA-4.0' for group in TRAIN_GROUPS):
        raise ValueError('Wikipedia学習区間のライセンス識別が不足しています')
    exclusions = set()
    for path in args.exclude:
        exclusions.update(evaluation_readings(json.loads(path.read_text(encoding='utf-8'))))
    if not exclusions:
        raise ValueError('評価入力の除外集合が空です')
    with args.corpus.open(encoding='utf-8') as stream:
        phrases, audit = collect((json.loads(line) for line in stream), exclusions)
    baseline = set(args.baseline.read_text(encoding='utf-8').splitlines()) - {''}
    combined = sorted(baseline | set(phrases))
    if not phrases or len(combined) > 100000:
        raise ValueError('連語抽出が空または想定サイズを超えました')
    args.out.mkdir(parents=True, exist_ok=False)
    output = args.out / 'fast_collocations.txt'
    output.write_text(''.join(value + '\n' for value in combined), encoding='utf-8', newline='\n')
    receipt = dict(
        source=manifest['source'], revision=REVISION, corpus_sha256=corpus_sha,
        corpus_manifest_sha256=sha(args.corpus_manifest), generator_sha256=sha(__file__),
        baseline_sha256=sha(args.baseline), exclusion_files={str(p.resolve()): sha(p) for p in args.exclude},
        exclusion_readings=len(exclusions), used_groups=sorted(TRAIN_GROUPS),
        license='CC-BY-SA-4.0 (additional phrases); BSD-3-Clause (upstream baseline)',
        min_length=MIN_LENGTH, max_length=MAX_LENGTH,
        min_distinct_sentences=MIN_SENTENCES, min_source_groups=MIN_GROUPS,
        audit=audit, baseline_entries=len(baseline), combined_entries=len(combined),
        result_sha256=sha(output), evaluation_labels_used=False,
    )
    (args.out / 'fast_collocations.json').write_text(json.dumps(receipt, ensure_ascii=False, indent=2), encoding='utf-8')
    print(json.dumps(receipt, ensure_ascii=False, indent=2))


if __name__ == '__main__':
    main()
