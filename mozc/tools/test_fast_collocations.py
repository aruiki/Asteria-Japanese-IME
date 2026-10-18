"""学習区間と評価除外の陰性試験。"""
import unittest

from build_fast_collocations import collect, fragments


class FastCollocationTest(unittest.TestCase):
    @staticmethod
    def rows():
        return [dict(input='ごれいをかいせつする' + str(i),
                     output='新しい語例を解説する' + suffix,
                     split='train', source_group=f'train_wikipedia.jsonl:{1 + i % 2:02}')
                for i, suffix in enumerate(['。', '！', '？', '、', 'です。', 'のです。'])]

    def test_held_out_groups_and_non_wikipedia_do_not_count(self):
        rows = self.rows()
        for i, row in enumerate(rows):
            if i < 3:
                row['split'] = 'dev'
            else:
                row['source_group'] = 'train_llm-jp-corpus-v3.jsonl:01'
        phrases, audit = collect(rows, set())
        self.assertEqual(phrases, [])
        self.assertEqual(audit['not_wikipedia_train'], 6)

    def test_repeated_sentence_cannot_create_a_rule(self):
        phrases, audit = collect([self.rows()[0]] * 50, set())
        self.assertEqual(phrases, [])
        self.assertEqual(audit['duplicate_sentence'], 49)

    def test_evaluation_readings_and_near_duplicates_are_excluded(self):
        rows = self.rows()
        for row in rows:
            row['input'] = 'これはひじょうにながいひょうかようのにゅうりょくです'
        phrases, audit = collect(rows, {'これはひじょうにながいひょうかようのにゅうりょくです。'})
        self.assertEqual(phrases, [])
        self.assertEqual(audit['evaluation_reading_or_near_duplicate'], 6)

    def test_multiple_independent_sentences_and_groups_required(self):
        phrases, _ = collect(self.rows(), set())
        self.assertIn('語例を解説', phrases)
        rows = self.rows()
        for row in rows:
            row['source_group'] = 'train_wikipedia.jsonl:01'
        self.assertEqual(collect(rows, set())[0], [])

    def test_ascii_numbers_and_punctuation_do_not_bridge_phrases(self):
        self.assertNotIn('語例を解説', fragments('語例を1解説'))
        self.assertNotIn('語例を解説', fragments('語例を、解説'))


if __name__ == '__main__':
    unittest.main()
