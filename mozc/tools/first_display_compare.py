"""同一の隔離Mozcセッションで初回候補・Tab・Spaceを比較する。

ビルド完了後に単独で実行する。インストール済みIMEには接続しない。
同一EXEで自由生成補正だけをOFF/ONにする。実アプリのTSF検証ではない。
入力の読みが一致しない場合・応答欠落・実行ファイル変更は失敗にする。
本番と同じ文節学習を有効にする。利用者の履歴を読む・変更する道具ではない。
"""
import argparse
import collections
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


def sha256(path):
    with Path(path).open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def romanize(reading, table_path):
    # Mozcの実際の表を逆引きする。促音は持越しのないxtu、撥音はn'に固定。
    inverse = {}
    for line in table_path.read_text(encoding="utf-8").splitlines():
        parts = line.split("\t")
        if len(parts) < 2 or (len(parts) > 2 and parts[2]):
            continue
        key, value = parts[:2]
        if not value or not re.fullmatch("[a-z'-]+", key):
            continue
        if value not in inverse or (len(key), key) < (len(inverse[value]), inverse[value]):
            inverse[value] = key
    inverse.update({"ん": "n'", "っ": "xtu", "ー": "-", "？": "?"})
    surfaces = sorted(inverse, key=lambda x: (-len(x), x))
    out, at = [], 0
    while at < len(reading):
        value = next((s for s in surfaces if reading.startswith(s, at)), None)
        if value is None:
            raise ValueError(f"対応しない読み: {reading[at:]!r}")
        out.append(inverse[value])
        at += len(value)
    return "".join(out)


def preedit(output, field):
    if "preedit {" not in output:
        return ""
    body = re.split(r"\n\w+ \{", output.split("preedit {", 1)[1], maxsplit=1)[0]
    return "".join(re.findall(r'^\s+' + field + r': "([^"\n]*)"', body, re.M))


class Session:
    def __init__(self, exe, env, profile, stderr_path, preserve_profile=False):
        self.lines = queue.Queue()
        self.err = stderr_path.open("wb")
        command = [str(exe), "--dictionary=oss", "--profile=" + str(profile / "session"),
                   "--use_history_rewriter=true"]
        if preserve_profile:
            command.append("--preserve_profile=true")
        self.proc = subprocess.Popen(
            command,
            cwd=str(exe) + ".runfiles/_main", env=env,
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=self.err)

        def read():
            for line in self.proc.stdout:
                try:
                    self.lines.put(line.decode("utf-8").rstrip())
                except UnicodeDecodeError:
                    self.lines.put("ERROR: non-UTF8 session stdout: " + repr(line[:200]))
                    self.lines.put(None)
                    return
            self.lines.put(None)
        self.reader = threading.Thread(target=read, daemon=True)
        self.reader.start()

    def send(self, command):
        start = time.monotonic()
        self.proc.stdin.write((command + "\nSHOW_OUTPUT\n").encode("utf-8"))
        self.proc.stdin.flush()
        output, tail = [], False
        deadline = start + 60
        while True:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise RuntimeError("セッション応答が60秒を超過: " + "\n".join(output)[-2000:])
            try:
                line = self.lines.get(timeout=remaining)
            except queue.Empty as exc:
                raise RuntimeError("セッション応答が60秒を超過: " + "\n".join(output)[-2000:]) from exc
            if line is None:
                raise RuntimeError("セッションが途中で終了: " + "\n".join(output))
            output.append(line)
            if tail and line.strip() == "}":
                break
            tail |= line.startswith("removed_candidate_words_for_debug {")
        text = "\n".join(output)
        if re.search(r"^ERROR:", text, re.M):
            raise RuntimeError("セッションがコマンドを拒否: " + text)
        match = re.search(r'candidate_window \{.*?value: "([^"\n]*)"', text, re.S)
        return {"command": command, "response_ms": (time.monotonic() - start) * 1000,
                "first_candidate": match.group(1) if match else None,
                "preedit_key": preedit(text, "key"), "preedit_value": preedit(text, "value"),
                "output": text}

    def close(self, failed=False):
        try:
            if failed:
                self.proc.kill()
            else:
                self.proc.stdin.close()
            self.proc.wait(timeout=60)
            if not failed and self.proc.returncode:
                raise RuntimeError(f"セッション終了コード: {self.proc.returncode}")
        finally:
            if self.proc.poll() is None:
                self.proc.kill()
                self.proc.wait()
            self.reader.join(timeout=2)
            self.proc.stdout.close()
            if not self.proc.stdin.closed:
                self.proc.stdin.close()
            self.err.close()


