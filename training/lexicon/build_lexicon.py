"""版固定の公開語彙を、共通語のコスト分布でMozc名詞辞書候補へ変換する。"""
import argparse
from bisect import bisect_left, bisect_right
from collections import Counter, defaultdict
import csv
import io
import json
from pathlib import Path
import re
import time
import unicodedata
import zipfile

from fetch_sources import load_sources, sha

READING = re.compile(r'[ぁ-ゖー]{2,40}')
POS_MAPPING = {
    ('普通名詞', '一般', '*'): '名詞,一般,*,*,*,*,*',
    ('普通名詞', 'サ変可能', '*'): '名詞,サ変接続,*,*,*,*,*',
    ('固有名詞', '一般', '*'): '名詞,固有名詞,一般,*,*,*,*',
    ('固有名詞', '人名', '一般'): '名詞,固有名詞,人名,一般,*,*,*',
    ('固有名詞', '人名', '名'): '名詞,固有名詞,人名,名,*,*,*',
    ('固有名詞', '人名', '姓'): '名詞,固有名詞,人名,姓,*,*,*',
    ('固有名詞', '地名', '一般'): '名詞,固有名詞,地域,一般,*,*,*',
    ('固有名詞', '地名', '国'): '名詞,固有名詞,地域,国,*,*,*',
}
MIN_COMMON = 100
KNOWN_READING_MARGIN = 1000


def candidate(row):
    """解答/評価IDを受け取らず、辞書全体に同じ規則を使う。"""
    if row['POS1'] != '名詞':
        return None, 'not_noun'
    pos = POS_MAPPING.get((row['POS2'], row['POS3'], row['POS4']))
    if pos is None:
        return None, 'unsupported_pos'
    if int(row['LeftId']) < 0 or int(row['RightId']) < 0:
        return None, 'not_indexed'
    reading = row['ReadingForm'] or row['IndexForm']
    reading = ''.join(chr(ord(c) - 0x60) if 'ァ' <= c <= 'ヶ' else c for c in reading)
    surface = row['Headword'] or row['IndexForm']
    if (not READING.fullmatch(reading) or not 1 <= len(surface) <= 32 or
            not all(unicodedata.category(c)[0] in 'LMN' or c == '・' for c in surface)):
        return None, 'invalid_reading_or_surface'
    cost = int(row['Cost'])
    if not -32768 <= cost <= 32767:
        raise ValueError('元辞書の単語コストが範囲外です')
    return (reading, surface, pos, cost), ''


def rows(source):
    for group in ('small', 'core', 'notcore'):
        with zipfile.ZipFile(source / (group + '_lex.zip')) as archive:
            with archive.open(group + '_lex.csv') as stream:
                for row in csv.DictReader(io.TextIOWrapper(stream, encoding='utf-8')):
                    yield row


def common_quantile(raw_cost, raw_common, mozc_common):
    # 同じ元コストの語は同じ分位へ対応する。元辞書の数値を直接移さない。
    q = (bisect_left(raw_common, raw_cost) + bisect_right(raw_common, raw_cost)) / (2 * len(raw_common))
    q = max(0.1, min(0.95, q))
    return mozc_common[round(q * (len(mozc_common) - 1))]


