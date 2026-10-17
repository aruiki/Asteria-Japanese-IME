import argparse
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

from conversion_quality import split_scoring_requests

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
spec = importlib.util.spec_from_file_location("request_trace_evaluator", Path(__file__).parents[1] / "eval_baseline.py")
evaluator = importlib.util.module_from_spec(spec)
spec.loader.exec_module(evaluator)


def wrapped(number, body=b""):
    return (f"[eval_trace] request={number} begin\n".encode() + body +
            f"[eval_trace] request={number} end\n".encode())


class RequestBoundaryTest(unittest.TestCase):
    def test_multiple_requests_preserve_warm_process_and_exact_log_bytes(self):
        first, second = b"[time] cands=1\r\n", "[kotori] 箸\r\n".encode()
        raw = b"model initialization\r\n" + wrapped(1, first) + wrapped(2, second)
        self.assertEqual(split_scoring_requests(raw, 2), [(1, first), (2, second)])

    def test_empty_request_body_does_not_invent_model_activity(self):
        self.assertEqual(split_scoring_requests(wrapped(1), 1), [(1, b"")])

    def test_missing_extra_nested_reordered_and_malformed_boundaries_fail_closed(self):
        invalid = [wrapped(1) + wrapped(1), wrapped(2), wrapped(1) + wrapped(2),
                   b"[eval_trace] request=1 begin\n", b"[eval_trace] request=1 end\n",
                   b"[eval_trace] request=1 begin\n" + wrapped(2),
                   b"[eval_trace] request=abc begin\n",
                   b"[eval_trace] request=1 begin\n[eval_trace] request=2 end\n"]
        for raw in invalid:
            with self.subTest(raw=raw), self.assertRaises(ValueError):
                split_scoring_requests(raw, 1)

    def test_unattributed_scoring_is_not_assigned_to_neighboring_request(self):
        for raw in (b"[time] cands=1\n" + wrapped(1),
                    wrapped(1) + b"[kotori] late result\n"):
            with self.subTest(raw=raw), self.assertRaises(ValueError):
                split_scoring_requests(raw, 1)

    def test_stderr_startup_header_can_use_acp_without_changing_candidate_utf8(self):
        header = "プロファイル".encode("cp932") + b"\n"
        body = "[kotori] 箸\n".encode()
        self.assertEqual(split_scoring_requests(header + wrapped(1, body), 1)[0][1], body)


class EvaluatorRequestTraceTest(unittest.TestCase):
    def test_one_native_process_maps_two_cases_to_distinct_ids_and_original_hashes(self):
        cases = [dict(index=i, input="はし", expected_output=["箸"]) for i in range(2)]
        output = ("---------- Segment 0/1 [FREE] ----------\n  0/1 箸\n"
                  "ExecCommand() return false\n").encode() * 2
        log = wrapped(1, b"first\n") + wrapped(2, b"second\n")
        commands = []

        def converter(command, **kwargs):
            commands.append(command)
            return subprocess.CompletedProcess(command, 0, output, log)

        with tempfile.TemporaryDirectory() as directory:
            args = argparse.Namespace(context=False, timeout=10, profile="", case_traces="",
                                      request_traces=directory)
            with patch.object(evaluator.subprocess, "run", side_effect=converter):
                blocks, _, batch_count = evaluator.run_conversions(args, Path("converter.exe"), Path("."), cases, {})
            self.assertEqual((len(commands), len(blocks), batch_count), (1, 2, 1))
            self.assertIn("--evaluation_trace=true", commands[0])
            self.assertEqual((Path(directory) / "1.stderr").read_bytes(), b"second\n")
            first = json.loads((Path(directory) / "0.link.json").read_text())
            second = json.loads((Path(directory) / "1.link.json").read_text())
            self.assertEqual((first["request_id"], second["request_id"]), (1, 2))
            self.assertNotEqual(first["stderr_sha256"], second["stderr_sha256"])
            self.assertEqual(first["batch_stderr_sha256"], second["batch_stderr_sha256"])

    def test_unsupported_converter_or_missing_boundaries_cannot_silently_fall_back(self):
        cases = [dict(index=1, input="はし", expected_output=["箸"])]
        for code in (0, 1):
            with tempfile.TemporaryDirectory() as directory:
                args = argparse.Namespace(context=False, timeout=10, profile="", case_traces="", request_traces=directory)
                result = subprocess.CompletedProcess([], code, b"ExecCommand() return false\n", b"unknown flag")
                with patch.object(evaluator.subprocess, "run", return_value=result), self.assertRaises(ValueError):
                    evaluator.run_conversions(args, Path("converter.exe"), Path("."), cases, {})


if __name__ == "__main__":
    unittest.main()
