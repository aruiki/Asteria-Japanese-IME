import hashlib
import json
from pathlib import Path
import tempfile
import unittest
from fetch_generator import stage


class GeneratorIdentityTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.source = self.root / 'original.gguf'
        self.source.write_bytes(b'GGUF fixed test payload')
        self.identity = {'filename': 'selected.gguf', 'sha256': hashlib.sha256(self.source.read_bytes()).hexdigest()}
        self.manifest = self.root / 'manifest.json'
        self.manifest.write_text(json.dumps(self.identity), encoding='utf-8')

    def test_stage_and_reuse_preserve_pinned_bytes(self):
        target = stage(self.root / 'out', self.source, self.manifest)
        self.assertEqual(target.read_bytes(), self.source.read_bytes())
        self.assertEqual(stage(target.parent, None, self.manifest), target)
        self.assertEqual(json.loads((target.parent / 'native_generator.json').read_text()), self.identity)

    def test_wrong_source_cannot_become_installable_payload(self):
        self.source.write_bytes(b'wrong')
        with self.assertRaises(ValueError):
            stage(self.root / 'out', self.source, self.manifest)
        self.assertFalse((self.root / 'out' / 'selected.gguf').exists())
        self.assertFalse((self.root / 'out' / 'native_generator.json').exists())

    def test_existing_wrong_model_is_preserved_and_rejected(self):
        out = self.root / 'out'
        out.mkdir()
        (out / 'selected.gguf').write_bytes(b'other owner bytes')
        with self.assertRaises(ValueError):
            stage(out, self.source, self.manifest)
        self.assertEqual((out / 'selected.gguf').read_bytes(), b'other owner bytes')

    def test_path_traversal_is_rejected_before_writing(self):
        self.identity['filename'] = '../outside.gguf'
        self.manifest.write_text(json.dumps(self.identity), encoding='utf-8')
        with self.assertRaises(ValueError):
            stage(self.root / 'out', self.source, self.manifest)
        self.assertFalse((self.root / 'out').exists())


if __name__ == '__main__':
    unittest.main()
