from pathlib import Path
import math
import tempfile
import unittest

from model import CharacterModel, ranked_choice, select_configuration
from prepare import split_rows


class ModelTest(unittest.TestCase):
    def test_repeated_natural_sequences_are_cheaper_than_unseen_sequences(self):
        model = CharacterModel.fit(['猫が庭で遊ぶ。'] * 10 + ['犬が庭で遊ぶ。'] * 10)
        self.assertLess(model.cost('猫が庭で遊ぶ。', 10), model.cost('猫が庭で泳ぐ。', 10))
        self.assertTrue(math.isfinite(model.cost('未知の𠀀', 4)))

    def test_roundtrip_and_repeated_exports_are_byte_identical(self):
        model = CharacterModel.fit(['猫が庭で遊ぶ。', '猫が家で眠る。'])
        with tempfile.TemporaryDirectory() as temp:
            a, b = [Path(temp) / name for name in ('a.gz', 'b.gz')]
            model.save(a); model.save(b)
            self.assertEqual(a.read_bytes(), b.read_bytes())
            self.assertEqual(model.cost('猫が眠る。', 5), CharacterModel.load(a).cost('猫が眠る。', 5))

    def test_zero_weight_keeps_current_display_even_when_lattice_order_differs(self):
        row = dict(top1='現在', references=['現在'], scored=[dict(text='別候補', cost_delta=0, stat_cost=1), dict(text='現在', cost_delta=0, stat_cost=9)])
        self.assertEqual(ranked_choice(row, 0, 0), '現在')
        chosen, _ = select_configuration([row])
        self.assertEqual(chosen['weight'], 0)
        self.assertEqual(chosen['regressions'], 0)

    def test_empty_or_reserved_boundary_input_is_rejected(self):
        with self.assertRaises(ValueError): CharacterModel.fit([])
        with self.assertRaises(ValueError): CharacterModel.fit(['秘密\x02境界'])
        with self.assertRaises(ValueError): select_configuration([])

    def test_statistics_cannot_restyle_numbers_english_or_punctuation(self):
        for current, other in [('第二弾。', '第2弾。'), ('Officeを使う', 'OFFICEを使う'),
                               ('東京・大阪', '東京大阪')]:
            row = dict(top1=current, references=[current], scored=[dict(text=current, cost_delta=0, stat_cost=100),
                                                                  dict(text=other, cost_delta=0, stat_cost=1)])
            self.assertEqual(ranked_choice(row, 400, 0), current)

    def test_split_rejects_other_sources_and_removes_same_reading_from_train(self):
        dev = dict(input='あいうえおかきくけこ', output='独立した較正文です', split='train', source_group='train_wikipedia.jsonl:07')
        rows = [dev, dict(dev, source_group='train_wikipedia.jsonl:01'), dict(dev, source_group='train_common_crawl.jsonl:01'),
                dict(input='かんぜんにべつのぶんしょう', output='別の学習文章です', split='train', source_group='train_wikipedia.jsonl:01')]
        train, calibration, audit = split_rows(rows, set(), count=1)
        self.assertEqual(len(train), 1)
        self.assertNotEqual(train[0]['input'], calibration[0]['input'])
        self.assertEqual(audit['wrong_source_group'], 1)


if __name__ == '__main__':
    unittest.main()
