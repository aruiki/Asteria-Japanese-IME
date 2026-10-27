"""2 つの session_eval の結果を 1 問ずつ比べ、正しかった問が外れたもの(回帰)と新しく正しくなったものを出す。

使い方: python mozc/tools/compare_runs.py <前の結果フォルダ> <後の結果フォルダ> 名前 [名前 ...]
  名前は <フォルダ>/<前の接頭辞><名前>.json の <名前>。--before-prefix / --after-prefix で接頭辞を指定する。
"""
import argparse
import json
from pathlib import Path


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("before", type=Path)
    ap.add_argument("after", type=Path)
    ap.add_argument("names", nargs="+")
    ap.add_argument("--before-prefix", default="")
    ap.add_argument("--after-prefix", default="")
    args = ap.parse_args()
    total_lost = 0
    for name in args.names:
        a = {r["index"]: r for r in json.load(open(args.before / f"{args.before_prefix}{name}.json", encoding="utf-8"))}
        b = {r["index"]: r for r in json.load(open(args.after / f"{args.after_prefix}{name}.json", encoding="utf-8"))}
        lost = [k for k in a if a[k].get("ok") and k in b and not b[k].get("ok")]
        won = [k for k in a if not a[k].get("ok") and not a[k].get("skipped") and k in b and b[k].get("ok")]
        total_lost += len(lost)
        print(f"{name}: {sum(bool(r.get('ok')) for r in a.values())} → {sum(bool(r.get('ok')) for r in b.values())}"
              f"(外れた {len(lost)}、正しくなった {len(won)})")
        for k in lost:
            print(f"  外れた {k}: {a[k]['top1']} → {b[k]['top1']}")
    print(f"回帰の合計: {total_lost}")


if __name__ == "__main__":
    main()
