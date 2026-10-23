"""導入検査の入力が利用者の学習履歴に残らないこと（保存→検査→自動で戻して一致）を確認する。"""
import os
from pathlib import Path
import stat
import tempfile
import unittest
from unittest import mock

import local_install_check as lic


class ProfileRestoreTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        root = Path(self.tmp.name)
        self.profile = root / "Kotori"
        self.profile.mkdir()
        (self.profile / "history.db").write_bytes(b"user history")
        (self.profile / "sub").mkdir()
        (self.profile / "sub" / "segment.db").write_bytes(b"segments")
        os.chmod(self.profile / "history.db", stat.S_IREAD)  # 読み取り専用でも戻せること
        (self.profile / "session.ipc").write_bytes(b"runtime")
        self.run_record = lic.Run(root / "out")
        patches = [mock.patch.object(lic, "PROFILE", self.profile),
                   mock.patch.object(lic.smoke, "stop_installed_processes", lambda install: None)]
        for p in patches:
            p.start()
            self.addCleanup(p.stop)
        self.before = lic.tree_hashes(self.profile)

    def tearDown(self):
        for path in Path(self.tmp.name).rglob("*"):
            if path.is_file():
                os.chmod(path, stat.S_IWRITE)
        self.tmp.cleanup()

    def learn(self):
        # 検査中の入力: 履歴の書き換え、新しいファイル、ファイルの削除。
        os.chmod(self.profile / "history.db", stat.S_IWRITE)
        (self.profile / "history.db").write_bytes(b"user history + nihongo + toukyou")
        (self.profile / "new_learning.db").write_bytes(b"learned")
        (self.profile / "sub" / "segment.db").unlink()

    def test_restored_after_test_input(self):
        with lic.profile_restored_after(self.run_record, "verify"):
            self.learn()
        self.assertEqual(lic.tree_hashes(self.profile), self.before)
        self.assertTrue(self.run_record.state["profile_restored"])
        self.assertEqual(self.run_record.state["profile_files"], 2)

    def test_restored_even_when_test_fails(self):
        with self.assertRaises(RuntimeError):
            with lic.profile_restored_after(self.run_record, "browser"):
                self.learn()
                raise RuntimeError("test failed")
        self.assertEqual(lic.tree_hashes(self.profile), self.before)

    def test_mismatch_is_reported(self):
        with mock.patch.object(lic, "restore_profile", lambda snapshot: None):
            with self.assertRaisesRegex(RuntimeError, "戻せません"):
                with lic.profile_restored_after(self.run_record, "verify"):
                    self.learn()
        self.assertFalse(self.run_record.state["profile_restored"])


if __name__ == "__main__":
    unittest.main()
