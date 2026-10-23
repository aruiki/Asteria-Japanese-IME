"""実際のIMEと同じセッション処理の経路で評価する（2026-10-23）。

converter_main の評価専用モード(KOTORI_LM_PRELOAD=0)はLLM補正を止め、非同期読込のCLIではAIの結果を
待たない。製品の変換は session の KotoriPrepareConversion / KOTORI_REFRESH / 補正を通るので、
session_handler_main を1プロセスで起動し、問ごとに:
  ローマ字の打鍵 → 取り直しの予約を待って KOTORI_REFRESH → Space → AIの結果が届くまで KOTORI_REFRESH
  → 表示された文(preeditの各文節の値)を第一候補とする → Escで取り消す(確定しないので学習しない)
前文は SEND_KEYS の context.preceding_text で渡す(--context)。

使い方: python mozc/tools/session_eval.py <session_handler_main> --data セット.json --out 結果.json [--context]
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


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("exe", type=Path)
    ap.add_argument("--data", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--context", action="store_true")
    ap.add_argument("--models", default=str(P / "target/accuracy-guide/short-word-package-check-v7/payload/ext/PFiles/Kotori"))
    ap.add_argument("--warm", type=float, default=15)
    ap.add_argument("--n", type=int, default=0)
    ap.add_argument("--immediate-space", action="store_true",
                    help="打ち終わってすぐSpaceを押す(取り直しの予約を待たない。速く打つ人やimebenchと同じ)")
    args, extra = ap.parse_known_args()
    items = json.load(open(args.data, encoding="utf-8"))
    if args.n:
        items = items[: args.n]
    env = dict(os.environ, KOTORI_MODEL_DIR=args.models, KOTORI_RUNTIME_DIR=args.models)
    env.update(dict(e.split("=", 1) for e in extra if "=" in e))
    exe = args.exe.resolve()
    lines = queue.Queue()
    rows = []
    with tempfile.TemporaryDirectory(prefix="kotori-session-eval-") as profile:
        p = subprocess.Popen([str(exe), "--dictionary=oss", "--profile=" + str(Path(profile) / "session")],
                             cwd=str(exe) + ".runfiles/_main", env=env, stdin=subprocess.PIPE,
                             stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
        threading.Thread(target=lambda: [lines.put(l.decode("utf-8", "replace").rstrip()) for l in p.stdout] +
                         [lines.put(None)], daemon=True).start()

        def send(command):
            p.stdin.write((command + "\nSHOW_OUTPUT\n").encode("utf-8"))
            p.stdin.flush()
            out, tail = [], False
            deadline = time.monotonic() + 60
            while True:
                line = lines.get(timeout=max(.01, deadline - time.monotonic()))
                if line is None:
                    raise RuntimeError("session handler exited: " + "\n".join(out[-20:]))
                out.append(line)
                if tail and line.strip() == "}":
                    return "\n".join(out)
                tail |= line.startswith("removed_candidate_words_for_debug {")

        def callback_ms(text):
            m = re.search(r"delay_millisec: (\d+)", text)
            return int(m.group(1)) if m else None

        def preedit(text):
            if "preedit {" not in text:
                return ""
            body = re.split(r"\n\w+ \{", text.split("preedit {", 1)[1], maxsplit=1)[0]
            return "".join(re.findall(r'^\s+value: "([^"\n]*)"', body, re.M))

        send("SEND_KEY\tON\nSEND_KEYS\tai")
        time.sleep(args.warm)
        send("SEND_KEY\tESC")
        for it in items:
            romaji = to_romaji(it["input"])
            if romaji is None:
                rows.append(dict(index=it["index"], skipped=True))
                continue
            started = time.monotonic()
            context = it.get("context_text", "") if args.context else ""
            # 文字列の項目はテキスト形式で引用符が要る(session_handler_toolの規則)。
            option = "context.preceding_text=\"" + context.replace("\\", "\\\\").replace('"', '\\"') + "\""
            if context:
                # 実際のTSFと同じく、すべてのキーに同じ周囲文を付ける。最初のキーだけに付けると、
                # 文脈の変化を欄の切り替えとみなして入力途中のローマ字が捨てられる。
                for ch in romaji:
                    text = send("SEND_KEY_WITH_OPTION\t" + ch + "\t" + option)
            else:
                text = send("SEND_KEYS\t" + romaji)
            for _ in range(0 if args.immediate_space else 6):
                delay = callback_ms(text)
                if delay is None:
                    break
                time.sleep(delay / 1000)
                text = send("KOTORI_REFRESH")
            text = send("SEND_KEY_WITH_OPTION\tSpace\t" + option if context else "SEND_KEY\tSpace")
            space_ms = (time.monotonic() - started) * 1000
            waited = 0
            while callback_ms(text) is not None and waited < 40:
                time.sleep(callback_ms(text) / 1000)
                text = send("KOTORI_REFRESH")
                waited += 1
            top1 = preedit(text)
            rows.append(dict(index=it["index"], input=it["input"], context_text=it.get("context_text", ""),
                             top1=top1, expected=it["expected_output"], ok=top1 in it["expected_output"],
                             refreshes_after_space=waited, ms=round((time.monotonic() - started) * 1000, 1)))
            send("SEND_KEY\tESC")
            send("SEND_KEY\tESC")
        p.stdin.close()
        p.wait(timeout=60)
    Path(args.out).write_text(json.dumps(rows, ensure_ascii=False, indent=1), encoding="utf-8")
    scored = [r for r in rows if not r.get("skipped")]
    print(f"{args.data}: {sum(r['ok'] for r in scored)}/{len(scored)} (skipped {len(rows) - len(scored)})")


if __name__ == "__main__":
    main()
