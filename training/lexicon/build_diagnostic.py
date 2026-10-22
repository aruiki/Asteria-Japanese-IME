"""追加語彙の探索用診断を出力前に固定する。一般精度や独立評価には使わない。"""
import argparse
from collections import defaultdict
import hashlib
import json
from pathlib import Path
import unicodedata

from build_lexicon import POS_MAPPING
from collect_frequency import Exclusions, evaluation_readings
from fetch_sources import sha


def width_variants(surface):
    normalized = unicodedata.normalize('NFKC', surface)
    full = ''.join(chr(ord(c) + 0xfee0) if 33 <= ord(c) <= 126 else c for c in normalized)
    return {surface, normalized, full}


def choose(groups, excluded, policy):
    overlap = Exclusions(excluded)
    by_pos = defaultdict(list)
    for reading, item in groups.items():
        if (not policy['min_reading_length'] <= len(reading) <= policy['max_reading_length'] or
                overlap.overlaps(reading) or len(item['targets']) > policy['max_target_aliases'] or
                len(item['expected']) > policy['max_expected_canonical_forms']):
            continue
        for pos in item['positions']:
            key = hashlib.sha256((policy['seed'] + '\0' + reading).encode('utf-8')).hexdigest()
            by_pos[pos].append((key, reading))
    selected, seen = [], set()
    for pos, values in sorted(by_pos.items()):
        count = 0
        for _, reading in sorted(values):
            if reading in seen:
                continue
            item = groups[reading]
            selected.append(dict(index=f'lexical-probe-{len(selected) + 1:03}', input=reading,
                                 expected_output=sorted({value for surface in item['expected'] for value in width_variants(surface)}),
                                 target_surfaces=sorted(item['targets']), context_text='', domain=pos,
                                 scope='dictionary-derived lexical diagnostic, not independent evaluation'))
            seen.add(reading);count += 1
            if count >= policy['per_pos_limit']:
                break
    return selected


def build(candidate, frequency, mozc, exclusions, out):
    policy_path = Path(__file__).with_name('diagnostic_policy.json')
    policy = json.loads(policy_path.read_text('utf-8'))
    lexical = json.loads((candidate / 'short_lexicon.json').read_text('utf-8'))
    if sha(candidate / 'short_lexicon.tsv') != lexical['sha256']:
        raise ValueError('診断対象の辞書候補SHAが一致しません')
    counts = json.loads((frequency / 'frequency.json').read_text('utf-8'))
    expected = next(item for item in counts['outputs'] if item['file'] == 'word_frequency.tsv')
    if counts['limit_bytes'] or sha(frequency / 'word_frequency.tsv') != expected['sha256']:
        raise ValueError('固定全量頻度が必要です')
    directory = mozc / 'src/data/dictionary_oss'
    for name, expected_sha in lexical['mozc_source_inputs'].items():
        if sha(directory / name) != expected_sha:
            raise ValueError('Mozcの基準辞書が一致しません')
    ids = {int(index): text for index, text in (line.split(' ', 1) for line in
           (directory / 'id.def').read_text('utf-8').splitlines())}
    noun_ids = {index for index, pos in ids.items() if pos in set(POS_MAPPING.values())}
    eligible = {}
    with (candidate / 'short_lexicon.tsv').open(encoding='utf-8') as stream:
        for line in stream:
            reading, left, right, cost, surface = line.rstrip('\n').split('\t')
            if policy['min_reading_length'] <= len(reading) <= policy['max_reading_length'] and len(surface) >= policy['min_surface_length']:
                eligible[(reading, unicodedata.normalize('NFKC', surface), ids[int(left)])] = surface
    groups = defaultdict(lambda: dict(targets=set(), expected=set(), positions=set()))
    with (frequency / 'word_frequency.tsv').open(encoding='utf-8') as stream:
        for line in stream:
            reading, surface, pos, occurrences, documents, buckets = line.rstrip('\n').split('\t')
            key = reading, surface, pos
            if key in eligible and int(documents) >= policy['min_documents']:
                item = groups[reading];item['targets'].add(eligible[key]);item['expected'].add(eligible[key]);item['positions'].add(pos)
    for name in sorted(lexical['mozc_source_inputs']):
        if not name.startswith('dictionary'):
            continue
        with (directory / name).open(encoding='utf-8') as stream:
            for line in stream:
                reading, left, right, cost, surface, *_ = line.rstrip('\n').split('\t')
                if reading in groups and int(left) in noun_ids and int(right) in noun_ids:
                    groups[reading]['expected'].add(surface)
    excluded = set()
    for path in exclusions:
        excluded.update(evaluation_readings(json.loads(path.read_text('utf-8'))))
    if not excluded:
        raise ValueError('既存評価の除外集合が空です')
    result = choose(groups, excluded, policy)
    if len(result) < 100:
        raise ValueError('十分な分野数の診断を作れませんでした')
    out.mkdir(parents=True, exist_ok=False)
    data = out / 'lexical-diagnostic.json'
    data.write_text(json.dumps(result, ensure_ascii=False, indent=2) + '\n', encoding='utf-8', newline='\n')
    metadata = dict(policy=policy, policy_sha256=sha(policy_path), generator_sha256=sha(__file__),
                    lexical_sha256=lexical['sha256'], frequency_sha256=expected['sha256'],
                    exclusion_files={str(path.resolve()): sha(path) for path in exclusions},
                    mozc_source_inputs=lexical['mozc_source_inputs'], items=len(result), sha256=sha(data),
                    independent_evaluation=False, representative_general_accuracy=False,
                    evaluation_labels_used_for_selection=False, source_lexical_references_used=True)
    (out / 'manifest.json').write_text(json.dumps(metadata, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    (out / 'NOTICE.txt').write_text((candidate / 'NOTICE-short-lexicon.txt').read_text('utf-8'), encoding='utf-8')
    return metadata


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('candidate', 'frequency', 'mozc', 'out'):
        parser.add_argument('--' + name, type=Path, required=True)
    parser.add_argument('--exclude', type=Path, action='append', required=True)
    args = parser.parse_args()
    print(json.dumps(build(args.candidate, args.frequency, args.mozc, args.exclude, args.out), ensure_ascii=False, indent=2))
