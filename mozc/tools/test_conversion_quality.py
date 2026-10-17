import argparse
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

from conversion_quality import compare_rows, parse_segments, reachable, score_case, scoring_trace, validate_items
from compare_quality import load_result, validate_conditions

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
spec = importlib.util.spec_from_file_location("eval_baseline", Path(__file__).parents[1] / "eval_baseline.py")
evaluator = importlib.util.module_from_spec(spec)
spec.loader.exec_module(evaluator)


def output(*segments, total=None):
    lines = []
    for i, values in enumerate(segments):
        lines.append(f"---------- Segment {i}/{len(segments)} [FREE] ----------")
        lines.extend(f"  {j}/{total or len(values)} {v}" for j, v in enumerate(values))
    return lines


def item(index=1, context="", expected=None):
    return dict(index=index, input="はし", context_text=context,
                expected_output=expected or ["箸"])


class CandidateTests(unittest.TestCase):
    def test_multiple_segments_and_alternative_reference(self):
        row = score_case(dict(index=1, input="はしをつかう", expected_output=["箸を使う", "箸をつかう"]),
                         output(["橋を", "箸を"], ["使う", "つかう"]))
        self.assertFalse(row["ok"])
        self.assertTrue(row["candidate_coverage"])
        self.assertEqual(row["diagnostic_stage"], "candidate_present_not_selected")

    def test_prefix_ambiguity_does_not_use_greedy_matching(self):
        segments = parse_segments(output(["a", "ab"], ["bc", "c"]))
        self.assertTrue(reachable(segments, "abc"))
        self.assertFalse(reachable(segments, "ab"))

    def test_k_is_per_segment_and_not_whole_sentence_rank(self):
        segments = parse_segments(output(["橋", "箸"], ["で", "を"]))
        self.assertTrue(reachable(segments, "箸を", 2))
        self.assertFalse(reachable(segments, "箸を", 1))

    def test_truncated_absence_is_unknown(self):
        row = score_case(item(), output(["橋"], total=100))
        self.assertEqual(row["diagnostic_stage"], "unobserved_candidates")

    def test_complete_absence_and_correct_are_distinct(self):
        self.assertEqual(score_case(item(), output(["橋"]))["diagnostic_stage"], "candidate_or_segmentation")
        self.assertEqual(score_case(item(), output(["箸"]))["diagnostic_stage"], "correct")

    def test_missing_and_malformed_segments_fail_closed(self):
        for lines in ([], output([]), output(["橋"])[1:],
                      ["---------- Segment 1/2 [FREE] ----------", "  0/1 橋"],
                      ["---------- Segment 0/1 [FREE] ----------", "  1/2 橋"],
                      ["---------- Segment 0/1 [HISTORY] ----------", "  0/1 橋"]):
            with self.subTest(lines=lines), self.assertRaises(ValueError):
                parse_segments(lines)

    def test_invalid_data_is_rejected(self):
        for items in ([], [item(), item()], [dict(item(), input="はし\nquit")],
                      [dict(item(), context_text="a\tb")], [dict(item(), expected_output=[])]):
            with self.subTest(items=items), self.assertRaises(ValueError):
                validate_items(items)


