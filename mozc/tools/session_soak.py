"""長時間の連続操作の試験(v3.2.0 の安定化)。実際の IME と同じセッション処理の経路で、打鍵・変換・候補の移動・
文節の移動・確定(学習が動く)・取り消し(確定直後の Backspace で学習を戻す)・Esc・F6/F7・とても長い読み・
空の Space・記号や絵文字を、決まった乱数で数千回まぜて送る。

見ること:
  - 落ちない(サーバーが終わったら、終了コードと異常終了の記録を残して止める)
  - 固まらない(1 回の応答の最大。AI の結果を待つ取り直しの合計も含めて 10 秒を上限とする)
  - 終わった後も変換が正しく動く(最後に決まった文を変換して、最初と同じ結果になるか)

使い方: python mozc/tools/session_soak.py <session_handler_main> --actions 3000 --out 結果.json [環境変数=値 ...]
"""
import argparse
import json
import os
from pathlib import Path
import queue
import random
import re
import subprocess
import sys
import tempfile
import threading
import time

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "eval" / "imebench"))
from imebench import to_romaji  # noqa: E402

P = Path(__file__).resolve().parents[2]
EXTRA = ["!", "?", "1", "23", "abc", "A", "-", ".", ",", "[", "]", "😀", "ｗ", "@", "~"]
PROBES = ["kyouhaiitennkidesune", "kaiginosiryouwookurimasu", "eigawomiru", "kanozyoniomoiwotutaeru"]


