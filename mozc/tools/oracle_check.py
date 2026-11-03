"""外れた問を「候補に正解がなかった(作る側)」と「候補にあったのに選べなかった(採点・合わせ方)」に分ける
(カード 89、2026-11-03)。

実際の IME と同じセッションの経路(session_handler_main)で、KOTORI_LM_DEBUG の記録から、読み全体を採点した
最後の回の候補と点(zenz・LLM・合計)を読み、1 番目と正解を比べる。前の文はセットの context_text を渡す。
確定しないので学習しない。

使い方: python mozc/tools/oracle_check.py <session_handler_main> --data セット.json --out 結果.json [--models 場所]
"""
import argparse
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import time

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "eval" / "imebench"))
from imebench import to_romaji  # noqa: E402

sys.path.insert(0, str(Path(__file__).resolve().parent))
import consistency_check  # noqa: E402
import eval_ai  # noqa: E402

LINE = re.compile(r"^\[kotori\] (.+) zenz=(-?[\d.]+|-?inf) llm=(-?[\d.]+|-?inf) (lattice|gen) score=(-?[\d.]+|-?inf)$")


def blocks(text, reading):
    """読み全体を採点した回ごとの候補 [(表記, zenz, llm, 種類, 合計)]。"""
    out, current = [], None
    for line in text.splitlines():
        if line.startswith("[kotori] select reading="):
            current = [] if line.split("=", 1)[1].split(" ", 1)[0] == reading else None
            if current is not None:
                out.append(current)
            continue
        m = LINE.match(line)
        if m and current is not None:
            current.append((m.group(1), float(m.group(2)), float(m.group(3)), m.group(4), float(m.group(5))))
    return [b for b in out if b]


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("exe", type=Path)
    ap.add_argument("--data", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--models", default=eval_ai.default_models())
    ap.add_argument("--warm", type=float, default=15)
    args, extra = ap.parse_known_args()
    os.environ.update(dict(e.split("=", 1) for e in extra if "=" in e))
    os.environ["KOTORI_LM_DEBUG"] = "1"
    log_path = Path(tempfile.mkdtemp()) / "debug.log"
    log = open(log_path, "w", encoding="utf-8")
    real = subprocess.Popen

    def popen(*a, **k):
        k["stderr"] = log
        return real(*a, **k)

    consistency_check.candidate_order.subprocess.Popen = popen
    s = consistency_check.Session(args.exe.resolve(), args.models)
    consistency_check.candidate_order.subprocess.Popen = real
    s.send("SEND_KEYS\tai")
    time.sleep(args.warm)
    s.send("SEND_KEY\tESC")
    rows = []
    for it in json.load(open(args.data, encoding="utf-8")):
        romaji = it.get("romaji") or to_romaji(it["input"])
        if romaji is None:
            continue
        context = it.get("context_text", "")
        option = ("context.preceding_text=\"" + context.replace("\\", "\\\\").replace('"', '\\"') + "\""
                  if context else "")
        log.flush()
        start = log_path.stat().st_size
        shown = s.top(romaji, option, 0.03, 0)
        time.sleep(0.2)
        log.flush()
        with open(log_path, encoding="utf-8", errors="replace") as f:
            f.seek(start)
            found = blocks(f.read(), it["input"])
        expected = it.get("expected_output") or []
        row = dict(index=it.get("index"), input=it["input"], expected=expected, shown=shown,
                   ok=shown in expected, scored=bool(found))
        if found:
            cands = sorted(found[-1], key=lambda c: -c[4])
            values = [c[0] for c in cands]
            hit = next((i for i, v in enumerate(values) if v in expected), None)
            row.update(candidates=[list(c) for c in cands[:12]], answer_rank=None if hit is None else hit + 1,
                       gap=None if hit is None else round(cands[0][4] - cands[hit][4], 2))
        rows.append(row)
    s.close()
    json.dump(rows, open(args.out, "w", encoding="utf-8"), ensure_ascii=False, indent=1)
    wrong = [r for r in rows if not r["ok"]]
    absent = [r for r in wrong if r.get("answer_rank") is None]
    present = [r for r in wrong if r.get("answer_rank") is not None]
    top_scored = [r for r in present if r["answer_rank"] == 1]
    print(f"{args.data}: 問 {len(rows)} 正解 {len(rows) - len(wrong)} 外れ {len(wrong)} "
          f"(候補になし {len(absent)}、候補にあり {len(present)}: そのうち採点では 1 位 {len(top_scored)})")
    gaps = sorted(r["gap"] for r in present if r["answer_rank"] > 1)
    if gaps:
        print("  候補にあった正解と 1 番目の点差:", gaps)


if __name__ == "__main__":
    main()
