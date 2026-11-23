import argparse
import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import ab_suite  # noqa: E402


def result(oks, tops, times=(), coverage=1.0):
    rows = [dict(ok=o, top1=t, expected=["正"]) for o, t in zip(oks, tops)]
    return {"rows": rows, "ok": sum(oks), "n": len(oks), "times": list(times), "ai_coverage": coverage}


class ReportTest(unittest.TestCase):
    def args(self, session=False):
        return argparse.Namespace(session=session, timing=None, max_flips=40)

    def test_counts_flips_against_first_variant(self):
        variants = [("a", "x", []), ("b", "y", ["K=1"])]
        results = {("a", "s"): result([True, False], ["正", "誤"], [100, 200]),
                   ("b", "s"): result([False, True], ["誤", "正"], [50])}
        md = ab_suite.report(self.args(), variants, results, ["s"])
        self.assertIn("| s | 1/2 | 1/2 |", md)
        self.assertIn("直った 1、外れた 1", md)
        self.assertIn("- 外れた [s] 正 | a: 正 → b: 誤", md)
        self.assertIn("| a | 150 | 200 | 2 |", md)

    def test_marks_runs_where_ai_did_not_run(self):
        variants = [("a", "x", [])]
        md = ab_suite.report(self.args(), variants, {("a", "s"): result([True], ["正"], coverage=0.87)}, ["s"])
        self.assertIn("無効?(AI 87%)", md)

    def test_failed_run_is_reported(self):
        md = ab_suite.report(self.args(), [("a", "x", [])], {("a", "s"): None}, ["s"])
        self.assertIn("| s | 失敗 |", md)


class HelpersTest(unittest.TestCase):
    def test_resolve_exe_accepts_snapshot_folder_or_file(self):
        with tempfile.TemporaryDirectory() as d:
            exe = Path(d) / "converter" / "converter_main.exe"
            exe.parent.mkdir()
            exe.write_bytes(b"")
            self.assertEqual(ab_suite.resolve_exe(d, False), exe)
            self.assertEqual(ab_suite.resolve_exe(exe, True), exe)
            with self.assertRaises(SystemExit):
                ab_suite.resolve_exe(d, True)

    def test_filler_set_prepends_neutral_context(self):
        with tempfile.TemporaryDirectory() as d:
            path = ab_suite.filler_set("kotori-daily", Path(d))
            items = json.loads(path.read_text(encoding="utf-8"))
            self.assertGreater(len(items[0]["context_text"]), 170)

    def test_lock_is_reentrant_across_sequential_runs(self):
        with tempfile.TemporaryDirectory() as d:
            lock = Path(d) / "gpu.lock"
            with ab_suite.gpu_lock(lock):
                pass
            with ab_suite.gpu_lock(lock):
                pass


if __name__ == "__main__":
    unittest.main()
