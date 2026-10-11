# ADR 0057: 応援案内をWindows標準通知へ変更

2026-10-11。利用者はカード69の独立QtダイアログではなくWindowsの通知を意図していた。

Windows.UI.NotificationsのToastGenericを使う。Win32 MSIなのでStartメニューの設定ショートカットにAUMIDとToastActivatorCLSIDを持たせ、LocalServer32から専用モードのCOM activatorを起動する。追加のWindows App SDKランタイムは必要ない。通知センターから、案内プロセス終了後にもボタン操作を受け取れる。

AUMIDはAruiki.Asteria.Support、CLSIDは378F9411-77E0-46BD-9E01-A0A14EF95391。COMコールバックはAUMIDとgithub/laterの固定値を検証し、Qtの主スレッドへ渡す。任意URLを受け付けない。ネットワークアクセスは利用者がGitHubリンクを選んだ時だけ。

GitHubを開けた場合のみ永久停止。いいえは押した時から7日延期。閉じる・無応答も通知送信前に保存した7日延期を維持する。既存のLocalLow/Kotori/support-reminder.iniと停止・延期状態を引き継ぐ。通知をOSが無効にしていれば独自画面で代替しない。無音、通知センターの同一tag/groupで置き換える。

検証結果はdocs/verification/windows-support-toast-20261011.mdに記録する。Windows API成功と実画面の確認を分ける。

参考: https://learn.microsoft.com/en-us/windows/apps/design/shell/tiles-and-notifications/send-local-toast-desktop-cpp-wrl
