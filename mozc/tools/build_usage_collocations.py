"""既存用例辞書の明示的な例文を、初期連語のコンパイル入力へ加える実験。"""
import argparse
from collections import Counter, defaultdict
import hashlib
import json
from pathlib import Path
import re
import unicodedata

from build_fast_collocations import fragments

USAGE_SHA = '9a5ff4ab0be399e01e6aeb8637a166d421299f92a3c39df93fab6bcbaf3e68b8'
BASE_SHA = 'f576ffc696ad81f244fb5791a2571ac1edb320f2a2f20d0773ea6a044f01b29c'
CFORMS_SHA = '947d82de803181bb9df8bb4fede6d0d3c3677c7de88c946ff3a35dbfc2bce0ab'
CONTENT = re.compile(r'[\u3400-\u4dbf\u4e00-\u9fffァ-ヺ]')


def digest(path):
    with Path(path).open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def forms(text):
    result = defaultdict(dict)
    for line in text.splitlines():
        if not line or line.startswith('#'):
            continue
        fields = line.split()
        if len(fields) < 4:
            raise ValueError('Malformed Mozc conjugation rule')
        pos, kind, suffix = fields[:3]
        suffix = '' if suffix == '*' else suffix
        result[pos].setdefault('suffixes', set()).add(suffix)
        if kind == '基本形':
            result[pos][kind] = suffix
    return result


def extract(text, conjugations):
    values, audit = set(), Counter()
    for line in text.splitlines():
        fields = line.split('\t')
        if len(fields) != 4:
            raise ValueError('Malformed usage dictionary row')
        reading, word, pos, meaning = fields
        base = conjugations.get(pos, {}).get('基本形', '')
        stem = word[:-len(base)] if base and word.endswith(base) else word
        stem = stem or word
        for example in re.findall(r'「([^「」]+)」', meaning):
            if not 3 <= len(example) <= 96 or stem not in example:
                continue
            variants = {unicodedata.normalize('NFKC', example)}
            if base and stem != word and example.endswith(word):
                prefix = example[:-len(word)]
                variants.update(prefix + stem + suffix for suffix in conjugations[pos]['suffixes'])
                audit['ending_conjugations'] += len(variants) - 1
            for variant in variants:
                for value in fragments(variant):
                    if stem in value and CONTENT.search(value.replace(stem, '')):
                        values.add(value)
            audit['eligible_quotes'] += 1
    audit['phrases'] = len(values)
    return sorted(values), dict(audit)


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    for name in ('baseline', 'usage', 'cforms', 'out'):
        ap.add_argument('--' + name, type=Path, required=True)
    args = ap.parse_args()
    if digest(args.baseline) != BASE_SHA or digest(args.usage) != USAGE_SHA or digest(args.cforms) != CFORMS_SHA:
        raise ValueError('Fixed baseline/usage SHA mismatch')
    phrases, audit = extract(args.usage.read_text(encoding='utf-8'), forms(args.cforms.read_text(encoding='utf-8')))
    baseline = set(args.baseline.read_text(encoding='utf-8').splitlines())
    result = sorted(baseline | set(phrases))
    if not phrases or len(result) > 50000:
        raise ValueError('Empty/excessive extracted collocations')
    args.out.mkdir(parents=True, exist_ok=False)
    output = args.out/'fast_collocations.txt'
    output.write_text(''.join(value+'\n' for value in result), encoding='utf-8', newline='\n')
    manifest = dict(source='hiroyuki-komatsu/japanese-usage-dictionary', tag='2025-01-25',
                    usage_sha256=digest(args.usage), cforms_sha256=digest(args.cforms),
                    baseline_sha256=digest(args.baseline), generator_sha256=digest(__file__),
                    baseline_entries=len(baseline), extracted_entries=len(phrases),
                    added_entries=len(result)-len(baseline), combined_entries=len(result), audit=audit,
                    result_sha256=digest(output), bytes=output.stat().st_size,
                    license='BSD-2-Clause additional usage phrases; previous baseline licenses retained',
                    evaluation_labels_used=False, product_adopted=False)
    (args.out/'fast_collocations.json').write_text(json.dumps(manifest, indent=2), encoding='utf-8')
    print(json.dumps(manifest), flush=True)


if __name__ == '__main__': main()
