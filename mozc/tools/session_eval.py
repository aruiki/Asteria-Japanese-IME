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
sys.path.insert(0, str(Path(__file__).resolve().parent))
import eval_ai  # noqa: E402


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("exe", type=Path)
    ap.add_argument("--data", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--context", action="store_true")
    ap.add_argument("--models", default=eval_ai.default_models())
    ap.add_argument("--ai-state", default="llm",
                    help="起動後に待つ AI の状態(kotori_ai_status の state、カンマ区切り。既定 llm。"
                         "AI を切る評価は off、CPU は zenz,llm。any で確かめない)")
    ap.add_argument("--warm", type=float, default=15)
    ap.add_argument("--n", type=int, default=0)
    ap.add_argument("--config", action="append", default=[],
                    help="セッション開始時に設定する項目 名前=値(例: kotori_lm_quality=KOTORI_LM_UNREAL)")
    ap.add_argument("--immediate-space", action="store_true",
                    help="打ち終わってすぐSpaceを押す(取り直しの予約を待たない。速く打つ人やimebenchと同じ)")
    ap.add_argument("--space-gap", type=float,
                    help="最後の打鍵からSpaceまでの時間(ms)。人が打つ速さの再現(例: 150)")
    ap.add_argument("--timeline", action="store_true",
                    help="KOTORI_LM_TIMELINEでサーバーの各段の時刻を取り、Space→候補の内訳を出す")
    args, extra = ap.parse_known_args()
    eval_ai.check_models(args.models)
    items = json.load(open(args.data, encoding="utf-8"))
    if args.n:
        items = items[: args.n]
    env = dict(os.environ, KOTORI_MODEL_DIR=args.models, KOTORI_RUNTIME_DIR=args.models)
    env.update(dict(e.split("=", 1) for e in extra if "=" in e))
    if args.timeline:
        env["KOTORI_LM_TIMELINE"] = "1"
    exe = args.exe.resolve()
    lines = queue.Queue()
    rows = []
    with tempfile.TemporaryDirectory(prefix="kotori-session-eval-", ignore_cleanup_errors=True) as profile:
        stderr_path = Path(profile) / "stderr.txt"
        stderr = open(stderr_path, "wb") if args.timeline else subprocess.DEVNULL
        p = subprocess.Popen([str(exe), "--dictionary=oss", "--profile=" + str(Path(profile) / "session")],
                             cwd=str(exe) + ".runfiles/_main", env=env, stdin=subprocess.PIPE,
                             stdout=subprocess.PIPE, stderr=stderr)
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
                    # 落ちた原因を残す(終了コード、異常終了の記録、標準エラーの末尾)。一時プロファイルは消えるため。
                    code = p.wait(timeout=30)
                    report = [f"exit code {code} ({code & 0xffffffff:#x}) at item {len(rows)}"]
                    for f in sorted((Path(profile) / "session" / "crash").glob("crash-*.txt")):
                        report.append(f"--- {f.name}\n" + f.read_text(encoding="utf-8", errors="replace"))
                    if args.timeline:
                        stderr.flush()
                        report.append("--- stderr tail\n" + "\n".join(
                            stderr_path.read_text(encoding="utf-8", errors="replace").splitlines()[-60:]))
                    Path(args.out + ".crash.txt").write_text("\n".join(report), encoding="utf-8")
                    raise RuntimeError("session handler exited: " + report[0] + "\n" + "\n".join(out[-20:]))
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

        for item in args.config:
            # 設定画面の項目をこのセッションだけ変える(品質の比較など)。モデルの読み込みより前に送る。
            name, value = item.split("=", 1)
            send("SET_CONFIG\t" + name + "\t" + value)
        send("SEND_KEY\tON\nSEND_KEYS\tai")
        time.sleep(args.warm)
        send("SEND_KEY\tESC")
        if args.ai_state != "any":
            state = eval_ai.wait_ai(Path(profile) / "session", lambda: (send("SEND_KEYS\tai"), send("SEND_KEY\tESC")),
                                    allow=tuple(args.ai_state.split(",")))
            print(f"AI: {state}", file=sys.stderr)
        for it in items:
            # 打ち間違いの問は、打ったキーをそのまま送る(かなから組み立て直すと間違いが消えるため)。
            romaji = it.get("romaji") or to_romaji(it["input"])
            if romaji is None:
                rows.append(dict(index=it["index"], skipped=True))
                continue
            started = time.monotonic()
            start_us = time.perf_counter_ns() // 1000
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
            # 手を止める時間は回数ではなく時間で決める(取り直しの間隔を変えても同じ間だけ止まる)。
            # 0.7秒は6446までの「最初の取り直し200ms + 100ms×5回」と同じ。製品のTIPと同じく、
            # 取り直しのタイマーがSpaceより前に来るときだけ取り直す。
            gap = 0 if args.immediate_space else (args.space_gap / 1000 if args.space_gap is not None else 0.7)
            typed = time.monotonic()
            while True:
                delay = callback_ms(text)
                if delay is None or time.monotonic() - typed + delay / 1000 > gap:
                    break
                time.sleep(delay / 1000)
                text = send("KOTORI_REFRESH")
            time.sleep(max(0, gap - (time.monotonic() - typed)) if args.space_gap is not None else 0)
            # Spaceから候補が表示されるまで。製品のTIPと同じく、サーバーが返す間隔で取り直しを送る
            # ので、AIの結果を待つ時間も含めて実際のIMEと同じ粒度で測れる。
            space_sent = time.monotonic()
            space_us = time.perf_counter_ns() // 1000  # サーバーの steady_clock と同じ QPC の時刻
            text = send("SEND_KEY_WITH_OPTION\tSpace\t" + option if context else "SEND_KEY\tSpace")
            space_ms = (time.monotonic() - started) * 1000
            waited = 0
            # 製品のTIPは回数で打ち切らない。取り直しの間隔が短くても同じ時間だけ待つ(上限10秒)。
            while callback_ms(text) is not None and time.monotonic() - space_sent < 10:
                time.sleep(callback_ms(text) / 1000)
                text = send("KOTORI_REFRESH")
                waited += 1
            space_to_candidate_ms = (time.monotonic() - space_sent) * 1000
            shown_us = time.perf_counter_ns() // 1000
            top1 = preedit(text)
            rows.append(dict(index=it["index"], input=it["input"], context_text=it.get("context_text", ""),
                             top1=top1, expected=it["expected_output"], ok=top1 in it["expected_output"],
                             refreshes_after_space=waited, space_to_candidate_ms=round(space_to_candidate_ms, 1),
                             ms=round((time.monotonic() - started) * 1000, 1), space_us=space_us,
                             shown_us=shown_us, start_us=start_us))
            send("SEND_KEY\tESC")
            send("SEND_KEY\tESC")
        p.stdin.close()
        p.wait(timeout=60)
        if args.timeline:
            stderr.close()
            timeline = stderr_path.read_text(encoding="utf-8", errors="replace")
            Path(args.out + ".timeline.txt").write_text(timeline, encoding="utf-8")
            attach_timeline(rows, timeline)
    Path(args.out).write_text(json.dumps(rows, ensure_ascii=False, indent=1), encoding="utf-8")
    scored = [r for r in rows if not r.get("skipped")]
    times = sorted(r["space_to_candidate_ms"] for r in scored)
    p95 = times[max(0, -(-len(times) * 95 // 100) - 1)] if times else 0
    print(f"{args.data}: {sum(r['ok'] for r in scored)}/{len(scored)} (skipped {len(rows) - len(scored)}) "
          f"Space→候補 中央値 {times[len(times) // 2] if times else 0:.0f}ms p95 {p95:.0f}ms 最大 {times[-1] if times else 0:.0f}ms")
    if args.timeline:
        print_breakdown(scored)


PHASES = ("before_space", "queue", "lock", "gen", "zenz", "llm", "corr", "finish", "pickup")


def attach_timeline(rows, text):
    """各問の最後のAIの計算(表示の前に終わったもの)を、Spaceからの内訳にする。単位はms。

    before_space: Spaceの前に済んだ計算の時間(Spaceの時点で結果があれば待ちは pickup だけ)
    queue: 仕事を積んでから計算を始める(ロックを取る)まで  gen: 変換用AIの生成
    zenz: 変換用AIの採点  llm: LLMの採点  corr: 補正の生成(Spaceの後は省く)  finish: 文節分け・保存
    pickup: 計算が終わってから候補が表示されるまで(取り直しの間隔とプロセス間の通信)
    """
    entries = []
    for line in text.splitlines():
        if line.startswith("[timeline] "):
            entries.append({k: int(v) for k, v in (f.split("=", 1) for f in line.split()[1:])})
    for r in rows:
        if r.get("skipped"):
            continue
        mine = [e for e in entries if r["start_us"] <= e["end"] <= r["shown_us"]]
        if not mine:
            r["breakdown"] = None
            continue
        e = mine[-1]
        space = r["space_us"]
        begin = e["queued"] or e["request"]
        clip = lambda t: max(t, space)  # Spaceより前の部分は before_space に入れる
        r["breakdown"] = dict(
            before_space=round((min(e["end"], space) - min(begin, space)) / 1000, 1),
            queue=round((clip(e["lock"]) - clip(begin)) / 1000, 1),
            lock=round((clip(e["load"]) - clip(e["lock"])) / 1000, 1),
            gen=round((clip(e["gen"]) - clip(e["load"])) / 1000, 1),
            zenz=round((clip(e["zenz"]) - clip(e["gen"])) / 1000, 1),
            llm=round((clip(e["llm"]) - clip(e["zenz"])) / 1000, 1),
            corr=round((clip(e.get("corr_end", e["llm"])) - clip(e.get("corr", e["llm"]))) / 1000, 1),
            finish=round((clip(e["end"]) - clip(e["llm"]) - (clip(e.get("corr_end", e["llm"])) -
                          clip(e.get("corr", e["llm"])))) / 1000, 1),
            pickup=round((r["shown_us"] - clip(e["end"])) / 1000, 1),
            chars=e["chars"], cands=e["cands"])


def print_breakdown(rows):
    with_b = [r for r in rows if r.get("breakdown")]
    print(f"内訳(AIの計算を突き合わせられた {len(with_b)}/{len(rows)} 問、ms)")
    slow = sorted(with_b, key=lambda r: r["space_to_candidate_ms"])[-max(1, len(with_b) // 20):]
    for name, group in (("全体の中央値", with_b), ("遅い5%の平均", slow)):
        if not group:
            continue
        cells = []
        for k in PHASES:
            values = sorted(r["breakdown"][k] for r in group)
            v = values[len(values) // 2] if name.endswith("中央値") else sum(values) / len(values)
            cells.append(f"{k} {v:.0f}")
        print(f"  {name}: " + " / ".join(cells))


if __name__ == "__main__":
    main()
