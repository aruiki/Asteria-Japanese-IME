"""日常の文章で、同じ語がかなと漢字のどちらで書かれるかの比率を数える（GOAL.md 指針2026-10-23「表記は日常で多い方」）。

出典: RealPersonaChat の人の発話と JMultiWOZ の発話のうち、評価セットで「開発用」に割り当てた文書だけ
（判定用に使った文書は数えない）。Sudachi（SplitMode C）で区切り、正規化表記と読みが同じで、
表層がかなだけの語と漢字を含む語の両方が現れるものを集める。活用形ごとの表層の組で数える（良い/いい、付け/つけ）。

出力: TSV（少数派の表層<TAB>多数派の表層<TAB>多数派の割合<TAB>出現数<TAB>品詞）と、出典・条件・SHAを書いたJSON。
使い方（SudachiPyを入れたPythonで）:
  python mozc/tools/build_kana_preference.py --sources target/accuracy-guide/eval-sources-v1 --out DIR
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

KANA = re.compile(r"^[ぁ-ゖー]+$")
KANJI = re.compile(r"[一-鿿々]")


def utterances(sources):
    root = sources / "realpersonachat/extracted/real-persona-chat-1.0.0/real_persona_chat/dialogues"
    for path in sorted(root.glob("*.json")):
        d = json.load(open(path, encoding="utf-8"))
        doc = f"rpc-{d['dialogue_id']}"
        for u in d["utterances"]:
            yield doc, u["text"]
    w = json.load(open(sources / "jmultiwoz/extracted/JMultiWOZ_1.0/dialogues.json", encoding="utf-8"))
    for name, d in sorted(w.items()):
        doc = f"mwoz-{d['dialogue_id']}"
        for t in d["turns"]:
            yield doc, t["utterance"]


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--sources", type=Path, required=True)
    ap.add_argument("--out", type=Path, required=True)
    ap.add_argument("--min-count", type=int, default=20)
    args = ap.parse_args()
    readers = base.Readers()
    # (正規化表記, 読み, 品詞の活用形) → 表層 → 出現数
    counts = collections.defaultdict(collections.Counter)
    used_docs = 0
    for doc, text in utterances(args.sources):
        if base.split_of(doc) != "dev":
            continue  # 判定用の文書は数えない
        used_docs += 1
        for m in readers.sudachi.tokenize(text.replace("\n", ""), readers.mode):
            s = m.surface()
            if not (KANA.match(s) or KANJI.search(s)):
                continue
            pos = m.part_of_speech()
            key = (m.normalized_form(), base.kata_to_hira(m.reading_form()), pos[0], pos[4], pos[5])
            counts[key][s] += 1
    # 同じ語(正規化表記・読み・品詞)の表層のうち、日常でいちばん多い表記を「多数派」とし、
    # 漢字を含む少数派の表層 → 多数派 の組を出す(良く→よく、身体→体)。かな→漢字の向きは出さない。
    rows = []
    have = set()

    def emit(c, pos0):
        if sum(c.values()) < args.min_count:
            return
        major, mn = max(c.items(), key=lambda x: (x[1], x[0]))
        for minor, n in c.items():
            if minor == major or not KANJI.search(minor) or (minor, major) in have or mn + n < args.min_count:
                continue
            rows.append((minor, major, round(mn / (mn + n), 4), mn + n, pos0))
            have.add((minor, major))

    for key, c in counts.items():
        emit(c, key[2])
    # 読みの違う表層も同じ語として数える(いい/良い: Sudachiは良いを「よい」と読む)。製品は同じ入力の
    # 候補にしか使わないので、読みの違う組(今日は/こんにちは)は入力が一致せず使われない。
    merged = collections.defaultdict(collections.Counter)
    for (norm, _reading, p0, p4, p5), c in counts.items():
        merged[(norm, p0, p4, p5)].update(c)
    for key, c in merged.items():
        emit(c, key[1])
    rows.sort(key=lambda r: (-r[2], -r[3], r[0]))
    args.out.mkdir(parents=True, exist_ok=True)
    tsv = args.out / "kana_preference.tsv"
    tsv.write_text("".join(f"{a}\t{b}\t{r}\t{n}\t{p}\n" for a, b, r, n, p in rows), encoding="utf-8")
    meta = dict(sources=["RealPersonaChat 1.0.0 (CC BY-SA 4.0)", "JMultiWOZ 1.0 (CC BY-SA 4.0)"],
                documents="evaluation dev split only (build_eval_sets.split_of == 'dev')", used_documents=used_docs,
                tokenizer="SudachiPy SplitMode C, sudachidict_core", min_count=args.min_count, entries=len(rows),
                sha256=hashlib.sha256(tsv.read_bytes()).hexdigest())
    (args.out / "kana_preference.json").write_text(json.dumps(meta, ensure_ascii=False, indent=1), encoding="utf-8")
    print(json.dumps(meta, ensure_ascii=False))
    for r in rows[:40]:
        print(r)


if __name__ == "__main__":
    main()
