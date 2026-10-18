"""移行後の配布名と、旧版を確認するフォールバック。"""
import unittest
from check_release import choose_asset_name, check_generator, check_collocations, product_release_pair, Checks
import hashlib
import json
from pathlib import Path
import tempfile

class AssetNameTest(unittest.TestCase):
    def test_prefers_asteria_when_legacy_alias_is_present(self):
        self.assertEqual(choose_asset_name(['Kotori64.msi', 'Kotori64.msi.sha256',
                                          'Asteria64.msi', 'Asteria64.msi.sha256']),
                         'Asteria64.msi')
    def test_existing_release_keeps_legacy_name(self):
        self.assertEqual(choose_asset_name(['Kotori64.msi', 'Kotori64.msi.sha256']),
                         'Kotori64.msi')
    def test_does_not_mix_msi_and_other_names_checksum(self):
        with self.assertRaises(ValueError):
            choose_asset_name(['Asteria64.msi', 'Kotori64.msi.sha256'])
    def test_missing_new_checksum_can_use_complete_legacy_pair(self):
        self.assertEqual(choose_asset_name(['Asteria64.msi', 'Kotori64.msi',
                                          'Kotori64.msi.sha256']), 'Kotori64.msi')
class ProductReleasePairTest(unittest.TestCase):
    def test_model_assets_between_products_do_not_become_previous_msi(self):
        releases = [dict(tagName='v1.1.4-rc.1', isLatest=False),
                    dict(tagName='model-native-generator-20261017', isLatest=False),
                    dict(tagName='v1.1.3', isLatest=True)]
        self.assertEqual(product_release_pair(releases, 'v1.1.4-rc.1'),
                         ('v1.1.4-rc.1', 'v1.1.3'))

    def test_drafts_and_model_tags_do_not_replace_latest_product(self):
        releases = [dict(tagName='v1.1.4-beta.1', isLatest=False, isDraft=True),
                    dict(tagName='model-native-generator-20261017', isLatest=False),
                    dict(tagName='v1.1.3', isLatest=True), dict(tagName='v1.1.2', isLatest=False)]
        self.assertEqual(product_release_pair(releases), ('v1.1.3', 'v1.1.2'))
        with self.assertRaises(ValueError):
            product_release_pair(releases, 'model-native-generator-20261017')

    def test_product_with_missing_assets_is_not_silently_skipped(self):
        releases = [dict(tagName='v1.1.4', isLatest=True),
                    dict(tagName='v1.1.3', isLatest=False, assets=[]),
                    dict(tagName='v1.1.2', isLatest=False)]
        self.assertEqual(product_release_pair(releases), ('v1.1.4', 'v1.1.3'))

    def test_absent_or_truncated_product_history_does_not_skip_version_gate(self):
        with self.assertRaises(ValueError):
            product_release_pair([dict(tagName='model-only', isLatest=True)])
        releases = [dict(tagName='v1.1.3', isLatest=True)]
        with self.assertRaises(ValueError):
            product_release_pair(releases, listing_truncated=True)
        self.assertEqual(product_release_pair(releases), ('v1.1.3', ''))


