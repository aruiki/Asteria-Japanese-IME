"""同じ読み・同じ前の文で、Space の押し方や学習の有無によらず同じ 1 番目が出るかを測る(カード 89、2026-11-03)。

実際の IME と同じセッションの経路(session_handler_main)で、人の速さで打ち(キーごとに間を空け、TIP と同じく
予約された取り直しを送る)、次の場合の 1 番目を比べる。
- すぐ: 打ち終えてすぐ Space
- 待つ: 打ち終えて --wait 秒待ってから Space(先回りの変換が終わった後)
- 2回目: 「すぐ」の後に取り消して、もう一度すぐ Space
- 学習あり: --profile のフォルダ(利用者の学習のコピー)を入れたセッションで、すぐ Space(指定したときだけ)
確定しないので学習しない(Esc で取り消す)。前の文はセットの context_text をすべてのキーに付けて渡す。

使い方: python mozc/tools/consistency_check.py <session_handler_main> --data セット.json --out 結果.json
        [--profile 学習のフォルダ] [--models モデルの場所] [--wait 1.5] [--gap-ms 30]
"""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "eval" / "imebench"))
from imebench import to_romaji  # noqa: E402

sys.path.insert(0, str(Path(__file__).resolve().parent))
import candidate_order  # noqa: E402
import eval_ai  # noqa: E402

import re  # noqa: E402


def preedit(text):
    if "preedit {" not in text:
        return ""
    body = text.split("preedit {", 1)[1]
    body = re.split(r"\n\w+ \{", body, maxsplit=1)[0]
    return "".join(re.findall(r'^\s+value: "([^"\n]*)"', body, re.M))


class Session(candidate_order.Session):
    def __init__(self, exe, models, profile=None):
        real = subprocess.Popen

        def popen(args, **kwargs):
            # 学習のコピーを、このセッションのプロファイルにする(元のフォルダには書かない)。
            if profile:
                d = tempfile.mkdtemp()
                shutil.copytree(profile, d + "/s")
                args = [a if not a.startswith("--profile=") else "--profile=" + d + "/s" for a in args]
            return real(args, **kwargs)

        candidate_order.subprocess.Popen = popen
        try:
            super().__init__(exe, models, [])
        finally:
            candidate_order.subprocess.Popen = real

    def key(self, key, option):
        return self.send("SEND_KEY_WITH_OPTION\t" + key + "\t" + option if option else "SEND_KEY\t" + key)

    def top(self, romaji, option, gap, wait):
        text = ""
        for ch in romaji:
            text = self.key(ch, option)
            until = time.monotonic() + gap
            while time.monotonic() < until:
                d = candidate_order.delay(text)
                if d is None or time.monotonic() + d / 1000 > until:
                    break
                time.sleep(d / 1000)
                text = self.send("KOTORI_REFRESH")
            time.sleep(max(0, until - time.monotonic()))
        if wait:
            end = time.monotonic() + wait
            while time.monotonic() < end:
                d = candidate_order.delay(text)
                time.sleep(min(d / 1000 if d is not None else 0.1, max(0, end - time.monotonic())))
                if d is not None:
                    text = self.send("KOTORI_REFRESH")
        shown = preedit(self.settle(self.key("Space", option), limit=10.0))
        for _ in range(3):
            self.send("SEND_KEY\tESC")
        return shown


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("exe", type=Path)
    ap.add_argument("--data", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--profile")
    ap.add_argument("--models", default=eval_ai.default_models())
    ap.add_argument("--wait", type=float, default=1.5)
    ap.add_argument("--gap-ms", type=float, default=30)
    ap.add_argument("--warm", type=float, default=15)
    args, extra = ap.parse_known_args()
    os.environ.update(dict(e.split("=", 1) for e in extra if "=" in e))
    items = json.load(open(args.data, encoding="utf-8"))
    exe = args.exe.resolve()
    sessions = {"すぐ": Session(exe, args.models), "待つ": Session(exe, args.models)}
    if args.profile:
        sessions["学習あり"] = Session(exe, args.models, args.profile)
    for s in sessions.values():
        s.send("SEND_KEYS\tai")
    time.sleep(args.warm)
    for s in sessions.values():
        s.send("SEND_KEY\tESC")
    gap = args.gap_ms / 1000
    rows = []
    for it in items:
        romaji = it.get("romaji") or to_romaji(it["input"])
        if romaji is None:
            continue
        context = it.get("context_text", "")
        option = ("context.preceding_text=\"" + context.replace("\\", "\\\\").replace('"', '\\"') + "\""
                  if context else "")
        tops = {"すぐ": sessions["すぐ"].top(romaji, option, gap, 0)}
        tops["2回目"] = sessions["すぐ"].top(romaji, option, gap, 0)
        tops["待つ"] = sessions["待つ"].top(romaji, option, gap, args.wait)
        if "学習あり" in sessions:
            tops["学習あり"] = sessions["学習あり"].top(romaji, option, gap, 0)
        expected = it.get("expected_output") or []
        rows.append(dict(index=it.get("index"), input=it["input"], context_text=context, expected=expected,
                         tops=tops, consistent=len(set(tops.values())) == 1,
                         ok={k: v in expected for k, v in tops.items()}))
    for s in sessions.values():
        s.close()
    json.dump(rows, open(args.out, "w", encoding="utf-8"), ensure_ascii=False, indent=1)
    n = len(rows)
    same = sum(r["consistent"] for r in rows)
    modes = list(rows[0]["tops"]) if rows else []
    oks = " ".join(f"{m} {sum(r['ok'][m] for r in rows)}" for m in modes)
    print(f"{args.data}: 問 {n} 一致 {same}/{n} ({100 * same / max(n, 1):.1f}%) 正解 {oks}")
    for r in rows:
        if not r["consistent"]:
            print("  不一致", r["index"], r["input"], r["tops"])


if __name__ == "__main__":
    main()
