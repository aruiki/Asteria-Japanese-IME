#!/usr/bin/env bash
# 回帰ゴールデン(SPEC 14.1)を製品の既定(Standard)で測り、1 問でも外れたら失敗する。
#
# 使い方: mozc/tools/eval_regression.sh <converter_main.exe> [インストール先(モデルのある所)]
# 環境: OUT(結果の置き場、既定 ${TMPDIR:-/tmp}/asteria-eval)、MIN_ACC(既定 100)。
# 注意: 直した問題だけをセットに入れる。赤い問題を足すと、このゲートは常に失敗する。
set -u
cd "$(dirname "$0")/../.."
M="$1"
if [ -n "${2:-}" ]; then
  I="$2"
elif [ -d "C:/Program Files (x86)/Asteria" ]; then
  I="C:/Program Files (x86)/Asteria"
else
  I="C:/Program Files (x86)/Asteria"
fi
OUT="${OUT:-${TMPDIR:-/tmp}/asteria-eval}"
MIN_ACC="${MIN_ACC:-100}"
mkdir -p "$OUT"
export ASTERIA_LM_PRELOAD=0

if [ ! -d "$I" ]; then
  echo "失敗: モデルのある所が見つかりません: $I" >&2
  echo "      インストール先を第2引数で渡してください(例: \"C:/Program Files (x86)/Asteria\")。" >&2
  exit 1
fi

rc=0
ASTERIA_RUNTIME_DIR="$I" ASTERIA_MODEL_DIR="$I" \
  cargo run --release --locked --bin asteria-tools -- eval-baseline --context \
    --converter-main "$M" \
    --data eval/sets/asteria-regression-golden.json \
    --out "$OUT/standard_regression-golden.json" \
    --min-acc "$MIN_ACC" || rc=$?

exit "$rc"
