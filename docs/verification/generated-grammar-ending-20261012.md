# AI生成候補の文末文法語保護（2026-10-12）

対象: main b1d73d8（PR158でUnicode修正済み）、Mozc a069a88d＋現行パッチ、専用mzsupport。カード76、ADR0060。

## 再現と修正

抽出済みnotation n42「でんきをけしてね」の正しいMozc候補が、LLM採点で生成「電気を消して寝」に逆転する。読み一致検査には通る。
内部境界の機能語だけを使った初案は、54テストが通っても実例0/1、386問の表記58/60のまま。失敗原票grammar-afterは残す。
実際のrid423はFunctional/SuffixWordだが、最後の内部境界value/contentがともに「消してね」で機能語空だった。辞書の読み/表記/右品詞の一致で末尾を補完し、生成候補のみを保護。対象1/1に回復。

## 修正前後の調整用評価

RTX3060、Standard、beams4/LLM上位4、PRELOAD=0、前の文あり、同じモデル/datasetハッシュ。専用の空プロファイル、日本語DLLパスと日本語/絵文字の文脈TSVを使用。

| セット | 前 | 後 |
| --- | ---: | ---: |
| 日常 | 79/81 | 79/81 |
| AJIMEE | 183/200 | 183/200 |
| 慣用句 | 24/25 | 24/25 |
| ニュアンス | 17/20 | 17/20 |
| 表記 | 58/60 | 59/60 |

386問で改善1/悪化0。AJIMEE91.5%以上を維持。残る不一致は24例、Mozc正解→AI不一致は「書く→描く」「うかがえる→伺える」の2例。これらには今回の文末保護で解決したとは扱わない。
比較工具quality_triage --require-aiで問題ID/データSHA/文脈/期待表記/採点ログを確認。
原票target/conversion-quality-debug/grammar-final、grammar-final-triage.json。比較先unicode-after。修正後converterのSHAはgrammar-finalのmanifestに保存。

## 最終評価

採用判断後にheldout300問だけを比較。調整に使用せず、その結果を見て規則を修正しない。
元mainの2ソースをversioned patchから復元してビルド/評価後、保存した修正ソースを戻して再ビルド/評価。try/finallyと同時変更検出付き。前後のexe/ソースコピーをheldout-pairedに保存し、モデル/dataset/条件とハッシュをmanifestで記録。
前289/300=96.3%、後289/300=96.3%、改善0/悪化0。分野ごとにもwork50/50、chat47/50、tech50/50、name47/50、num45/50、news50/50で一致。CLIの中央値は前後59ms、p9577ms。速度向上の主張には使わない。
heldout-paired/comparison.jsonに結果。評価後に空の生成なら不要な検索を省く条件だけを追加し、最終54テストを再実行してPASS。

## 最終確認と境界

複合候補の機能語をcontentから辞書で補完し、名詞/別品詞の同音語を誤って固定しないテストを追加。lm_rewriter_test54件PASS。
Bazel停止後make_patches.shで再生成、素のMozcに全3パッチが順に適用成功。temporaryの対象文字列デバッグ出力は削除済み。
一般CIはPRで確認してからmain統合。Mozcクラウドビルドはローカル指定でキャンセルし成功としない。公開MSI、利用者の設定/履歴は変更しない。CLI評価を実アプリTSFの精度/通知/導入の受入れと混同しない。