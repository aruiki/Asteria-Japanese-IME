"""1 打鍵ごとの処理時間を、実際の IME と同じセッション処理の経路で測る(2026-11-03、docs/adr/0081)。

session_handler_main を 1 プロセスで起動し、問ごとに読みのローマ字を 1 キーずつ送って、送ってから応答が返るまでの
時間を記録する(手が止まった後の取り直しは送らない)。Esc で取り消す(確定しないので学習しない)。

使い方: python mozc/tools/keystroke_latency.py <session_handler_main> --data セット.json --out 結果.json
        [--config 名前=値 ...]
"""
import argparse
import json
import os
from pathlib import Path
import queue
import subprocess
import sys
import tempfile
import threading
import time

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "eval" / "imebench"))
from imebench import to_romaji  # noqa: E402

sys.path.insert(0, str(Path(__file__).resolve().parent))
import eval_ai  # noqa: E402


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("exe", type=Path)
    ap.add_argument("--data", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--models", default=eval_ai.default_models())
    ap.add_argument("--config", action="append", default=[])
    ap.add_argument("--warm", type=float, default=15)
    ap.add_argument("--gap", type=float, default=60, help="キーの間隔(ms)")
    args = ap.parse_args()
    eval_ai.check_models(args.models)
    items = json.load(open(args.data, encoding="utf-8"))
    env = dict(os.environ, KOTORI_MODEL_DIR=args.models, KOTORI_RUNTIME_DIR=args.models)
    exe = args.exe.resolve()
    times = []
    with tempfile.TemporaryDirectory(prefix="kotori-keystroke-", ignore_cleanup_errors=True) as profile:
        p = subprocess.Popen([str(exe), "--dictionary=oss", "--profile=" + str(Path(profile) / "session")],
                             cwd=str(exe) + ".runfiles/_main", env=env, stdin=subprocess.PIPE,
                             stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
        lines = queue.Queue()
        threading.Thread(target=lambda: [lines.put(l.decode("utf-8", "replace").rstrip()) for l in p.stdout] +
                         [lines.put(None)], daemon=True).start()

        def send(command):
            p.stdin.write((command + "\nSHOW_OUTPUT\n").encode("utf-8"))
            p.stdin.flush()
            tail = False
            while True:
                line = lines.get(timeout=60)
                if line is None:
                    raise RuntimeError("session handler exited")
                if tail and line.strip() == "}":
                    return
                tail |= line.startswith("removed_candidate_words_for_debug {")

        for item in args.config:
            name, value = item.split("=", 1)
            send("SET_CONFIG\t" + name + "\t" + value)
        send("SEND_KEY\tON\nSEND_KEYS\tai")
        time.sleep(args.warm)
        send("SEND_KEY\tESC")
        for it in items:
            romaji = it.get("romaji") or to_romaji(it["input"])
            if romaji is None:
                continue
            for ch in romaji:
                started = time.perf_counter()
                send("SEND_KEY\t" + ch)
                times.append((time.perf_counter() - started) * 1000)
                time.sleep(args.gap / 1000)
            send("SEND_KEY\tESC")
            send("SEND_KEY\tESC")
        p.stdin.close()
        p.wait(timeout=60)
    times.sort()
    pick = lambda q: round(times[min(len(times) - 1, int(len(times) * q))], 2)
    summary = dict(keys=len(times), median_ms=pick(0.5), p95_ms=pick(0.95), p99_ms=pick(0.99),
                   max_ms=round(times[-1], 2), config=args.config)
    Path(args.out).write_text(json.dumps(summary, ensure_ascii=False, indent=1), encoding="utf-8")
    print(json.dumps(summary, ensure_ascii=False))


if __name__ == "__main__":
    main()
