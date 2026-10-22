import copy
import hashlib
import json
from pathlib import Path
import tempfile
import unittest

from stage_package import identity, stage, verify


class PackageTest(unittest.TestCase):
    def setUp(self):
        self.work = tempfile.TemporaryDirectory()
        self.addCleanup(self.work.cleanup)
        self.root = Path(self.work.name)
        self.source = self.root / 'source'
        self.source.mkdir()
        data = 'あい\t123\t123\t6000\t愛\n'.encode()
        metadata = dict(output_entries=1, sha256=hashlib.sha256(data).hexdigest(),
                        evaluation_labels_used=False, source_commit='source',
                        frequency_data_sha256='frequency')
        self.values = {'short_lexicon.tsv': data,
                       'short_lexicon.json': json.dumps(metadata).encode(),
                       'NOTICE-short-lexicon.txt': b'fixture license'}
        self.spec = dict(schema=1, tag='data-short-lexicon-20261022', entries=1,
                         sudachi_commit='source', frequency_data_sha256='frequency', files=[])
        for name, content in self.values.items():
            (self.source / name).write_bytes(content)
            self.spec['files'].append(dict(name=name, bytes=len(content),
                sha256=hashlib.sha256(content).hexdigest(),
                url='https://github.com/aruiki/Asteria-Japanese-IME/releases/download/' + self.spec['tag'] + '/' + name))
        self.manifest = self.root / 'package.json'
        self.manifest.write_text(json.dumps(self.spec), encoding='utf-8')

    def test_fixed_payload_stages_and_reuses_without_modification(self):
        out = self.root / 'out'
        stage(out, self.source, self.manifest)
        stage(out, self.source, self.manifest)
        self.assertEqual({p.name: p.read_bytes() for p in out.iterdir()}, self.values)

    def test_tampered_data_metadata_and_notice_each_rejected(self):
        for name in self.values:
            with self.subTest(name=name):
                original = (self.source / name).read_bytes()
                (self.source / name).write_bytes(original + b'x')
                with self.assertRaises(ValueError):
                    stage(self.root / 'out', self.source, self.manifest)
                (self.source / name).write_bytes(original)

    def test_existing_different_file_is_preserved_and_other_files_not_copied(self):
        out = self.root / 'out'
        out.mkdir()
        (out / 'NOTICE-short-lexicon.txt').write_bytes(b'preserve me')
        with self.assertRaises(ValueError):
            stage(out, self.source, self.manifest)
        self.assertEqual(list(out.iterdir()), [out / 'NOTICE-short-lexicon.txt'])
        self.assertEqual((out / 'NOTICE-short-lexicon.txt').read_bytes(), b'preserve me')

    def test_partial_download_is_not_silently_overwritten(self):
        out = self.root / 'out'
        out.mkdir()
        (out / 'short_lexicon.tsv.partial').write_bytes(b'partial')
        with self.assertRaises(FileExistsError):
            stage(out, self.source, self.manifest)
        self.assertEqual((out / 'short_lexicon.tsv.partial').read_bytes(), b'partial')

    def test_external_origin_path_escape_and_wrong_payload_shape_rejected(self):
        for field, value in [('url', 'https://example.org/x'), ('name', '../x'),
                             ('sha256', 'invalid'), ('bytes', 0)]:
            spec = copy.deepcopy(self.spec)
            spec['files'][0][field] = value
            self.manifest.write_text(json.dumps(spec), encoding='utf-8')
            with self.subTest(field=field), self.assertRaises(ValueError):
                identity(self.manifest)

    def test_correct_hashes_do_not_replace_metadata_semantic_validation(self):
        metadata = json.loads(self.values['short_lexicon.json'])
        metadata['evaluation_labels_used'] = True
        payload = json.dumps(metadata).encode()
        (self.source / 'short_lexicon.json').write_bytes(payload)
        spec = copy.deepcopy(self.spec)
        spec['files'][1].update(bytes=len(payload), sha256=hashlib.sha256(payload).hexdigest())
        with self.assertRaises(ValueError):
            verify(self.source, spec)


if __name__ == '__main__':
    unittest.main()
