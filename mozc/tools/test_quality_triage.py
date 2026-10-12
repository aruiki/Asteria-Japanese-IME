"""評価結果の対応付けの破損で改善を誤報しない。"""
import unittest
from quality_triage import compare


def row(index, text='正解', ok=True):
    return dict(index=index, input='せいかい', expected=['正解'], top1=text, ok=ok)


class TriageTest(unittest.TestCase):
    def test_pairs_by_index_and_recomputes_correctness(self):
        result = compare([row('a', '誤り'), row('b')],
                         [row('b'), row('a', ok=False)], [row('a'), row('b')])
        self.assertEqual(result['correct'], 2)
        self.assertEqual(result['improved'], ['a'])
        self.assertFalse(result['regressed'])

    def test_rejects_missing_or_duplicate_indices(self):
        with self.assertRaises(ValueError):
            compare([row('a')], [row('b')], [row('a')])
        with self.assertRaises(ValueError):
            compare([row('a'), row('a')], [row('a')], [row('a')])

    def test_rejects_changed_expectations(self):
        changed = row('a')
        changed['expected'] = ['誤り']
        with self.assertRaises(ValueError):
            compare([row('a')], [changed], [row('a')])

    def test_records_regression_and_mozc_reference(self):
        result = compare([row('a')], [row('a', '誤り', ok=True)], [row('a')])
        self.assertEqual(result['regressed'], ['a'])
        self.assertTrue(result['failures'][0]['mozc_was_correct'])
        self.assertEqual(result['correct'], 0)