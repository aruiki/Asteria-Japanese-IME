import hashlib
import json
from pathlib import Path
import tempfile
import unittest

from check_release import check_short_lexicon


class ShortLexiconPackageTest(unittest.TestCase):
    def setUp(self):
        self.work = tempfile.TemporaryDirectory()
        self.addCleanup(self.work.cleanup)
        self.root = Path(self.work.name)
        self.spec = {'files': []}
        for name in ('short_lexicon.tsv', 'short_lexicon.json', 'NOTICE-short-lexicon.txt'):
            data = name.encode()
            (self.root / name).write_bytes(data)
            self.spec['files'].append(dict(name=name, bytes=len(data), sha256=hashlib.sha256(data).hexdigest()))

    def check_payload(self, required=True):
        rows = []
        check_short_lexicon(self.root, lambda ok, text: rows.append((ok, text)), required, self.spec)
        return rows

    def test_all_fixed_files_are_required_and_checked(self):
        rows = self.check_payload()
        self.assertEqual(len(rows), 6)
        self.assertTrue(all(ok for ok, _ in rows))

    def test_mutation_or_removal_of_any_file_fails(self):
        for f in self.spec['files']:
            p = self.root / f['name']
            data = p.read_bytes()
            p.write_bytes(data + b'x')
            self.assertTrue(any(not ok for ok, _ in self.check_payload()))
            p.unlink()
            self.assertTrue(any(not ok for ok, _ in self.check_payload()))
            p.write_bytes(data)

    def test_old_release_allowed_only_when_absent_and_not_required(self):
        for p in self.root.iterdir():
            p.unlink()
        self.assertEqual(self.check_payload(False), [])
        self.assertTrue(any(not ok for ok, _ in self.check_payload(True)))
        (self.root / 'short_lexicon.json').write_bytes(b'changed')
        self.assertTrue(any(not ok for ok, _ in self.check_payload(False)))


if __name__ == '__main__':
    unittest.main()
