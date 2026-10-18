"""コールド起動検証が応答欠落・異なるPID・空の結果を成功扱いしない。"""
import unittest
from large_input_cold import ready_status, summarize


class ColdReadinessTest(unittest.TestCase):
    def test_requires_the_current_process_and_actual_device(self):
        status = 'state=zenz\npid=123\ndevice=CPU\n'
        self.assertTrue(ready_status(status, 123, True))
        self.assertFalse(ready_status(status, 124, True))
        self.assertFalse(ready_status(status.replace('CPU', 'GPU'), 123, True))
        self.assertFalse(ready_status(status.replace('zenz', 'loading'), 123, True))


class ColdCompletionTest(unittest.TestCase):
    def row(self, defects=None, reviews=None):
        return dict(defects=defects or [], reviews=reviews or [],
                    events={'space': {'ms': 12}}, key_events=[{'ms': 2}])

    def test_empty_or_partial_run_never_passes(self):
        self.assertFalse(summarize([], [], [])['input_path_checks_passed'])
        self.assertFalse(summarize([self.row()], ['case'], ['no-warmup'])['input_path_checks_passed'])
        self.assertFalse(summarize([self.row(), self.row()], ['case'], ['no-warmup', 'loaded'], 'EOF')['input_path_checks_passed'])

    def test_reading_or_commit_loss_fails_completed_transport(self):
        report = summarize([self.row(), self.row(['commit_different_from_preedit'])], ['case'], ['no-warmup', 'loaded'])
        self.assertTrue(report['complete'])
        self.assertFalse(report['input_path_checks_passed'])

    def test_surface_accuracy_is_not_claimed_from_transport_completion(self):
        report = summarize([self.row(), self.row(reviews=['expected_surface_mismatch'])], ['case'], ['no-warmup', 'loaded'])
        self.assertTrue(report['input_path_checks_passed'])
        self.assertEqual(report['reference_matches'], 1)
        self.assertEqual(report['reference_mismatches'], 1)


if __name__ == '__main__':
    unittest.main()
