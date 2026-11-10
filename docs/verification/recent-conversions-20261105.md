# 誤変換の報告(直近の変換を残す)の検証(2026-11-05)

カード 92、[ADR0087](../adr/0087-recent-conversions.md)。Mozc の作業ツリー `~/ag16` の枝
`asteria/recent-conversions`、HEAD `764b7b7`。モデルはインストール済み Asteria 3.34.6491.100。
実際のセッションの経路(`session_handler_main`)で確かめた。

## 変えたこと
- `session/kotori_recent_conversions`: 確定した変換を 20 件まで、読み・前の文(末尾 96 文字)・
  文節ごとの候補・確定した語・1 番目のままだったかを、1 行 1 件(base64 のタブ区切り)で
  `<プロファイル>/asteria_recent_conversions.txt` に保存する。
- 設定 `kotori_recent_conversions`(既定オフ)。パスワード欄・シークレットでは記録しない
  (既存の `KotoriCountsUsage` と同じ条件)。
- 設定画面「プライバシー → 誤変換の報告」: 直近 5 件の表示、書き出し(保存先は利用者が選ぶ)、消去。
- 文節ごとの候補を返す `KotoriRecentCandidates` と、検査用の `SWITCH_INPUT_FIELD_TYPE`。

## 単体テスト(ビルドと実行)
| 対象 | 結果 |
| --- | --- |
| `//session:kotori_recent_conversions_test` | 6 件すべて成功 |
| `//gui/config_dialog:kotori_settings_bridge_test` | 成功(新しい `RecentConversionsReadsExportsAndClears` を含む) |
| `//session:session_test` | 8 シャードすべて成功(145 秒) |

## 実機のセッション経路(モデルあり)
`session_handler_main` に「くうかんこうへんちょうき」を打ち、Space→Enter で確定した。

| 設定・欄 | 記録 |
| --- | --- |
| オン・普通の欄 | 1 件。読み `くうかんこうへんちょうき`、確定 `空間光変調器`、1 番目のまま=true、候補 7 件 |
| オフ | なし |
| オン・シークレットモード | なし |
| オン・パスワード欄 | なし |

オンのときのファイル(先頭 1 行。文字は base64):
```text
# asteria-recent-conversions v1
2026-11-04T22:14:04+0000	1	44GP44GG44GL44KT44GT44GG44G444KT44Gh44KH44GG44GN	56m66ZaT5YWJ5aSJ6Kq/5Zmo		56m66ZaT5YWJ5aSJ6Kq/5Zmo	56m66ZaT5b6M57eo6ZW35pyf	...
```
(時刻の +0000 は、この検査で使った Python から起動したプロセスの時間帯。製品の設定画面の「今日」は
既存の `asteria_usage.txt` と同じ扱い。)

## 設定画面
ヘッドレスブラウザ(Edge、1200×900)で `gui/config_dialog/web` のプレビューを開き、プライバシーの
「誤変換の報告」にスイッチ・ボタン・説明が出ること、オフのときは「まだ何も保存していません」と
出ることを確認した。

## 導入物の画面(抽出した MSI、管理インストール)
CI と同じコードのローカル MSI(3.34.6464.100、1,668,423,680 bytes、SHA-256
`0B0F7840AC69D47362623FD1E854062359C9D28235368E518F9FB4A6236396C9`)を `msiexec /a` で
`%TEMP%` に抽出し、`mozc_tool.exe --mode=config_dialog` を WebView2 のデバッグポート経由で操作した。

- スイッチ「直近の変換をこの PC に残す」をオン → 「直近の変換」に 1 件(読み・確定した語・前の文・候補)が出た。
- 「消す…」→ 確認 → 一覧が「まだ記録がありません。変換して確定すると増えます。」になり、
  `asteria_recent_conversions.txt` が消えた(ファイルの存在も確認)。
- スイッチをオフに戻した。
- 「書き出す…」の保存ダイアログは自動操作が安定せず、**ダイアログでの実保存は未確認**。
  ブリッジのテストで `SaveDiagnostics` が呼ばれることは確認している。

この MSI はインストールせず抽出だけに使った(ローカルの版 3.34.6464.100 は、導入済みの
3.34.6491.100 より古いため)。

