import unittest
from native_pairs import validate


class NativePairsTest(unittest.TestCase):
    def row(self, split, reading, group):
        return dict(split=split, reading=reading, source_groups=[group], positive='書く', negative='描く')

    def test_eval_reading_and_near_duplicates_are_rejected(self):
        with self.assertRaises(ValueError):
            validate([self.row('train', 'きょうのよていをかくにんする', 'a')],
                     {'きょうのよていをかくにんする'})
        with self.assertRaises(ValueError):
            validate([self.row('train', 'きょうのよていをかくにんする。', 'a')],
                     {'きょうのよていをかくにんする'})

    def test_corpus_group_and_reading_leakage_are_rejected(self):
        for reading, group in [('かく', 'b'), ('えがく', 'a')]:
            with self.assertRaises(ValueError):
                validate([self.row('train', 'かく', 'a'), self.row('dev', reading, group)], set())

    def test_independent_pairs_are_accepted(self):
        validate([self.row('train', 'かく', 'a'), self.row('dev', 'えがく', 'b')], set())

    def test_near_duplicate_train_dev_readings_are_rejected(self):
        with self.assertRaises(ValueError):
            validate([self.row('train', 'きょうのよていをかくにんする', 'a'),
                      self.row('dev', 'きょうのよていをかくにんする。', 'b')], set())

    def test_nonfinite_frozen_score_is_rejected(self):
        with self.assertRaises(ValueError):
            validate([dict(self.row('train', 'かく', 'a'),
                           frozen_llm_score_difference=float('nan'))], set())


if __name__ == '__main__':
    unittest.main()
