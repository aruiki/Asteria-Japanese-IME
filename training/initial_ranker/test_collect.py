import unittest
from collect import parse_blocks


def raw_trace(key='ねこ', text='猫', rank=0):
    return (f'---------- Segment 0/1 [FREE] ----------\nねこ\n  0/1 猫\n'
            f'KOTORI_LATTICE\t{rank}\t1000\t0\t{key.encode().hex()}\t{text.encode().hex()}\n'
            'ExecCommand() return false\n').encode()


class TraceTest(unittest.TestCase):
    def test_composer_comma_variants_are_the_same_reading_but_kana_changes_fail(self):
        row = parse_blocks(raw_trace(key='ねこ,', text='猫、'), ['ねこ、'])[0]
        self.assertEqual(row['reading'], 'ねこ、')
        with self.assertRaises(ValueError):
            parse_blocks(raw_trace(key='いぬ,', text='犬、'), ['ねこ、'])

    def test_utf8_hex_roundtrip_and_original_cost(self):
        row = parse_blocks(raw_trace(), ['ねこ'])[0]
        self.assertEqual(row['top1'], '猫')
        self.assertEqual(row['candidates'][0]['cost'], 1000)

    def test_missing_extra_and_wrong_reading_never_form_valid_observation(self):
        for raw, readings in [(b'', ['ねこ']), (raw_trace()*2, ['ねこ']),
                              (raw_trace(key='いぬ'), ['ねこ']), (raw_trace(rank=1), ['ねこ'])]:
            with self.assertRaises(ValueError): parse_blocks(raw, readings)


if __name__ == '__main__': unittest.main()
