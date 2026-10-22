import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


class PatchEnvironmentGuardTest(unittest.TestCase):
    def test_bazel_only_path_settings_stop_before_any_file_operation(self):
        script = Path(__file__).with_name('make_patches.sh').resolve()
        bash = 'C:/Program Files/Git/bin/bash.exe' if os.name == 'nt' else shutil.which('bash')
        if not bash or not Path(bash).is_file():
            self.skipTest('bash unavailable')
        for key, value in [('MSYS_NO_PATHCONV', '1'), ('MSYS2_ARG_CONV_EXCL', '*')]:
            with self.subTest(setting=key), tempfile.TemporaryDirectory() as tmp:
                env = dict(os.environ, TMPDIR=tmp)
                env.pop('MSYS_NO_PATHCONV', None)
                env.pop('MSYS2_ARG_CONV_EXCL', None)
                env[key] = value
                result = subprocess.run([bash, str(script)], cwd=tmp, env=env,
                                        capture_output=True, timeout=15)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn('パッチ生成ではMSYS_NO_PATHCONV'.encode(), result.stderr)
                self.assertEqual(list(Path(tmp).iterdir()), [])


if __name__ == '__main__':
    unittest.main()
