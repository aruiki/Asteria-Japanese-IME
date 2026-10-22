import json
from pathlib import Path
import unittest

from build_diagnostic import choose, width_variants


class DiagnosticTest(unittest.TestCase):
    def test_width_rules_are_explicit_and_preserve_original_spelling(self):
        self.assertEqual(width_variants('日本AI2'), {'日本AI2', '日本ＡＩ２'})
        self.assertEqual(width_variants('日本ＡＩ２'), {'日本AI2', '日本ＡＩ２'})

    def test_exclusions_aliases_and_unique_readings_apply_before_output(self):
        policy = json.loads(Path(__file__).with_name('diagnostic_policy.json').read_text('utf-8'))
        reading = 'じんこうことばあつめ'
        excluded = 'じんこうことばしらべ'
        groups = {
            reading: dict(targets={'新しい語'}, expected={'新しい語', '従来の同音語'}, positions={'名詞1', '名詞2'}),
            excluded: dict(targets={'除外語'}, expected={'除外語'}, positions={'名詞1'}),
        }
        result = choose(groups, {excluded}, policy)
        self.assertEqual(len(result), 1)
        self.assertEqual(result[0]['input'], reading)
        self.assertEqual(set(result[0]['expected_output']), {'新しい語', '従来の同音語'})
        self.assertEqual(result[0]['target_surfaces'], ['新しい語'])


if __name__ == '__main__':
    unittest.main()
