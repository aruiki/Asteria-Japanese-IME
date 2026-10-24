"""実セッションの初回候補と遅延更新を記録する。実行中の IME には接続しない。

変更後の session_handler_main が必要。SHOW_OUTPUT の末尾を待って応答時間を測る。
実際の TSF / アプリ上の遅延とは区別する。モデルはインストール先から読む。
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import queue
import re
import subprocess
import tempfile
import threading
import time


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("exe", type=Path)
    ap.add_argument("--install", default=(r"C:\Program Files (x86)\Asteria" if os.path.isdir(r"C:\Program Files (x86)\Asteria") else r"C:\Program Files (x86)\Kotori"))
    ap.add_argument("--romaji", default="ashiwoitametanode")
    ap.add_argument("--warm", type=float, default=12)
    ap.add_argument("--key-interval", type=float, default=0, help="1文字ずつ送る間隔（秒）")
    ap.add_argument("--expect-variant", help="初回表示・Tab・Spaceに必要な候補")
    ap.add_argument("--expect-second-space", help="2回目Spaceで変更語へフォーカスする表記")
    ap.add_argument("--expect-reading", help="最終打鍵時の読み。ローマ字の送り間違いも拒否する")
    ap.add_argument("--expect-first", help="最初に表示する候補とSpace後の文全体に必要な表記")
    ap.add_argument("--check-lifecycle", action="store_true",
                    help="取消・AI無効・保護欄・シークレット切替後の補正を拒否する")
    ap.add_argument("--out", type=Path, required=True)
    args = ap.parse_args()
    if args.check_lifecycle and not args.expect_first:
        ap.error("--check-lifecycleには--expect-firstが必要です")
    exe = args.exe.resolve()
    exe_sha256 = hashlib.sha256(exe.read_bytes()).hexdigest()
    session_started = time.monotonic()
    lines = queue.Queue()
    env = dict(os.environ, KOTORI_MODEL_DIR=args.install,
               KOTORI_RUNTIME_DIR=args.install)
    with tempfile.TemporaryDirectory(prefix="kotori-first-") as profile:
        assert Path(profile).resolve().is_relative_to(Path(tempfile.gettempdir()).resolve())
        with args.out.with_suffix(".stderr.txt").open("wb") as err:
            p = subprocess.Popen(
                [str(exe), "--dictionary=oss", "--profile=" + str(Path(profile) / "session")],
                cwd=str(exe) + ".runfiles/_main", env=env,
                stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=err)

            def read():
                for line in p.stdout:
                    lines.put(line.decode("utf-8").rstrip())
                lines.put(None)

            reader = threading.Thread(target=read, daemon=True)
            reader.start()

            def send(command):
                started = time.monotonic()
                p.stdin.write((command + "\nSHOW_OUTPUT\n").encode("utf-8"))
                p.stdin.flush()
                output, tail = [], False
                deadline = started + 60
                while True:
                    line = lines.get(timeout=max(.01, deadline - time.monotonic()))
                    if line is None:
                        raise RuntimeError("session handler exited unexpectedly: " + "\n".join(output))
                    output.append(line)
                    if tail and line.strip() == "}":
                        break
                    tail |= line.startswith("removed_candidate_words_for_debug {")
                text = "\n".join(output)
                match = re.search(r'candidate_window \{.*?value: "([^"]*)"',
                                  text, re.S)
                return {"command": command,
                        "response_ms": (time.monotonic() - started) * 1000,
                        "observed_at_ms": (time.monotonic() - session_started) * 1000,
                        "first_candidate": match.group(1) if match else None,
                        "output": text}

            try:
                send("SEND_KEY\tON\nSEND_KEYS\tai")
                time.sleep(args.warm)
                send("SEND_KEY\tESC")
                if args.key_interval > 0:
                    for char in args.romaji:
                        last = send("SEND_KEYS\t" + char)
                        time.sleep(args.key_interval)
                    rows = [last]
                else:
                    rows = [send("SEND_KEYS\t" + args.romaji)]
                for _ in range(5):
                    callback = re.search(r"delay_millisec: (\d+)", rows[-1]["output"])
                    if not callback:
                        break
                    time.sleep(int(callback.group(1)) / 1000)
                    rows.append(send("KOTORI_REFRESH"))
                if args.expect_variant:
                    rows.append(send("SEND_KEY\tTab"))
                    rows.append(send("SEND_KEY\tESC"))
                lifecycle = []
                if args.check_lifecycle:
                    # TEST_SEND_KEY uses an independent clone/Undo context.
                    lifecycle.append({"phase": "test-key-clone",
                                      **send("TEST_SEND_KEY\tSpace")})
                    clone_refresh = send("KOTORI_REFRESH")
                    lifecycle.append({"phase": "after-test-key-clone", **clone_refresh})
                    if clone_refresh["first_candidate"] != args.expect_first:
                        raise AssertionError("TestSendKeyのコピーが元の補正を失効しました")
                rows.append(send("SEND_KEY\tSpace"))
                if args.expect_second_space:
                    rows.append(send("SEND_KEY\tSpace"))
                if args.check_lifecycle:
                    def clear():
                        send("SEND_KEY\tESC")
                        send("SEND_KEY\tESC")

                    def pending():
                        send("SEND_KEYS\t" + args.romaji)
                        send("KOTORI_REFRESH")

                    def reject(phase):
                        # Let an already queued worker finish after the boundary.
                        time.sleep(1)
                        event = send("KOTORI_REFRESH")
                        lifecycle.append({"phase": phase, **event})
                        if 'consumed: true' in event["output"] or event["first_candidate"]:
                            raise AssertionError(phase + "の後に古い補正が表示されました")

                    clear()
                    pending()
                    clear()
                    reject("after-cancel")
                    pending()
                    send("SET_CONFIG\tkotori_lm_enabled\tfalse")
                    reject("after-ai-disabled")
                    clear()
                    send("SET_CONFIG\tkotori_lm_enabled\ttrue")
                    pending()
                    send("SEND_KEY_WITH_OPTION\tESC\ttype=SEND_COMMAND"
                         "\tcommand.type=SWITCH_INPUT_FIELD_TYPE"
                         "\tcontext.input_field_type=PASSWORD")
                    reject("after-password-field")
                    send("SEND_KEY_WITH_OPTION\tESC\ttype=SEND_COMMAND"
                         "\tcommand.type=SWITCH_INPUT_FIELD_TYPE"
                         "\tcontext.input_field_type=NORMAL")
                    clear()
                    pending()
                    send("SET_REQUEST\tis_incognito_mode\ttrue")
                    reject("after-incognito")
                p.stdin.close()
                p.wait(timeout=60)
                if p.returncode:
                    raise RuntimeError(f"session handler exit {p.returncode}")
                if hashlib.sha256(exe.read_bytes()).hexdigest() != exe_sha256:
                    raise RuntimeError("試験中に実行ファイルが変更されました")
                last_key_ms = rows[0]["observed_at_ms"] - rows[0]["response_ms"]
                for row in rows:
                    row["since_last_key_ms"] = row["observed_at_ms"] - last_key_ms
                args.out.write_text(json.dumps({"exe": str(exe),
                    "exe_sha256": exe_sha256,
                    "romaji": args.romaji, "warm_seconds": args.warm,
                    "events": rows, "lifecycle": lifecycle},
                    ensure_ascii=False, indent=2), encoding="utf-8")
                for row in rows:
                    print(f'{row["response_ms"]:.1f} ms: {row["first_candidate"]}')
                for event in lifecycle:
                    print(f'{event["phase"]}: {event["first_candidate"]}')
                def preedit_fields(row, field):
                    text = row["output"]
                    if "preedit {" not in text:
                        return ""
                    body = re.split(r"\n\w+ \{", text.split("preedit {", 1)[1], maxsplit=1)[0]
                    return "".join(re.findall(r'^\s+' + field + r': "([^"\n]*)"', body, re.M))

                if args.expect_reading:
                    actual = preedit_fields(rows[0], "key")
                    if actual != args.expect_reading:
                        raise AssertionError(f"読みが異なります: {actual!r}")
                if args.expect_first:
                    first = next((r for r in rows if r["first_candidate"]), None)
                    if first is None or first["first_candidate"] != args.expect_first:
                        raise AssertionError("初回候補が期待した補正文ではありません")
                    if preedit_fields(rows[-1], "value") != args.expect_first:
                        raise AssertionError("Space後の文が初回候補と一致しません")
                if args.expect_second_space:
                    output = rows[-1]["output"]
                    preedit = output.split("preedit {", 1)[1].split("\n}", 1)[0]
                    highlighted = re.findall(r'Segment \{\s*annotation: HIGHLIGHT\s*value: "([^"\n]*)"', preedit, re.I)
                    if not any(args.expect_second_space in text for text in highlighted):
                        raise AssertionError("2回目Spaceで変更語へフォーカスしていません: " + repr(highlighted))
                    before = rows[-2]["output"].split("preedit {", 1)[1].split("\n}", 1)[0]
                    before_values = re.findall(r'value: "([^"\n]*)"', before)
                    after_values = re.findall(r'value: "([^"\n]*)"', preedit)
                    if len(before_values) != len(after_values) or sum(a != b for a, b in zip(before_values, after_values)) != 1:
                        raise AssertionError("2回目Spaceで変更語以外も変わりました")
                if args.expect_variant:
                    needle = 'value: "' + args.expect_variant + '"'
                    first = next((r for r in rows if r["first_candidate"]), None)
                    tab = next(r for r in rows if r["command"] == "SEND_KEY\tTab")
                    for label, row in (("初回", first), ("Tab", tab), ("Space", rows[-1])):
                        if row is None or needle not in row["output"]:
                            raise AssertionError(f"{label}に{args.expect_variant}がありません")
            finally:
                if p.poll() is None:
                    p.kill()
                    p.wait()
                reader.join(timeout=2)
                p.stdout.close()


if __name__ == "__main__":
    main()