def evaluate(args, items, enabled):
    mode = "on" if enabled else "off"
    # 候補数/ビーム/生成時間を試験用に減らさず、通常のStandard設定を使う。
    # 利用者環境に残った実験用変数は除き、比較の設定を固定する。
    env = {k: v for k, v in os.environ.items() if not k.startswith("KOTORI_")}
    env.update(KOTORI_MODEL_DIR=str(args.install), KOTORI_RUNTIME_DIR=str(args.install),
               KOTORI_LM_CORRECTION="1" if enabled else "0", KOTORI_LM_DEBUG="1")
    if args.cpu:
        env["KOTORI_LM_DEVICE"] = "cpu"
    rows = []
    with tempfile.TemporaryDirectory(prefix="kotori-compare-" + mode + "-") as profile:
        session = Session(args.exe, env, Path(profile), args.out.with_suffix("." + mode + ".stderr.txt"))
        failed = True
        try:
            session.send("SEND_KEY\tON\nSEND_KEYS\tai")
            time.sleep(args.warm)
            session.send("SEND_KEY\tESC\nRESET_CONTEXT")
            status_path = Path(profile) / "session" / "kotori_ai_status.txt"
            initial_status = status_path.read_text(encoding="utf-8") if status_path.exists() else None
            for index, item in enumerate(items):
                session.send("SEND_KEY\tESC\nSEND_KEY\tESC\nRESET_CONTEXT")
                romaji = romanize(item["reading"], args.romanji_table)
                key_ms = []
                for char in romaji:
                    last = session.send("SEND_KEYS\t" + char)
                    key_ms.append(last["response_ms"])
                    if args.key_interval:
                        time.sleep(args.key_interval)
                if last["preedit_key"] != item["reading"]:
                    raise AssertionError(f"{index}: 読みが異なる: {last['preedit_key']!r}, {item['reading']!r}")
                events = [last]
                deadline = time.monotonic() + args.settle
                while time.monotonic() < deadline:
                    callback = re.search(r"delay_millisec: (\d+)", events[-1]["output"])
                    if not callback:
                        break
                    delay = int(callback.group(1)) / 1000
                    remaining = deadline - time.monotonic()
                    if delay > remaining:
                        time.sleep(max(0, remaining))
                        events.append(session.send("KOTORI_REFRESH"))
                        break
                    time.sleep(delay)
                    events.append(session.send("KOTORI_REFRESH"))
                settled = events[-1]
                tab = session.send("SEND_KEY\tTab")
                session.send("SEND_KEY\tESC")
                space = session.send("SEND_KEY\tSpace")
                rows.append(dict(item, romaji=romaji, key_max_ms=max(key_ms),
                                 immediate=last, settled=settled, updates=events[1:],
                                 tab=tab, space=space))
                if (index + 1) % 10 == 0 or index + 1 == len(items):
                    print(f"{mode}: {index + 1}/{len(items)}", flush=True)
            final_status = status_path.read_text(encoding="utf-8") if status_path.exists() else None
            args.out.with_suffix("." + mode + ".status.json").write_text(
                json.dumps({"initial": initial_status, "final": final_status,
                            "environment": {k: v for k, v in env.items() if k.startswith("KOTORI_")}},
                           ensure_ascii=False, indent=2), encoding="utf-8")
            failed = False
        finally:
            session.close(failed)
    args.out.with_suffix("." + mode + ".json").write_text(
        json.dumps(rows, ensure_ascii=False, indent=2), encoding="utf-8")
    return rows


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("exe", type=Path)
    ap.add_argument("--data", type=Path, required=True)
    ap.add_argument("--romanji-table", type=Path, required=True)
    ap.add_argument("--install", type=Path, default=Path(r"C:\Program Files (x86)\Kotori"))
    ap.add_argument("--warm", type=float, default=12)
    ap.add_argument("--settle", type=float, default=2.4)
    ap.add_argument("--key-interval", type=float, default=.01)
    ap.add_argument("--limit", type=int, help="道具の動作確認用。100件の試験とは区別する")
    ap.add_argument("--cpu", action="store_true")
    ap.add_argument("--out", type=Path, required=True)
    args = ap.parse_args()
    args.exe = args.exe.resolve()
    args.romanji_table = args.romanji_table.resolve()
    args.out.parent.mkdir(parents=True, exist_ok=True)
    items = json.loads(args.data.read_text(encoding="utf-8"))
    if args.limit is not None:
        items = items[:args.limit]
    if not items:
        ap.error("比較対象が空です")
    for item in items:
        romanize(item["reading"], args.romanji_table)
    exe_hash = sha256(args.exe)
    started = time.monotonic()
    off = evaluate(args, items, False)
    on = evaluate(args, items, True)
    if sha256(args.exe) != exe_hash:
        raise RuntimeError("比較中にEXEが変更されました")
    changes = []
    for i, (a, b) in enumerate(zip(off, on)):
        changed = [field for field in ("immediate", "settled", "tab", "space")
                   if a[field]["first_candidate"] != b[field]["first_candidate"] or
                   (field == "space" and a[field]["preedit_value"] != b[field]["preedit_value"])]
        if changed:
            changes.append({"index": i, "category": a["category"], "reading": a["reading"],
                            "expected": a["expected"], "fields": changed,
                            "off": {k: a[k]["first_candidate"] for k in changed},
                            "on": {k: b[k]["first_candidate"] for k in changed},
                            "space_off": a["space"]["preedit_value"],
                            "space_on": b["space"]["preedit_value"]})
    summary = {"scope": "CLI session with real dictionary/models; not installed Windows TSF",
               "exe": str(args.exe), "exe_sha256": exe_hash,
               "data_sha256": sha256(args.data), "romanji_table_sha256": sha256(args.romanji_table),
               "available_model_sha256": {p.name: sha256(p) for p in args.install.glob("*.gguf")},
               "model_status": {mode: json.loads(args.out.with_suffix("." + mode + ".status.json").read_text(encoding="utf-8"))
                                for mode in ["off", "on"]},
               "adoption_logs": {mode: args.out.with_suffix("." + mode + ".stderr.txt").read_bytes().count(b"correction=adopted")
                                  for mode in ["off", "on"]},
               "items": len(items), "categories": dict(collections.Counter(i["category"] for i in items)),
               "history_rewriter": True, "cpu_forced": args.cpu, "warm_seconds": args.warm, "settle_seconds": args.settle,
               "key_interval_seconds": args.key_interval, "elapsed_seconds": time.monotonic() - started,
               "changed_items": len(changes), "changes": changes,
               "modes": {mode: {"key_max_ms": max(r["key_max_ms"] for r in data),
                                   "space_max_ms": max(r["space"]["response_ms"] for r in data),
                                   "space_expected": sum(r["space"]["preedit_value"] in r["expected"] for r in data),
                                   "full_first_expected": sum(r["settled"]["first_candidate"] in r["expected"] for r in data)}
                         for mode, data in [("off", off), ("on", on)]}}
    args.out.write_text(json.dumps(summary, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps({k: summary[k] for k in ["items", "changed_items", "modes", "elapsed_seconds"]}, ensure_ascii=False), flush=True)


if __name__ == "__main__":
    main()
