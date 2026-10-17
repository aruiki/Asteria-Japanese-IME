"""Mix observed converter preferences with ordinary corpus pairs; exclude eval inputs."""
import argparse
import hashlib
import json
import math
from pathlib import Path
import random
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'mozc/tools'))
from conversion_training import reading_key, evaluation_readings, grams


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def load_observed(path, split):
    raw = Path(path).read_bytes()
    manifest = json.loads(Path(path).with_name('manifest.json').read_text(encoding='utf8'))
    if hashlib.sha256(raw).hexdigest() != manifest['observations_sha256']:
        raise ValueError('Observed candidate SHA mismatch')
    rows = [json.loads(line) for line in raw.splitlines()]
    if any(r['split'] != split for r in rows):
        raise ValueError('Observed split mismatch')
    pairs = []
    for row in rows:
        references = set(row['references'])
        alternatives = {}
        positives = [c for c in row['candidates'] if c['text'] in references
                     and all(math.isfinite(float(c[k])) for k in ('zenz', 'llm'))]
        if not positives:
            continue
        positive = max(positives, key=lambda c: float(c['zenz']) + 1.2 * float(c['llm']))
        for c in row['candidates']:
            score = float(c['zenz']) + 1.2 * float(c['llm'])
            if c['text'] not in references and math.isfinite(score):
                if c['text'] not in alternatives or score > alternatives[c['text']][0]:
                    alternatives[c['text']] = (score, float(c['llm']))
        for negative in sorted(alternatives, key=lambda t: alternatives[t][0], reverse=True)[:3]:
            pairs.append(dict(reading=row['reading'], context=row['context'],
                              positive=positive['text'], negative=negative,
                              frozen_llm_score_difference=float(positive['llm']) - alternatives[negative][1],
                              split=split, source_groups=row['source_groups'],
                              kind='native_contrast', supervision='corpus_reference_preference',
                              alternatives_semantically_reviewed=False))
    return pairs


def exclusion_filter(excluded):
    index = {}
    for value in excluded:
        g = grams(value)
        if g:
            index.setdefault(len(g), []).append(g)
    cache = {}
    def overlaps(key):
        if key not in cache:
            g = grams(key)
            cache[key] = key in excluded or bool(g and any(
                len(g & other) / len(g | other) >= .8
                for size, group in index.items()
                if min(len(g), size) / max(len(g), size) >= .8
                for other in group))
        return cache[key]
    return overlaps


def validate(pairs, excluded):
    keys = {'train': set(), 'dev': set()}
    groups = {'train': set(), 'dev': set()}
    overlaps = exclusion_filter(excluded)
    for r in pairs:
        if r['split'] not in keys or not r.get('source_groups'):
            raise ValueError('Missing split or corpus groups')
        key = reading_key(r['reading'])
        if overlaps(key):
            raise ValueError('Evaluation reading leaked into training')
        keys[r['split']].add(key)
        groups[r['split']].update(r['source_groups'])
        if not r['positive'] or not r['negative'] or r['positive'] == r['negative']:
            raise ValueError('Empty or identical pair')
        if not math.isfinite(r.get('frozen_llm_score_difference', 0.)):
            raise ValueError('Non-finite frozen LLM score')
    if keys['train'] & keys['dev'] or groups['train'] & groups['dev']:
        raise ValueError('Train/dev reading or corpus-group leakage')
    near_dev = exclusion_filter(keys['dev'])
    if any(near_dev(key) for key in keys['train']):
        raise ValueError('Near-duplicate train/dev reading leakage')


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--observed', type=Path, required=True)
    ap.add_argument('--general', type=Path, required=True)
    ap.add_argument('--out', type=Path, required=True)
    ap.add_argument('--source-root', type=Path, default=Path.cwd())
    args = ap.parse_args()
    if args.out.exists() or args.out.with_suffix('.manifest.json').exists():
        raise FileExistsError('Preserve prior dataset outputs')
    source_manifest = json.loads(args.general.with_suffix('.manifest.json').read_text(encoding='utf8'))
    if digest(args.general) != source_manifest['pairs_sha256']:
        raise ValueError('General corpus SHA mismatch')
    excluded = set()
    exclusions = []
    for item in source_manifest['excluded_evaluations']:
        path = Path(item['file'])
        if not path.is_absolute():
            path = args.source_root / path
        if digest(path) != item['sha256']:
            raise ValueError('Evaluation source changed')
        excluded.update(evaluation_readings(json.loads(path.read_text(encoding='utf8'))))
        exclusions.append(dict(file=str(path.resolve()), sha256=item['sha256']))
    pairs = []
    inputs = {}
    for split in ('train', 'dev'):
        path = args.observed / f'{split}-collected/observations.jsonl'
        pairs.extend(load_observed(path, split))
        inputs[split] = digest(path)
    native_keys = {reading_key(r['reading']) for r in pairs}
    overlaps = exclusion_filter(excluded)
    rng = random.Random(20261017)
    general = [json.loads(line) for line in args.general.read_bytes().splitlines()]
    for split, limit in (('train', 2000), ('dev', 256)):
        rows = [r for r in general if r['split'] == split and r.get('kind', 'original') == 'original'
                and reading_key(r['reading']) not in native_keys
                and not overlaps(reading_key(r['reading']))]
        rng.shuffle(rows)
        selected, seen = [], set()
        for row in rows:
            key = reading_key(row['reading'])
            if key in seen:
                continue
            seen.add(key)
            selected.append(dict(row, source_groups=[row['source_group']]))
            if len(selected) >= limit:
                break
        pairs.extend(selected)
    validate(pairs, excluded)
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(''.join(json.dumps(r, ensure_ascii=False) + '\n' for r in pairs), encoding='utf8')
    manifest = dict(pairs_sha256=digest(args.out), observed_sha256=inputs,
                    general_pairs_sha256=digest(args.general), tool_sha256=digest(__file__),
                    excluded_evaluations=exclusions,
                    counts={f'{s}/{k}': sum(r['split']==s and r.get('kind','original')==k for r in pairs)
                            for s in ('train','dev') for k in ('original','native_contrast')},
                    supervision='corpus_reference_preference', product_adopted=False)
    args.out.with_suffix('.manifest.json').write_text(json.dumps(manifest, indent=2), encoding='utf8')
    print(json.dumps(manifest['counts']), flush=True)


if __name__ == '__main__':
    main()
