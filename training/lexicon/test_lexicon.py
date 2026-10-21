"""既存語の保護、読み/品詞の境界、固定原票の破損拒否を検査する。"""
import hashlib
import tempfile
from pathlib import Path
import unittest
import zipfile

from build_lexicon import candidate, common_quantile
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


if __name__ == '__main__':
    unittest.main()
