"""既定の学習データ(同音の語の意味ごとの手がかりの語)を、LmRewriter に埋め込む表にする(2026-10-27、v3.2.0)。

元は Sonnet が書いた手がかりの語(mozc/data/context_cues_20261027.json)。同音の語の組は調整用のセット
(asteria-choice-dev、asteria-sonnet-choice)と、Sonnet が足した日常の組から作った。判定用のセットは使わない。
読みは Sonnet を信じず、表記の読みが Sudachi か UniDic の読みと一致した語だけを残す。手がかりの語は:
  - 漢字かカタカナだけの語。組のどの表記も含まない。
  - 同じ組のほかの意味の手がかりにもある語は捨てる(意味を分けない)。
  - 10 以上の組に出る語は捨てる(確認・判断のようにどこにでもある語で、広く効きすぎる)。
表の 1 行は「読み、選ぶ表記、退ける表記、手がかり(、区切り)」(docs/adr/0075)。

使い方(SudachiPy/fugashi を入れた Python で):
  python mozc/tools/gen_context_defaults.py --src target/accuracy-guide/sonnet-cues-v1 \\
      --json mozc/data/context_cues_20261027.json --out <Mozc>/src/rewriter/kotori_context_defaults.inc
"""
import argparse
import collections
import hashlib
import json
from pathlib import Path
import re
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent))
import build_eval_sets as base  # noqa: E402

KANJI = re.compile(r"^[一-鿿々]+$")
CUE = re.compile(r"^(?:[一-鿿々]+|[ァ-ヺー]{2,})$")
HIRA = re.compile(r"^[ぁ-ゟー]+$")


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--src", type=Path, help="Sonnet の出力(out*.json)のフォルダ。省略すると --json を読む")
    ap.add_argument("--json", type=Path, required=True, help="検証済みの手がかりの語(書き出し・読み込み)")
    ap.add_argument("--out", type=Path, required=True)
    args = ap.parse_args()
    stats = collections.Counter()
    if args.src:
        readers = base.Readers()
        groups = {}
        for path in sorted(args.src.glob("out*.json")):
            for g in json.load(open(path, encoding="utf-8")):
                stats["groups_in"] += 1
                reading = g.get("reading", "")
                if not HIRA.match(reading) or reading in groups:
                    stats["bad_or_duplicate_group"] += 1
                    continue
                senses = []
                for s in g.get("senses", []):
                    surface = s.get("surface", "")
                    if not KANJI.match(surface) or reading not in (readers.sudachi_reading(surface),
                                                                   readers.unidic_reading(surface)):
                        stats["sense_reading_mismatch"] += 1
                        continue
                    senses.append(dict(surface=surface, cues=[c for c in dict.fromkeys(s.get("cues", []))]))
                if len(senses) >= 2:
                    groups[reading] = senses
        # 組の中で意味を分けない語、組をまたいで多く出る語を捨てる。
        spread = collections.Counter()
        for reading, senses in groups.items():
            for c in {c for s in senses for c in s["cues"]}:
                spread[c] += 1
        for reading, senses in groups.items():
            surfaces = [s["surface"] for s in senses]
            count = collections.Counter(c for s in senses for c in s["cues"])
            for s in senses:
                kept = []
                for c in s["cues"]:
                    if not CUE.match(c) or any(x in c for x in surfaces) or reading in c:
                        stats["cue_bad"] += 1
                    elif count[c] > 1:
                        stats["cue_shared_in_group"] += 1
                    elif spread[c] >= 10:
                        stats["cue_too_common"] += 1
                    else:
                        kept.append(c)
                s["cues"] = kept
        data = [dict(reading=r, senses=s) for r, s in sorted(groups.items())]
        args.json.parent.mkdir(parents=True, exist_ok=True)
        args.json.write_text(json.dumps(data, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
    data = json.load(open(args.json, encoding="utf-8"))
    lines = []
    for g in data:
        for chosen in g["senses"]:
            if not chosen["cues"]:
                continue
            for rejected in g["senses"]:
                if rejected is chosen:
                    continue
                lines.append((g["reading"], chosen["surface"], rejected["surface"], "、".join(chosen["cues"])))
    stats["groups"] = len(data)
    stats["entries"] = len(lines)
    sha = hashlib.sha256(args.json.read_bytes()).hexdigest()
    esc = lambda s: s.replace("\\", "\\\\").replace('"', '\\"')  # noqa: E731
    body = "".join(f'{{"{esc(a)}", "{esc(b)}", "{esc(c)}", "{esc(d)}"}},\n' for a, b, c, d in lines)
    args.out.write_text("// 生成ファイル(mozc/tools/gen_context_defaults.py)。手で編集しない。\n"
                        f"// 元: mozc/data/context_cues_20261027.json(Sonnet が書いた手がかりの語を検証したもの)"
                        f" SHA-256 {sha}; {len(lines)}件\n" + body, encoding="utf-8", newline="\n")
    print(json.dumps(stats, ensure_ascii=False))


if __name__ == "__main__":
    main()
