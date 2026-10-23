"""採用の条件（GOAL.md 指針2026-10-23）を、2つの結果の組で判定する。

総合スコア = 文脈の組30% + 日常の短文25% + 文書15% + 技術15% + 固有名詞15%（各分野は表記ゆれ一致の正解率）。
条件: (1) 総合が上がる (2) 変化した問で 正解に変わった数 >= 2 × 誤りに変わった数 (3) どの分野も1.0ポイントを超えて下がらない。

使い方（SudachiPyを入れたPythonで）: python mozc/tools/adoption_check.py <基準の結果フォルダ> <候補の結果フォルダ> <split>
フォルダには <分野>-<split>.json（session_eval.py の出力）が要る。
"""
import json
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent))
from lenient_score import lenient_ok  # noqa: E402

WEIGHTS = {"ctxpair": 0.30, "short": 0.25, "doc": 0.15, "tech": 0.15, "names": 0.15}


def load(folder, domain, split):
    rows = json.load(open(Path(folder) / f"{domain}-{split}.json", encoding="utf-8"))
    return {r["index"]: lenient_ok(r["top1"], r["expected"]) for r in rows if not r.get("skipped")}


def main():
    base, cand, split = sys.argv[1:4]
    total_base = total_cand = 0.0
    improved = regressed = 0
    worst_drop = 0.0
    report = {}
    for domain, w in WEIGHTS.items():
        a, b = load(base, domain, split), load(cand, domain, split)
        keys = sorted(set(a) & set(b))
        ra = 100 * sum(a[k] for k in keys) / len(keys)
        rb = 100 * sum(b[k] for k in keys) / len(keys)
        imp = sum((not a[k]) and b[k] for k in keys)
        reg = sum(a[k] and not b[k] for k in keys)
        improved += imp
        regressed += reg
        worst_drop = max(worst_drop, ra - rb)
        total_base += w * ra
        total_cand += w * rb
        report[domain] = dict(n=len(keys), base=round(ra, 2), cand=round(rb, 2), improved=imp, regressed=reg)
    ok1 = total_cand > total_base
    ok2 = improved >= 2 * regressed and improved > 0
    ok3 = worst_drop <= 1.0
    result = dict(split=split, domains=report, composite_base=round(total_base, 2), composite_cand=round(total_cand, 2),
                  improved=improved, regressed=regressed, worst_domain_drop=round(worst_drop, 2),
                  conditions={"composite_up": ok1, "improved_at_least_2x_regressed": ok2, "no_domain_drop_over_1pt": ok3},
                  adopt=ok1 and ok2 and ok3)
    print(json.dumps(result, ensure_ascii=False, indent=1))


if __name__ == "__main__":
    main()
