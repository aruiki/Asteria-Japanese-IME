"""Sonnet が作った文脈の選択の組を、調整用の評価セットにする（2026-10-27、v3.2.0）。

Sonnet の文は調整だけに使う（最終判定は人が書いた文の asteria-choice-judge / asteria-passage-judge）。
読みは Sonnet が書いたものを信じず、Sudachi と UniDic の両方の読みと一致したものだけを残す。対象の語が文に
ない・組の相手が対象の語と同じ・同じ文の重複も捨てる。

使い方（SudachiPy/fugashi を入れた Python で）:
  python mozc/tools/import_sonnet_choice.py --src target/accuracy-guide/sonnet-choice-v1 --out eval/sets/X.json
"""
import argparse
import collections
import hashlib
import json
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent))
import build_eval_sets as base  # noqa: E402


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--src", type=Path, required=True)
    ap.add_argument("--out", type=Path, required=True)
    args = ap.parse_args()
    readers = base.Readers()
    stats = collections.Counter()
    seen = set()
    rows = []
    for path in sorted(args.src.glob("*.json")):
        domain = path.stem
        for n, it in enumerate(json.load(open(path, encoding="utf-8"))):
            stats["in"] += 1
            s, reading = it.get("sentence", "").strip(), it.get("reading", "").strip()
            target, rivals = it.get("target", ""), [r for r in it.get("rival", it.get("rivals", [])) if r]
            if not s or not reading or target not in s or not rivals or target in rivals:
                stats["bad_fields"] += 1
                continue
            if s in seen:
                stats["duplicate"] += 1
                continue
            a, b = readers.sudachi_reading(s), readers.unidic_reading(s)
            if not (reading == a == b):
                stats["reading_mismatch"] += 1
                continue
            seen.add(s)
            rows.append(dict(index=f"sonnet-{domain}-{n:03d}", input=reading, context_text=it.get("context", ""),
                             expected_output=base.expected_for(s, readers.tokens(s)), domain=f"sonnet_{domain}",
                             target=target, rival=rivals, clue=it.get("clue", ""), reason=it.get("reason", ""),
                             pair=f"sonnet-{domain}-{n:03d}"))
            stats["kept"] += 1
            stats["clue_" + it.get("clue", "")] += 1
    args.out.write_text(json.dumps(rows, ensure_ascii=False, indent=1), encoding="utf-8")
    print(json.dumps(dict(stats=stats, out=str(args.out),
                          sha256=hashlib.sha256(args.out.read_bytes()).hexdigest()), ensure_ascii=False, indent=1))


if __name__ == "__main__":
    main()
