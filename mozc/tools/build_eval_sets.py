"""4分野の評価セット（開発用・判定用）を公開コーパスから自動で作る（GOAL.md 2026-10-22）。

分野と出典（zenz-v2.5の学習データ: 2024年2月のWikipedia・CC-MAIN-2023-40と重ならないものを選ぶ）:
  short  日常の短文・チャット: RealPersonaChat の人の発話、JMultiWOZ の利用者発話（前文なし）
  doc    仕事の文書・メール: JMultiWOZ のオペレーター発話（直前の発話を前文）、ウィキニュースの文（記事内の直前の文を前文）
  tech   技術・専門用語: 2024年3月以降に作られたWikipedia記事の、題名を含まない文
  names  固有名詞・新語: 同じ記事の、題名を含む文
読み: SudachiとUniDic(MeCab)で独立に付け、ひらがなに揃えて一致した文だけを採用する（人手確認なし）。
正解集合: 原文、数字・英記号の全角/半角、事前に定めた「かなが自然な語」の漢字/かなの両方。
開発用/判定用は出典の文書(対話・記事)単位のハッシュで分ける。判定用は調整に使わない。

使い方（読み付けのためSudachiPy/fugashi/unidic-liteを入れたPythonで実行）:
  python mozc/tools/build_eval_sets.py --sources target/accuracy-guide/eval-sources-v1 --out eval/sets/generated
"""
import argparse
import bz2
import collections
import hashlib
import json
from pathlib import Path
import re
import unicodedata
import xml.etree.ElementTree as ET

import fugashi
from sudachipy import dictionary, tokenizer

# 公用文・新聞の表記でかな書きが一般的な語。どちらの表記も正解として許す（事前に固定。結果を見て足さない）。
KANA_NATURAL = {
    "事": "こと", "時": "とき", "所": "ところ", "物": "もの", "為": "ため", "様": "よう", "通り": "とおり",
    "殆ど": "ほとんど", "共に": "ともに", "全て": "すべて", "様々": "さまざま", "又": "また", "及び": "および",
    "但し": "ただし", "既に": "すでに", "更に": "さらに", "未だ": "いまだ", "先ず": "まず", "是非": "ぜひ",
    "下さい": "ください", "頂く": "いただく", "頂き": "いただき", "致し": "いたし", "致す": "いたす",
    "出来る": "できる", "出来": "でき", "有難う": "ありがとう", "有り難う": "ありがとう", "宜しく": "よろしく",
    "可哀想": "かわいそう", "凄く": "すごく", "凄い": "すごい", "沢山": "たくさん", "勿論": "もちろん",
    "何故": "なぜ", "色々": "いろいろ", "一緒": "いっしょ", "身体": "からだ", "体": "からだ", "等": "など",
    "迄": "まで", "位": "くらい", "程": "ほど", "毎": "ごと", "頃": "ころ",
}
ALLOWED = re.compile(r"^[ぁ-ゖァ-ヺー一-鿿々〆ヶ0-9０-９、。，．！？!?（）()「」『』・ー〜～%％]+$")


def kata_to_hira(s):
    return "".join(chr(ord(c) - 0x60) if "ァ" <= c <= "ヶ" else c for c in s)


class Readers:
    def __init__(self):
        self.sudachi = dictionary.Dictionary().create()
        self.mode = tokenizer.Tokenizer.SplitMode.C
        self.mecab = fugashi.Tagger()

    @staticmethod
    def surface_reading(surface, reading):
        # 数字は打ったとおり半角で、%は人が打つ「ぱーせんと」で入力する。読みのない記号は表記のまま。
        if re.fullmatch(r"[0-9０-９]+", surface):
            return unicodedata.normalize("NFKC", surface)
        if surface in ("%", "％"):
            return "パーセント"
        if not reading or reading == "*" or all(not ("ァ" <= c <= "ヶ" or c == "ー") for c in reading):
            return surface
        return reading

    def tokens(self, text):
        return [m.surface() for m in self.sudachi.tokenize(text, self.mode)]

    def sudachi_reading(self, text):
        return kata_to_hira("".join(self.surface_reading(m.surface(), m.reading_form())
                                    for m in self.sudachi.tokenize(text, self.mode)))

    def unidic_reading(self, text):
        out = []
        for word in self.mecab(text):
            kana = getattr(word.feature, "kana", None)
            out.append(self.surface_reading(word.surface, kana))
        return kata_to_hira("".join(out))


def width_variants(text):
    half = unicodedata.normalize("NFKC", text)
    full = "".join(chr(ord(c) + 0xFEE0) if "0" <= c <= "9" or c == "%" else c for c in half)
    # NFKCは「〜」等も変えるので、数字と%以外は原文のまま残した版も作る。
    digits_half = "".join(chr(ord(c) - 0xFEE0) if "０" <= c <= "９" or c == "％" else c for c in text)
    digits_full = "".join(chr(ord(c) + 0xFEE0) if "0" <= c <= "9" or c == "%" else c for c in text)
    return {text, digits_half, digits_full} | ({half, full} if half != text else set())


