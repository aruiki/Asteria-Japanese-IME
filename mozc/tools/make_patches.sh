#!/usr/bin/env bash
# Mozc の作業ツリーの変更から mozc/patches/0002・0003 を作り直す(0001 はそのまま)。
#
# 使い方(WSL などの bash): mozc/tools/make_patches.sh <Mozc の作業ツリー> <素の Mozc の clone>
#   作業ツリー: google/mozc a069a88d に 0001〜0003 を当てて手を入れたもの(Windows なら /mnt/c/... で指す)
#   基点: 作業ツリーとは独立した clone / bare clone(a069a88d を含む)。作業用 clone は毎回一時フォルダへ作る。
# モデル(*.gguf)、生成連語辞書、third_party、bazel の出力は含めない。インストーラと同梱物(src/win32/installer、
# src/data/kotori)は 0003、それ以外は 0002 に入れる。最後に素のコミットに 0001〜0003 が順に当たるか確かめる。
set -euo pipefail
R="$(cd "$(dirname "$0")/../.." && pwd)"
MZ="$1"
BASE_REPO="$2"
BASE=a069a88d4cb5c011de0f9aebb6c149a1c808d904
SOURCE_REAL="$(cd "$MZ" && pwd -P)"
BASE_REAL="$(cd "$BASE_REPO" && pwd -P)"
if [ "$SOURCE_REAL" = "$BASE_REAL" ]; then
  echo "基点には作業ツリーとは独立したcloneまたはbare cloneを指定する" >&2
  exit 1
fi
WORK="$(mktemp -d "${TMPDIR:-/tmp}/kotori-patch.XXXXXX")"
echo "patch scratch: $WORK"
cd "$MZ"
FILES=$( (git -c core.quotepath=off diff --name-only $BASE; git -c core.quotepath=off ls-files --others --exclude-standard) |
  grep -v '\.gguf$' | grep -vE '^src/data/kotori/(fast_collocations\.(txt|json)|NOTICE-fast-collocations\.txt|short_lexicon\.(tsv|json)|NOTICE-short-lexicon\.txt)$' |
  grep -v '^src/third_party' | grep -v '^src/bazel-' | sort -u)
echo "$(echo "$FILES" | wc -l) files"
# 作り直す clone は 1 行ずつ確かめる(&& でつなぐと set -e が効かず、失敗すると Mozc の作業ツリーのまま
# 続けて、ファイルを自分自身に写して空にしてしまう。2026-10-02 に起きた)。
# 既存の作業用 clone は他の作業が使っていることがあるので削除しない。
git clone -q "$BASE_REPO" "$WORK"
cd "$WORK"
WORK_REAL="$(pwd -P)"
if [ "$WORK_REAL" = "$SOURCE_REAL" ] || [ "$WORK_REAL" = "$BASE_REAL" ] || [ "$WORK_REAL" = "$R" ] || [ "$WORK_REAL" = / ]; then
  echo "作業用の clone に移れていない" >&2
  exit 1
fi
git checkout -q $BASE
git -c user.email=k@k -c user.name=k commit -q --allow-empty -m base
git apply "$R/mozc/patches/0001-kotori-branding.patch"
git add -A
git -c user.email=k@k -c user.name=k commit -q -m 0001
for f in $FILES; do
  mkdir -p "$(dirname "$f")"
  case "$f" in
    # バイナリはそのまま写す(改行を消すと壊れる)
    *.bmp|*.ico|*.png|*.jpg|*.dll|*.exe) cp "$MZ/$f" "$f" ;;
    *) tr -d '\r' < "$MZ/$f" > "$f" ;;
  esac
done
git add -A
git diff --cached --binary -- . ':!src/win32/installer' ':!src/data/kotori' > "$R/mozc/patches/0002-kotori-lm-rerank.patch"
git diff --cached --binary -- src/win32/installer src/data/kotori > "$R/mozc/patches/0003-kotori-bundle-model.patch"
git diff --cached --stat | tail -1
if [ "$(pwd -P)" != "$WORK_REAL" ] || [ ! -d "$WORK_REAL/.git" ]; then
  echo "作業用cloneの絶対パスを確認できないためreset/cleanを実行しない" >&2
  exit 1
fi
git reset -q --hard $BASE
git clean -fdq
for p in "$R"/mozc/patches/*.patch; do
  git apply --check "$p"
  git apply "$p"
  echo "ok $(basename "$p")"
done
