"""日常表記の使用比率の表(build_kana_preference.pyのTSV)から、LmRewriterに組み込む表を作る。

多数派の割合が --min-share 以上(事前に0.7で固定)の組だけを、長い表層から順に出す。
使い方: python mozc/tools/gen_notation_table.py <kana_preference.tsv> <kana_preference.json> <出力.inc>
"""
import json
import sys


def esc(s):
    return s.replace("\\", "\\\\").replace('"', '\\"')


def main():
    tsv, meta_path, out = sys.argv[1:4]
    min_share = 0.7
    meta = json.load(open(meta_path, encoding="utf-8"))
    rows = []
    for line in open(tsv, encoding="utf-8"):
        minor, major, share, n, pos = line.rstrip("\n").split("\t")
        if float(share) >= min_share:
            rows.append((minor, major, float(share), int(n)))
    rows.sort(key=lambda r: (-len(r[0]), -r[2], r[0]))
    with open(out, "w", encoding="utf-8", newline="\n") as f:
        f.write("// 生成ファイル(mozc/tools/gen_notation_table.py)。手で編集しない。\n")
        f.write(f"// 出典: {', '.join(meta['sources'])}; {meta['documents']}; {meta['tokenizer']}\n")
        f.write(f"// 元のTSV SHA-256: {meta['sha256']}; 多数派の割合 >= {min_share}; {len(rows)}件\n")
        for minor, major, share, n in rows:
            f.write(f'{{"{esc(minor)}", "{esc(major)}", {share:.4f}f}},  // {n}\n')
    print(len(rows))


if __name__ == "__main__":
    main()
