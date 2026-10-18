import unittest
from build_usage_collocations import extract, forms


class UsageCollocationTest(unittest.TestCase):
    def test_definition_and_bare_synonym_are_not_collocations(self):
        phrases, _ = extract('あう\t会う\tverb\t人にあう。「会う」に同じ。\n', {})
        self.assertEqual(phrases, [])

    def test_explicit_example_and_conjugations_at_the_end_are_used(self):
        rules = forms('verb 基本形 る る\nverb 連用タ接続 っ っ\n')
        phrases, _ = extract('ふる\t降る\tverb\t水滴が落ちる。「雨が降る」\n', rules)
        self.assertIn('雨が降る', phrases)
        self.assertIn('雨が降っ', phrases)
        self.assertNotIn('水滴が落ちる', phrases)

    def test_left_adjective_does_not_create_invalid_noun_connection(self):
        rules = forms('adj 基本形 い い\nadj 連用形 く く\n')
        phrases, _ = extract('あつい\t厚い\tadj\t幅がある。「厚い本」\n', rules)
        self.assertIn('厚い本', phrases)
        self.assertNotIn('厚く本', phrases)

    def test_same_form_name_can_keep_multiple_valid_suffixes(self):
        rules = forms('verb 基本形 く く\nverb 未然形 こ こ\nverb 未然形 か か\n')
        phrases, _ = extract('なく\t鳴く\tverb\t「鳥が鳴く」\n', rules)
        self.assertIn('鳥が鳴こ', phrases)
        self.assertIn('鳥が鳴か', phrases)

    def test_other_words_definition_examples_are_not_assigned_to_this_word(self):
        phrases, _ = extract('あう\t会う\tverb\t「雨が降る」\n', {})
        self.assertEqual(phrases, [])


if __name__ == '__main__': unittest.main()
