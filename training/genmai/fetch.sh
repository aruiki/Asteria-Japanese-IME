#!/usr/bin/env bash
# genmai-310M(Hugging Face の subdiox/genmai-310M、CC BY-SA 4.0)を取得して SHA-256 を検証する(docs/adr/0085)。
# zenz-v2.5-medium を変換用に追加学習したモデル。モデルはコミットしない。取得先は training/genmai/models/。
#
# 使い方: fetch.sh [出力ディレクトリ]
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
out="${1:-$here/models/genmai-310M}"
REPO="subdiox/genmai-310M"
REVISION="143e574c6a25f545770461cc42e3c48baa89bc75"

if command -v sha256sum >/dev/null 2>&1; then
  sha256() { sha256sum "$1" | cut -d' ' -f1; }
else
  sha256() { shasum -a 256 "$1" | cut -d' ' -f1; }
fi

mkdir -p "$out"
while read -r expected name; do
  [ -z "$name" ] && continue
  dest="$out/$name"
  if [ -f "$dest" ] && [ "$(sha256 "$dest")" = "$expected" ]; then
    continue
  fi
  echo "取得: $name" >&2
  curl -fsSL --retry 3 -o "$dest.part" "https://huggingface.co/$REPO/resolve/$REVISION/$name"
  actual="$(sha256 "$dest.part")"
  if [ "$actual" != "$expected" ]; then
    rm -f "$dest.part"
    echo "SHA-256 が一致しない: $name (期待 $expected, 実際 $actual)" >&2
    exit 1
  fi
  mv "$dest.part" "$dest"
done < "$here/genmai-310M.sha256"
echo "genmai-310M を $out に用意した" >&2
