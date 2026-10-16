# 精度改善の評価仕様

2026-10-16。利用者指定の `AI-NihongoIME/docs/AI_ACCURACY_IMPROVEMENT_GUIDE.md` を実装へ接続する。製品の安全性・配布条件はAGENTSと既存の受入れ条件を維持する。

## 固定する条件

一問は一回の変換要求。`index`、`input`、`expected_output`（許容表記集合）、`context_text`、`domain` を事前に固定する。測定後に正解集合を広げない。AJIMEE・日常・慣用句・ニュアンスは調整と回帰用、`kotori-heldout.json` は最終報告用とし、重み・閾値・モデルの選択に使わない。表記が曖昧な問題は既存の許容集合で採点し、別途レビューする。

データ・変換器・辞書・モデル・評価器・結果のSHA-256、評価器のgit commitと変更有無、KOTORI環境変数、OS/CPU/GPUをmanifestへ保存する。変換器のビルド元は評価器のgit commitから推定しない。未指定は `unverified`。ビルド元の自己申告だけでバイナリの対応確認が済んだとは扱わない。

プロファイル未指定時は一時プロファイルを使う。明示指定時は再利用されうるため、履歴や設定を揃えた責任は実行者にある。同じ正規化読みで前文が違う問題は別プロセスに分け、読み→文脈マップの上書きを防ぐ。外部 `KOTORI_LM_CONTEXT_MAP` と `KOTORI_LM_PRECEDING` は引き継がない。出力保存時には測定前後でデータ・変換器・辞書・モデル・評価器のハッシュも照合し、途中変更は失敗にする。

## 測定する値と境界

- 第一候補完全一致: 各文節の第一候補を連結し、許容表記集合と比較する。Wilson 95%区間と分母を報告する。
- 出力候補包含: 各文節の観測候補を組み合わせて正解へ到達できるか。正解の接頭辞を追うため全直積を列挙しない。候補表示上限で切れた不在は `unobserved_candidates` とし、辞書欠落と断定しない。
- 文節ごとの上位K到達性: K=5。全文上位5候補の精度や実操作数とは呼ばない。
- AI内部採点: `--case-traces` で一問ずつ実行したstderrを対応づける。`[time] cands` と候補ログ件数が一致したときだけ、正解が内部候補へ入ったか、LLM採点から除外されたか、有限の統合スコアを持つかを記録する。ログ欠落・複数採点は未検証。通常ログへの本文収集は増やさない。
- 正味改善: 同じID・読み・正解・文脈・分野の結果を対応づけ、改善と改悪を別々に数える。並び順が違っても対応し、結果SHA・データSHA・辞書・文脈/要求分離条件の不一致は比較を拒否する。ハードウェアや環境変更は両manifestを見て別途判定する。

出力候補に正解があってもAI内部の全文候補に入るとは限らない。これを区別して最大の誤り群を選ぶ。内部で採点したことはTSFで候補を表示・確定した証拠ではない。

## 再現コマンド

空のモデル/ランタイムディレクトリでMozc対照、同梱モデル/ランタイムでStandardを順に走らせる。親のKOTORI変数を除去し、`KOTORI_LM_PRELOAD=0`、Standardは `KOTORI_LM_BEAMS=4` と `KOTORI_LM_LLM_TOP=4` を固定する。

```text
python mozc/eval_baseline.py <固定converter_main.exe> --context --data <固定データ.json> --out <MozcまたはStandard.json> --stderr <原票.stderr>
python mozc/tools/compare_quality.py <Mozc.json> <Standard.json> --out <比較台帳.json>
python mozc/eval_baseline.py <固定converter_main.exe> --data <同意済み診断.json> --case-traces <原票ディレクトリ> --out <診断結果.json>
python -m unittest discover -s mozc/tools -p "test_*.py" -v
```

`--case-traces` は本文を含む診断原票を保存する。公開しない。毎回モデルを読み込むため、このモードの所要時間やp95を通常入力の性能として使わない。

## 残る製品判定

要求IDによるTSFの入力→採点→表示→Space→確定の追跡、期限内反映率、修正操作数、意味を変える重大誤変換の判定、低メモリ/CPUのみ/即時Space/障害時フォールバック、RAM/VRAMは別途測定する。本評価器はこれらをPASSにしない。既存のStandard AJIMEE90%以上を維持し、変更の前後比較・未使用データ・実入力で採用を決める。数値閾値を結果に合わせて下げず、最終完成は独立検証に委ねる。