KANA_TO_KANJI = collections.defaultdict(set)
for _kanji, _kana in KANA_NATURAL.items():
    KANA_TO_KANJI[_kana].add(_kanji)


def kana_variants(tokens, limit=16):
    """単語単位でだけ漢字/かなを入れ替える（「時間」の「時」は単語ではないので変えない）。"""
    # 「共に」のように2単語に分かれる語は、隣とつないだ形でも照合する。
    merged, i = [], 0
    while i < len(tokens):
        pair = "".join(tokens[i:i + 2])
        if i + 1 < len(tokens) and (pair in KANA_NATURAL or pair in KANA_TO_KANJI):
            merged.append(pair)
            i += 2
        else:
            merged.append(tokens[i])
            i += 1
    out = [""]
    for token in merged:
        options = {token}
        if token in KANA_NATURAL:
            options.add(KANA_NATURAL[token])
        options |= KANA_TO_KANJI.get(token, set())
        out = [prefix + o for prefix in out for o in sorted(options)][:limit]
    return set(out)


def expected_for(text, tokens):
    variants = set()
    for v in kana_variants(tokens) | {text}:
        variants |= width_variants(v)
    return sorted(variants, key=lambda v: (v != text, v))


def sentences(text):
    for part in re.split(r"(?<=[。！？!?])", text.replace("\n", "")):
        part = part.strip()
        if part:
            yield part


def chunks(sentence, low=8, high=40):
    """長い文は「、」で区切って評価しやすい長さにする（AJIMEEの問と同程度）。"""
    if len(sentence) <= high:
        yield sentence
        return
    buf = ""
    for piece in re.split(r"(?<=、)", sentence):
        if len(buf) + len(piece) > high and len(buf) >= low:
            yield buf
            buf = ""
        buf += piece
    if len(buf) >= low:
        yield buf


def split_of(doc_key):
    return "judge" if int(hashlib.sha256(doc_key.encode()).hexdigest(), 16) % 2 == 0 else "dev"


def short_candidates(sources):
    root = sources / "realpersonachat/extracted/real-persona-chat-1.0.0/real_persona_chat/dialogues"
    for path in sorted(root.glob("*.json")):
        d = json.load(open(path, encoding="utf-8"))
        for u in d["utterances"]:
            for s in sentences(u["text"]):
                if 4 <= len(s) <= 40:
                    yield f"rpc-{d['dialogue_id']}", s, ""
    w = json.load(open(sources / "jmultiwoz/extracted/JMultiWOZ_1.0/dialogues.json", encoding="utf-8"))
    for name, d in sorted(w.items()):
        for t in d["turns"]:
            if t["speaker"] == "USER":
                for s in sentences(t["utterance"]):
                    if 4 <= len(s) <= 40:
                        yield f"mwoz-{d['dialogue_id']}", s, ""


def doc_candidates(sources):
    w = json.load(open(sources / "jmultiwoz/extracted/JMultiWOZ_1.0/dialogues.json", encoding="utf-8"))
    for name, d in sorted(w.items()):
        previous = ""
        for t in d["turns"]:
            if t["speaker"] == "SYSTEM":
                for s in sentences(t["utterance"]):
                    for c in chunks(s):
                        yield f"mwoz-{d['dialogue_id']}", c, previous[-60:]
            previous = t["utterance"].replace("\n", "")
    with bz2.open(sources / "wikinews/jawikinews-latest-pages-articles.xml.bz2") as f:
        for _, elem in ET.iterparse(f):
            if not elem.tag.endswith("page"):
                continue
            ns = elem.find("{*}ns")
            text = elem.find(".//{*}text")
            title = elem.find("{*}title")
            if ns is not None and ns.text == "0" and text is not None and text.text and not text.text.startswith("#"):
                body = clean_wikitext(text.text)
                before = ""
                for s in sentences(body):
                    for c in chunks(s):
                        yield f"news-{title.text}", c, before[-60:]
                        before += c
            elem.clear()


def clean_wikitext(t):
    t = re.sub(r"\{\{[^{}]*\}\}", "", t)
    t = re.sub(r"\[\[(?:[^|\]]*\|)?([^\]]*)\]\]", r"\1", t)
    t = re.sub(r"\[https?://\S+ ([^\]]*)\]", r"\1", t)
    t = re.sub(r"<[^>]+>|'''?|==+[^=]+==+", "", t)
    lines = [l for l in t.splitlines() if l and not l.startswith(("*", "{", "|", "!", ":", ";", "#", "[["))]
    return "".join(lines)


