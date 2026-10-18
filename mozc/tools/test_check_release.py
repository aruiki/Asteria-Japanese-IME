"""移行後の配布名と、旧版を確認するフォールバック。"""
import unittest
from check_release import choose_asset_name, check_generator, product_release_pair, Checks
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


if __name__ == '__main__': unittest.main()
