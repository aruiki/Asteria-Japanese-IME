"""移行後の配布名と、旧版を確認するフォールバック。"""
import unittest
from check_release import choose_asset_name, check_generator, Checks
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