# 技術・専門の記事とみなすカテゴリの語（事前に固定）。スポーツ・芸能・人物一般は含めない。
TECH_CATEGORY = re.compile(
    r"科学|技術|工学|医学|医療|医薬|薬|疾患|病|化学|物理|生物|生化学|遺伝|数学|統計|情報|計算機|コンピュータ|"
    r"ソフトウェア|プログラミング|インターネット|電子|電気|機械|半導体|通信|鉄道車両|航空機|宇宙|天文|地質|"
    r"気象|法律|法令|条約|裁判|経済|金融|会計|経営|建築|土木|農業|水産|材料|化合物|元素|言語学|心理学")


def wiki_candidates(sources):
    for line in open(sources / "wikipedia-new-v2/articles.jsonl", encoding="utf-8"):
        a = json.loads(line)
        title = a["title"].split(" (")[0]
        tech = any(TECH_CATEGORY.search(c) for c in a.get("categories", []))
        before = ""
        for s in sentences(a["extract"]):
            s = re.sub(r"（[^（）]*）|\([^()]*\)", "", s)  # 読み仮名・生没年などの括弧書きを除く
            for c in chunks(s):
                # 題名を含む文は固有名詞・新語。技術系カテゴリの記事の残りの文は専門用語。それ以外は使わない。
                domain = "names" if title in c else ("tech" if tech else None)
                if domain:
                    yield domain, f"wiki-{a['pageid']}", c, before[-60:]
                before += c


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--sources", type=Path, required=True)
    ap.add_argument("--out", type=Path, required=True)
    ap.add_argument("--judge-per-domain", type=int, default=500)
    ap.add_argument("--dev-per-domain", type=int, default=300)
    args = ap.parse_args()
    readers = Readers()
    pools = collections.defaultdict(list)
    stats = collections.Counter()
    seen = set()

    def consider(domain, doc, text, context):
        stats[f"{domain}:seen"] += 1
        if text in seen or not ALLOWED.match(text):
            stats[f"{domain}:filtered"] += 1
            return
        # 壊れたテンプレート由来の文: 同じ並びの繰り返し、または漢字もカタカナもない長いかなの列。
        if re.search(r"(.{10,})\1", text) or (len(text) > 30 and not re.search(r"[一-鿿ァ-ヺ]", text)):
            stats[f"{domain}:junk"] += 1
            return
        seen.add(text)
        a, b = readers.sudachi_reading(text), readers.unidic_reading(text)
        if a != b:
            stats[f"{domain}:reading_disagree"] += 1
            return
        if not re.fullmatch(r"[ぁ-ゖー0-9、。！？!?（）()「」『』・〜～%]+", unicodedata.normalize("NFKC", a).replace("、", "、")):
            stats[f"{domain}:untypable"] += 1
            return
        context = context.replace("\t", " ")
        pools[(domain, split_of(doc))].append(dict(doc=doc, input=a, context_text=context, text=text))
        stats[f"{domain}:accepted"] += 1

    for doc, text, ctx in short_candidates(args.sources):
        consider("short", doc, text, ctx)
    for doc, text, ctx in doc_candidates(args.sources):
        consider("doc", doc, text, ctx)
    for domain, doc, text, ctx in wiki_candidates(args.sources):
        consider(domain, doc, text, ctx)

    args.out.mkdir(parents=True, exist_ok=True)
    summary = dict(stats=dict(sorted(stats.items())), sets={})
    for (domain, split), pool in sorted(pools.items()):
        want = args.judge_per_domain if split == "judge" else args.dev_per_domain
        # 文書ごとの偏りを抑えるため、文書を順に巡って1問ずつ取る（決定的な順序）。
        by_doc = collections.defaultdict(list)
        for item in sorted(pool, key=lambda r: hashlib.sha256(r["text"].encode()).hexdigest()):
            by_doc[item["doc"]].append(item)
        docs = sorted(by_doc, key=lambda d: hashlib.sha256(d.encode()).hexdigest())
        picked = []
        while len(picked) < want and any(by_doc[d] for d in docs):
            for d in docs:
                if by_doc[d] and len(picked) < want:
                    picked.append(by_doc[d].pop(0))
        items = [dict(index=f"{domain}-{split}-{i:04d}", input=r["input"], context_text=r["context_text"],
                      expected_output=expected_for(r["text"], readers.tokens(r["text"])), domain=domain,
                      source_doc=r["doc"])
                 for i, r in enumerate(picked)]
        path = args.out / f"asteria-{domain}-{split}.json"
        path.write_text(json.dumps(items, ensure_ascii=False, indent=1), encoding="utf-8")
        summary["sets"][path.name] = dict(items=len(items), pool=len(pool), docs=len(docs),
                                          sha256=hashlib.sha256(path.read_bytes()).hexdigest())
    (args.out / "build-summary.json").write_text(json.dumps(summary, ensure_ascii=False, indent=1), encoding="utf-8")
    print(json.dumps(summary, ensure_ascii=False, indent=1))


if __name__ == "__main__":
    main()
