# v1.1.4の公開準備と紹介資料更新（2026-10-19）

製品リリース前の記録。PRは[#168](https://github.com/aruiki/Asteria-Japanese-IME/pull/168)。実アプリ全般の受入れや独立verifierの合格をこの記録で代替しない。

## 追加モデルの公開と取得

追加生成専用モデルを[モデル資産のプレリリース](https://github.com/aruiki/Asteria-Japanese-IME/releases/tag/model-native-generator-20261017)として公開した。製品MSIのリリースとは別で、Latestにはしていない。

- GGUF：`zenz-v2.5-native-generator-20261017-q8_0.gguf`、98,970,560bytes、SHA-256 `49112e8b52e5da18fdb74d066111d44f92beaffe06c3d559807fb1aab1e38924`。
- GGUF・checksum・NOTICE・MODEL-ASSETの4資産すべて、GitHubのsize/digestとローカルの実バイトが一致。
- `python -X utf8 training/contrastive/fetch_generator.py --out target/accuracy-guide/published-generator-redownload-v1`を`--source`なしで実行し、固定公開URLからの新規取得とSHA検査がexit0。
- 帰属・CC BY-SA 4.0・改変内容を掲載。元モデル3件と既存の採点・保護条件は保持。重み・コーパスをGitへ追加していない。

原票は`target/accuracy-guide/published-generator-verification-v1.json`。アップロード前後でモデルのSHAは同一。公開状態metadataのみをsource `b5b144bd9ca717d2768dee7d52fa904ec7a4a0f8`で更新し、生成パッチの再適用後139ファイルがソースと一致、実ソースのwhitespace検査も成功した。原票`published-generator-patch-restore-v1.json`。

最初のパッチ生成v1は基点cloneの相対パスがソース側で解決されて失敗した（exit128、生成前に停止）。絶対パスでv2を実行し、適用と照合に成功。元の失敗ログは保持した。

ローカル6432 MSI（SHA `068ed5c56cadd644f45557593b943f2ec2997b838ca21a4df0341e8df2598c28`）はsource449828・公開状態更新前metadataの固定成果物である。これを新metadataを含むMSIとは扱わない。CIでは公開済みmetadataから新規ビルドする。[ローカル検証](accuracy-normal-conversion-20261019.md)。

## 紹介資料

README・ブランドプレビュー・accuracy/compare/examples/lightness図を更新。主要表示を次期版Standardの188/200（94.0%）へ変更し、AI準備後・前文あり・オフライン開発評価の条件を併記した。前文なし175/200（87.5%）、AIなし103/200（51.5%）も表示する。

旧Kotori88.4%は198問の2026-10-01実入力比較であり、94.0%とは同条件ではない。旧比較のCSVと数値を保持し、日付入りの過去資料へ移した。未測定の品質設定や他IMEの最新値を補間しない。

図版用JSONへ固定原票の正解数12件・改善例4件を転記し、原票と全件一致を確認。CPU/GPUの入力応答値も実原票から転記。Pillow生成の幅・描画範囲チェック、ローカルリンク検査、差分のwhitespace検査が成功。実PNGと500px/360pxの縮小表示で文字・値・条件・棒の比率・重なりを目視確認した。プレビューは`target/accuracy-guide/branding-20261019-preview/`。

利用者向けの[測定条件](../BENCHMARKS.md)、[図版データ](../images/evaluation-2026-10-19.json)、[制作記録](../design/2026-10-asteria/generation.md)を参照。

## CIと残る条件

PR作成後、8件のチェックが登録されたことを確認。最初の7件（Rust/Linux・Windows、MSRV、ライセンス、TIP32/64、Python）は成功、Windows MSIは進行中。未登録のチェックを成功に数えていない。紹介資料更新後の最終コミットの結果は続けて確認する。

新MSIの実導入・実アプリAI ON/OFF・複数アプリ・保護欄/フォーカス・削除、低性能実機、独立verifierは未完了。ローカルの正規computer-use初期化は認証1326で失敗し、別のUI操作へ迂回していない。公開製品を新リリースへ置き換える条件が揃ったとは扱わない。
