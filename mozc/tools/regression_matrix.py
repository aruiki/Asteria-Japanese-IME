"""既存の評価セットを、3 つのタイミングで 1 つずつ順に測り、前の原票と 1 問ずつ比べる(2026-10-27)。

session_eval.py を 1 回ずつ(並べずに)呼ぶ。タイミングは ADR0074 の検証と同じ 既定(手を止めてから Space)・
すぐ(--immediate-space)・0.15 秒(--space-gap 150)。AJIMEE は既定だけ。結果の名前は v3.2.0 の原票
(target/accuracy-guide/v320-final2/)と同じ <セット>-<タイミング>.json(利用者の報告は user-reports.json)。
各回の標準エラー(正解数と Space→候補の p95)は <名前>.log。最後に compare_runs.py で前の原票と比べ、p95 を並べる。

使い方: python mozc/tools/regression_matrix.py <session_handler_main> --out <フォルダ> --before <前の原票のフォルダ>
          [--ajimee eval/data/ajimee-bench.json] [--only 名前 ...] [名前=値 ...(サーバーの環境変数)]
"""
import argparse
import os
from pathlib import Path
import re
import subprocess
import sys

P = Path(__file__).resolve().parents[2]
TIMINGS = {"default": [], "immediate": ["--immediate-space"], "gap150": ["--space-gap", "150"]}
SETS = ["kotori-typo", "asteria-typo-keys-20261026", "kotori-daily", "asteria-short-dev-20261022",
        "kotori-homophone"]


def runs(ajimee):
    for name in SETS:
        for timing, flags in TIMINGS.items():
            yield f"{name}-{timing}", P / "eval" / "sets" / f"{name}.json", flags
    yield "user-reports", P / "eval" / "sets" / "asteria-user-reports-20261027.json", []
    if ajimee:
        yield "ajimee-default", Path(ajimee), []


def p95(log):
    m = re.search(r"p95 (\d+)ms", log.read_text(encoding="utf-8", errors="replace")) if log.exists() else None
    return int(m.group(1)) if m else None


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("exe", type=Path)
    ap.add_argument("--out", type=Path, required=True)
    ap.add_argument("--before", type=Path, required=True)
    ap.add_argument("--ajimee")
    ap.add_argument("--only", nargs="*", help="測る名前(例: kotori-typo-default)。省くとすべて")
    args, env = ap.parse_known_args()
    args.out.mkdir(parents=True, exist_ok=True)
    names = []
    for name, data, flags in runs(args.ajimee):
        if args.only and name not in args.only:
            continue
        names.append(name)
        out = args.out / f"{name}.json"
        cmd = [sys.executable, str(P / "mozc" / "tools" / "session_eval.py"), str(args.exe), "--data", str(data),
               "--out", str(out), "--context", "--timeline", *flags, "KOTORI_LM_DEBUG=1", *env]
        print(" ".join(cmd), flush=True)
        log_path = args.out / f"{name}.log"
        with open(log_path, "w", encoding="utf-8") as log:
            code = subprocess.run(cmd, stdout=log, stderr=subprocess.STDOUT,
                                  env=dict(os.environ, PYTHONUTF8="1")).returncode
        lines = log_path.read_text(encoding="utf-8", errors="replace").splitlines()
        summary = next((l for l in lines if "Space→候補" in l), lines[-1] if lines else "")
        print(f"{name}: exit {code}  {summary}", flush=True)
        if code != 0:
            sys.exit(f"{name} が失敗した(終了コード {code})。{log_path} を見る")
        # AI が動いていた回だけを数える(session_eval が state=llm を待って確かめる。ここでも記録を見る)。
        if "AI: llm" not in lines:
            sys.exit(f"{name} は AI が動いていない状態で測られた。{log_path} を見る")
    subprocess.run([sys.executable, str(P / "mozc" / "tools" / "compare_runs.py"), str(args.before), str(args.out),
                    *names], check=True)
    print("Space→候補 p95(ms): 名前 前 → 後")
    for name in names:
        print(f"  {name}: {p95(args.before / (name + '.log'))} → {p95(args.out / (name + '.log'))}")


if __name__ == "__main__":
    main()
