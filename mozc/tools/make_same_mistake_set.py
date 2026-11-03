"""「同じ間違いを二度しない」を確かめる場面を作る(2026-11-03、docs/adr/0081 の目標)。

session_eval.py の結果から、同音語の取り違え(rival を選んで target を選ばなかった問)を拾い、learning_flow.py の
場面にする。利用者がその語を選び直して確定し(teach)、同じ文をもう一度変換したら選び直した語になること(check の
same_cue、入力は teach と同じ)を確かめる。手がかりのない別の文(no_cue)は、セットの同じ組のもう片方の文を使う
(選び直しが別の文脈に広がらないこと)。

使い方: python mozc/tools/make_same_mistake_set.py <セット.json> <session_eval の結果.json> <出力.json>
"""
import json
import sys


def main():
    items = {x["index"]: x for x in json.load(open(sys.argv[1], encoding="utf-8"))}
    rows = json.load(open(sys.argv[2], encoding="utf-8"))
    by_pair = {}
    for x in items.values():
        by_pair.setdefault(x.get("pair"), []).append(x)
    scenes = []
    for r in rows:
        it = items.get(r["index"])
        if not it or r.get("ok") or not it.get("target") or not it.get("rival"):
            continue
        top = r.get("top1", "")
        rival = next((v for v in it["rival"] if v in top), None)
        # 取り違えだけ(target が出ず rival が 1 か所だけ出た)。打ち間違い・書き方の違いなどは外す。
        if rival is None or it["target"] in top or top.count(rival) != 1:
            continue
        other = [x for x in by_pair.get(it.get("pair"), []) if x["index"] != it["index"]]
        check = [{"kind": "same_cue", "input": it["input"], "expect": it["target"]}]
        if other:
            check.append({"kind": "no_cue", "input": other[0]["input"]})
        scenes.append({"name": f"{it['index']} {rival}→{it['target']}",
                       "teach": {"input": it["input"], "rejected": rival, "chosen": it["target"]},
                       "check": check})
    json.dump(scenes, open(sys.argv[3], "w", encoding="utf-8"), ensure_ascii=False, indent=1)
    print(f"{len(scenes)} 場面")


if __name__ == "__main__":
    main()
