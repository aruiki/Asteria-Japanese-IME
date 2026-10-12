"""大量測定で検査側の誤りを製品の不具合と誤報しない。"""
import io
import json
from pathlib import Path
import tempfile
import types
import unittest
from unittest.mock import patch
from large_input_cases import romanize, roman_table
from large_input_campaign import assess, norm
from first_display_compare import Session


class CampaignAssessmentTest(unittest.TestCase):
    def case(self):
        return dict(reading='でんきをけしてね',expected=['電気を消してね'],predicates=[])

    def events(self, value='電気を消してね'):
        return dict(typed=dict(key='でんきをけしてね',candidates=[]),
                    space=dict(value=value,result='',candidates=[],ms=20),
                    commit=dict(result=value))

    def test_actual_reading_loss_is_not_a_surface_review(self):
        events=self.events(); events['typed']['key']='でんきをけして'
        defects,reviews=assess(self.case(),events)
        self.assertIn('typed_reading_mismatch',defects)
        self.assertFalse(reviews)

    def test_semantic_mismatch_is_distinct_from_transport_defect(self):
        defects,reviews=assess(self.case(),self.events('電気を消して寝'))
        self.assertFalse(defects)
        self.assertEqual(reviews,['expected_surface_mismatch'])

    def test_commit_must_preserve_the_selected_preedit(self):
        events=self.events(); events['commit']['result']='電気を消して'
        defects,_=assess(self.case(),events)
        self.assertIn('commit_different_from_preedit',defects)

    def test_allowed_widths_do_not_create_false_mismatches(self):
        case=dict(reading='１０じ',expected=['10時'],predicates=[])
        events=dict(typed=dict(key='10じ',candidates=[]),space=dict(value='１０時',result='',candidates=[],ms=20),commit=dict(result='10時'))
        self.assertEqual(assess(case,events),([],[]))

    def test_negative_upstream_oracle_checks_actual_surface(self):
        case=self.case(); case['expected']=[]
        case['predicates']=[{'command':'Conversion Not Match','value':'寝'}]
        _,reviews=assess(case,self.events('電気を消して寝'))
        self.assertEqual(reviews,['upstream_forbidden_surface'])

    def test_unknown_meaning_is_not_marked_correct_or_incorrect(self):
        case=self.case(); case['expected']=[]
        self.assertEqual(assess(case,self.events('別の表記')),([],[]))

    def test_one_name_variant_does_not_prove_all_variants_accessible(self):
        case=self.case(); case['surname_targets']=['道島','道嶋']
        events=self.events(); events['space']['candidates']=['道島']
        defects,reviews=assess(case,events)
        self.assertFalse(defects)
        self.assertEqual(reviews,['surname_some_variants_not_observed'])

    def test_missing_commit_is_not_silently_counted_as_verified(self):
        events=self.events(); del events['commit']
        with self.assertRaises(ValueError): assess(self.case(),events)
        self.assertEqual(assess(self.case(),events,check_commit=False),([],[]))


class RomanizationTest(unittest.TestCase):
    def test_pending_n_small_tsu_and_punctuation_are_unambiguous(self):
        with tempfile.TemporaryDirectory() as d:
            path=Path(d)/'table.tsv'; path.write_text('a\tあ\nn\tん\nnn\tん\nkka\tっ\tk\n.\t。\n',encoding='utf-8')
            table=roman_table(path)
            self.assertEqual(romanize('あんっ。',table),"an'xtu.")

    def test_control_characters_cannot_be_injected_as_commands(self):
        with self.assertRaises(ValueError): romanize('あ\nSEND_KEY',{'あ':'a'})


class SessionEncodingFailureTest(unittest.TestCase):
    def test_invalid_utf8_wakes_reader_instead_of_silently_hanging(self):
        class FakeProcess:
            def __init__(self):
                self.stdin=io.BytesIO(); self.stdout=io.BytesIO(b'ERROR: bad path \x83\n'); self.returncode=None
            def kill(self): self.returncode=-1
            def wait(self,timeout=None): self.returncode=0 if self.returncode is None else self.returncode
            def poll(self): return self.returncode
        with tempfile.TemporaryDirectory() as d, patch('first_display_compare.subprocess.Popen',return_value=FakeProcess()):
            session=Session(Path('dummy'),{},Path(d),Path(d)/'stderr')
            try:
                with self.assertRaisesRegex(RuntimeError,'non-UTF8'):
                    session.send('SEND_KEY\tON')
            finally: session.close(failed=True)

class EvidenceIntegrityTest(unittest.TestCase):
    def setUp(self):
        self.case = dict(id='c1',reading='でんきをけしてね',family='curated',expected=['電気を消してね'],predicates=[])
        self.row = dict(scenario_id='c1:immediate',case_id='c1',reading=self.case['reading'],family='curated',
                        expected=self.case['expected'],mode='immediate',defects=[],reviews=[],events={})

    def test_duplicate_operations_cannot_inflate_input_count(self):
        from large_input_summary import summarize
        with self.assertRaisesRegex(ValueError,'duplicate scenario'):
            summarize([self.case],[self.row,self.row])

    def test_replay_does_not_count_as_another_unique_input(self):
        from large_input_summary import summarize
        replay = dict(self.row,scenario_id='c1:settled',mode='settled')
        summary,_=summarize([self.case],[self.row,replay])
        self.assertEqual(summary['unique_primary_inputs_executed'],1)
        self.assertEqual(summary['scenarios_total'],2)

    def test_modified_gold_cannot_be_mixed_with_old_results(self):
        from large_input_summary import summarize
        altered=dict(self.row,expected=['別の正解'])
        with self.assertRaisesRegex(ValueError,'pinned corpus'):
            summarize([self.case],[altered])

    def test_unexecuted_input_is_not_marked_complete(self):
        from large_input_summary import summarize
        extra=dict(self.case,id='c2',reading='べつのよみ')
        summary,_=summarize([self.case,extra],[self.row])
        self.assertFalse(summary['primary_complete'])

    def test_unknown_selected_case_must_fail_replay(self):
        from large_input_replay import selected_cases
        with self.assertRaisesRegex(ValueError,'unknown'):
            selected_cases([self.case],{'unknown'})

    def test_repeated_corpus_id_must_fail_replay(self):
        from large_input_replay import selected_cases
        with self.assertRaisesRegex(ValueError,'duplicate'):
            selected_cases([self.case,self.case],{'c1'})


class SessionTimeoutTest(unittest.TestCase):
    def test_missing_response_has_a_diagnostic_instead_of_queue_empty(self):
        import queue
        session=object.__new__(Session)
        session.proc=types.SimpleNamespace(stdin=io.BytesIO())
        session.lines=types.SimpleNamespace(get=lambda **kwargs: (_ for _ in ()).throw(queue.Empty()))
        with self.assertRaisesRegex(RuntimeError,'60秒'):
            session.send('SEND_KEY\tSpace')


class CpuSubsetTest(unittest.TestCase):
    def test_stratification_is_reproducible_and_keeps_curated_cases(self):
        from large_input_cases import cpu_subset
        cases = [dict(id=str(i), family='business') for i in range(220)]
        curated = [dict(id='curated', family='curated-daily')]
        original = list(cases + curated)
        selected = cpu_subset(original)
        self.assertEqual(selected, cpu_subset(original))
        self.assertEqual(len(selected), 201)
        self.assertIn(curated[0], selected)
        self.assertEqual(original, cases + curated)
        self.assertEqual(len({c['id'] for c in selected}), len(selected))