def preedit(text):
    if "preedit {" not in text:
        return ""
    body = re.split(r"\n\w+ \{", text.split("preedit {", 1)[1], maxsplit=1)[0]
    return "".join(re.findall(r'^\s+value: "([^"\n]*)"', body, re.M))


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("exe", type=Path)
    ap.add_argument("--actions", type=int, default=3000)
    ap.add_argument("--seed", type=int, default=20261027)
    ap.add_argument("--out", required=True)
    ap.add_argument("--models", default=str(P / "target/accuracy-guide/short-word-package-check-v7/payload/ext/PFiles/Kotori"))
    ap.add_argument("--warm", type=float, default=15)
    args, extra = ap.parse_known_args()
    rnd = random.Random(args.seed)
    words = []
    for name in ("kotori-daily.json", "asteria-short-dev-20261022.json", "kotori-homophone.json"):
        for it in json.load(open(P / "eval/sets" / name, encoding="utf-8")):
            r = to_romaji(it["input"])
            if r:
                words.append(r)
    env = dict(os.environ, KOTORI_MODEL_DIR=args.models, KOTORI_RUNTIME_DIR=args.models)
    env.update(dict(e.split("=", 1) for e in extra if "=" in e))
    lines = queue.Queue()
    log = []
    worst = []
    result = dict(actions=0, crashed=False)
    with tempfile.TemporaryDirectory(prefix="kotori-soak-", ignore_cleanup_errors=True) as profile:
        prof = Path(profile) / "session"
        p = subprocess.Popen([str(args.exe.resolve()), "--dictionary=oss", "--profile=" + str(prof), "--preserve_profile"],
                             cwd=str(args.exe.resolve()) + ".runfiles/_main", env=env, stdin=subprocess.PIPE,
                             stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
        threading.Thread(target=lambda: [lines.put(l.decode("utf-8", "replace").rstrip()) for l in p.stdout] +
                         [lines.put(None)], daemon=True).start()

        class Exited(Exception):
            pass

        def send(command, label):
            t0 = time.monotonic()
            p.stdin.write((command + "\nSHOW_OUTPUT\n").encode("utf-8"))
            p.stdin.flush()
            out, tail = [], False
            while True:
                try:
                    line = lines.get(timeout=60)
                except queue.Empty:
                    raise Exited(f"no response for 60s: {label}")
                if line is None:
                    raise Exited(f"exited during: {label}")
                out.append(line)
                if tail and line.strip() == "}":
                    break
                tail |= line.startswith("removed_candidate_words_for_debug {")
            ms = (time.monotonic() - t0) * 1000
            worst.append((ms, label))
            worst.sort(reverse=True)
            del worst[10:]
            return "\n".join(out)

        def key(k):
            return send("SEND_KEY\t" + k, k)

        def settle(text, label):
            # AI の結果を待つ取り直し(製品の TIP と同じ)。合計の待ちも測る。
            t0 = time.monotonic()
            while (m := re.search(r"delay_millisec: (\d+)", text)) and time.monotonic() - t0 < 10:
                time.sleep(int(m.group(1)) / 1000)
                text = send("KOTORI_REFRESH", label + " refresh")
            waited = (time.monotonic() - t0) * 1000
            worst.append((waited, label + " (取り直しの合計)"))
            worst.sort(reverse=True)
            del worst[10:]
            return text

        def convert(romaji):
            send("SEND_KEYS\t" + romaji, "type " + romaji[:20])
            return settle(key("Space"), "Space " + romaji[:20])

        try:
            send("SEND_KEY\tON\nSEND_KEYS\tai", "on")
            time.sleep(args.warm)
            key("ESC")
            before = []
            for r in PROBES:
                before.append(preedit(convert(r)))
                key("ESC")
                key("ESC")
            for i in range(args.actions):
                result["actions"] = i + 1
                a = rnd.random()
                w = rnd.choice(words)
                if a < 0.35:  # 変換して候補・文節を動かして確定(学習が動く)
                    convert(w)
                    for _ in range(rnd.randint(0, 3)):
                        key(rnd.choice(["Down", "Up", "Right", "Left", "Space", "Shift Right", "Shift Left"]))
                    key("Enter")
                    if rnd.random() < 0.3:
                        key("Backspace")  # 確定直後の取り消し(Revert)
                elif a < 0.55:  # 変換して取り消す
                    convert(w)
                    key("ESC")
                    key("ESC")
                elif a < 0.65:  # 打っている途中で消す・F6/F7
                    send("SEND_KEYS\t" + w[: rnd.randint(1, len(w))], "partial")
                    key(rnd.choice(["Backspace", "F6", "F7", "F8", "ESC"]))
                    key(rnd.choice(["Enter", "ESC"]))
                elif a < 0.72:  # Tab の予測
                    send("SEND_KEYS\t" + w[: max(1, len(w) // 2)], "partial for tab")
                    settle(key("Tab"), "Tab")
                    key(rnd.choice(["Enter", "ESC"]))
                    key("ESC")
                elif a < 0.78:  # とても長い読み
                    long = "".join(rnd.choice(words) for _ in range(rnd.randint(4, 12)))[:600]
                    convert(long)
                    key(rnd.choice(["Enter", "ESC"]))
                    key("ESC")
                elif a < 0.84:  # 記号・英数字・絵文字まじり
                    s = "".join(rnd.choice(EXTRA) if rnd.random() < 0.3 else c for c in w)
                    convert(s)
                    key(rnd.choice(["Enter", "ESC"]))
                    key("ESC")
                elif a < 0.90:  # 空の Space、続けての Space
                    key("Space")
                    convert(w)
                    for _ in range(rnd.randint(1, 6)):
                        key("Space")
                    key("ESC")
                    key("ESC")
                elif a < 0.95:  # すぐ Space(取り直しを待たない)
                    send("SEND_KEYS\t" + w, "type")
                    key("Space")
                    key("Enter")
                else:  # 打ちながらすぐ消す
                    for ch in w[:8]:
                        send("SEND_KEYS\t" + ch, "char")
                    for _ in range(rnd.randint(1, 10)):
                        key("Backspace")
                    key("ESC")
                if (i + 1) % 500 == 0:
                    print(f"{i + 1} actions, worst {worst[0][0]:.0f}ms ({worst[0][1]})", flush=True)
            after = []
            for r in PROBES:
                after.append(preedit(convert(r)))
                key("ESC")
                key("ESC")
            result.update(probes_before=before, probes_after=after)
        except Exited as e:
            result["crashed"] = True
            result["reason"] = str(e)
            code = p.poll()
            result["exit_code"] = None if code is None else f"{code & 0xffffffff:#x}"
            result["crash_logs"] = [f.read_text(encoding="utf-8", errors="replace")
                                    for f in sorted((prof / "crash").glob("crash-*.txt"))]
        finally:
            try:
                p.stdin.close()
                p.wait(timeout=60)
            except Exception:  # noqa: BLE001
                p.kill()
        learned = prof / "asteria_context.tsv"
        result["learned_entries"] = len(learned.read_text(encoding="utf-8").splitlines()) if learned.exists() else 0
    result["worst"] = [dict(ms=round(ms), at=label) for ms, label in worst]
    Path(args.out).write_text(json.dumps(result, ensure_ascii=False, indent=1), encoding="utf-8")
    print(json.dumps({k: result[k] for k in result if k != "crash_logs"}, ensure_ascii=False, indent=1))
    sys.exit(1 if result["crashed"] else 0)


if __name__ == "__main__":
    main()