class ComparisonTests(unittest.TestCase):
    def test_identical_dictionary_bytes_in_different_build_directories_can_be_compared(self):
        before = dict(data={"sha256": "same-data"}, dictionary={"path": "old/mozc.data", "sha256": "same-dictionary"},
                      context=True, case_isolated=False, profile_policy="isolated_temporary")
        after = dict(before, dictionary={"path": "new/mozc.data", "sha256": "same-dictionary"})
        validate_conditions(before, after)
        for altered in (dict(after, dictionary={"path": "new/mozc.data", "sha256": "different"}),
                        dict(after, dictionary={}), dict(after, context=False),
                        dict(after, profile_policy="explicit")):
            with self.subTest(altered=altered), self.assertRaises(ValueError):
                validate_conditions(before, altered)

    def test_join_by_id_and_separate_improvement_regression(self):
        before = [score_case(item(1), output(["橋", "箸"])), score_case(item(2), output(["箸"]))]
        after = [score_case(item(2), output(["橋"])), score_case(item(1), output(["箸"]))]
        report = compare_rows(before, after)
        self.assertEqual((report["improved"], report["regressed"], report["net_improvement"]), (1, 1, 0))

    def test_changed_truth_context_ids_or_forged_ok_rejected(self):
        row = score_case(item(), output(["箸"]))
        for altered in (dict(row, index=2), dict(row, expected=["橋"]),
                        dict(row, context_text="渡河"), dict(row, ok=False)):
            with self.subTest(altered=altered), self.assertRaises(ValueError):
                compare_rows([row], [altered])
        with self.assertRaises(ValueError):
            compare_rows([row, row], [row])

    def test_unbound_result_cannot_be_compared(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "result.json"
            path.write_text("[]", encoding="utf-8")
            Path(str(path) + ".manifest.json").write_text(json.dumps({"result_sha256": "wrong"}), encoding="utf-8")
            with self.assertRaises(ValueError):
                load_result(path)


class ContextTests(unittest.TestCase):
    def test_normalized_reading_collision_is_split(self):
        items = [item(1, "食事"), dict(item(2, "渡河"), input="ハシ"), item(3, "")]
        self.assertEqual(evaluator.context_batches(items, True), [[0], [1], [2]])
        self.assertEqual(evaluator.context_batches(items, False), [[0, 1, 2]])

    def test_same_reading_different_contexts_reach_converter_and_temp_is_removed(self):
        items = [item(1, "食事"), item(2, "渡河", ["橋"])]
        paths = []

        def convert(command, **kwargs):
            path = Path(kwargs["env"]["KOTORI_LM_CONTEXT_MAP"])
            paths.append(path)
            context = path.read_text(encoding="utf-8")
            value = "箸" if "食事" in context else "橋"
            text = "\n".join(output([value])) + "\nExecCommand() return false\n"
            return subprocess.CompletedProcess(command, 0, text.encode(), b"")

        args = argparse.Namespace(context=True, timeout=10, profile="")
        with patch.object(evaluator.subprocess, "run", side_effect=convert):
            blocks, _, count = evaluator.run_conversions(args, Path("converter.exe"), Path("."), items, {})
        self.assertEqual(count, 2)
        self.assertEqual([score_case(it, block)["ok"] for it, block in zip(items, blocks)], [True, True])
        self.assertTrue(all(not p.exists() for p in paths))

    def test_extra_output_and_crash_are_errors_and_temp_is_removed(self):
        args = argparse.Namespace(context=True, timeout=10, profile="")
        for returncode in (0, 1):
            paths = []

            def broken(command, **kwargs):
                paths.append(Path(kwargs["env"]["KOTORI_LM_CONTEXT_MAP"]))
                return subprocess.CompletedProcess(command, returncode,
                                                   b"ExecCommand() return false\n" * 2, b"failure")

            with patch.object(evaluator.subprocess, "run", side_effect=broken), self.assertRaises(ValueError):
                evaluator.run_conversions(args, Path("converter.exe"), Path("."), [item()], {})
            self.assertTrue(all(not p.exists() for p in paths))

    def test_acp_header_allowed_but_corrupt_candidate_rejected(self):
        args = argparse.Namespace(context=False, timeout=10, profile="")
        header = b"User profile dir: " + "プログラム".encode("cp932") + b"\n"
        good = "\n".join(output(["箸"])).encode() + b"\nExecCommand() return false\n"
        with patch.object(evaluator.subprocess, "run", return_value=subprocess.CompletedProcess([], 0, header + good, b"")):
            blocks, _, _ = evaluator.run_conversions(args, Path("converter.exe"), Path("."), [item()], {})
        self.assertTrue(score_case(item(), blocks[0])["ok"])
        bad = good.replace("箸".encode(), b"\xff")
        with patch.object(evaluator.subprocess, "run", return_value=subprocess.CompletedProcess([], 0, header + bad, b"")), self.assertRaises(UnicodeError):
            evaluator.run_conversions(args, Path("converter.exe"), Path("."), [item()], {})

    def test_timeout_removes_context_and_isolated_profile(self):
        args = argparse.Namespace(context=True, timeout=10, profile="")
        paths = []

        def timeout(command, **kwargs):
            paths.append(Path(kwargs["env"]["KOTORI_LM_CONTEXT_MAP"]))
            paths.append(Path(command[-1].split("=", 1)[1]))
            raise subprocess.TimeoutExpired(command, 10)

        with patch.object(evaluator.subprocess, "run", side_effect=timeout), self.assertRaises(subprocess.TimeoutExpired):
            evaluator.run_conversions(args, Path("converter.exe"), Path("."), [item()], {})
        self.assertTrue(all(not p.exists() for p in paths))


class ScoringTraceTests(unittest.TestCase):
    def test_dictionary_presence_is_distinct_from_internal_search(self):
        log = b"[time] cands=1 llm_on=1\n[kotori] wrong zenz=-1 llm=-2 lattice score=-3\n"
        trace = scoring_trace(log, ["right"])
        self.assertEqual(trace["status"], "complete_logged_pool")
        self.assertFalse(trace["expected_in_pool"])

    def test_llm_shortlist_exclusion_is_distinct_from_ranking(self):
        log = b"[time] cands=1 llm_on=1\n[kotori] right zenz=-1 llm=-inf lattice score=-inf\n"
        trace = scoring_trace(log, ["right"])
        self.assertTrue(trace["expected_in_pool"])
        self.assertFalse(trace["expected_llm_scored"])
        self.assertFalse(trace["expected_jointly_selectable"])

    def test_no_llm_does_not_count_dummy_zero_as_llm_scoring(self):
        log = b"[time] cands=1 llm_on=0\n[kotori] right zenz=-1 llm=0 lattice score=-1\n"
        self.assertIsNone(scoring_trace(log, ["right"])["expected_llm_scored"])

    def test_missing_or_multiple_records_cannot_prove_pool_absence(self):
        for log in (b"", b"[time] cands=2 llm_on=1\n",
                    b"[time] cands=0 llm_on=1\n[time] cands=0 llm_on=1\n"):
            self.assertEqual(scoring_trace(log, ["right"])["status"], "unverified")


if __name__ == "__main__":
    unittest.main()
