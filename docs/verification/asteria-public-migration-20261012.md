# Asteria公開名移行の検証（2026-10-12）

対象: カード73 / ADR0058。移行前main3457c07e4b20e1a878969389777349c589d13201。

## 公開メタデータ

GitHubリポジトリID1393298702を維持し、aruiki/Asteria-Japanese-IMEへ改名。説明/Topics更新。旧Kotori-AI-Japanese-IMEおよびKotoriIME-japanese-のURLは新URLへ転送されHTTP200。
23件のリリースは表示名にAsteriaを付与、URLのみ正規化。全GitタグSHAとリリースID/公開日/作成日/区分/参照コミットを移行前後で一致確認。既存添付のID/名前/サイズ/digest一致、ダウンロード数非減少。Stars1/Forks0を保持。Latestはv1.1.2、release409720573のまま。
既存Kotori64.msiは保持。最新版にAsteria64.msiと専用sha256を追加。旧URL経由のlatest/downloadは両名ともHEAD200。最新本文はmozc/release-notes.mdと一致。

## 配布とソース

Asteria64.msiを公開先から再取得。check_release.py v1.1.2 --work target/asteria-migration/check-release（実際には絶対パス指定）は終了0・全項目OK。新名優先とv1.1.1の旧名フォールバックを実確認。
SHA a968312279d33c67679436d071d78409e6c74bdebf1824e475895fe9266e420e。3.34.6338.100、1,569,386,496bytes。以前のKotori64.msiとバイト一致、未署名。同梱物/ライセンス/除外物/サーバー版一致、前版6336よりBUILDが大きい。
MSIの再ビルド、利用者のインストールや案内状態の変更はしていない。

Pythonリリース工具の14テスト成功（4件の添付名/ハッシュ対テストを含む）。Mozcの応援リンクを新URLへ変更し、ローカルWindowsで//gui/tool:kotori_support_reminder_testの8テスト成功。初回はworkspaceの1階層上で実行して失敗、srcからの実行で成功。Bazel停止後make_patches.shで再生成、全3パッチが素のMozc a069a88dへ順に適用できた。
公開中6338の応援リンクは旧URLから転送される。今回の正規URLソースは次回ビルドで反映される。公開済みMSIのソースタグv1.1.2/e94a9b23は維持。

## 証拠と限界

target/asteria-migrationに移行前後API、タグ一覧、比較スクリプト、公開MSIと展開結果を保持。旧版の測定/受入れ記録は改変しない。今回の展開/単体テストを実導入・IME実入力・Windows通知の実表示検証としては扱わない。
一般CIはPRで確認してからmainへ統合する。Mozcクラウドビルドはローカルビルド指定によりキャンセルし、成功扱いしない。