def build(source, mozc, out, config):
    started = time.monotonic()
    out.mkdir(parents=True, exist_ok=False)
    directory = mozc / 'src/data/dictionary_oss'
    ids = {text: int(index) for index, text in (line.split(' ', 1) for line in
           (directory / 'id.def').read_text('utf-8').splitlines())}
    missing = set(POS_MAPPING.values()) - ids.keys()
    if missing:
        raise ValueError('Mozcに品詞がありません: ' + repr(missing))
    known, matching_pos, reading_min, reading_pos_min = {}, {}, {}, {}
    noun_ids = {ids[pos] for pos in POS_MAPPING.values()}
    inputs = sorted(directory.glob('dictionary[0-9][0-9].txt'))
    if len(inputs) != 10:
        raise ValueError('固定Mozcの辞書10ファイルが必要です')
    for file in inputs:
        for line in file.read_text('utf-8').splitlines():
            fields = line.split('\t')
            reading, left_id, right_id, cost, surface = fields[:5]
            cost = int(cost)
            pair = reading, surface
            known[pair] = min(cost, known.get(pair, cost))
            if left_id == right_id and int(left_id) in noun_ids:
                matching_key = (reading, surface, int(left_id))
                matching_pos[matching_key] = min(cost, matching_pos.get(matching_key, cost))
                reading_pos_key = reading, int(left_id)
                reading_pos_min[reading_pos_key] = min(cost, reading_pos_min.get(reading_pos_key, cost))
            reading_min[reading] = min(cost, reading_min.get(reading, cost))
    pairs, common, audit = {}, defaultdict(dict), Counter()
    for row in rows(source):
        audit['source_rows'] += 1
        word, reason = candidate(row)
        if word is None:
            audit[reason] += 1
            continue
        reading, surface, pos, raw_cost = word
        pair = reading, surface
        if pair in known:
            # 重複するソース行に較正分布を偏らせない。
            matching_key = (reading, surface, ids[pos])
            if matching_key in matching_pos:
                common[pos][pair] = (min(raw_cost, common[pos].get(pair, (raw_cost, 0))[0]),
                                     matching_pos[matching_key])
            audit['already_in_mozc'] += 1
            continue
        key = reading, surface, pos
        pairs[key] = min(raw_cost, pairs.get(key, raw_cost))
    distributions = {pos: (sorted(raw for raw, _ in values.values()), sorted(cost for _, cost in values.values()))
                     for pos, values in common.items() if len(values) >= MIN_COMMON}
    count = Counter()
    path = out / 'short_lexicon.tsv'
    with path.open('w', encoding='utf-8', newline='\n') as stream:
        for (reading, surface, pos), raw in sorted(pairs.items()):
            if pos not in distributions:
                audit['insufficient_common_pos'] += 1
                continue
            mapped = common_quantile(raw, *distributions[pos])
            if reading in reading_min:
                mapped = max(mapped, reading_min[reading] + KNOWN_READING_MARGIN)
            # 数詞/助数詞など異なる接続IDの安い経路を、一般名詞の下限に流用しない。
            if (reading, ids[pos]) in reading_pos_min:
                mapped = max(mapped, reading_pos_min[(reading, ids[pos])] + KNOWN_READING_MARGIN)
            if mapped > 32767:
                audit['cost_overflow'] += 1
                continue
            stream.write(f'{reading}\t{ids[pos]}\t{ids[pos]}\t{mapped}\t{surface}\n')
            count[pos] += 1
    summary = dict(source=config['repository'], version=config['version'], source_commit=config['commit'],
                   source_config_sha256=sha(Path(__file__).with_name('sources.json')),
                   generator_sha256=sha(__file__), audit=dict(audit), output_entries=sum(count.values()),
                   by_pos=dict(count), calibration_common={p: len(v) for p, v in common.items()},
                   min_common=MIN_COMMON, known_reading_margin=KNOWN_READING_MARGIN,
                   same_pos_reading_floor=True,
                   mozc_source_inputs={p.name: sha(p) for p in inputs + [directory / 'id.def']},
                   bytes=path.stat().st_size, sha256=sha(path), seconds=time.monotonic() - started,
                   evaluation_labels_used=False, product_adopted=False)
    (out / 'short_lexicon.json').write_text(json.dumps(summary, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    notice = ('Asteria短文向け語彙候補 / SudachiDict ' + config['version'] + '\n'
              '出典: Works Applications Co., Ltd. / https://github.com/WorksApplications/SudachiDict\n'
              '変更: 名詞の読み/表記を抽出し、Mozc品詞・共通語彙コスト分布へ対応。既存語は上書きしない。\n'
              '元の接続行列・形態素解析エンジンは同梱しない。\n\n')
    notice += (source / 'LEGAL').read_text('utf-8') + '\n\n' + (source / 'LICENSE-2.0.txt').read_text('utf-8')
    (out / 'NOTICE-short-lexicon.txt').write_text(notice, encoding='utf-8', newline='\n')
    return summary


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source', type=Path, required=True)
    parser.add_argument('--mozc', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    config = load_sources(args.source)
    print(json.dumps(build(args.source, args.mozc, args.out, config), ensure_ascii=False, indent=2), flush=True)
