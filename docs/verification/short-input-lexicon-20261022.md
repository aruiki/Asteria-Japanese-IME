# 短文辞書の比較（2026-10-22、初案は不採用）

利用者指定の「変換精度の高い日本語IME」へ向け、単語・短文・前文なしの候補欠落と順位を測った。SudachiDict由来の新規165万語を既存辞書へ加えた初案は、AIなしの既存変換に2件の改悪があるため採用しない。Standardの新短文も第一候補の改善はなかった。公開v2.0.0にはこの辞書を追加していない。

## 固定した対象

- 製品基点: main `769ed9e0c514afdb7c05854bbf051da1fcded373`。作業枝 `codex/short-input-quality-20261022`。評価/生成を1695d90、全量コーパスの取得設定を3d252ffで保存。
- Mozc基準: `785bd83a7f236b49925f7a83764d5beadf3bb378`。追加辞書の比較ソース: `e7f26b4fb8f9f0e3d3db0537663defeb50a4d510`。最適化release build、`--define=asteria_short_lexicon=true`。
- 比較前にEXEとrunfilesを別ディレクトリへ実コピー。基準converter SHA `14284c10d8ef4193eb95dc37e052ce67de4b0457ac223ecdccab0adc3f2be4d2`、比較converter SHA `0ee0598b879d9c269d55385bc5f697c980500536a830894541c78761f81487ba`。
- 基準mozc.data: 19,135,899bytes / `bfbafc9640b07a3c5a51a694413f4ac196e9994ac968331261cbd0624eafccea`。比較mozc.data: 40,833,303bytes / `2155b3213015e6ec7e1736d0b1965fa318ab9d50af876ee918b9df52f650efb0`。
- 追加TSV: 1,653,763語 / 104,619,077bytes / `94ee958727f3d23cf6f4581c20e49d0ec867c742705376feb1a7a72939919c9d`。SudachiDict v20261015のsmall/core/notcore全2,880,855行を処理し、同一読み/表記/品詞の共通語だけで品詞別コスト分位を対応。既存語の上書きはしない。
- 公式ZIP、LEGAL、Apache-2.0、UniDic/NEologd等の帰属を `training/lexicon/sources.json` でSHA固定。これは頻度コーパスを使わない初案である。

短文は変換結果を見る前に、プロジェクト内で作成した開発120件と最終報告120件へ分割した。人手による独立受入れではない。`eval/short-input-20261022.json`に各SHA・用途を固定。参照を結果に合わせて変更せず、最終報告用は未実行・未調整。

## AIなし

| 条件 | 第一候補・基準→候補 | 改善 / 改悪 | 正解候補包含・基準→候補 |
| --- | --- | --- | --- |
| 新短文・前文なし | 111→113 /120 | 2 / 0 | 118→119 |
| AJIMEE | 103→104 /200 | 3 / **2** | 180→183 |
| 慣用句 | 22→23 /25 | 1 / 0 | 各原票を参照 |
| ニュアンス | 11→11 /20 | 0 / 0 | 各原票を参照 |
| 日常 | 69→69 /81 | 0 / 0 | 各原票を参照 |

短文の改善は「東京会場日動→東京海上日動」「炭疽旬刊→炭素循環」。一方、既存AJIMEEで「挑戦した一作→挑戦した一昨」「富野由悠季監督→富野喜幸監督」へ変わった。全体の正解数が増えても、改悪を残したまま採用しない。

「生成エーアイ→生成ＡＩ」は厳密な事前参照「生成AI」と一致しないため、第一候補の正解数へ加えていない。英数字幅の扱いを見た後で緩めていない。

原票: `target/accuracy-guide/short-input-candidate-off-comparison-v1.json`、`short-input-regression-off-v1/summary.json`。既存4セットも両側を新規実行した。

## Standard・GPU AI

