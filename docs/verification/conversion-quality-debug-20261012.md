# 内部変換の初回デバッグ（2026-10-12）

対象main7cf6239f39e6acb78e77e4db046841f40f6e3734。Mozc a069a88dへ現行3パッチを適用済みの専用mzsupportでビルド/修正。公開MSI/利用者の入力設定は変更しない。ADR0059、カード75。

## 再現と修正

KOTORI_LM_PRELOAD=0、beams4/LLM上位4、同じsmall/medium/Q5_K_Mモデル、RTX3060、各評価専用プロファイル、前の文あり。runtime DLLは旧英数字のProgram Filesと日本語を含むMSI展開先でllama.dll SHA11b7b693e5ba137a88be44b1cd2cd21dda64e046a333253acdff3d23bda33d7c一致。
修正前の日本語runtimeパスはモデル採点ログがなくMozcと全出力一致。日常65/81、AJIMEE102/200。これをStandardのモデル精度とは扱わない。
環境変数をUnicode API→UTF-8へ統一し、文脈/状態のstreamをMozcのUTF-8パス対応に変更。
修正前converter SHA44a3233aa94cba97176f39c33ed0c2d0b9e65a6293e3fa201a9be336afe8868a。
修正後converter SHAe8ccf9915f7073e232b6cf6e90d91bcf9402cd5534907c4920d3839e167acb54。
修正後はruntime日本語パス＋文脈TSVに日本語/絵文字の保存先を実使用。GPUあり/LLMありを採点時間ログで確認。

| セット | Mozc | 英数字パスの修正前Standard | 日本語パスの修正後Standard |
| --- | ---: | ---: | ---: |
| 日常 | 65/81 | 79/81 | 79/81 |
| AJIMEE | 102/200 | 183/200 | 183/200 |
| 慣用句 | 22/25 | 24/25 | 24/25 |
| ニュアンス | 11/20 | 17/20 | 17/20 |
| 表記 | 52/60 | 58/60 | 58/60 |

386問すべてで修正前の英数字対照と出力一致。AJIMEE91.5%維持。日本語パス修正後の日常中央値52ms/p9569ms、AJIMEE84ms/p95223ms。時間は個別要求のCLI値で、実入力の待ち時間や性能改善の主張には使わない。

## テストと抽出

新Windows環境回帰テストは修正前FAIL→後PASS。最初はcleanupのローカル変数参照でコンパイル失敗、テスト修正後の旧getenv実装でFAILを再現。最終lm_rewriter_testは53件PASS（環境変更/Unicodeを含む）。Python工具18件PASS（新4件は問題IDの対応、欠落/重複、期待表記変更、保存ok値への依存を検出）。
Bazel停止後make_patches.shで再生成。素のMozcへ全3パッチが順に適用成功。差分はrewriterのBUILD/cc/h/testのみ。
quality_triage.pyを実結果へ適用し、5セットの改善0/悪化0、25件不一致、Mozc正解→AI不一致3件を抽出。抽出コーパスは再生成物で新規の独立評価ではない。

コマンド: python mozc/tools/quality_triage.py --before target/conversion-quality-debug/ascii-runtime --after target/conversion-quality-debug/unicode-after --baseline target/conversion-quality-debug/unicode-after --out target/conversion-quality-debug/triage.json
原票: target/conversion-quality-debugに全出力/manifest/stderr/修正前FAILログ/抽出JSON。

未検証: 変更したソースの新MSI導入、実アプリTSF、CPU専用機の広範な受入れ。公開v1.1.2の内容は不変。一般CIはPRで成功を確認してからmainへ統合。
quality_triage --require-aiは正常な比較を通し、初回のAI無効だったStandard_dailyに対してAI scoring was not observedで失敗した。数値だけでAI有効と誤認する再発を防ぐ。
