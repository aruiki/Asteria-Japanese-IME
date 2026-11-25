# Windows フロントエンド(TSF TIP)

`asteria_tip.dll` は Rust 製(`crates/asteria-tip-win`)の TSF テキストサービスで、キーを
`asteria-client` 経由でエンジン(`asteria-server.exe`)へ送り、返ってきたプリエディットと
確定文字列を編集する。候補ウィンドウは `asteria-renderer.exe`(Direct2D/DirectWrite)が描画する
(docs/SPEC.md 10.1、docs/adr/0009、docs/adr/0010)。

## リリースから入れる(かんたん)

[Releases](https://github.com/aruiki/Asteria-Japanese-IME/releases) から `Asteria-<版>-windows.zip` を
ダウンロードして展開し、`install.cmd` をダブルクリックする(管理者の確認で「はい」)。外すときは
`uninstall.cmd`。手順は zip の `README.txt`(元は `README.release.txt`)にある。

リリースは `.github/workflows/release.yml` が作る。Actions の「Release」を手で実行するか、`v` で
始まるタグを push する。手で実行するときバージョン(SPEC 16.3 の `0.x.y-beta` の形)を空にすると、
ワークスペースの版の beta の番号を既にあるタグの次にする(例 `v0.1.0-beta.1` があれば `v0.1.0-beta.2`)。
サーバー・レンダラー・設定GUI・x64 と x86 の TIP・辞書・`THIRD_PARTY_NOTICES.txt`・インストールの
スクリプトを zip にまとめ、最新のリリース(Latest)として公開する。リリースの名前は日本時間の日時と版
(例 `2026.929.151234 (v0.1.0-beta.2)`)。LM のモデルは同梱しない(docs/licenses.md)。
このワークフローやインストールのスクリプトを変える PR では、zip を作って Actions の成果物に置く
だけにする(公開しない)。

## 作って入れる

リポジトリの最上位から、PowerShell で実行する。Rust・CMake・Visual Studio 2022(`asteria-lm` の
`llama.cpp` 用)が要る。32 bit のアプリ用に i686 のターゲットも入れておく
(`rustup target add x86_64-pc-windows-msvc i686-pc-windows-msvc`)。

```powershell
just dict                             # 1. 辞書を作る(target/asteria/system.dict)
./frontends/windows/build.ps1         # 2. サーバー・レンダラー・設定GUIと x64・x86 の TIP を作り、dist に集める
./frontends/windows/install.ps1       # 3. 管理者の PowerShell で。%ProgramFiles%\Asteria に置いて登録する
```

置く形は次のとおり。サーバーは `--dict` がなければ、自分と同じフォルダの `data\system.dict` を読む。
32 bit の TIP は1つ上のフォルダの `asteria-server.exe`・`asteria-renderer.exe` を起動する。

```
%ProgramFiles%\Asteria\
  asteria-server.exe
  asteria-renderer.exe
  asteria-config.exe
  asteria_tip.dll          (x64)
  x86\asteria_tip.dll      (x86)
  data\system.dict
```

設定の「時刻と言語」→「言語と地域」→「日本語」→「キーボードの追加」で Asteria を選ぶ。サーバーは
最初のキーで自動で起動する(REQ-4-1)。辞書の読み込み(1 秒弱)が終わるまでは直接入力になる。

外すときは、管理者の PowerShell で `./frontends/windows/uninstall.ps1` を実行する。登録を外し、
プロセスを止めてフォルダを消す。アプリが DLL を使っていて消せなければ、再起動後に
フォルダを消すよう案内が出る。入れ直すときも、そのまま実行して差し替えられる。

機能: ローマ字入力、マルチ境界ラティス変換、ライブ変換、予測・表記正規化・ニュアンス候補、
文脈学習(`ContextHistory`)、直近変換・ユーザー辞書、Direct2D 候補ウィンドウ(`asteria-renderer.exe`)、
WebView2 設定画面(`asteria-config.exe`)、パスワード欄バイパス(REQ-10-3)、左文脈送信(REQ-10-2)、
LM 2段階非同期リランク(REQ-6-2)。
