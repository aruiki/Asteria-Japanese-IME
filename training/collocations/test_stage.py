import hashlib
import io
from pathlib import Path
import tempfile
import unittest
from unittest import mock

from stage import fetch_range, load_chunk, save_fixed


class Response(io.BytesIO):
    status = 206
    def __init__(self, data, content_range='bytes 2-4/10'):
        super().__init__(data)
        self.headers = {'Content-Range': content_range}


class StageTest(unittest.TestCase):
    def test_range_must_be_bounded_and_exact(self):
        with mock.patch('urllib.request.urlopen', return_value=Response(b'abc')) as call:
            self.assertEqual(fetch_range('https://example.test/fixed', 2, 3, 10), b'abc')
            self.assertEqual(call.call_args.args[0].get_header('Range'), 'bytes=2-4')
        for response in [Response(b'abc', 'bytes 1-3/10'), Response(b'abcd'), Response(b'ab')]:
            with mock.patch('urllib.request.urlopen', return_value=response):
                with self.assertRaises(ValueError):
                    fetch_range('https://example.test/fixed', 2, 3, 10)

    def test_full_download_instead_of_range_is_rejected(self):
        response = Response(b'abc'); response.status = 200
        with mock.patch('urllib.request.urlopen', return_value=response):
            with self.assertRaises(ValueError):
                fetch_range('https://example.test/fixed', 2, 3, 10)

    def test_mismatched_cache_is_preserved_and_not_replaced(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory); cache = root / 'cache'; cache.mkdir()
            path = cache / 'train_wikipedia.jsonl.01.raw';path.write_bytes(b'wrong')
            chunk = dict(file='train_wikipedia.jsonl', group='train_wikipedia.jsonl:01',
                         bytes=3, sha256=hashlib.sha256(b'abc').hexdigest())
            with mock.patch('stage.fetch_range') as fetch:
                with self.assertRaises(ValueError):
                    load_chunk({}, chunk, root / 'out', cache)
                fetch.assert_not_called()
            self.assertEqual(path.read_bytes(), b'wrong')

    def test_staging_refuses_different_existing_artifact(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'data.txt'
            save_fixed(path, b'first');save_fixed(path, b'first')
            with self.assertRaises(ValueError):save_fixed(path, b'different')
            self.assertEqual(path.read_bytes(), b'first')


if __name__ == '__main__':
    unittest.main()
