"""文脈に合った語を選べるかを測る評価セットを作る（2026-10-27、v3.2.0 のゴール）。

人が書いた文章（Wikinews・Wikipedia の新しい記事・JMultiWOZ・RealPersonaChat。eval-sources-v1）から 2 種類を作る。

1. 文の中の文脈の組（asteria-choice-{judge,dev}.json）
   同じ読みで別の語になる語（体重を測る／時間を計る、厚い本／熱いお茶）を含む文の切れ端を、意味ごとに 1 つずつ、
   別々の文書から取って組にする。前の文なしで切れ端を打つので、語を決める手がかりは同じ文の中だけにある。
   いつも片方の意味を選ぶ IME は、組の半分しか当たらない。
   語の区別: 名詞は Sudachi の正規化表記、動詞・形容詞は辞書形（正規化は 測る→計る のように表記をまとめるため使わない）。
   表記ゆれだけの組（子供／子ども）は除く。各問に、その問の語（target）と組の相手の語（rival）を入れる。
2. 長い文章（asteria-passage-{judge,dev}.json）
   同じ文書の続いた 4 つの切れ端を、前の切れ端を前文にして順に打つ（文章を通して打つ形）。

読み: 切れ端全体の読みが Sudachi と UniDic で一致したものだけ。開発用／判定用は、組は読みの、文章は文書のハッシュで分け、
同じ読み・同じ文書が両方に出ない。判定用は調整に使わない（eval/README.md）。

使い方（SudachiPy/fugashi を入れた Python で）:
  python mozc/tools/build_context_choice.py --sources target/accuracy-guide/eval-sources-v1 --out DIR
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
CONTENT = {"名詞", "動詞", "形容詞"}


def h(text):
    return hashlib.sha256(text.encode()).hexdigest()


def targets(readers, chunk):
    """切れ端の中の、読みが同じで別の語になりうる語の候補: (表層, 読み, 語の区別の鍵, 品詞)。"""
    out = []
    for m in readers.sudachi.tokenize(chunk, readers.mode):
        p = m.part_of_speech()
        if p[0] not in CONTENT or not KANJI.search(m.surface()):
            continue
        if p[0] == "名詞" and p[1] != "普通名詞":
            continue
        if p[0] == "名詞" and len(m.surface()) < 2:
            continue
        # 動詞・形容詞は活用するので、読みは辞書形の読みで束ねる。
        if p[0] == "名詞":
            reading, key = base.kata_to_hira(m.reading_form()), m.normalized_form()
        else:
            lemma = m.dictionary_form()
            reading = readers.sudachi_reading(lemma)
            key = lemma
        if reading and KANJI.search(key):
            out.append((m.surface(), reading, key, p[0]))
    return out


# 「異字同訓」の漢字の使い分け（文化審議会国語分科会、2014）で、意味によって使い分ける漢字の組の主なもの。
# Sudachi の正規化は 計る・量る→測る のようにこれらもまとめるので、この組の違いは別の語として残す。
DISTINCT_KANJI = [set(g) for g in (
    "測計量図謀諮", "効利", "熱暑厚", "温暖", "早速", "収納治修", "勧進薦", "変代替換", "会合遭",
    "上揚挙", "表現著", "写映", "冒犯侵", "堅固硬", "尋訪", "説解溶", "止留泊", "執採捕取撮",
    "付着就", "空開明", "帰返", "聞効", "見診観看", "立建", "降下", "冷覚", "誤謝", "厚篤",
    "初始", "柔軟", "絞搾", "差指刺挿射", "勤努務", "備供", "伸延", "乗載", "張貼", "沈鎮",
    "超越", "成為", "望臨", "生産", "鳴泣", "斬切", "混交", "異違")]


def same_spelling_family(a, b, normalized_same=False):
    """表記ゆれだけの違いか。かなを除いた漢字が同じ・片方がもう片方を含む、または Sudachi の正規化が同じで、
    違う漢字の組が異字同訓の使い分けにないとき（活かす／生かす）。"""
    ka = "".join(KANJI.findall(a))
    kb = "".join(KANJI.findall(b))
    if ka == kb or ka in kb or kb in ka:
        return True
    if not normalized_same:
        return False
    diff = {c for c in ka if c not in kb} | {c for c in kb if c not in ka}
    return not any(diff <= g for g in DISTINCT_KANJI)


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--sources", type=Path, required=True)
    ap.add_argument("--out", type=Path, required=True)
    ap.add_argument("--judge-pairs", type=int, default=150)
    ap.add_argument("--dev-pairs", type=int, default=150)
    ap.add_argument("--judge-passages", type=int, default=60)
    ap.add_argument("--dev-passages", type=int, default=60)
    ap.add_argument("--passage-len", type=int, default=4)
    args = ap.parse_args()
    readers = base.Readers()
    stats = collections.Counter()

    # 1. 文の中の文脈の組の素材: 読み → 語の鍵 → 出現
    by_reading = collections.defaultdict(lambda: collections.defaultdict(list))
    # 2. 長い文章の素材: 文書 → 切れ端の列
    docs = collections.defaultdict(list)

    def consider(doc, chunk):
        if not 8 <= len(chunk) <= 30:
            return
        stats["chunks"] += 1
        for surface, reading, key, pos in targets(readers, chunk):
            by_reading[reading][key].append(dict(doc=doc, chunk=chunk, surface=surface, pos=pos))

    for doc, chunk, _ in base.doc_candidates(args.sources):
        consider(doc, chunk)
        docs[doc].append(chunk)
    for doc, chunk, _ in base.short_candidates(args.sources):
        consider(doc, chunk)
    for _, doc, chunk, _ in base.wiki_candidates(args.sources):
        consider(doc, chunk)
        docs[doc].append(chunk)

    def reading_of(chunk):
        a, b = readers.sudachi_reading(chunk), readers.unidic_reading(chunk)
        if a != b:
            stats["reading_disagree"] += 1
            return None
        return a

    # 組を作る。出現の多い 2 つの語を組にし、文書が同じなら組にしない。
    pairs = []
    for reading, groups in sorted(by_reading.items()):
        if len(groups) < 2:
            continue
        top = sorted(groups.items(), key=lambda kv: (-len(kv[1]), kv[0]))[:2]
        (ka, ia), (kb, ib) = top
        # 出現の少ない語は、人名・珍しい書き方・元の文の書き間違いが混ざる(開発用で確かめた)。
        # どちらの語も 3 回以上、2 つ以上の文書に出るものだけにする。
        if min(len(ia), len(ib)) < 3 or min(len({r["doc"] for r in ia}), len({r["doc"] for r in ib})) < 2:
            stats["rare"] += 1
            continue
        def norm(word):
            ms = list(readers.sudachi.tokenize(word, readers.mode))
            return ms[0].normalized_form() if len(ms) == 1 else word
        if same_spelling_family(ka, kb, normalized_same=norm(ka) == norm(kb)):
            stats["spelling_family"] += 1
            continue
        picked = []
        for key, occ, rival_key, rival_occ in ((ka, ia, kb, ib), (kb, ib, ka, ia)):
            used = {p["doc"] for p in picked}
            for r in sorted(occ, key=lambda r: h(r["doc"] + r["chunk"])):
                if r["doc"] in used or rival_key in r["chunk"]:
                    continue
                inp = reading_of(r["chunk"])
                if inp:
                    picked.append(dict(r, key=key, input=inp,
                                       rival=sorted({o["surface"] for o in rival_occ})[:4]))
                    break
        if len(picked) == 2:
            pairs.append((reading, picked))
    stats["pairs"] = len(pairs)

    args.out.mkdir(parents=True, exist_ok=True)
    summary = dict(stats=stats, sets={})

    def write(name, rows):
        path = args.out / name
        path.write_text(json.dumps(rows, ensure_ascii=False, indent=1), encoding="utf-8")
        summary["sets"][name] = dict(items=len(rows), sha256=h(path.read_text(encoding="utf-8")))

    for split, want in (("judge", args.judge_pairs), ("dev", args.dev_pairs)):
        chosen = [p for p in pairs if base.split_of("choice-" + p[0]) == split]
        chosen = sorted(chosen, key=lambda p: h(p[0]))[:want]
        rows = []
        for n, (reading, items) in enumerate(chosen):
            for side, it in zip("ab", items):
                tokens = readers.tokens(it["chunk"])
                rows.append(dict(index=f"choice-{split}-{n:03d}{side}", input=it["input"], context_text="",
                                 expected_output=base.expected_for(it["chunk"], tokens), domain="context_choice",
                                 pair=f"{split}-{n:03d}", word_reading=reading, target=it["surface"],
                                 rival=it["rival"], pos=it["pos"], source_doc=it["doc"]))
        write(f"asteria-choice-{split}.json", rows)

    # 長い文章: 読みがそろう続いた切れ端を passage-len 個。
    passages = []
    for doc, chunks in sorted(docs.items()):
        run = []
        for c in chunks:
            inp = reading_of(c) if 8 <= len(c) <= 40 else None
            if inp is None:
                run = []
                continue
            run.append((c, inp))
            if len(run) == args.passage_len:
                passages.append((doc, list(run)))
                break
    stats["passages"] = len(passages)
    for split, want in (("judge", args.judge_passages), ("dev", args.dev_passages)):
        chosen = [p for p in passages if base.split_of("passage-" + p[0]) == split]
        chosen = sorted(chosen, key=lambda p: h(p[0]))[:want]
        rows = []
        for n, (doc, run) in enumerate(chosen):
            before = ""
            for k, (c, inp) in enumerate(run):
                rows.append(dict(index=f"passage-{split}-{n:03d}-{k}", input=inp, context_text=before[-60:],
                                 expected_output=base.expected_for(c, readers.tokens(c)), domain="passage",
                                 passage=f"{split}-{n:03d}", source_doc=doc))
                before += c
        write(f"asteria-passage-{split}.json", rows)

    summary["stats"] = dict(stats)
    (args.out / "context-choice-summary.json").write_text(json.dumps(summary, ensure_ascii=False, indent=1),
                                                         encoding="utf-8")
    print(json.dumps(summary, ensure_ascii=False, indent=1))


if __name__ == "__main__":
    main()
