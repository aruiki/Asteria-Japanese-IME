import tempfile
import unittest
from pathlib import Path

import regression_add

REPO = Path(__file__).resolve().parents[2]
GOLDEN = REPO / "eval" / "sets" / "asteria-regression-golden.json"


def item(index="rg-0001", input_="くうかんこうへんちょうき", context="", expected=None):
    return {"index": index, "context_text": context, "input": input_,
            "expected_output": expected or ["空間光変調器"]}


class ValidateTests(unittest.TestCase):
    def test_accepts_valid_items(self):
        regression_add.validate([item(), item("rg-0002", "ないと", "", ["ナイト"])])

    def test_rejects_empty_set(self):
        with self.assertRaises(ValueError):
            regression_add.validate([])

    def test_rejects_duplicate_index(self):
        with self.assertRaisesRegex(ValueError, "問題ID"):
            regression_add.validate([item(), item(input_="ないと", expected=["ナイト"])])

    def test_rejects_empty_reading(self):
        with self.assertRaisesRegex(ValueError, "読み"):
            regression_add.validate([item(input_=" ")])

    def test_rejects_empty_expected(self):
        bad = item()
        bad["expected_output"] = []
        with self.assertRaisesRegex(ValueError, "正解"):
            regression_add.validate([bad])

    def test_rejects_newline_in_context(self):
        with self.assertRaisesRegex(ValueError, "context_text"):
            regression_add.validate([item(context="前\n文")])

    def test_rejects_same_reading_and_context(self):
        with self.assertRaisesRegex(ValueError, "重複"):
            regression_add.validate([item(), item(index="rg-0002")])

    def test_treats_katakana_as_same_reading(self):
        with self.assertRaisesRegex(ValueError, "重複"):
            regression_add.validate([item(input_="くうかんこうへんちょうき"),
                                     item(index="rg-0002", input_="クウカンコウヘンチョウキ")])


class AddTests(unittest.TestCase):
    def test_assigns_next_number(self):
        items = [item(), item("rg-0007", "ないと", "", ["ナイト"])]
        index = regression_add.add(items, "へんかん", ["変換"])
        self.assertEqual(index, "rg-0008")
        self.assertEqual(items[-1]["index"], "rg-0008")
        self.assertEqual(items[-1]["context_text"], "")
        self.assertEqual(items[-1]["expected_output"], ["変換"])

    def test_keeps_context(self):
        items = []
        regression_add.add(items, "へんかん", ["変換"], "もしかしたら２度目の")
        self.assertEqual(items[0]["context_text"], "もしかしたら２度目の")

    def test_rejects_duplicate(self):
        items = [item()]
        with self.assertRaisesRegex(ValueError, "重複"):
            regression_add.add(items, "くうかんこうへんちょうき", ["空間光変調器"])

    def test_duplicate_ignores_katakana(self):
        items = [item()]
        with self.assertRaisesRegex(ValueError, "重複"):
            regression_add.add(items, "クウカンコウヘンチョウキ", ["空間光変調器"])


class FileTests(unittest.TestCase):
    def test_committed_set_is_valid(self):
        items = regression_add.load(GOLDEN)
        self.assertGreaterEqual(len(items), 3)

    def test_write_round_trip(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "set.json"
            items = [item()]
            regression_add.write(path, items)
            self.assertEqual(regression_add.load(path), items)
            self.assertTrue(path.read_text(encoding="utf-8").endswith("\n"))

    def test_main_check_uses_exit_zero(self):
        self.assertEqual(regression_add.main(["--check", "--file", str(GOLDEN)]), 0)

    def test_main_add_writes_and_detects_duplicate(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "set.json"
            regression_add.write(path, [item()])
            argv = ["--file", str(path), "--input", "へんかん", "--expected", "変換"]
            self.assertEqual(regression_add.main(argv), 0)
            self.assertEqual(regression_add.main(argv), 3)
            self.assertEqual(len(regression_add.load(path)), 2)


if __name__ == "__main__":
    unittest.main()