## インストール済み TSF での実入力(2026-11-05)
同じコードのローカル MSI(3.34.6494.100、1,668,440,064 bytes、SHA-256
`C30027A69FC678895F1ED91ED5F99C795276862976DAF0BB9C06AC2AA606DEA3`)を、導入済みの
3.34.6491.100 へ上書き導入した(`msiexec /i /qn`、終了コード 0)。導入後の TIP は
`mozc_tip64-3.34.6494.100.dll`、サーバーは 3.34.6494.100。
設定「直近の変換をこの PC に残す」を実 UI でオンにし、実 TSF(`ImeBench.exe`、CLSID
`{2A1ADAE4-8061-4AC0-A7D4-713BF8DCB591}`、プロファイル `{455A87E7-275F-4582-92CB-6600917A436D}`)で
「くうかんこうへんちょうき」を打って Space→Enter で確定した。

記録された内容:
- `にほんご` → `日本語`(warm、候補 5 件)
- `くうかんこうへんちょうき` → `空間光変調器`(候補: 空間光変調器/空間後編長期/…、first_kept=1)

確認後は設定をオフに戻し、`asteria_recent_conversions.txt` を消した。導入したのは main のコードから
作ったローカル MSI で、CI の成果物(3.34.6494.100、SHA-256 `9f520a99…c9bd`)とはビルドが別のため
ハッシュは異なる。

## CI(2026-11-05)
- CI(run 41926680030): licenses、mozc tools (python)、MSRV 1.80、TSF TIP Win32/x64、
  just ci (Ubuntu/Windows) がすべて成功。
- Mozc (Windows)(run 41926680011): **成功(35分14秒)**。MSI のビルド、単体テスト、installer_smoke、
  成果物の保存まで通った。

### CI の MSI と installer_smoke
- 成果物: `Asteria64.msi`(**1,665,924,089 bytes**、ProductVersion **3.34.6494.100**)。
  `installer-smoke` の `result.json`: SHA-256 `9f520a99e3f1a2423ed71a3b65f94eb5b5c19eb296d31e2972363cb44989c9bd`。
- `installer_smoke.py --acceptance --duplicate-stress --browser-duplicate-stress --reinstall`:
  install / input / uninstall = **true**、errors **0**、native_tsf **true**、8 checks、再導入も成功。
  読み込まれた TIP は `C:\Program Files (x86)\Asteria\mozc_tip64-3.34.6494.100.dll`。
- 範囲: WinForms の EDIT/RichEdit(Notepad・ブラウザー・Office・低性能実機ではない)。
  保護欄の AI/学習は `secure_field_internal_ai_or_learning_verified: false`(未検証)。

## 限界
- 書き出しの保存ダイアログは、ブリッジのテスト(FakeHost)で確認した。実機のダイアログでの保存は未確認。
- 保護欄の AI/学習は CI でも未検証(`secure_field_internal_ai_or_learning_verified: false`)。パスワード欄で
  記録しない動作は、セッションの経路で検査用コマンドにより確認した(実際の TIP の欄では未確認)。
- 導入したローカル MSI は main のコードのビルドで、公開済みリリースではない。

## 追記(2026-11-10): パスワード欄で AI と学習が止まらない不具合
- 原因 1: Windows の TIP が欄の InputScope(IS_PASSWORD)をサーバーへ伝えていなかった。サーバーの
  保護欄の判定(`KotoriCountsUsage` など)は、検査用コマンドで欄の種類を切り替えたときしか働かなかった。
- 原因 2: 変換要求のシークレット判定(`ConversionRequest::incognito_mode()`)が欄の種類を見ていなかった。
  確定などのコマンドではクライアントの context に欄の種類が載らないため、文脈つきの学習・入力履歴・
  区切りの学習・AI の予測が、パスワード欄でも動き得た。
- 修正: TIP がキーごとに欄の種類(PASSWORD / NORMAL)を送り、セッションが composer に反映する。
  変換要求は composer か context がパスワード欄ならシークレットと同じに扱う(REQ-8-3、REQ-10-8)。
  覚えた区切りもパスワード欄では使わない(`NoResizeWhenAiIsOn` の期待を変えた)。
- 確認: CI の「テストする」の 19 本がすべて成功(`conversion_request_test` に
  `KotoriPasswordFieldIsIncognito`、`session_test` に `KotoriKeyContextSetsInputFieldType` を追加)。
  TIP のライブラリ(`//win32/tip:tip_dll_module` ほか)のビルドが成功。
- 未確認: 導入した MSI の実際のパスワード欄での動作(CI の installer_smoke と実機で確かめる)。
