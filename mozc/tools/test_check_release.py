"""移行後の配布名と、旧版を確認するフォールバック。"""
import unittest
from check_release import choose_asset_name

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
if __name__ == '__main__': unittest.main()
