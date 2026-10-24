# 0072. 画面1枚・再起動なしで更新できるインストーラーと Asteria のフォルダ名

- 状態: 採用
- 日付: 2026-10-24
- 関係: docs/adr/0012(Mozc をベースにする)、docs/tasks/84-input-mode-consistency.md

## 背景

v2 までのインストーラーは WiX の WixUI_Minimal を使い、使用許諾への同意、「アプリを閉じますか」(使用中のファイル)、
完了の画面などで利用者が 3 回ほど OK を押す必要があった。IME の DLL(mozc_tip64.dll など)は入力中の全アプリに
読み込まれているので、上書きできずに再起動要求(終了コード 3010)になることが多かった。アンインストールも
再起動を予約していた。導入先は `Program Files (x86)\Kotori`、設定と学習は `LocalLow\Kotori` のままだった。

## 決めたこと

- 画面は 1 枚(`AsteriaInstallDlg`)。初回は「インストール」、導入済みなら「更新」と表示し、「デスクトップに設定の
  ショートカットを作る」(前回の選択を `HKLM\Software\Kotori\Shortcuts\desktop_choice` から引き継ぐ)と、
  ライセンスへのリンクを置く。同意のクリックはなくす(ライセンスはリンクと導入先の文書で示す)。
- `MSIRESTARTMANAGERCONTROL=Disable` とし、使用中のファイルの画面(FilesInUse/MsiRMFilesInUse)を参照しない。
- IME の DLL は版ごとの名前(`mozc_tip64-<版>.dll`)で導入先の直下に置く。サブフォルダにしないのは、Mozc が
  登録された TIP の置き場所からサーバーの場所を決めるため(`SystemUtil::GetServerDirectory`)。
- 更新では、ファイルを入れる前(`MoveAsideOldTip`)に古い版の DLL を `<名前>.<新しい版>-<時刻>.asteria-old` に改名して
  退け、すぐ消せなければ次の再起動で消す(`MoveFileEx` の `MOVEFILE_DELAY_UNTIL_REBOOT`)。使用中のアプリは古い
  DLL のまま動き続け、新しく入力を始めたアプリから新しい版を読む。前回までに退けたファイルは次の更新で消す。
- 途中で失敗したら、ロールバックの処理(`RestoreMovedTip`)で退けた DLL を元の名前に戻し、元の版が動く状態にする。
- アンインストールでも DLL を退けるだけにし、再起動の予約(`ScheduleReboot`)をやめる。
- 導入先を `Program Files (x86)\Asteria`、設定と学習を `LocalLow\Asteria` にする。旧 `LocalLow\Kotori` は初回に
  丸ごと移し(履歴・設定・ユーザー辞書を含む)、旧版の TIP がまだ動いている間のために旧パスへジャンクションを置く。
  ジャンクションは再起動後に片付ける(`legacy-profile-link.txt` に起動時刻を記録)。
- IPC の名前、レジストリのキー、サービス名は旧名のまま(旧版のプロセスと新しい版が同時に動いても通信できるように)。
  利用者の目に触れる名前(製造元、スタートメニュー、自動起動の値、アイコン)は Asteria に揃える。

## 結果

実機での数(画面と確認の数、終了コード、使用中・新規アプリの入力、引き継ぎ)は
`docs/verification/release-v3.0.0-20261024.md` に記録する。
