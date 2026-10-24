"""実機の打ち方(eval/imebench/ImeBench.cs)を、セッションの経路でそのまま再現して確定した文を集める。

1 文字ごとに --key-ms 空けて打ち(その間に取り直しの予約が来れば TIP と同じく送る)、--pause-ms 待ってから Space、
--after-space-ms 待って Enter。Enter で確定した文字列(result)を記録する。実機で起きた誤りがサーバー側で
起きるかどうかを切り分けるために使う。

使い方: python mozc/tools/commit_replay.py <session_handler_main> --data セット.json --out 結果.json [--n 20]
"""
import argparse
import json
import os
from pathlib import Path
import re
import shutil
import sys
import tempfile
import time

sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "eval" / "imebench"))
from first_conversion import Session  # noqa: E402
from imebench import to_romaji  # noqa: E402

P = Path(__file__).resolve().parents[1]


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("exe", type=Path)
    ap.add_argument("--data", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--models", default=str(P / "target/accuracy-guide/release-6446-package-v1/check/PFiles/Kotori"))
    ap.add_argument("--n", type=int, default=20)
    ap.add_argument("--key-ms", type=float, default=12)
    ap.add_argument("--pause-ms", type=float, default=250)
    ap.add_argument("--after-space-ms", type=float, default=1500)
    ap.add_argument("--config", action="append", default=[], help="設定の項目 名前=値(SET_CONFIG)")
    ap.add_argument("--profile-copy", type=Path, help="このフォルダ(利用者のプロファイル等)の写しで動かす")
    ap.add_argument("--test-keys", action="store_true",
                    help="TIP と同じく、各キーの前に TEST_SEND_KEY を送る")
    args, extra = ap.parse_known_args()
    env = dict(os.environ, KOTORI_MODEL_DIR=args.models, KOTORI_RUNTIME_DIR=args.models)
    env.update(dict(e.split("=", 1) for e in extra if "=" in e))
    items = json.load(open(args.data, encoding="utf-8"))[: args.n]
    rows = []
    with tempfile.TemporaryDirectory(prefix="kotori-commit-replay-") as tmp:
        profile = Path(tmp) / "p"
        if args.profile_copy:
            # 利用者の設定と学習履歴の写しで動かす(元のフォルダには書かない)。
            shutil.copytree(args.profile_copy, profile,
                            ignore=lambda folder, names: [n for n in names if n.endswith((".ipc", ".lock"))])
        s = Session(args.exe.resolve(), profile, env)
        for item in args.config:
            name, value = item.split("=", 1)
            s.send("SET_CONFIG\t" + name + "\t" + value)
        s.send("SEND_KEY\tON")
        s.send("SEND_KEYS\tai")
        time.sleep(15)
        s.send("SEND_KEY\tESC")

        def wait_with_callbacks(text, seconds):
            # TIP と同じく、予約された時刻が来れば取り直しを送る。それ以外は待つだけ。
            end = time.monotonic() + seconds
            due = None
            while True:
                m = re.search(r"delay_millisec: (\d+)", text)
                if m:
                    due = time.monotonic() + int(m.group(1)) / 1000
                if due is None or due > end:
                    time.sleep(max(0, end - time.monotonic()))
                    return text
                time.sleep(max(0, due - time.monotonic()))
                due = None
                text = s.send("KOTORI_REFRESH")

        for it in items:
            romaji = to_romaji(it["input"])
            if romaji is None:
                continue
            text = ""
            for ch in romaji:
                if args.test_keys:
                    s.send("TEST_SEND_KEY\t" + ch)
                text = s.send("SEND_KEY\t" + ch)
                text = wait_with_callbacks(text, args.key_ms / 1000)
            text = wait_with_callbacks(text, args.pause_ms / 1000)
            if args.test_keys:
                s.send("TEST_SEND_KEY\tSpace")
            text = s.send("SEND_KEY\tSpace")
            text = wait_with_callbacks(text, args.after_space_ms / 1000)
            if args.test_keys:
                s.send("TEST_SEND_KEY\tEnter")
            text = s.send("SEND_KEY\tEnter")
            m = re.search(r'result \{\s*type: \w+\s*value: "([^"]*)"', text)
            rows.append(dict(index=it["index"], expected=it["expected_output"], committed=m.group(1) if m else ""))
            print(it["index"], rows[-1]["committed"], flush=True)
            s.send("SEND_KEY\tESC")
        s.close()
    Path(args.out).write_text(json.dumps(rows, ensure_ascii=False, indent=1), encoding="utf-8")


if __name__ == "__main__":
    main()
