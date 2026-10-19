# 固定CI候補の取得と自動受入れ（2026-10-19）

利用者は実機確認の方針を「自分で確認」から「やっぱり自動にしてください」へ変更した。
ローカル画面操作の正規ランタイムは再初期化時もZドライブ列挙の認証1326で起動できなかった。
利用者PCの認証・セキュリティ設定を変更せず、製品の既存Windows CI入力試験を拡張する。

## 成果物と出所

- [PR168](https://github.com/aruiki/Asteria-Japanese-IME/pull/168)の全8チェックが成功し、main `71300bff41d14b81a9c2f5d12830bfeeb7670fe0`へmerge commitで統合済み。
- [Windows run 39158232305](https://github.com/aruiki/Asteria-Japanese-IME/actions/runs/39158232305)のheadは`6272fa113eaa63826b2c48bd8f10f03056ae5235`、実checkoutは`0ab7fdaabd2cf5880fe15261b5fa33679eb2f7ec`。3コミットのtreeは一致。
- 取得したAsteria64.msiは1,662,763,008 bytes、SHA-256 `0f12eb1db8726e7a3c49186626098a100c64c36ffd9cab97c9dbe526ffcdf57a`。CI receiptのSHAと実バイトが一致。
- ProductVersion `3.34.6435.100`、ProductCode `{1FCBC881-1D42-436E-A82D-94B884FDEBAB}`。公開v1.1.3の先頭3版6431より大きい。実更新の検証とは別。
- AuthenticodeはNotSigned。4モデル・追加生成モデルmetadata・34,661連語と出典/帰属・RPC非同梱・server版の管理展開検査が成功。
- 元CIは導入exit0、WinForms TextBoxの「日本語」「東京」、削除exit3010を確認。削除時の再起動要求を隠さない。広いアプリ受入れやAI寄与の確認ではない。

ローカル原票は`target/accuracy-guide/ci-artifact-39158232305/`（MSI、smoke/result.json、package-check.log、summary.json）、`pr168-merge-verification.json`。

## 実CIランタイムの限定対照

CIのggml-base.dllは`758a9469ee5b9a46e6e78af3238eec9751879b5e8940b3c4aa61731b68ac1f81`。
以前のローカル検証DLLとは異なるため、実CIのモデル/DLLを使って固定source8439のSessionHandlerで対照した。CIのserver/TSFを検査した扱いにはしない。

- CPUとRTX 3060で、各新規PIDのAI readyを確認。各3文の準備後の出力は正解。初回各3文には「明日の会議は従事からです」が残り、準備後は「明日の会議は１０時からです」。
- 両方の6シナリオで入力欠落・表示確定差分・応答上限違反0。ただし各1件の初回誤変換があるため`--require-exact`全件判定はexit1。成功へ書き換えない。
- 同じ実CIランタイムで256MiBの自分の子プロセスだけのcommit上限を使い、AIなし対照3文/実AI初期化error時3文の入力継続が成功。正常起動の対照を先に行っており、常時AI停止を低メモリ成功に数えない。物理低性能機の検証ではない。

原票は`ci6435-runtime-cold-{cpu,gpu}-v1/`、`ci6435-runtime-memory-v1/`。各summaryは入力・PID・モデル/DLL・exeのSHAと環境を保持する。

## 追加する自動検証

`Candidate installer acceptance`は上記run/head/MSI SHAを固定し、同じMSIを取得する。製品の再ビルドや公開は行わない。`installer_smoke.py --acceptance --reinstall`を使い捨てGitHub Windowsランナーだけで実行する。

1. 実際に導入したTIPを読み込むWinForms EDITとRichEditで、ネイティブ打鍵→未確定文字→Space→確定を検査する。
2. 候補Down/PageDown/PageUp後の未確定表示とEnterの結果、Esc2回と入力再開、Backspace、F6〜F10を検査する。
3. 通常欄のフォーカス移動と、合成した公開文字列によるパスワード欄の見える挙動を検査する。内部AI停止/非学習の証明にはしない。
4. 同じ起動中のWindowsで削除→同一MSI再導入→ファイルSHA一致→再入力→削除を行う。再起動後の復旧・旧版からの上書き・失敗時rollbackは別条件。

追加8項目は空/部分成功・重複・異なるTIPでは成功にしない。ローカルC#コンパイルとPython113件が成功。

[run39183067546](https://github.com/aruiki/Asteria-Japanese-IME/actions/runs/39183067546)で追加8項目、再導入exit0、再入力、削除exit3010が全成功。実MSIのSHA/版・導入server/renderer/TIPのSHAを確認した。PR169の全9チェックも成功し、mainc5473b24acabca1e16b38a9b63f704cbb8bd8e95へ統合した。

最初の追加試験run39182787163はRichEdit観測中の意図しない確定で失敗した。RichTextBox.Textによる観測を、未確定文字の直接取得とWM_GETTEXTへ変更した同候補の後続試験が成功。失敗原票も`target/accuracy-guide/candidate-acceptance-39182787163/`に保持。成功原票は`candidate-acceptance-39183067546/`。製品MSIは変更していない。

## 残る範囲

実アプリでのAI ON/OFF寄与、Notepad/ブラウザ/Office等、候補ウィンドウの見た目、保護欄の内部AI/学習、低性能実機、初回「アイデア局」、旧版更新・再起動復旧・失敗rollback、独立verifierは未確認または未解決。追加試験の成功だけで製品完成や新リリース公開にしない。
