"""文脈つきの学習の流れを、実際のIMEと同じセッション処理の経路で確かめる(docs/adr/0075)。

session_handler_main を 1 プロセスで起動し、場面ごとに:
  1. 確かめる文(check)を変換して、学習の前の結果を記録する(Esc で取り消す。確定しないので学習しない)
  2. 教える文(teach)を変換し、退けたい表記(rejected)の文節へ移って、選びたい表記(chosen)の候補まで
     ↓で進めて Enter で確定する(利用者が選び直して確定するのと同じ)
  3. もう一度 check を変換して、学習の後の結果を比べる
check の種類:
  same_cue  同じ手がかりの語がある別の文。chosen になるべき
  no_cue    手がかりのない文。学習の前と同じであるべき(別の文脈に引きずられない)
  hiragana  teach がひらがなの確定。学習の前と同じであるべき(ひらがなの確定を覚えない)

使い方: python mozc/tools/learning_flow.py <session_handler_main> --data 場面.json --out 結果.json
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


def segments(text):
    """preedit の文節(値、選択中か)。"""
    if "preedit {" not in text:
        return []
    body = re.split(r"\n\w+ \{", text.split("preedit {", 1)[1], maxsplit=1)[0]
    out = []
    for block in re.findall(r"Segment \{(.*?)\n\s*\}", body, re.S):
        value = re.search(r'value: "([^"\n]*)"', block)
        out.append((value.group(1) if value else "", "HIGHLIGHT" in block))
    return out


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("exe", type=Path)
    ap.add_argument("--data", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--models", default=str(P / "target/accuracy-guide/short-word-package-check-v7/payload/ext/PFiles/Kotori"))
    ap.add_argument("--warm", type=float, default=15)
    ap.add_argument("--log", help="サーバーの標準エラーを書き出すファイル(KOTORI_LM_DEBUG=1 と合わせて調べる)")
    ap.add_argument("--restart", action="store_true",
                    help="教えた後にサーバーを終わらせて起動し直してから確かめる(学習が保存され、読み直されるか)")
    ap.add_argument("--clear", action="store_true",
                    help="確かめた後に学習履歴を消し(設定の「学習履歴のクリア」と同じ)、学習の前の結果に戻るか確かめる")
    args, extra = ap.parse_known_args()
    scenes = json.load(open(args.data, encoding="utf-8"))
    env = dict(os.environ, KOTORI_MODEL_DIR=args.models, KOTORI_RUNTIME_DIR=args.models)
    env.update(dict(e.split("=", 1) for e in extra if "=" in e))
    with tempfile.TemporaryDirectory(prefix="kotori-learning-flow-", ignore_cleanup_errors=True) as profile:
        log = open(args.log, "wb") if args.log else subprocess.DEVNULL
        state = {}

        def start():
            # --preserve_profile: 終わるときに学習を保存して残す(起動し直しの確認のため)。
            p = subprocess.Popen([str(args.exe.resolve()), "--dictionary=oss",
                                  "--profile=" + str(Path(profile) / "session"), "--preserve_profile"],
                                 cwd=str(args.exe.resolve()) + ".runfiles/_main", env=env, stdin=subprocess.PIPE,
                                 stdout=subprocess.PIPE, stderr=log)
            lines = queue.Queue()
            threading.Thread(target=lambda: [lines.put(l.decode("utf-8", "replace").rstrip()) for l in p.stdout] +
                             [lines.put(None)], daemon=True).start()
            state.update(p=p, lines=lines)
            send("SEND_KEY\tON\nSEND_KEYS\tai")
            time.sleep(args.warm)
            send("SEND_KEY\tESC")

        def stop():
            state["p"].stdin.close()
            state["p"].wait(timeout=60)

        def send(command):
            p, lines = state["p"], state["lines"]
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

        def settle(text, limit=10):
            started = time.monotonic()
            while time.monotonic() - started < limit:
                m = re.search(r"delay_millisec: (\d+)", text)
                if not m:
                    break
                time.sleep(int(m.group(1)) / 1000)
                text = send("KOTORI_REFRESH")
            return text

        def convert(item):
            context = item.get("context", "")
            option = "context.preceding_text=\"" + context.replace("\\", "\\\\").replace('"', '\\"') + "\""
            romaji = item.get("romaji") or to_romaji(item["input"])
            for ch in romaji:
                text = send("SEND_KEY_WITH_OPTION\t" + ch + "\t" + option)
            settle(text, 0.7)
            text = send("SEND_KEY_WITH_OPTION\tSpace\t" + option)
            return settle(text), option

        def top1(item):
            text, _ = convert(item)
            send("SEND_KEY\tESC")
            send("SEND_KEY\tESC")
            return "".join(v for v, _ in segments(text))

        def teach(item):
            text, option = convert(item)
            segs = segments(text)
            at = next((i for i, (v, _) in enumerate(segs) if item["rejected"] in v), None)
            if at is None:
                send("SEND_KEY\tESC")
                send("SEND_KEY\tESC")
                shown = "".join(v for v, _ in segs)
                # AI が初めから選ぶ表記なら、教える場面にならない(数えない)。
                return dict(taught=False, already=item["chosen"] in shown, reason="退ける表記の文節がない",
                            shown=shown)
            for _ in range(at):
                text = send("SEND_KEY_WITH_OPTION\tRight\t" + option)
            for _ in range(40):
                focused = next((v for v, h in segments(text) if h), "")
                if item["chosen"] in focused and item["rejected"] not in focused:
                    break
                text = send("SEND_KEY_WITH_OPTION\tDown\t" + option)
            else:
                send("SEND_KEY\tESC")
                send("SEND_KEY\tESC")
                return dict(taught=False, reason="選ぶ表記の候補がない")
            committed = "".join(v for v, _ in segments(text))
            send("SEND_KEY_WITH_OPTION\tEnter\t" + option)
            return dict(taught=True, committed=committed)

        start()
        rows = []
        for scene in scenes:
            before = [top1(c) for c in scene["check"]]
            taught = teach(scene["teach"])
            if args.restart and taught.get("taught"):
                stop()
                start()
            after = [top1(c) for c in scene["check"]]
            cleared = None
            if args.clear and taught.get("taught"):
                send("CLEAR_USER_PREDICTION")
                cleared = [top1(c) for c in scene["check"]]
            checks = []
            for n, (c, b, a) in enumerate(zip(scene["check"], before, after)):
                if c["kind"] == "same_cue":
                    ok = c["expect"] in a
                else:
                    ok = a == b
                if cleared is not None:
                    ok = ok and cleared[n] == b  # 消した後は学習の前と同じ
                checks.append(dict(kind=c["kind"], input=c.get("input", c.get("romaji")), before=b, after=a,
                                   cleared=None if cleared is None else cleared[n], ok=ok))
            if taught.get("already"):
                rows.append(dict(name=scene["name"], teach=taught, checks=checks, ok=None))
                print(f"-- {scene['name']}: AI が初めから {scene['teach']['chosen']} を選ぶ(数えない)")
                continue
            rows.append(dict(name=scene["name"], teach=taught, checks=checks,
                             ok=taught.get("taught", False) and all(x["ok"] for x in checks)))
            print(f"{'OK ' if rows[-1]['ok'] else 'NG '} {scene['name']}: " +
                  " / ".join(f"{x['kind']} {x['before']}→{x['after']}" +
                             (f"→消去後 {x['cleared']}" if x["cleared"] is not None else "") for x in checks) +
                  ("" if taught.get("taught") else f" (教えられない: {taught.get('reason')})"))
        stop()
    Path(args.out).write_text(json.dumps(rows, ensure_ascii=False, indent=1), encoding="utf-8")
    counted = [r for r in rows if r["ok"] is not None]
    print(f"{args.data}: {sum(r['ok'] for r in counted)}/{len(counted)}(数えない {len(rows) - len(counted)})")


if __name__ == "__main__":
    main()
