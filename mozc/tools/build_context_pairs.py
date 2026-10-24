"""前文がないと決まらない同音語の「最小対」評価セットを作る（2026-10-23、利用者の提案）。

同じ読みで意味の違う名詞（汽車/記者、機会/機械）を含む短い句（名詞＋後続の助詞、例「記者が」）を、
別々の文書から1つずつ取り、それぞれの文書で句より前の本文を前文にして組にする。前文を使わないIMEは
組の片方しか当てられない（同じ句なら最大50%）ので、前文あり/なしの差が文脈の効き目になる。

読み: 句の読みがSudachiとUniDicで一致したものだけ。同音語は両方とも漢字を含む普通名詞で、Sudachiの
正規化表記が異なるもの（表記ゆれの組は除く）。開発用/判定用は読みのハッシュで分け、同じ読みが両方に出ない。

使い方（SudachiPy/fugashiを入れたPythonで）:
  python mozc/tools/build_context_pairs.py --sources target/accuracy-guide/eval-sources-v1 --out DIR
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

KANJI = re.compile(r"[一-鿿々]")


def phrases(readers, text):
    """(句, 句の開始位置, 名詞, 名詞の読み, 名詞の正規化表記) を返す。句は名詞＋直後の助詞1つ。"""
    ms = list(readers.sudachi.tokenize(text, readers.mode))
    pos = 0
    out = []
    for i, m in enumerate(ms):
        start = pos
        pos += len(m.surface())
        p = m.part_of_speech()
        if p[0] != "名詞" or p[1] != "普通名詞" or not KANJI.search(m.surface()) or len(m.surface()) < 2:
            continue
        if i + 1 >= len(ms) or ms[i + 1].part_of_speech()[0] != "助詞":
            continue
        phrase = m.surface() + ms[i + 1].surface()
        out.append((phrase, start, m.surface(), base.kata_to_hira(m.reading_form()), m.normalized_form()))
    return out


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--sources", type=Path, required=True)
    ap.add_argument("--out", type=Path, required=True)
    ap.add_argument("--judge-pairs", type=int, default=150)
    ap.add_argument("--dev-pairs", type=int, default=100)
    args = ap.parse_args()
    readers = base.Readers()
    # 読み → 正規化表記 → 出現（文書、句、前文）
    by_reading = collections.defaultdict(lambda: collections.defaultdict(list))
    stats = collections.Counter()

    def scan(doc, chunk, context):
        for phrase, start, noun, noun_reading, norm in phrases(readers, chunk):
            stats["phrases"] += 1
            before = (context + chunk[:start]).replace("\t", " ")[-60:]
            if len(before) < 8:
                continue  # 前文がほとんどない句は、この評価の目的に合わない
            by_reading[noun_reading][norm].append(dict(doc=doc, phrase=phrase, noun=noun, context=before))

    for doc, chunk, ctx in base.doc_candidates(args.sources):
        scan(doc, chunk, ctx)
    for _, doc, chunk, ctx in base.wiki_candidates(args.sources):
        scan(doc, chunk, ctx)

    pairs = []
    for reading, groups in sorted(by_reading.items()):
        if len(groups) < 2:
            continue
        # 出現の多い2つの意味を組にする。文書が同じなら組にしない。
        top = sorted(groups.items(), key=lambda kv: (-len(kv[1]), kv[0]))[:2]
        (na, ia), (nb, ib) = top
        a = sorted(ia, key=lambda r: hashlib.sha256((r["doc"] + r["phrase"]).encode()).hexdigest())[0]
        b = next((r for r in sorted(ib, key=lambda r: hashlib.sha256((r["doc"] + r["phrase"]).encode()).hexdigest())
                  if r["doc"] != a["doc"]), None)
        if b is None or a["noun"] == b["noun"]:
            continue
        items = []
        for r in (a, b):
            ra, rb = readers.sudachi_reading(r["phrase"]), readers.unidic_reading(r["phrase"])
            if ra != rb:
                stats["reading_disagree"] += 1
                break
            items.append(dict(r, input=ra))
        # 名詞の読みは同じ。後続の助詞(が/を)は違ってよい(前文なしでの曖昧さは名詞で決まる)。
        if len(items) != 2:
            continue
        pairs.append((reading, items))
    args.out.mkdir(parents=True, exist_ok=True)
    summary = dict(stats=dict(stats), candidate_pairs=len(pairs), sets={})
    for split, want in (("judge", args.judge_pairs), ("dev", args.dev_pairs)):
        chosen = [p for p in pairs if base.split_of("pair-" + p[0]) == split]
        chosen = sorted(chosen, key=lambda p: hashlib.sha256(p[0].encode()).hexdigest())[:want]
        rows = []
        for n, (reading, items) in enumerate(chosen):
            for side, it in zip("ab", items):
                rows.append(dict(index=f"ctxpair-{split}-{n:03d}{side}", input=it["input"], context_text=it["context"],
                                 expected_output=sorted(base.width_variants(it["phrase"])), domain="context_pair",
                                 pair=f"{split}-{n:03d}", source_doc=it["doc"]))
        path = args.out / f"asteria-ctxpair-{split}.json"
        path.write_text(json.dumps(rows, ensure_ascii=False, indent=1), encoding="utf-8")
        summary["sets"][path.name] = dict(pairs=len(chosen), items=len(rows),
                                          sha256=hashlib.sha256(path.read_bytes()).hexdigest())
    (args.out / "ctxpair-summary.json").write_text(json.dumps(summary, ensure_ascii=False, indent=1), encoding="utf-8")
    print(json.dumps(summary, ensure_ascii=False, indent=1))


if __name__ == "__main__":
    main()
