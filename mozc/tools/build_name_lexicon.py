"""2024年以降に作られた日本語版Wikipediaの記事の題名から、固有名詞・新語の辞書を作る（2026-10-23）。

読みは2つの方法で確かめる:
  A. 記事の冒頭の括弧書き（「江名（えな）は、…」）。題名がカタカナだけなら、その読み。
  B. 題名を形態素解析した読み（SudachiDict または UniDic）。
A と B のどちらかの解析器が一致した語だけを入れる。評価セット（判定用・開発用）に使った記事は、
ページIDと題名の両方で除く。Mozcの辞書と短い入力の語彙にすでにある（読み, 表記）は入れない。

品詞はカテゴリの語から決め、コストはMozcの辞書の同じ品詞のコストの中央値に --cost-offset を足す
（事前に固定。開発用セットで確かめる）。

使い方: python -X utf8 mozc/tools/build_name_lexicon.py --articles A.jsonl --exclude E.jsonl
          --mozc-dict DIR --short-lexicon S.tsv --out DIR
"""
import argparse
import collections
import hashlib
import json
from pathlib import Path
import re
import statistics
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent))
from build_eval_sets import Readers, kata_to_hira  # noqa: E402

POS_IDS = {
    "名詞,固有名詞,一般,*,*,*,*": 1920,
    "名詞,固有名詞,人名,一般,*,*,*": 1921,
    "名詞,固有名詞,地域,一般,*,*,*": 1924,
    "名詞,固有名詞,組織,*,*,*,*": 1929,
}
# カテゴリの語から品詞を決める（事前に固定）。人名を先に見る。
PERSON = re.compile(r"存命人物|\d+年生|\d+年没|の人物$|選手$|俳優$|歌手$|政治家$|作家$|漫画家$|声優$|アイドル$")
PLACE = re.compile(r"の町・字$|の地名$|の駅$|の島$|の山$|の川$|の湖$|の峠$|の集落$|の市町村$|の自治体$|の道路$")
ORGANIZATION = re.compile(r"の企業$|の会社$|の団体$|の学校$|の大学$|の高等学校$|の政党$|のクラブ$|のバンド$|の音楽グループ$")
KANA = re.compile(r"[ァ-ヺー・]+")
SURFACE = re.compile(r"[一-鿿々〆ヶぁ-ゖァ-ヺー・A-Za-z0-9０-９Ａ-Ｚａ-ｚ]+")


def title_surface(title):
    return title.split(" (")[0].strip()


def lead_reading(surface, extract):
    """冒頭の「表記（よみ、…）」の読み。なければ None。"""
    if KANA.fullmatch(surface):
        return kata_to_hira(surface.replace("・", ""))
    # 読みにカタカナを含むこともある（「ウィートン大学（ウィートンだいがく、…」）。
    m = re.match(re.escape(surface) + r"\s*[（(]([ぁ-ゖァ-ヺー・ 　]+)[、，,）)]", extract)
    if not m:
        return None
    return kata_to_hira(re.sub(r"[・ 　]", "", m.group(1)))


def pos_of(categories):
    if any(PERSON.search(c) for c in categories):
        return "名詞,固有名詞,人名,一般,*,*,*"
    if any(PLACE.search(c) for c in categories):
        return "名詞,固有名詞,地域,一般,*,*,*"
    if any(ORGANIZATION.search(c) for c in categories):
        return "名詞,固有名詞,組織,*,*,*,*"
    return "名詞,固有名詞,一般,*,*,*,*"


