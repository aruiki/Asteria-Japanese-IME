"""Mozcと同じprotocからSessionHandlerの応答スキーマを生成する。"""
import argparse
from pathlib import Path
import subprocess


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--protoc', type=Path, required=True)
    ap.add_argument('--mozc', type=Path, required=True, help='Mozc src directory')
    ap.add_argument('--out', type=Path, required=True)
    args = ap.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    sources = ['commands','config','candidate_window','engine_builder','user_dictionary_storage']
    subprocess.run([str(args.protoc.resolve()), '--proto_path=' + str(args.mozc.resolve()),
                    '--python_out=' + str(args.out.resolve())] + ['protocol/' + p + '.proto' for p in sources],
                   check=True)
    print('Generated Python protocol files in', args.out.resolve())


if __name__ == '__main__': main()
