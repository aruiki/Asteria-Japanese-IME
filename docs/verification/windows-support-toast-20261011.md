# Windows標準応援通知 検証記録（2026-10-11）

対象: codex/asteria-brand、Mozc a069a88dに既存パッチと今回変更。BUILD6338。

## 成功

- Windows SDKのWRL/WinRTを使った通知・COM activatorのコンパイル。
- 応援案内8テスト成功。7日境界、再起動・時計巻き戻り、GitHub固定URLと永久停止、ブラウザー失敗で停止しない、延期、保存失敗時に繰り返さない、未知・停止済みアクション拒否、トースト内容。
- 実際の製品ビルドmozc_tool.exeをQt DLLと共に隔離した場所で起動。-Embedding引数を含むCOMサーバー起動が成功。CoCreateInstanceからINotificationActivationCallbackを取得し、別AUMID・未知アクションを実際に呼び出してE_INVALIDARGを確認。実利用者のsupport-reminder.iniの実行前後SHA一致（不在なら不在）。COM登録はこの検証プロセスの生存中のみ。レジストリ変更なし。

## 試行時の失敗

- 初回のBazelテストはQt DLLを見つけられず-1073741515。PATH/Qt plugin設定を付けて8テスト成功。
- 製品exeをBazel出力から直接起動したCOM試験もDLLロードで3221225781。exeとQt DLLを隔離した同一フォルダーへ置いた試験は成功。インストール済みファイルは変更していない。

## 未確認

- 新MSIの導入、実際に表示されたWindows通知の外観、通知センターからの後日クリック、ブラウザー実起動、フォーカスアシストの実動作、アンインストール。単体テストやCOM呼出しをその代わりにしない。
- 今回のMSIは未署名。公開版・インストール済みバイナリにはまだ反映していない。

ローカルの補助証拠: target/windows-support-toast/check_com_host.py、com-host.json、verify-msi.ps1。

## MSIとパッチの確定

- package build exit0、694.102秒。
- MSI 3.34.6338.100、Asteria日本語入力、48ファイル、1,569,386,496 bytes、NotSigned。
- SHA256: a968312279d33c67679436d071d78409e6c74bdebf1824e475895fe9266e420e。
- UpgradeCodeは既存の45FB8DEB-0CFE-4924-96FB-9022888112E1を維持。
- MsiShortcutPropertyのAUMID/ToastActivatorCLSIDとRegistryのHKLM LocalServer32・専用モードをMSIデータベースで照合。
- 配布用mozc_tool.exe 3.34.6338.100のSHA256: b4ff1e3cfdaff9f75338f487f0d5c7d64703f3a81b44f7ab6ce9efe4da244da4。
- Bazel停止後make_patches.shで再生成。3パッチ適用成功。素のMozcへ適用した127入力が実ビルド作業ツリーと一致。
- README、リリース本文、設定/辞書アイコンは今回の変更に含めない。

## v1.1.2公開後の照合

利用者が「そのまま公開」を指示。ローカルビルド指定（ADR0054）に従いMSIを再ビルドせず公開。
PR153の一般CI run38147576524は7ジョブすべてsuccess。Mozcクラウドrun38147576662はcancelledであり、成功ではない。
公開用head55a9c37c06af34eb973cd47b738b9e6fa098b30a、main統合/タグe94a9b23b891ccc498ed121f51c8a19cb9ef539f。両tree一致。
v1.1.2は正式版・Latest、release409720573、2026-10-11T14:36:52Z公開。
公開APIのMSI digest/size、ソースタグ、リリース本文の完全一致を確認。旧公開本文を全文保持し、v1.1.2の更新内容を末尾追加。
check_release.py --localの全項目成功。初回は相対TARGETDIRのため1603、絶対workパスへ直した実行は成功。
MSIのモデル/推論runtime22ファイルがv1.1.1と同じSHA。MSIから展開したmozc_tool.exeもCOM試験の対象と同じSHA。

公開URL: https://github.com/aruiki/Kotori-AI-Japanese-IME/releases/tag/v1.1.2
実導入・実通知画面・通知センターからの操作・IME入力・削除は引き続き未実施。
公開後check_release.py v1.1.2 --work <絶対パスtarget/windows-support-toast/postpublish>はexit0・全項目成功。公開MSIを再取得してSHA一致、前のv1.1.1よりProductVersion先頭3項目が増加（6336→6338）、サイズ1.8GiB以下、必須モデル/実行ファイル/第三者表示、ggml-rpc不在、server版一致を確認した。
