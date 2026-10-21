"""実入力の成功記録を空/部分的な結果や違うTIPで満たせないことを確認する。"""
import copy
from pathlib import Path
import unittest

from installer_smoke import ACCEPTANCE_CHECKS, validate_acceptance, validate_duplicate_stress


class AcceptanceReceiptTest(unittest.TestCase):
    def setUp(self):
        self.install = Path("test-installed-candidate").resolve()
        self.report = dict(complete=True, native_tsf=True,
                           loaded_tip=str(self.install / "mozc_tip64.dll"),
                           secure_field_internal_ai_or_learning_verified=False,
                           checks=[dict(name=name, passed=True) for name in ACCEPTANCE_CHECKS])

    def test_complete_receipt(self):
        validate_acceptance(self.report, self.install)

    def test_incomplete_or_failed_receipts_are_rejected(self):
        variants = [dict(self.report, complete=False), dict(self.report, checks=[]),
                    dict(self.report, checks=self.report["checks"][:-1]),
                    dict(self.report, error="native input failed"),
                    dict(self.report, loaded_tip=str(self.install / "other.dll")),
                    dict(self.report, native_tsf=False),
                    dict(self.report, secure_field_internal_ai_or_learning_verified=True)]
        failed = copy.deepcopy(self.report)
        failed["checks"][0]["passed"] = False
        variants.append(failed)
        repeated = copy.deepcopy(self.report)
        repeated["checks"][0] = repeated["checks"][1]
        variants.append(repeated)
        for variant in variants:
            with self.subTest(variant=variant), self.assertRaises(RuntimeError):
                validate_acceptance(variant, self.install)


class DuplicateReceiptTest(unittest.TestCase):
    def test_all_native_paths_are_required(self):
        rows = [dict(host=host, mode=mode, space_to_action_ms=delay, passed=True)
                for host in ("EDIT", "RichEdit")
                for mode in ("space_enter", "enter", "down_enter", "focus")
                for delay in (0, 30, 250)]
        rows += [dict(host=host, mode=mode, passed=True)
                 for host in ("EDIT", "RichEdit") for mode in ("click", "intentional_repeat")]
        report = dict(duplicate_stress=dict(complete=True, trials=rows))
        validate_duplicate_stress(report)
        for missing in range(len(rows)):
            partial = copy.deepcopy(report)
            partial["duplicate_stress"]["trials"].pop(missing)
            with self.assertRaises(RuntimeError):
                validate_duplicate_stress(partial)
        failed = copy.deepcopy(report)
        failed["duplicate_stress"]["trials"][-1]["passed"] = False
        with self.assertRaises(RuntimeError):
            validate_duplicate_stress(failed)
        with self.assertRaises(RuntimeError):
            validate_duplicate_stress({})


if __name__ == "__main__":
    unittest.main()
