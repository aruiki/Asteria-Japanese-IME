#!/usr/bin/env bash
# 品質 3 段階 × 評価セット 4 つの Acc@1 を出す(前の文あり)。Mozc 単体より悪くなった問題も出す。
#
# 使い方: mozc/tools/eval_all.sh <converter_main.exe> [インストール先(モデルのある所)]
# Mozc 単体の結果は、モデルのない空のフォルダで動かして作る(最初に 1 回)。
# 結果は $OUT(既定 ${TMPDIR:-/tmp}/asteria-ime-eval)に、品質とセットごとに <品質>_<セット>.json と
# 実行条件(.manifest.json)を残す。評価が 1 つでも失敗したら、最後に終了コード 1 を返す。
set -u
cd "$(dirname "$0")/../.."
M="$1"
I="${2:-C:\Program Files (x86)\Asteria}"
OUT="${OUT:-${TMPDIR:-/tmp}/asteria-ime-eval}"
mkdir -p "$OUT/empty"
export ASTERIA_LM_PRELOAD=0
cargo build --release --locked --bin asteria-tools
TOOLS="$(pwd)/target/release/asteria-tools"
SETS="eval/data/ajimee-bench.json eval/sets/asteria-idioms.json eval/sets/asteria-nuance.json eval/sets/asteria-daily.json"
failed=0
# 1 回の評価。失敗したら覚えておき、残りは続ける。
run() {
  local label=$1 out=$2
  shift 2
  local r
  if r=$("$@" "$TOOLS" eval-baseline --context --data "$d" --out "$out" --converter-main "$M"); then
    echo "$label: $(echo "$r" | head -1)"
  else
    echo "$label: 失敗"
    failed=1
    return 1
  fi
}
for d in $SETS; do
  n=$(basename "$d" .json)
  run "Mozc $n" "$OUT/Mozc_$n.json" env ASTERIA_RUNTIME_DIR="$OUT/empty" ASTERIA_MODEL_DIR="$OUT/empty"
done
# 品質ごとの設定は rewriter/lm_rewriter.cc の QualityOf と同じ(生成の数、LLM で採点する数、zenz)。
# Low の zenz(medium)は、パスに空白があってもよいように別に渡す(区切りは / にする)。
LOW_ZENZ="$(printf '%s' "$I" | tr '\\' /)/genmai-310M-q8_0.gguf"
# Standardは製品の既定値を使う。追加生成モデル同梱時のLLM採点数32も上書きしない。
for q in "Low ASTERIA_LM_K=4 ASTERIA_LM_BEAMS=2 ASTERIA_LM_LLM_TOP=0" "Standard" \
         "High ASTERIA_LM_K=16 ASTERIA_LM_BEAMS=8 ASTERIA_LM_LLM_TOP=32"; do
  set -- $q; name=$1; shift
  zenz=()
  [ "$name" = Low ] && zenz=("ASTERIA_ZENZ_MODEL=$LOW_ZENZ")
  for d in $SETS; do
    n=$(basename "$d" .json)
    run "$name $n" "$OUT/${name}_$n.json" env ASTERIA_RUNTIME_DIR="$I" ASTERIA_MODEL_DIR="$I" "$@" "${zenz[@]}" \
      || continue
    "$TOOLS" compare-runs "$OUT" "$OUT" --before-prefix "Mozc_" --after-prefix "${name}_" "$n"
  done
done
exit $failed
