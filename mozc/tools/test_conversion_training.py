import hashlib
from pathlib import Path
import tempfile
import unittest

from conversion_training import evaluation_readings, prepare, preferences, reading_key


def row(split, reading, positive="参照表記", context="", group=None, kind="original"):
    return dict(split=split, reading=reading, positive=positive, context=context,
                source_group=group or split + "-corpus", kind=kind)


class TrainingSeparationTest(unittest.TestCase):
    def test_prediction_typed_reading_is_also_excluded_without_using_truth(self):
        rows = [{"typed": "ハシヲツカ", "conv": "橋を使", "truth": "箸を使う"}]
        self.assertEqual(list(evaluation_readings(rows)), ["はしをつか"])

    def test_nested_reading_schema_is_supported_but_reference_is_not_extracted(self):
        value = {"cases": [{"reading": "ハシ", "expected_output": ["箸"]}]}
        self.assertEqual(list(evaluation_readings(value)), ["はし"])

    def test_normalization_blocks_katakana_and_width_variants(self):
        self.assertEqual(reading_key("ｶﾀｶﾅ ヨミ"), reading_key("かたかなよみ"))

    def test_evaluation_readings_and_near_duplicates_are_excluded(self):
        rows = [row("train", "きょうはあめがふっています"),
                row("train", "あすのかいぎはごごです"), row("dev", "いぬとこうえんをさんぽする")]
        selected, counts = prepare(rows, {"きょうはあめがふっています。"}, 1, 1)
        self.assertEqual(selected["train"][0]["input"], "あすのかいぎはごごです")
        self.assertEqual(counts["evaluation_reading"], 1)

    def test_same_reading_cross_split_is_rejected_after_normalization(self):
        rows = [row("train", "かたかなのよみ"), row("dev", "カタカナノヨミ")]
        with self.assertRaises(ValueError):
            prepare(rows, set(), 1, 1)

    def test_source_group_leak_is_rejected_even_for_different_readings(self):
        rows = [row("train", "あすのかいぎのじかん", group="same-document"),
                row("dev", "いぬとこうえんをさんぽする", group="same-document")]
        with self.assertRaises(ValueError):
            prepare(rows, set(), 1, 1)

    def test_repeated_negative_pairs_do_not_duplicate_cases_or_erase_references(self):
        rows = [row("train", "あすのかいぎのじかん", "表記A"),
                row("train", "あすのかいぎのじかん", "表記B"),
                row("dev", "いぬとこうえんをさんぽする")]
        selected, _ = prepare(rows, set(), 1, 1)
        self.assertEqual(selected["train"][0]["expected_output"], ["表記A", "表記B"])

    def test_synthetic_answer_bearing_contexts_are_not_mixed_into_originals(self):
        rows = [row("train", "あすのかいぎのじかん"),
                row("train", "ぜんぶこたえをふくむぶん", kind="agreement_context"),
                row("dev", "いぬとこうえんをさんぽする")]
        selected, counts = prepare(rows, set(), 1, 1)
        self.assertEqual(counts["non_original"], 1)
        self.assertEqual(selected["train"][0]["input"], "あすのかいぎのじかん")

    def test_missing_independent_examples_cannot_silently_shrink_sample(self):
        with self.assertRaises(ValueError):
            prepare([row("train", "あすのかいぎのじかん")], set(), 1, 1)


class PreferenceCollectionTest(unittest.TestCase):
    def test_non_reference_is_not_automatically_labeled_semantically_wrong(self):
        raw = b"[time] cands=2 llm_on=1\n[kotori] reference zenz=-2 llm=-3 lattice score=-4\n[kotori] other zenz=-1 llm=-2 gen score=-3\n"
        case = dict(index="train-1", input="あすのかいぎのじかん", expected_output=["reference"],
                    context_text="", split="train", source_groups=["public-group"])
        result = dict(index=case["index"], input=case["input"], expected=["reference"], context_text="",
                      top1="other", ok=False, trace={"stderr_sha256": hashlib.sha256(raw).hexdigest()})
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "0.stderr"
            path.write_bytes(raw)
            observations, counts = preferences([case], [result], directory)
            self.assertFalse(observations[0]["alternatives_semantically_reviewed"])
            self.assertEqual(observations[0]["supervision"], "corpus_reference_preference")
            self.assertEqual(counts["reference_in_pool"], 1)
            path.write_bytes(raw + b"changed")
            with self.assertRaises(ValueError):
                preferences([case], [result], directory)


if __name__ == "__main__":
    unittest.main()
