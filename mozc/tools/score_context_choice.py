"""文脈に合った語を選べたかを採点する（build_context_choice.py の評価セット、session_eval.py の結果）。

文の中の文脈の組（asteria-choice-*）は、文全体の一致に加えて、その問の語を選べたか（target を含み、組の相手の語
rival を含まない）と、組の両方を選べたか（組の正解）を数える。いつも片方の意味を選ぶ IME は、組の正解が 0 になる。
長い文章（asteria-passage-*）は、文全体の一致と、文章の 4 つの切れ端がすべて合った文章の数を数える。

使い方: python mozc/tools/score_context_choice.py <評価セット.json> <session_eval の結果.json> [...の組を続ける]
"""
import collections
import json
import sys


def score(set_path, result_path):
    items = {r["index"]: r for r in json.load(open(set_path, encoding="utf-8"))}
    rows = {r["index"]: r for r in json.load(open(result_path, encoding="utf-8"))}
    s = collections.Counter()
    groups = collections.defaultdict(list)
    for k, it in items.items():
        r = rows.get(k)
        if r is None or r.get("skipped"):
            s["skipped"] += 1
            continue
        top = r.get("top1", "")
        ok = bool(r.get("ok"))
        s["items"] += 1
        s["sentence_ok"] += ok
        if "target" in it:
            # 対象の語をひらがなで書いた形も正解にする（かけた／掛けた は好みの違いで、取り違えではない）。
            written = it["target"] in top or (it.get("target_reading") and it["target_reading"] in top)
            word_ok = bool(written) and not any(v in top and v not in it["target"] for v in it["rival"])
            s["word_ok"] += word_ok
            groups[it["pair"]].append(word_ok)
        else:
            groups[it["passage"]].append(ok)
    s["groups"] = len(groups)
    s["groups_all_ok"] = sum(all(v) for v in groups.values())
    return dict(s)


def main():
    args = sys.argv[1:]
    for set_path, result_path in zip(args[::2], args[1::2]):
        s = score(set_path, result_path)
        n = max(1, s.get("items", 0))
        line = f"{result_path}: 文 {s.get('sentence_ok', 0)}/{n}"
        if "word_ok" in s:
            line += f"、語 {s['word_ok']}/{n}、組の両方 {s['groups_all_ok']}/{s['groups']}"
        else:
            line += f"、文章の全部 {s['groups_all_ok']}/{s['groups']}"
        print(line)


if __name__ == "__main__":
    main()
