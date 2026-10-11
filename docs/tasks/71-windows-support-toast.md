# 71 Windows標準の応援通知

利用者は独立ダイアログではなくWindows通知を希望。カード69の状態ファイルを引き継ぎ、GitHubボタンは固定URLを開けた時だけ永久停止、いいえは7日延期。閉じる・無応答も7日延期。通知センターからの後日操作はCOM activatorで処理する。Windowsの通知設定に従い、ダイアログへの代替表示はしない。

読むもの: HANDOFF、DEVELOPMENT、ADR0056、gui/tool/kotori_support_reminder、installer_oss_64bit.wxs。

確認: 状態・アクションの単体テスト、Windowsビルド、MSIのAUMID/CLSID/COM登録、実通知と通知センターからの操作。実表示未確認をAPI成功で置き換えない。

## 公開

v1.1.2を2026-10-11に正式版・Latestとして公開。PR153、source/tag e94a9b23b891ccc498ed121f51c8a19cb9ef539f。
一般CI7項目成功、ローカルMSI6338、公開後check_release.py全項目成功。Mozcクラウドはローカル指定でキャンセル。実表示/実機受入れは未実施。
