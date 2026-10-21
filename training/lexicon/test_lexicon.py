"""既存語の保護、読み/品詞の境界、固定原票の破損拒否を検査する。"""
import hashlib
import csv
import io
import json
import tempfile
from pathlib import Path
import unittest
import zipfile

from build_lexicon import POS_MAPPING, build, candidate, common_quantile
from fetch_sources import fetch, verify


def word(**changes):
    row = dict(IndexForm='新語', Headword='', ReadingForm='シンゴ', Cost='5000',
               POS1='名詞', POS2='普通名詞', POS3='一般', POS4='*', LeftId='1', RightId='1')
    return {**row, **changes}


class LexiconTest(unittest.TestCase):
    def test_reading_and_original_case_are_preserved(self):
        result, _ = candidate(word(IndexForm='example', Headword='Example', ReadingForm='エグザンプル'))
        self.assertEqual(result[:2], ('えぐざんぷる', 'Example'))

    def test_person_and_geographic_pos_are_not_general_nouns(self):
        result, _ = candidate(word(POS2='固有名詞', POS3='人名', POS4='姓',
                                   IndexForm='𠮷田', ReadingForm='ヨシダ'))
        self.assertIn(',人名,姓,', result[2])
        self.assertEqual(result[1], '𠮷田')
        result, _ = candidate(word(POS2='固有名詞', POS3='地名', POS4='一般'))
        self.assertIn(',地域,一般,', result[2])

    def test_bad_readings_control_text_and_unindexed_rows_are_rejected(self):
        for changes in [dict(ReadingForm='しん\nご'), dict(ReadingForm='*'), dict(ReadingForm='ABC'),
                        dict(Headword='新\t語'), dict(Headword='空 白'), dict(Headword=r'\u002c'),
                        dict(LeftId='-1'), dict(POS1='動詞'), dict(POS3='未知の品詞')]:
            with self.subTest(changes=changes):
                self.assertIsNone(candidate(word(**changes))[0])

    def test_cost_mapping_is_monotone_and_keeps_equal_cost_ties_equal(self):
        raw = [1, 1, 4, 9, 10]
        target = [3000, 5000, 7000, 9000, 11000]
        costs = [common_quantile(c, raw, target) for c in [-10, 1, 4, 8, 9, 10, 30]]
        self.assertEqual(costs, sorted(costs))
        self.assertEqual(common_quantile(1, raw, target), 5000)
        self.assertGreaterEqual(min(costs), 3000)

    def test_corrupt_cache_is_not_used_or_overwritten(self):
        with tempfile.TemporaryDirectory() as name:
            root = Path(name);cache = root / 'cache';cache.mkdir();out = root / 'out';out.mkdir()
            source = cache / 'data';source.write_bytes(b'wrong')
            spec = dict(name='data', bytes=5, sha256=hashlib.sha256(b'right').hexdigest(),
                        url='https://invalid.example/data')
            with self.assertRaises(ValueError):
                fetch(spec, out, cache)
            self.assertEqual(source.read_bytes(), b'wrong')
            self.assertFalse((out / 'data').exists())

    def test_zip_with_unexpected_member_is_rejected_even_with_matching_hash(self):
        with tempfile.TemporaryDirectory() as name:
            path = Path(name) / 'data.zip'
            with zipfile.ZipFile(path, 'w') as archive:
                archive.writestr('../unexpected.csv', 'word')
            spec = dict(name='data.zip', bytes=path.stat().st_size,
                        sha256=hashlib.sha256(path.read_bytes()).hexdigest(),
                        members=[dict(name='lex.csv', bytes=4)])
            with self.assertRaises(ValueError):
                verify(path, spec)

    def test_generated_dictionary_does_not_replace_known_words_and_protects_reading_cost(self):
        with tempfile.TemporaryDirectory() as name:
            root = Path(name);source = root / 'source';source.mkdir()
            directory = root / 'mozc/src/data/dictionary_oss';directory.mkdir(parents=True)
            positions = list(dict.fromkeys(POS_MAPPING.values()))
            ids = {pos: i + 1 for i, pos in enumerate(positions)}
            (directory / 'id.def').write_text(''.join(f'{value} {pos}\n' for pos, value in ids.items()), encoding='utf-8')
            pos_id = ids['名詞,一般,*,*,*,*,*']
            rows, baseline = [], []
            for n in range(100):
                reading = 'きょうつう' + 'あいうえおかきくけこ'[n // 10] + 'あいうえおかきくけこ'[n % 10]
                rows.append(word(IndexForm='共通' + str(n), ReadingForm=reading, Cost=str(1000 + n * 20)))
                baseline.append(f'{reading}\t{pos_id}\t{pos_id}\t{5000+n*10}\t共通{n}\n')
            # 同じ表記でも品詞が違う基準語は較正の共通語へ混ぜない。
            rows.append(word(IndexForm='動詞', ReadingForm='どうし'))
            baseline.append('どうし\t999\t999\t100\t動詞\n')
            protected = rows[1]['ReadingForm']
            rows += [word(), word(IndexForm='新表記', ReadingForm=protected, Cost='0')]
            for n in range(10):
                (directory / f'dictionary{n:02}.txt').write_text(''.join(baseline) if n == 0 else '', encoding='utf-8')
            before = (directory / 'dictionary00.txt').read_bytes()
            for group in ('small', 'core', 'notcore'):
                text = io.StringIO();writer = csv.DictWriter(text, fieldnames=list(word()));writer.writeheader()
                if group == 'small':writer.writerows(rows)
                with zipfile.ZipFile(source / (group + '_lex.zip'), 'w') as archive:
                    archive.writestr(group + '_lex.csv', text.getvalue())
            for filename in ('LEGAL', 'LICENSE-2.0.txt'):(source / filename).write_text('fixture', encoding='utf-8')
            report = build(source, root / 'mozc', root / 'out', dict(repository='fixture', version='test', commit='fixed'))
            output = (root / 'out/short_lexicon.tsv').read_text('utf-8').splitlines()
            self.assertEqual(len(output), 2)
            self.assertFalse(any('\t共通' in line or line.endswith('\t動詞') for line in output))
            line = next(line for line in output if line.endswith('\t新表記'))
            self.assertGreaterEqual(int(line.split('\t')[3]), 6010)
            self.assertEqual(report['calibration_common']['名詞,一般,*,*,*,*,*'], 100)
            self.assertEqual((directory / 'dictionary00.txt').read_bytes(), before)


if __name__ == '__main__':
    unittest.main()
