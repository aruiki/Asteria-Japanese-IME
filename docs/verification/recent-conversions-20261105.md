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
- この PC では sudo が無効で、MSI の導入に UAC が必要なため、**導入済み TSF で設定をオンにして
  実入力する確認は未実施**。CI の installer_smoke は導入・実 TSF 入力・再導入・削除を満たしたが、
  新しい設定をオンにした実入力は含まない。
- パスワード欄は検査用のコマンドで切り替えて確認した。実際の TIP が同じ `input_field_type` を送ることは
  既存の使い方の統計と同じ経路だが、実機の欄では未確認(CI の secure_field も未検証)。
- 書き出しの保存ダイアログは、ブリッジのテスト(FakeHost)で確認した。実機のダイアログは未確認。
