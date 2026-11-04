#!/usr/bin/env python3
"""回帰ゴールデン(SPEC 14.1)へ、直した誤変換を1問ずつ足す。

評価器(mozc/eval_baseline.py)が読む JSON 配列へ追記する。形式は評価セットと同じ。
- index は "rg-0001" の連番を自動で振る。
- 同じ読み・同じ前の文の組は重複として足さない(カタカナとひらがなは同じとみなす)。
- 追加するのは、直ったことを確認した問題だけ。赤いまま足すとゲートが常に失敗する。
- 前の文が要る問題は context_text を残す(無いと再現しない)。

使い方:
  python mozc/tools/regression_add.py --input くうかんこうへんちょうき --expected 空間光変調器
  python mozc/tools/regression_add.py --input ないと --expected ナイト --expected ないと --context "前の文"
  python mozc/tools/regression_add.py --check
  python mozc/tools/regression_add.py --list

終了コード: 0 成功、1 入力か既存データが不正、2 ファイル入出力の失敗、3 重複(追加していない)。
"""
import argparse
import json
import os
import re
import sys
import tempfile
from pathlib import Path

DEFAULT_FILE = (Path(__file__).resolve().parents[2] / "eval" / "sets"
                / "asteria-regression-golden.json")
INDEX_PATTERN = re.compile(r"^rg-(\d+)$")


def kata_to_hira(text: str) -> str:
    """評価器(eval_baseline.py)と同じ正規化。重複の判定に使う。"""
    return "".join(chr(ord(c) - 0x60) if "ァ" <= c <= "ヶ" else c for c in text)


def load(path: Path) -> list:
    """評価セットを読み、形式を確かめる。"""
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError:
        raise ValueError(f"評価セットがありません: {path}")
    except json.JSONDecodeError as error:
        raise ValueError(f"JSON として読めません: {path}: {error}")
    validate(data)
    return data


def validate(items) -> None:
    """評価器の validate_items と同じ条件と、この道具の重複条件を確かめる。"""
    if not isinstance(items, list) or not items:
        raise ValueError("評価セットは空でない配列が必要です")
    indexes, pairs = set(), set()
    for item in items:
        if not isinstance(item, dict):
            raise ValueError("各問題はオブジェクトが必要です")
        index = item.get("index")
        if not isinstance(index, (str, int)) or isinstance(index, bool) or index in indexes:
            raise ValueError(f"問題IDが不正または重複しています: {index!r}")
        indexes.add(index)
        for key in ("input", "context_text"):
            value = item.get(key, "")
            if not isinstance(value, str) or any(c in value for c in "\r\n\t\0"):
                raise ValueError(f"{key} が不正です: {index!r}")
        if not item.get("input", "").strip():
            raise ValueError(f"読みが空です: {index!r}")
        expected = item.get("expected_output")
        if not isinstance(expected, list) or not expected or any(
                not isinstance(value, str) or not value for value in expected):
            raise ValueError(f"空でない正解表記集合が必要です: {index!r}")
        pair = (kata_to_hira(item["input"].strip()), item.get("context_text", ""))
        if pair in pairs:
            raise ValueError(f"同じ読み・同じ前の文が重複しています: {index!r}")
        pairs.add(pair)


def next_index(items) -> str:
    """rg- の連番の次の番号を返す。"""
    numbers = [int(match.group(1)) for item in items
               if isinstance(item.get("index"), str)
               and (match := INDEX_PATTERN.match(item["index"]))]
    return f"rg-{max(numbers, default=0) + 1:04d}"


def add(items, input_text: str, expected, context: str = "") -> str:
    """1問足して、振った index を返す。重複なら ValueError。"""
    input_text = input_text.strip()
    context = context or ""
    if not input_text:
        raise ValueError("読みが空です")
    if any(c in input_text + context for c in "\r\n\t\0"):
        raise ValueError("読みと前の文に改行・タブ・NUL は使えません")
    expected = [value.strip() for value in expected]
    if not expected or any(not value for value in expected):
        raise ValueError("空でない正解を1つ以上指定してください")
    pair = (kata_to_hira(input_text), context)
    if any((kata_to_hira(item["input"].strip()), item.get("context_text", "")) == pair
           for item in items):
        raise ValueError("同じ読み・同じ前の文が既にあります(重複)")
    index = next_index(items)
    items.append({"index": index, "context_text": context, "input": input_text,
                  "expected_output": expected})
    return index


def write(path: Path, items) -> None:
    """同じフォルダの一時ファイルへ書いてから置き換える(途中で壊れたファイルを残さない)。"""
    path.parent.mkdir(parents=True, exist_ok=True)
    descriptor, temporary = tempfile.mkstemp(prefix=path.name + ".", suffix=".tmp",
                                             dir=path.parent)
    try:
        with os.fdopen(descriptor, "w", encoding="utf-8", newline="\n") as stream:
            json.dump(items, stream, ensure_ascii=False, indent=1)
            stream.write("\n")
        os.replace(temporary, path)
    except BaseException:
        try:
            os.unlink(temporary)
        except OSError:
            pass
        raise


def describe(item) -> str:
    context = item.get("context_text", "")
    head = f'{item["index"]}  {item["input"]}'
    if context:
        head += f"  [前: {context}]"
    return head + "  =>  " + " / ".join(item["expected_output"])


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(
        description="回帰ゴールデンへ直した誤変換を足す(SPEC 14.1)")
    parser.add_argument("--file", type=Path, default=DEFAULT_FILE,
                        help=f"評価セット(既定: {DEFAULT_FILE})")
    parser.add_argument("--input", help="読み(ひらがな・カタカナ)")
    parser.add_argument("--expected", action="append",
                        help="正解の表記(複数回指定できる)")
    parser.add_argument("--context", default="", help="前の文(要る問題だけ)")
    parser.add_argument("--check", action="store_true", help="既存の評価セットを検査する")
    parser.add_argument("--list", action="store_true", help="既存の問題を並べる")
    parser.add_argument("--dry-run", action="store_true", help="追加せずに、足す内容だけ出す")
    args = parser.parse_args(argv)

    try:
        if args.check or args.list:
            items = load(args.file)
            if args.check:
                print(f"OK: {len(items)} 問  {args.file}")
            else:
                for item in items:
                    print(describe(item))
            return 0
        if not args.input or not args.expected:
            parser.error("追加には --input と --expected が必要です(--check か --list も選べます)")
        items = load(args.file) if args.file.exists() else []
        add(items, args.input, args.expected, args.context)
        if args.dry_run:
            print(f"追加します(まだ書いていません): {describe(items[-1])}")
            return 0
        write(args.file, items)
        print(f"追加しました: {describe(items[-1])}")
        print(f"{len(items)} 問  {args.file}")
        return 0
    except ValueError as error:
        message = str(error)
        duplicate = "重複" in message
        print(f"{'重複' if duplicate else '失敗'}: {message}", file=sys.stderr)
        return 3 if duplicate else 1
    except OSError as error:
        print(f"失敗: ファイルを読み書きできない: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())

