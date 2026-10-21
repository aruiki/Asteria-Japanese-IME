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


class FrequencyTest(unittest.TestCase):
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
