"""固定Wikipedia区間の初期統計モデルと、別取得区間の較正入力を作る。"""
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import random
import sys

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'mozc/tools'))
sys.path.insert(0, str(ROOT / 'training/contrastive'))
from conversion_training import reading_key, evaluation_readings
from native_pairs import exclusion_filter
from model import CharacterModel, normalize


def digest(path):
    with Path(path).open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def split_rows(rows, excluded, count=256):
    overlaps = exclusion_filter(excluded)
    buckets = {'train': [], 'dev': []}
    seen = set()
    audit = Counter()
    for row in rows:
        group = row.get('source_group', '')
        if row.get('split') != 'train' or group not in {f'train_wikipedia.jsonl:{n:02}' for n in range(1, 8)}:
            audit['wrong_source_group'] += 1
            continue
        reading = reading_key(row['input'])
        text = normalize(row['output'])
        if not 3 <= len(text) <= 512 or not reading or any(c.isspace() or ord(c) < 32 for c in reading):
            audit['invalid'] += 1
            continue
        if overlaps(reading):
            audit['evaluation_reading_or_near'] += 1
            continue
        identity = (reading, text)
        if identity in seen:
            audit['duplicate'] += 1
            continue
        seen.add(identity)
        which = 'dev' if group.endswith(':07') else 'train'
        buckets[which].append(dict(input=reading, output=text, source_group=group))
    random.Random(20261018).shuffle(buckets['dev'])
    selected, readings = [], set()
    for row in buckets['dev']:
        if 10 <= len(row['input']) <= 80 and row['input'] not in readings:
            readings.add(row['input'])
            selected.append(row)
            if len(selected) == count:
                break
    if len(selected) != count:
        raise ValueError('Insufficient distinct calibration inputs')
    overlap_dev = exclusion_filter(readings)
    texts = {row['output'] for row in selected}
    train = [row for row in buckets['train'] if row['output'] not in texts and not overlap_dev(row['input'])]
    audit['train_dev_overlap_removed'] = len(buckets['train']) - len(train)
    return train, selected, dict(audit)


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--corpus', type=Path, required=True)
    ap.add_argument('--ajimee', type=Path, required=True)
    ap.add_argument('--out', type=Path, required=True)
    args = ap.parse_args()
    source = ROOT / 'training/collocations/sources.json'
    config = json.loads(source.read_text(encoding='utf-8'))
    if digest(args.corpus) != config['sample_sha256']:
        raise ValueError('Fixed Wikipedia corpus SHA mismatch')
    if digest(args.ajimee) != 'e9eb668fd6aa14b1e26436f429b5550108af0a1dfd443b8cea0bcb3ab3028fca':
        raise ValueError('AJIMEE exclusion SHA mismatch')
    evaluations = sorted((ROOT / 'eval/sets').glob('*.json')) + [ROOT / 'eval/normal-conversion-basics.json', args.ajimee]
    excluded = set()
    for path in evaluations:
        excluded.update(evaluation_readings(json.loads(path.read_text(encoding='utf-8'))))
    with args.corpus.open(encoding='utf-8') as stream:
        train, dev, audit = split_rows((json.loads(line) for line in stream), excluded)
    args.out.mkdir(parents=True, exist_ok=False)
    model = CharacterModel.fit(row['output'] for row in train)
    model_path = args.out / 'character-model.json.gz'
    model.save(model_path)
    for split, selected in [('train', train), ('calibration', dev)]:
        path = args.out / (split + '.jsonl')
        path.write_text(''.join(json.dumps(row, ensure_ascii=False) + '\n' for row in selected), encoding='utf-8', newline='\n')
    manifest = dict(source=config['source'], revision=config['revision'], corpus_sha256=digest(args.corpus),
                    generator_sha256=digest(__file__), model_script_sha256=digest(Path(__file__).with_name('model.py')),
                    model_sha256=digest(model_path), model_bytes=model_path.stat().st_size,
                    train_sha256=digest(args.out/'train.jsonl'), calibration_sha256=digest(args.out/'calibration.jsonl'),
                    exclusion_files={str(p): digest(p) for p in evaluations}, exclusion_readings=len(excluded),
                    train_count=len(train), calibration_count=len(dev), audit=audit,
                    ngram_counts=[len(c) for c in model.counts], product_adopted=False, evaluation_labels_used=False)
    (args.out / 'manifest.json').write_text(json.dumps(manifest, indent=2), encoding='utf-8')
    print(json.dumps({k: v for k, v in manifest.items() if k != 'exclusion_files'}), flush=True)


if __name__ == '__main__':
    main()
