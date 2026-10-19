"""非同期変換の検査側が、打鍵応答と結果の到着を混同しないことを確認する。"""
import types
import unittest
from unittest.mock import Mock, patch

from first_display_compare import Session


class AsyncConversionTest(unittest.TestCase):
    def pending(self):
        return dict(command='SEND_KEY\tTab', output='consumed: true\ncallback {\n session_command { type: KOTORI_REFRESH_SUGGESTION }\n delay_millisec: 100\n}', response_ms=2, first_candidate=None)

    def ready(self):
        return dict(command='KOTORI_REFRESH', output='consumed: true\ncandidate_window {}', response_ms=3, first_candidate='AIの候補')

    @patch('first_display_compare.time.sleep')
    def test_pending_is_polled_until_result_without_replacing_key_latency(self, sleep):
        session=types.SimpleNamespace(send=Mock(side_effect=[self.pending(),self.pending(),self.ready()]))
        result=Session.send_conversion(session,'SEND_KEY\tTab')
        self.assertEqual(session.send.call_args_list[1].args,('KOTORI_REFRESH',))
        self.assertEqual(len(result['callback_events']),2)
        self.assertEqual(result['response_ms'],2)
        self.assertEqual(result['completion_response_ms'],3)
        self.assertEqual(result['first_candidate'],'AIの候補')
        self.assertEqual(result['command'],'SEND_KEY\tTab')
        self.assertEqual(result['initial_event']['first_candidate'],None)

    def test_synchronous_conversion_does_not_poll(self):
        session=types.SimpleNamespace(send=Mock(return_value=self.ready()))
        self.assertEqual(Session.send_conversion(session,'SEND_KEY\tSpace')['callback_events'],[])
        session.send.assert_called_once_with('SEND_KEY\tSpace')

    def test_candidate_text_cannot_request_a_callback(self):
        event=self.ready(); event['output']='candidate_window { value: "KOTORI_REFRESH_SUGGESTION" }'
        session=types.SimpleNamespace(send=Mock(return_value=event))
        self.assertEqual(Session.send_conversion(session,'SEND_KEY\tSpace')['callback_events'],[])
        session.send.assert_called_once()

    def test_delivered_commit_does_not_follow_timer_for_new_composition(self):
        event=self.pending(); event['output']='result { value: "確定" }\n'+event['output']
        session=types.SimpleNamespace(send=Mock(return_value=event))
        self.assertEqual(Session.send_conversion(session,'SEND_KEY\tEnter')['callback_events'],[])
        session.send.assert_called_once()

    @patch('first_display_compare.time.sleep')
    @patch('first_display_compare.time.monotonic',side_effect=[0,1])
    def test_missing_completion_is_an_error(self, clock, sleep):
        session=types.SimpleNamespace(send=Mock(return_value=self.pending()))
        with self.assertRaises(TimeoutError): Session.send_conversion(session,'SEND_KEY\tTab',timeout=.1)
        sleep.assert_not_called()

    @patch('first_display_compare.time.sleep')
    def test_cancelled_callback_is_not_a_successful_empty_result(self, sleep):
        cancelled=self.ready(); cancelled['output']='consumed: false\n'
        session=types.SimpleNamespace(send=Mock(side_effect=[self.pending(),cancelled]))
        with self.assertRaises(RuntimeError): Session.send_conversion(session,'SEND_KEY\tTab')


if __name__=='__main__': unittest.main()
