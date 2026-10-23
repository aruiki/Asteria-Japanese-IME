"""imebench の結果(問番号<TAB>確定文字)を、lenient_score.py が読む形(top1/expected/ok)へ変換する。

使い方: python eval/imebench/to_rows.py <評価セット.json> <imebench結果.tsv> <出力.json>
打てない文字で除いた問(結果に番号がない問)は skipped として残し、採点から外す。
"""
import json
import sys
import unicodedata


def main():
    data, result, out = sys.argv[1:4]
    items = json.load(open(data, encoding="utf-8"))
    got = {}
    for line in open(result, encoding="utf-8"):
        if "\t" in line:
            k, v = line.rstrip("\n").split("\t", 1)
            got[k] = v
    rows = []
    for i, it in enumerate(items):
        if str(i) not in got:
            rows.append(dict(index=it["index"], skipped=True))
            continue
        top1 = got[str(i)]
        # imebenchの方針どおり、全角/半角の違いはNFKCで揃えて完全一致とみなす。
        ok = unicodedata.normalize("NFKC", top1) in {unicodedata.normalize("NFKC", e) for e in it["expected_output"]}
        rows.append(dict(index=it["index"], input=it["input"], top1=top1, expected=it["expected_output"], ok=ok))
    json.dump(rows, open(out, "w", encoding="utf-8"), ensure_ascii=False, indent=1)


if __name__ == "__main__":
    main()