def sha256(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--articles", type=Path, required=True)
    ap.add_argument("--exclude", type=Path, action="append", required=True, help="評価セットに使った記事")
    ap.add_argument("--mozc-dict", type=Path, required=True)
    ap.add_argument("--short-lexicon", type=Path, required=True)
    ap.add_argument("--out", type=Path, required=True)
    ap.add_argument("--cost-offset", type=int, default=0)
    args = ap.parse_args()

    excluded_ids, excluded_titles = set(), set()
    for path in args.exclude:
        for line in open(path, encoding="utf-8"):
            a = json.loads(line)
            excluded_ids.add(a["pageid"])
            excluded_titles.add(title_surface(a["title"]))

    known = set()
    costs = collections.defaultdict(list)
    id_to_pos = {v: k for k, v in POS_IDS.items()}
    for path in sorted(args.mozc_dict.glob("dictionary*.txt")):
        for line in open(path, encoding="utf-8"):
            f = line.rstrip("\n").split("\t")
            if len(f) < 5:
                continue
            known.add((f[0], f[4]))
            if int(f[1]) in id_to_pos:
                costs[id_to_pos[int(f[1])]].append(int(f[3]))
    for line in open(args.short_lexicon, encoding="utf-8"):
        f = line.rstrip("\n").split("\t")
        known.add((f[0], f[4]))
    base_cost = {pos: int(statistics.median(costs[pos])) for pos in POS_IDS}

    readers = Readers()
    audit = collections.Counter()
    entries = {}
    for line in open(args.articles, encoding="utf-8"):
        a = json.loads(line)
        audit["articles"] += 1
        surface = title_surface(a["title"])
        if a["pageid"] in excluded_ids or surface in excluded_titles:
            audit["excluded_eval_article"] += 1
            continue
        if not (2 <= len(surface) <= 20 and SURFACE.fullmatch(surface)):
            audit["unsupported_surface"] += 1
            continue
        if not re.search(r"[一-鿿々ァ-ヺ]", surface):
            audit["no_kanji_or_katakana"] += 1
            continue
        a_reading = lead_reading(surface, a.get("extract", ""))
        if not a_reading or not re.fullmatch(r"[ぁ-ゖー]{2,40}", a_reading):
            audit["no_lead_reading"] += 1
            continue
        b_readings = {re.sub(r"[・ 　]", "", readers.sudachi_reading(surface)),
                      re.sub(r"[・ 　]", "", readers.unidic_reading(surface))}
        if a_reading not in b_readings:
            audit["reading_disagree"] += 1
            continue
        if (a_reading, surface) in known:
            audit["already_in_mozc"] += 1
            continue
        pos = pos_of(a.get("categories", []))
        entries[(a_reading, surface)] = (pos, a["pageid"], a.get("lastrevid"))
        audit["accepted"] += 1

    args.out.mkdir(parents=True, exist_ok=True)
    rows = []
    for (reading, surface), (pos, _, _) in sorted(entries.items()):
        cost = max(0, min(32767, base_cost[pos] + args.cost_offset))
        rows.append(f"{reading}\t{POS_IDS[pos]}\t{POS_IDS[pos]}\t{cost}\t{surface}\n")
    tsv = args.out / "name_lexicon.tsv"
    tsv.write_text("".join(rows), encoding="utf-8", newline="\n")
    sources = sorted(entries.values(), key=lambda v: v[1])
    meta_path = args.articles.parent / "meta.json"
    manifest = dict(
        source="ja.wikipedia.org 記事の題名と冒頭の読み（2024-03以降に作成された記事）",
        source_meta=json.loads(meta_path.read_text(encoding="utf-8")) if meta_path.exists() else None,
        articles_sha256=sha256(args.articles),
        excluded=[dict(path=str(p), sha256=sha256(p)) for p in args.exclude],
        license="CC BY-SA 4.0（Wikipedia）。題名と読みの組は、出典の記事の版（pageid、lastrevid）を pages に記録する",
        generator_sha256=sha256(__file__),
        rule=dict(reading="記事冒頭の括弧書きの読み（カタカナの題名はその読み）が SudachiDict または UniDic の読みと一致",
                  cost="Mozc辞書の同じ品詞のコストの中央値 + cost_offset", cost_offset=args.cost_offset,
                  base_cost=base_cost),
        audit=dict(audit),
        output_entries=len(rows),
        by_pos=dict(collections.Counter(v[0] for v in entries.values())),
        sha256=hashlib.sha256("".join(rows).encode("utf-8")).hexdigest(),
        pages=[dict(pageid=p, lastrevid=r) for _, p, r in sources],
    )
    (args.out / "name_lexicon.json").write_text(json.dumps(manifest, ensure_ascii=False, indent=1), encoding="utf-8")
    print(json.dumps({k: v for k, v in manifest.items() if k != "pages"}, ensure_ascii=False, indent=1))


if __name__ == "__main__":
    main()
