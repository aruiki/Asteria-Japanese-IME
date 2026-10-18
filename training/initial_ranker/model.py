"""初期順位のオフライン実験。製品の打鍵経路からは読み込まない。"""
from collections import Counter
import gzip
import json
import math
from pathlib import Path
import re
import unicodedata


def normalize(text):
    if any(c in text for c in ('\x02', '\x03')):
        raise ValueError('Reserved boundary character')
    return ' '.join(unicodedata.normalize('NFKC', text).split())


class CharacterModel:
    def __init__(self, counts):
        self.counts = counts
        self.total = sum(counts[0].values())
        if not self.total:
            raise ValueError('Empty training text')
        self.contexts = [Counter(), Counter()]
        for n in (1, 2):
            for key, count in counts[n].items():
                self.contexts[n - 1][key[:-1]] += count

    @classmethod
    def fit(cls, texts):
        counts = [Counter(), Counter(), Counter()]
        for text in texts:
            chars = '\x02\x02' + normalize(text) + '\x03'
            for i in range(2, len(chars)):
                for order in (1, 2, 3):
                    counts[order - 1][chars[i - order + 1:i + 1]] += 1
        return cls(counts)

    def cost(self, text, reading_length):
        chars = '\x02\x02' + normalize(text) + '\x03'
        loss = 0.
        for i in range(2, len(chars)):
            char = chars[i]
            probability = (self.counts[0][char] + .1) / (self.total + .1 * (len(self.counts[0]) + 1))
            for order in (2, 3):
                key = chars[i - order + 1:i + 1]
                probability = (self.counts[order - 1][key] + 8 * probability) / (self.contexts[order - 2][key[:-1]] + 8)
            loss -= math.log(probability)
        return loss / (len(chars) - 2) * max(1, reading_length)

    def save(self, path):
        # 同じモデルのSHAを再現するためgzipの時刻・ファイル名も固定する。
        payload = json.dumps([dict(sorted(c.items())) for c in self.counts], ensure_ascii=False,
                             separators=(',', ':')).encode('utf-8')
        with Path(path).open('xb') as stream:
            with gzip.GzipFile(fileobj=stream, filename='', mode='wb', mtime=0) as packed:
                packed.write(payload)

    @classmethod
    def load(cls, path):
        with gzip.open(path, 'rt', encoding='utf-8') as stream:
            return cls([Counter(c) for c in json.load(stream)])


WEIGHTS = (0, 25, 50, 100, 200, 400)
MARGINS = (0, 100, 250)
MAX_COST_DELTA = 3453


def preserves_explicit_forms(current, candidate):
    """文脈の弱い文字統計で数字表記・英字・区切りを勝手に変えない。"""
    current, candidate = normalize(current), normalize(candidate)
    numbers = r'[0-9一二三四五六七八九十百千万億兆〇零壱弐参拾]+'
    original_numbers = re.findall(numbers, current)
    if original_numbers and original_numbers != re.findall(numbers, candidate):
        return False
    if re.findall('[A-Za-z]+', current) != re.findall('[A-Za-z]+', candidate):
        return False
    separators = lambda text: ''.join(c for c in text if unicodedata.category(c)[0] in 'PS')
    return separators(current) == separators(candidate)


def ranked_choice(row, weight, margin):
    """現在の候補を対照にし、元コストと統計スコア両方で判断する。"""
    if not weight:
        return row['top1']
    current = next(c for c in row['scored'] if c['text'] == row['top1'])
    eligible = [c for c in row['scored'] if preserves_explicit_forms(row['top1'], c['text'])]
    winner = min(eligible, key=lambda c: c['cost_delta'] + weight * c['stat_cost'])
    gain = (current['cost_delta'] + weight * current['stat_cost'] -
            winner['cost_delta'] - weight * winner['stat_cost'])
    return winner['text'] if gain > margin else row['top1']


def select_configuration(rows):
    if not rows:
        raise ValueError('Empty calibration')
    results = []
    for weight in WEIGHTS:
        for margin in MARGINS:
            choices = [ranked_choice(row, weight, margin) for row in rows]
            improvements = sum(row['top1'] not in row['references'] and value in row['references']
                               for row, value in zip(rows, choices))
            regressions = sum(row['top1'] in row['references'] and value not in row['references']
                              for row, value in zip(rows, choices))
            results.append(dict(weight=weight, margin=margin, improvements=improvements,
                                regressions=regressions, changed=sum(row['top1'] != value for row, value in zip(rows, choices))))
    # 評価セットの結果は選択へ渡さない。改悪を優先し、その条件下の改善を最大化。
    chosen = min(results, key=lambda r: (r['regressions'], -r['improvements'], r['weight'], -r['margin']))
    return chosen, results
