"""表示名の版・段階・公開日を確かめる。"""
from datetime import date
import unittest

from release_name import release_name


class ReleaseNameTest(unittest.TestCase):
    def test_brand_version_and_stage_for_each_stage(self):
        for tag, stage in [("v1.0.0", "正式版"), ("v0.3.0-beta.8", "ベータ版8"),
                           ("v1.1.0-rc.3", "リリース候補3"),
                           ("v0.0.0-pr146", "開発確認用")]:
            with self.subTest(tag=tag):
                self.assertEqual(release_name(tag, date(2026, 10, 9)),
                                 f"Asteria {tag} — {stage}（2026-10-09）")

    def test_keeps_original_version_and_publication_date(self):
        self.assertEqual(release_name("v12.34.56-rc.123", date(2026, 1, 2)),
                         "Asteria v12.34.56-rc.123 — リリース候補123（2026-01-02）")

    def test_rejects_ambiguous_tags(self):
        for tag in ["1.1.0", "v1.1", "v1.1.0-rc.0", "v1.1.0-rc.3x", "v1.1.0\n"]:
            with self.subTest(tag=tag), self.assertRaises(ValueError):
                release_name(tag, date(2026, 10, 9))


if __name__ == "__main__":
    unittest.main()
