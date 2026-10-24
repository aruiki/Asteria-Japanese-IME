"""最初の変換までの待ちを、実際のIMEと同じセッションの経路で測る（ログイン時の常駐、GOAL 2026-10-22）。

session_handler_main を新しく起動し、場面ごとに1文を打ってSpaceを押し、AIの結果が届いて候補が表示される
までの時間を測る。場面:
  cold     起動してすぐ（ログイン直後、読み込みの途中で変換する）
  loaded   AIの状態が読み込み済みになってから（ログイン時に読み込み終えた後）
  idle     読み込み後に --idle 秒何もしない（既定の常駐なら待ちなし。外す設定なら読み込み直しを待つ）
  no_model モデルのないフォルダ（AIが起動できない）。Mozcの候補で変換が続くこと
  no_runtime llama.dll のないフォルダ（同上）

使い方: python mozc/tools/first_conversion.py <session_handler_main> --out 結果.json [--repeat 3] [名前=値 ...]
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

P = Path(__file__).resolve().parents[2]
SENTENCE = "きょうはいいてんきですね"


class Session:
    def __init__(self, exe, profile, env):
        self.profile = Path(profile)
        self.p = subprocess.Popen([str(exe), "--dictionary=oss", "--profile=" + str(self.profile)],
                                  cwd=str(exe) + ".runfiles/_main", env=env, stdin=subprocess.PIPE,
                                  stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
        self.lines = queue.Queue()
        threading.Thread(target=lambda: [self.lines.put(l.decode("utf-8", "replace").rstrip())
                                         for l in self.p.stdout] + [self.lines.put(None)], daemon=True).start()

    def send(self, command):
        self.p.stdin.write((command + "\nSHOW_OUTPUT\n").encode("utf-8"))
        self.p.stdin.flush()
        out, tail = [], False
        deadline = time.monotonic() + 120
        while True:
            line = self.lines.get(timeout=max(.01, deadline - time.monotonic()))
            if line is None:
                raise RuntimeError("session handler exited")
            out.append(line)
            if tail and line.strip() == "}":
                return "\n".join(out)
            tail |= line.startswith("removed_candidate_words_for_debug {")

    def ai_state(self):
        path = self.profile / "kotori_ai_status.txt"
        text = path.read_text(encoding="utf-8", errors="replace") if path.exists() else ""
        m = re.search(r"^state=(\w+)", text, re.M)
        return m.group(1) if m else "none"

    def convert(self):
        """1文を打ってSpace。Spaceから候補が表示されるまでの時間と、表示された文を返す。"""
        self.send("SEND_KEY\tON")
        text = self.send("SEND_KEYS\t" + to_romaji(SENTENCE))
        started = time.monotonic()
        text = self.send("SEND_KEY\tSpace")
        waited = 0
        while (m := re.search(r"delay_millisec: (\d+)", text)) and waited < 400:
            time.sleep(int(m.group(1)) / 1000)
            text = self.send("KOTORI_REFRESH")
            waited += 1
        ms = (time.monotonic() - started) * 1000
        body = text.split("preedit {", 1)[1] if "preedit {" in text else ""
        body = re.split(r"\n\w+ \{", body, maxsplit=1)[0]
        shown = "".join(re.findall(r'^\s+value: "([^"\n]*)"', body, re.M))
        self.send("SEND_KEY\tESC")
        self.send("SEND_KEY\tESC")
        return dict(space_to_candidate_ms=round(ms, 1), shown=shown, refreshes=waited)

    def wait_state(self, states, timeout):
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            if self.ai_state() in states:
                return True
            time.sleep(.2)
        return False

    def close(self):
        self.p.stdin.close()
        self.p.wait(timeout=60)


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("exe", type=Path)
    ap.add_argument("--out", required=True)
    ap.add_argument("--models", default=str(P / "target/accuracy-guide/release-2.1.0-package-v2/check/ext/PFiles/Kotori"))
    ap.add_argument("--repeat", type=int, default=3)
    ap.add_argument("--idle", type=float, default=45, help="idle場面で何もしない秒数")
    ap.add_argument("--scenes", default="cold,loaded,idle,no_model,no_runtime")
    args, extra = ap.parse_known_args()
    exe = args.exe.resolve()
    base = dict(os.environ, KOTORI_MODEL_DIR=args.models, KOTORI_RUNTIME_DIR=args.models)
    base.update(dict(e.split("=", 1) for e in extra if "=" in e))
    results = []
    with tempfile.TemporaryDirectory(prefix="kotori-first-conversion-") as tmp:
        empty = Path(tmp) / "empty"
        empty.mkdir()
        no_runtime = Path(tmp) / "no-runtime"
        no_runtime.mkdir()
        for gguf in Path(args.models).glob("*.gguf"):
            os.link(gguf, no_runtime / gguf.name)  # モデルはあるがllama.dllがない
        for scene in args.scenes.split(","):
            for i in range(args.repeat):
                env = dict(base)
                if scene == "no_model":
                    env.update(KOTORI_MODEL_DIR=str(empty))
                if scene == "no_runtime":
                    env.update(KOTORI_MODEL_DIR=str(no_runtime), KOTORI_RUNTIME_DIR=str(no_runtime))
                s = Session(exe, Path(tmp) / f"{scene}-{i}", env)
                row = dict(scene=scene, trial=i)
                try:
                    if scene in ("loaded", "idle"):
                        row["loaded_in_time"] = s.wait_state(("llm", "zenz"), 120)
                    if scene == "idle":
                        time.sleep(args.idle)
                    row["state_before"] = s.ai_state()
                    row.update(s.convert())
                    row["state_after"] = s.ai_state()
                finally:
                    s.close()
                # Mozcだけでも漢字の候補になっていること（読みのままではない）。
                row["converted"] = bool(re.search(r"[一-鿿]", row.get("shown", "")))
                print(json.dumps(row, ensure_ascii=False), flush=True)
                results.append(row)
    Path(args.out).write_text(json.dumps(dict(exe=str(exe), models=args.models, extra=extra, idle=args.idle,
                                              results=results), ensure_ascii=False, indent=1), encoding="utf-8")


if __name__ == "__main__":
    main()
