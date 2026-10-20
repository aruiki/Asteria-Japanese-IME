import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

import scored_alternatives_acceptance as acceptance


class ScoredAlternativesAcceptanceTest(unittest.TestCase):
    def test_rejects_empty_duplicate_path_or_command_injection_cases(self):
        good=dict(index='case-1',reading='あし',alternative='足',expected='足')
        acceptance.validate_cases([good])
        for cases in ([],[good,good],[dict(good,index='../case')],[dict(good,alternative='足\nSEND_KEY\tEnter')]):
            with self.subTest(cases=cases),self.assertRaises(ValueError):
                acceptance.validate_cases(cases)

    def test_wrong_exe_identity_fails_before_proto_import_or_process(self):
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary)
            exe=root/'not-an-executable.exe'
            exe.write_bytes(b'source identity guard')
            identity=root/'identity.json'
            identity.write_text(json.dumps(dict(source_commit='a'*40,session_handler_sha256='0'*64)),encoding='utf-8')
            out=root/'output'
            command=[sys.executable,'-X','utf8',str(Path(acceptance.__file__)),'--exe',str(exe),
                     '--identity',str(identity),'--cases',str(root/'missing-cases.json'),
                     '--install',str(root/'missing-models'),'--proto',str(root/'missing-proto'),
                     '--romanji-table',str(root/'missing-table'),'--out',str(out)]
            result=subprocess.run(command,capture_output=True,text=True,timeout=20)
            self.assertNotEqual(result.returncode,0)
            self.assertIn('source identity and EXE SHA differ',result.stderr)
            self.assertNotIn('ModuleNotFoundError',result.stderr)
            self.assertFalse(out.exists())


if __name__=='__main__':
    unittest.main()
