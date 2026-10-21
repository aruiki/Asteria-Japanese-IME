"""評価混入、分割時の重複、集計値と展開境界を検査する。"""
import json
from pathlib import Path
import random
import sqlite3
import tempfile
import unittest
import zipfile

from collect_frequency import Exclusions, flush, merge_counts, reserved, shard_corpus
from stage_tokenizer import extract_verified
from build_frequency_lexicon import select
from build_frequency_collocations import association, endpoints


class FrequencyTest(unittest.TestCase):
    def test_collocations_need_association_not_only_high_marginal_frequency(self):
        policy = json.loads(Path(__file__).with_name('collocation_policy.json').read_text('utf-8'))
        self.assertIsNotNone(association(30, 100, 200, 100000, policy))
        self.assertIsNone(association(30, 1000, 3000, 100000, policy))
        self.assertIsNone(association(10, 100, 200, 100000, policy))
        self.assertIsNone(association(300, 100, 200, 100000, policy))

    def test_collocations_keep_supported_boundaries_and_do_not_make_generic_no_pairs(self):
        policy = json.loads(Path(__file__).with_name('collocation_policy.json').read_text('utf-8'))
        noun = ('名詞', '普通名詞', '一般')
        verb = ('動詞', '一般', '*')
        particle = ('助詞', '格助詞', '*')
        self.assertEqual(endpoints([('本', noun), ('を', particle), ('読む', verb)], policy), ('本', '読む'))
        self.assertIsNone(endpoints([('本', noun), ('の', particle), ('値段', noun)], policy))
        self.assertIsNone(endpoints([('人名', ('名詞', '固有名詞', '人名')), ('作品', noun)], policy))

    def test_rare_words_and_names_do_not_get_unearned_cost_advantage(self):
        policy = json.loads(Path(__file__).with_name('frequency_lexicon_policy.json').read_text('utf-8'))
        noun = '名詞,一般,*,*,*,*,*'
        person = '名詞,固有名詞,人名,一般,*,*,*'
        self.assertIsNone(select(5000, 'しんご', noun, (10, 4, 7), policy)[0])
        self.assertIsNone(select(5000, 'しんご', noun, (10, 6, 3), policy)[0])
        self.assertIsNone(select(5000, 'しんご', person, (30, 19, 7), policy)[0])
        costs = [select(5000, 'しんご', noun, (count, count, 7), policy)[0] for count in (5, 10, 20, 80, 1000)]
        self.assertEqual(costs, sorted(costs, reverse=True))
        self.assertGreaterEqual(min(costs), 5000)
        self.assertGreater(select(5000, 'しんご' * 6, noun, (80, 80, 7), policy)[0], 5000)

    def test_near_duplicate_index_matches_exhaustive_jaccard(self):
        randomizer = random.Random(1122)
        values = {''.join(randomizer.choices('あいうえおかきくけこさしすせそ', k=20)) for _ in range(80)}
        index = Exclusions(values)
        queries = list(values) + [v[:8] + 'ん' + v[9:] for v in values]
        queries += [''.join(randomizer.choices('あいうえおかきくけこさしすせそ', k=20)) for _ in range(200)]
        grams = lambda value: {value[i:i + 3] for i in range(len(value) - 2)}
        for query in queries:
            actual = query in values or any(len(grams(query) & grams(v)) / len(grams(query) | grams(v)) >= .8
                                            for v in values)
            self.assertEqual(index.overlaps(query), actual)
        self.assertFalse(index.overlaps('あ'))

    def test_reserved_interval_excludes_partly_overlapping_records(self):
        policy = {'reserved_byte_ranges': [{'offset': 10, 'bytes': 10}]}
        self.assertFalse(reserved(0, 10, policy))
        self.assertTrue(reserved(5, 11, policy))
        self.assertTrue(reserved(19, 21, policy))
        self.assertFalse(reserved(20, 25, policy))

    def test_normalized_duplicate_text_goes_to_same_shard(self):
        with tempfile.TemporaryDirectory() as name:
            root = Path(name)
            source = root / 'corpus.jsonl'
            rows = [{'input': 'エーアイ', 'output': text} for text in ['ＡＩ技術', 'AI技術', '日本語変換']]
            source.write_text(''.join(json.dumps(row, ensure_ascii=False) + '\n' for row in rows), encoding='utf-8')
            policy = dict(reserved_byte_ranges=[], min_text_length=2, max_text_length=512)
            shard_corpus(source, root, 3, policy, 0)
            containing = []
            for i in range(3):
                records = [json.loads(line) for line in (root / f'input-{i:02}.jsonl').read_text('utf-8').splitlines()]
                matches = [row for row in records if row[1] == 'AI技術']
                if matches:
                    containing.append(matches)
            self.assertEqual(len(containing), 1)
            self.assertEqual(len(containing[0]), 2)
            self.assertEqual(containing[0][0][2], containing[0][1][2])

    def test_sql_flush_and_external_merge_sum_counts_and_or_buckets(self):
        with tempfile.TemporaryDirectory() as name:
            root = Path(name)
            for i, counts in enumerate([{(1, '日本語入力'): [5, 4, 1], (1, '少数句'): [1, 1, 1]},
                                        {(1, '日本語入力'): [7, 3, 2]}]):
                db = sqlite3.connect(root / f'counts-{i:02}.sqlite')
                db.execute('CREATE TABLE counts (kind INTEGER,key TEXT,occurrences INTEGER,documents INTEGER,'
                           'buckets INTEGER,PRIMARY KEY(kind,key)) WITHOUT ROWID')
                flush(db, counts)
                self.assertEqual(counts, {})
                flush(db, {(1, '日本語入力'): [2, 1, 4]})
                db.close()
            merge_counts(root, 2, {'min_output_phrase_documents': 5})
            self.assertEqual((root / 'phrase_frequency.tsv').read_text('utf-8'), '日本語入力\t16\t9\t7\n')

    def test_zip_path_escape_and_changed_existing_runtime_are_rejected(self):
        with tempfile.TemporaryDirectory() as name:
            root = Path(name)
            for index, filename in enumerate(['../escape', '/absolute', r'..\escape', 'C:/absolute']):
                archive = root / f'invalid-{index}.zip'
                with zipfile.ZipFile(archive, 'w') as stream:
                    stream.writestr(filename, 'content')
                with self.assertRaises(ValueError):
                    extract_verified(archive, root / 'output')
            archive = root / 'valid.zip'
            with zipfile.ZipFile(archive, 'w') as stream:
                stream.writestr('package/main.py', 'original')
            extract_verified(archive, root / 'output')
            target = root / 'output/package/main.py'
            target.write_text('changed', encoding='utf-8')
            with self.assertRaises(ValueError):
                extract_verified(archive, root / 'output')
            self.assertEqual(target.read_text('utf-8'), 'changed')


if __name__ == '__main__':
    unittest.main()
