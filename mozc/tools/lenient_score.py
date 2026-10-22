"""表記ゆれを同一とみなす採点（完全一致と並べて報告する、2026-10-23）。

実際の文から作った評価では、正解は書き手の表記の好みの1つにすぎない（見たり/観たり、わかる/分かる）。
Sudachiの正規化表記(normalized_form、短単位)の並びと読みが、正解のいずれかと一致すれば「表記ゆれ一致」とする。
意味の違う同音語（注射/駐車、効く/聞く）は正規化表記が異なるので正解にならない。
規則は開発用セットの外れを見た後に定めた。判定用セットにはこの規則を固定して一度だけ使う。

使い方（SudachiPyを入れたPythonで）: python mozc/tools/lenient_score.py 結果.json [...]
"""
import json
import sys
import unicodedata

from sudachipy import dictionary, tokenizer

_TOKENIZER = dictionary.Dictionary().create()
_MODE = tokenizer.Tokenizer.SplitMode.A


def key(text):
    text = unicodedata.normalize("NFKC", text)
    ms = _TOKENIZER.tokenize(text, _MODE)
    return tuple(m.normalized_form() for m in ms), "".join(m.reading_form() for m in ms)


def lenient_ok(top1, expected):
    if top1 in expected:
        return True
    k = key(top1)
    return any(key(e) == k for e in expected)


def main():
    for path in [a for a in sys.argv[1:] if not a.startswith("--")]:
        rows = [r for r in json.load(open(path, encoding="utf-8")) if not r.get("skipped")]
        strict = sum(bool(r["ok"]) for r in rows)
        lenient = [r for r in rows if lenient_ok(r["top1"], r["expected"])]
        print(f"{path}: 完全一致 {strict}/{len(rows)}  表記ゆれ一致 {len(lenient)}/{len(rows)}")
        if "--show" in sys.argv:
            for r in rows:
                if not lenient_ok(r["top1"], r["expected"]):
                    print("   ", r["top1"], "|", r["expected"][0])


if __name__ == "__main__":
    main()
