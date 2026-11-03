"""候補の一覧の 2 番目以降の質を測る(Issue #208、2026-11-03)。

実際の IME と同じセッションの経路(session_handler_main)で、読みを打って Space を 2 回押し、候補の一覧を取り出す。
同じ読みを AI なし(Mozc と同じ)でも取り出し、次を数える。
- 作文: AI ありの上位 N 件にあって、AI なしの一覧のどこにもない候補(AI が生成して差し込んだ文)。何位にあるか。
- 英字: 上位 N 件の英字だけの候補(カタカナ語の英語表記の差し込み)。
- 正解の順位: 1 番目が正解でないとき、正解が何番目にあるか(選び直す手間)。
確定しないので学習しない(Esc で取り消す)。

使い方: python mozc/tools/candidate_order.py <session_handler_main> --data セット.json --out 結果.json [--top 5]
"""
import argparse
import json
import os
from pathlib import Path
import queue
import re
import subprocess
import sys
import tempfile
import threading
import time

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "eval" / "imebench"))
from imebench import to_romaji  # noqa: E402

sys.path.insert(0, str(Path(__file__).resolve().parent))
import eval_ai  # noqa: E402


def candidates(text):
    if "candidate_window {" not in text:
        return []
    body = text.split("candidate_window {", 1)[1]
    body = re.split(r"\n\w+ \{", body, maxsplit=1)[0]
    return re.findall(r'^\s+value: "([^"\n]*)"', body, re.M)


def delay(text):
    m = re.search(r"delay_millisec: (\d+)", text)
    return int(m.group(1)) if m else None


class Session:
    def __init__(self, exe, models, config):
        env = dict(os.environ, KOTORI_MODEL_DIR=models, KOTORI_RUNTIME_DIR=models)
        self.dir = tempfile.TemporaryDirectory(ignore_cleanup_errors=True)
        self.p = subprocess.Popen([str(exe), "--dictionary=oss", "--profile=" + self.dir.name + "/s"],
                                  cwd=str(exe) + ".runfiles/_main", env=env, stdin=subprocess.PIPE,
                                  stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
        self.lines = queue.Queue()
        threading.Thread(target=lambda: [self.lines.put(l.decode("utf-8", "replace").rstrip())
                                         for l in self.p.stdout] + [self.lines.put(None)], daemon=True).start()
        for c in config:
            k, v = c.split("=", 1)
            self.send("SET_CONFIG\t" + k + "\t" + v)
        self.send("SEND_KEY\tON")

    def send(self, command):
        self.p.stdin.write((command + "\nSHOW_OUTPUT\n").encode("utf-8"))
        self.p.stdin.flush()
        out, tail = [], False
        while True:
            line = self.lines.get(timeout=60)
            if line is None:
                raise SystemExit("session_handler_main が終わった")
            out.append(line)
            if tail and line.strip() == "}":
                return "\n".join(out)
            tail |= line.startswith("removed_candidate_words_for_debug {")

    def settle(self, text, limit=3.0):
        start = time.monotonic()
        while delay(text) is not None and time.monotonic() - start < limit:
            time.sleep(delay(text) / 1000)
            text = self.send("KOTORI_REFRESH")
        return text

    def candidate_list(self, romaji):
        text = ""
        for ch in romaji:
            text = self.send("SEND_KEY\t" + ch)
        self.settle(text)
        self.settle(self.send("SEND_KEY\tSpace"))
        shown = candidates(self.settle(self.send("SEND_KEY\tSpace")))
        self.send("SEND_KEY\tESC")
        self.send("SEND_KEY\tESC")
        self.send("SEND_KEY\tESC")
        return shown

    def close(self):
        self.p.stdin.close()
        self.p.wait(60)
        self.dir.cleanup()


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("exe", type=Path)
    ap.add_argument("--data", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--top", type=int, default=5)
    ap.add_argument("--models", default=eval_ai.default_models())
    ap.add_argument("--warm", type=float, default=15)
    args, extra = ap.parse_known_args()
    os.environ.update(dict(e.split("=", 1) for e in extra if "=" in e))
    items = json.load(open(args.data, encoding="utf-8"))
    exe = args.exe.resolve()
    rows = []
    plain = Session(exe, args.models, ["kotori_lm_enabled=false"])
    ai = Session(exe, args.models, [])
    ai.send("SEND_KEYS\tai")
    time.sleep(args.warm)
    ai.send("SEND_KEY\tESC")
    for it in items:
        romaji = it.get("romaji") or to_romaji(it["input"])
        if romaji is None:
            continue
        base = plain.candidate_list(romaji)
        got = ai.candidate_list(romaji)
        expected = it.get("expected_output") or []
        top = got[: args.top]
        invented = [(i + 1, v) for i, v in enumerate(top) if v not in base and i > 0]
        ascii_words = [(i + 1, v) for i, v in enumerate(top) if v and all(ord(c) < 128 for c in v) and i > 0]
        rank = next((i + 1 for i, v in enumerate(got) if v in expected), None)
        rows.append(dict(index=it.get("index"), input=it["input"], expected=expected, ai=got[:12], plain=base[:12],
                         invented=invented, ascii=ascii_words, answer_rank=rank))
    plain.close()
    ai.close()
    json.dump(rows, open(args.out, "w", encoding="utf-8"), ensure_ascii=False, indent=1)
    n = len(rows)
    with_invented = sum(1 for r in rows if r["invented"])
    invented_total = sum(len(r["invented"]) for r in rows)
    with_ascii = sum(1 for r in rows if r["ascii"])
    wrong_top = [r for r in rows if r["expected"] and r["answer_rank"] != 1]
    ranks = [r["answer_rank"] for r in wrong_top if r["answer_rank"]]
    print(f"{args.data}: 問 {n} 上位{args.top}に作文がある {with_invented}(作文 {invented_total} 件) "
          f"上位{args.top}に英字 {with_ascii} 1番目が正解でない {len(wrong_top)} "
          f"そのときの正解の順位 平均 {round(sum(ranks) / len(ranks), 2) if ranks else '-'} 一覧にない {len(wrong_top) - len(ranks)}")


if __name__ == "__main__":
    main()