公開v2.0.0 MSIから展開した同一4モデルと推論DLLを使用。RTX3060、GPU指定、隔離した新規プロファイル、KOTORI_LM_PRELOAD=0、同期converterのオフライン検査。モデル/辞書/EXE/評価器/入力SHAを開始・終了で照合した。Pの試験/CIファイルには検査中の追記があるため、P全体が検査中ずっとcleanだったとは扱わない。

| 条件 | 第一候補・基準→候補 | 改善 / 改悪 | 正解候補包含・基準→候補 |
| --- | --- | --- | --- |
| 新短文・前文なし | 118→118 /120 | 0 / 0 | 119→119 |
| AJIMEE・前文なし | 175→175 /200 | 0 / 0 | 193→194 |
| AJIMEE・前文あり | 188→188 /200 | 0 / 0 | 195→196 |
| 慣用句・前文なし/あり | 各24→25 /25 | 各1 / 0 | 各24→25 |
| ニュアンス・前文なし/あり | 各17→17 /20 | 0 / 0 | 各原票を参照 |
| 日常・前文なし/あり | 各79→79 /81 | 0 / 0 | 各原票を参照 |

両側×9条件の18実行がexit0。`short-input-standard-v1/plan.json`、`summary.json`、各stdout/stderr/manifestを保存。新短文での第一候補改善や、導入済みTSFのAI寄与を証明した検査ではない。新短文118/120を一般精度の広告へ使用しない。

## ビルドと初案の限界

物理LLVMパス `C:/Users/aruik/mz/src/third_party/llvm` とGit Bashを明示し、R/srcからbuild成功（268.104秒、1341 actions）。初回はworkspace位置、WSL bash選択、junctionのLLVM論理パスで失敗した。失敗ログを保持し、チェックを無効化せず修正した。再利用する実行器は `target/accuracy-guide/run_ag16_bazel_v1.py`。

フラグなしの空filegroupではdatasetの`$(locations)`が成立せず、`7702a998c`で明示的な空TSVへ変更。通常build成功、mozc.dataの全SHAが基準と一致した。ログ `short-lexicon-default-off-build-v3.log`、exec98390はexit0回収済み。

元辞書間のコスト分位だけでは、長い単語と既存の分節列の競合、古い人名表記、実際の使用頻度を扱えていない。評価語を例外登録せず、コーパスの頻度と連語を次の独立した候補規則へ使う。

## 次の頻度実験と残る範囲

Wikipedia日本語由来の `train_wikipedia.jsonl` 全4,131,415,181bytesを取得し、SHA `c477d5d75ee3359949839f56465ba65b2e3c7c1f81b812ef7c5d85bed7323476` を確認した。出典revisionとCC-BY-SA4.0を `training/lexicon/corpus_sources.json` に固定。Common Crawl部分は使わない。

`collect_frequency.py`はNFKC本文をhashで分割し、同一本文を必ず同じworkerで重複除去する。固定SudachiPy0.7.0/full辞書で語彙と2〜3形態素連語を集計し、上限付きCounter→SQLite→ソートmergeで全語彙の一括メモリ展開を避ける。旧Wikipedia dev00は固定範囲SHAを検査して重なる行を除外し、その全入力の近重複も他区間から除外。評価ファイルは読みだけを除外集合へ渡し、参照表記を学習に使わない。

読みはSudachiの優先読みであり、発音を原コーパスと照合した頻度ではない。本文hashの16区分は独立サイト/記事ではない。8MiB試走は35,491行を読み、45,372語/1,603連語を出力した。生成処理3.19秒、取得SHA/展開確認等は別。全量実行は `wikipedia-frequency-full-v1/` で進行中で、まだ採用結果ではない。境界・混入・merge・展開と辞書生成のPython12件成功。

全量頻度を用いた規則の比較、最終未使用評価、現行Google日本語入力との同条件比較、実MSI/active TIP、製品パッチ/CI/配布候補、紹介文/3画像/公式7ページは未完了。AI主体の非同期候補表示、フォールバック、保護欄、ユーザー辞書、二重入力修正、過去の独立FAILを維持する。