class GeneratorPayloadTest(unittest.TestCase):
    def test_missing_generator_fails_new_release_but_legacy_remains_checkable(self):
        with tempfile.TemporaryDirectory() as temp:
            identity = {"filename": "native.gguf", "sha256": "0" * 64}
            legacy = Checks()
            check_generator(Path(temp), legacy, identity=identity)
            self.assertTrue(legacy.ok)
            candidate = Checks()
            check_generator(Path(temp), candidate, required=True, identity=identity)
            self.assertFalse(candidate.ok)

    def test_base_model_substitution_cannot_retain_selected_bundle_identity(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            identity = {"filename": "native.gguf", "sha256": hashlib.sha256(b"native").hexdigest(),
                        "base_models": {"original.gguf": hashlib.sha256(b"original").hexdigest()}}
            (root / "native.gguf").write_bytes(b"native")
            (root / "native_generator.json").write_text(json.dumps(identity))
            (root / "original.gguf").write_bytes(b"substituted")
            check = Checks()
            check_generator(root, check, required=True, identity=identity)
            self.assertFalse(check.ok)

    def test_rejects_changed_model_even_if_packaged_manifest_claims_new_hash(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            payload = b"pinned"
            identity = {"filename": "native.gguf", "sha256": hashlib.sha256(payload).hexdigest()}
            (root / "native.gguf").write_bytes(payload)
            (root / "native_generator.json").write_text(json.dumps(identity))
            check = Checks()
            check_generator(root, check, required=True, identity=identity)
            self.assertTrue(check.ok)
            (root / "native.gguf").write_bytes(b"substituted")
            other = dict(identity, sha256=hashlib.sha256(b"substituted").hexdigest())
            (root / "native_generator.json").write_text(json.dumps(other))
            check = Checks()
            check_generator(root, check, required=True, identity=identity)
            self.assertFalse(check.ok)


class CollocationsPayloadTest(unittest.TestCase):
    def fixture(self, root):
        data = '司会を務め\n猫を飼う\n'.encode('utf-8')
        identity = dict(source='public/wiki', revision='pinned-revision',
                        sample_sha256='fixed-corpus', result_sha256=hashlib.sha256(data).hexdigest())
        metadata = dict(identity, corpus_sha256=identity['sample_sha256'],
                        filename='fast_collocations.txt', evaluation_labels_used=False, combined_entries=2)
        (root / 'fast_collocations.txt').write_bytes(data)
        (root / 'fast_collocations.json').write_text(json.dumps(metadata), encoding='utf-8')
        (root / 'NOTICE-fast-collocations.txt').write_text(
            'CC BY-SA 4.0 ' + identity['source'] + ' ' + identity['revision'] + ' ' + identity['result_sha256'],
            encoding='utf-8')
        return identity, metadata

    def check_payload(self, root, identity):
        check = Checks()
        check_collocations(root, check, required=True, identity=identity)
        return check.ok

    def test_missing_new_payload_fails_but_legacy_can_be_checked(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            identity = dict(result_sha256='0' * 64)
            legacy = Checks()
            check_collocations(root, legacy, identity=identity)
            self.assertTrue(legacy.ok)
            self.assertFalse(self.check_payload(root, identity))

    def test_original_passes_but_dictionary_and_metadata_substitution_fails(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            identity, metadata = self.fixture(root)
            self.assertTrue(self.check_payload(root, identity))
            data = b'substituted\n'
            (root / 'fast_collocations.txt').write_bytes(data)
            metadata.update(result_sha256=hashlib.sha256(data).hexdigest(), combined_entries=1)
            (root / 'fast_collocations.json').write_text(json.dumps(metadata), encoding='utf-8')
            self.assertFalse(self.check_payload(root, identity))

    def test_missing_notice_and_wrong_corpus_are_rejected(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            identity, metadata = self.fixture(root)
            (root / 'NOTICE-fast-collocations.txt').unlink()
            self.assertFalse(self.check_payload(root, identity))
            identity, metadata = self.fixture(root)
            metadata['corpus_sha256'] = 'different-corpus'
            (root / 'fast_collocations.json').write_text(json.dumps(metadata), encoding='utf-8')
            self.assertFalse(self.check_payload(root, identity))

    def test_malformed_manifest_and_evaluation_labels_are_rejected(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            identity, metadata = self.fixture(root)
            (root / 'fast_collocations.json').write_bytes(b'{')
            self.assertFalse(self.check_payload(root, identity))
            (root / 'fast_collocations.json').write_text(json.dumps(metadata), encoding='utf-8')
            self.assertTrue(self.check_payload(root, identity))
            metadata['evaluation_labels_used'] = True
            (root / 'fast_collocations.json').write_text(json.dumps(metadata), encoding='utf-8')
            self.assertFalse(self.check_payload(root, identity))

    def test_usage_source_or_attribution_cannot_be_substituted(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            identity, metadata = self.fixture(root)
            usage = dict(repository='fixed/usage', tag='2025-01-25', usage_sha256='a'*64,
                         license_sha256='b'*64, cforms_sha256='c'*64)
            identity['usage_source'] = dict(usage)
            metadata['usage_source'] = dict(usage)
            path = root / 'fast_collocations.json'
            path.write_text(json.dumps(metadata), encoding='utf-8')
            notice = root / 'NOTICE-fast-collocations.txt'
            notice.write_text(notice.read_text(encoding='utf-8') + ' BSD-2-Clause Hiroyuki Komatsu Shoji Ogura Tsuyoshi Horo ' + ' '.join(usage.values()), encoding='utf-8')
            self.assertTrue(self.check_payload(root, identity))
            metadata['usage_source']['tag'] = 'substituted'
            path.write_text(json.dumps(metadata), encoding='utf-8')
            self.assertFalse(self.check_payload(root, identity))
            metadata['usage_source'] = dict(usage)
            path.write_text(json.dumps(metadata), encoding='utf-8')
            self.assertTrue(self.check_payload(root, identity))
            notice.write_text(notice.read_text(encoding='utf-8').replace('Hiroyuki Komatsu', ''), encoding='utf-8')
            self.assertFalse(self.check_payload(root, identity))


if __name__ == '__main__': unittest.main()
