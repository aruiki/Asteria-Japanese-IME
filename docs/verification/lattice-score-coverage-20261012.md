# 辞書候補の採点取りこぼし修正（2026-10-12）

対象: v1.1.3公開commit dc9323cからcodex/accuracy-999-ranking、専用Mozc mzsupport。カード78、ADR0063。公開MSIは変更しない。

## 公開の完了

v1.1.3を2026-10-12 21:30:24 JSTに正式Latestとして公開。公開後check_release.pyの全項目成功。MSI3.34.6431.100、SHA b7dbaee7e99dcea16544b169cadb7ea1e0c5b66f1225cbc01a044ad4293688d5、1,569,517,568bytes。PR160/161はCI成功後にmain統合。新たな開発はこの公開と再取得の後に開始した。

## 実験の採用・却下

重み0.7/1.0/1.2を同じ7調整セット466問で比較。期待値・入力を変更せず、原データSHAとプロファイルを記録。1.0と1.2は既存正解を悪化させた。ガード付き1.0は調整では改善したが、採用後の独立評価で288/300となり既定に採用しない。対応コードを除去し、失敗原票heldout-final.jsonと実験ソースを保持した。

次の修正は、調整用AJIMEE2018のログで既に確認していた正解候補の未採点が根拠。独立の問題内容を取り出して調整していない。採用判断をcoverage-adoption-before-independent.jsonに固定してから最終評価を行った。

## 最終実装

既存の上位枠/元の辞書1位を維持し、近い辞書候補最大2件を補う。配分0.7・誤入力閾値・保護条件は維持。ゼロ予算・重複・不正index/NaN/inf・支持の低い候補を追加しないテストを実行。

| 調整セット | 公開版の配分 | 修正後 |
| --- | ---: | ---: |
| AJIMEE | 183/200 | 184/200 |
| 日常 | 79/81 | 79/81 |
| 慣用句 | 24/25 | 24/25 |
| ニュアンス | 17/20 | 17/20 |
| 表記 | 59/60 | 59/60 |
| 誤入力 | 16/40 | 16/40 |
| 同音語 | 38/40 | 38/40 |

合計416→417/466、改善1/悪化0。追加文脈対照24/24→24/24。意味の誤りと厳密表記の不一致を混同しない。この集計は一般の実入力の正解率ではない。
独立heldout300は289→289=96.3%、改善0/悪化0/出力変更0。各分野も不変。問題の内容を調整に使用しない。

## 入力操作とビルド

6例×3条件×AI OFF/ON、各条件に新規隔離プロファイル。旧配分0.7の対照と新実装を同じモデル/条件で比較。即Space/待機/通常打鍵で「各３番」→「拡散板」、他15 ON出力は不変。OFF18出力はすべて不変。読み欠落などのdefect 0。Enterによる確定はこの再現では未検査、Windows TSFの実入力とは区別する。
最終lm_rewriter、kotori_diagnostics、sessionの3対象PASS（lmはcached、他2対象を実行）、converter/session_handler build成功、Python37テストPASS。実験版のテスト結果を最終版へ流用しない。

パッチ3本を再生成し、新規checkoutへの適用がすべて成功。検証ソース130ファイルの一致をfinal-patch-parity.jsonへ記録した。

## 原票と残る境界

target/accuracy-999のtuning-sources.json、weight-*、guarded/heldout-final、coverage-final/coverage-controls/coverage-independent-report、各比較JSON、native-baseline/native-coverage、各build/test log。モデルは公開MSIから展開したもの、旧評価モデルとSHA一致。
データセット・モデル・原応答はGitへ追加しない。今回の修正の新MSI作成、実アプリのAI ON/OFF、CIとmain統合は別に確認する。99.9%は未達成であり、今回の独立評価も96.3%のまま。
