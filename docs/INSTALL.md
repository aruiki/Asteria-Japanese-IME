# Asteriaを使い始める

Windows 10 1809以降 / Windows 11・64bit向けの日本語入力です。モデル・辞書・実行環境はMSIに同梱されています。

## ダウンロードと導入

1. [公式GitHub Releaseの最新版](https://github.com/aruiki/Asteria-Japanese-IME/releases/latest)から、[Asteria64.msi](https://github.com/aruiki/Asteria-Japanese-IME/releases/latest/download/Asteria64.msi)をダウンロードします（約1.7 GB）。
2. MSIを開き、インストーラーの案内に従います。管理者の確認が表示されます。
3. **Windows + Space**から「Asteria日本語入力」を選びます。設定はスタートメニューの「Asteria」から開けます。

手動でのモデル配置やコマンド入力は不要です。OSが再起動を求める場合は、その案内に従ってください。

AIにはVulkan対応の専用GPU（VRAM 3 GB以上）を使います。GPUのないPCはCPU向けの設定（Low）で動きます。メモリ8 GB以上（16 GB推奨）、ディスクに約1.7 GBの空きが必要です。内蔵GPUはAIに使いません。

## SmartScreenが表示されたとき

配布MSIは**未署名**です。実行時に「WindowsによってPCが保護されました」というSmartScreen画面が表示される場合があります。SmartScreenはファイルや発行元の評判も確認するため、この画面だけでマルウェアが検出されたとは判断できません。これは安全性を保証する説明ではありません。[Microsoft公式の説明](https://learn.microsoft.com/ja-jp/windows/apps/package-and-deploy/smartscreen-reputation)。

配布元が `github.com/aruiki/Asteria-Japanese-IME` のReleaseであり、取得したファイルが `Asteria64.msi` であることを確認してください。続行する場合は、**詳細情報 → 表示内容を確認 → 実行**を選びます。

「実行」が表示されない管理対象PCや、別の脅威名を伴う検出が出た場合は、この手順の対象外です。[報告窓口](https://github.com/aruiki/Asteria-Japanese-IME/issues/new/choose)をご利用ください。SmartScreenやMicrosoft Defenderを無効にする操作は必要ありません。

## ファイルを照合する

ダウンロードした `Asteria64.msi` のSHA-256を、同じReleaseの本文または [Asteria64.msi.sha256](https://github.com/aruiki/Asteria-Japanese-IME/releases/latest/download/Asteria64.msi.sha256) と比べます。PowerShellで確認する場合:

~~~powershell
Get-FileHash -Algorithm SHA256 .\Asteria64.msi
~~~

ハッシュは、公開されたファイルと同じものかを確かめるためのものです。各版の対応ソースと検証範囲は、Release本文から[検証記録](verification/)へリンクしています。

更新・アンインストール・保存データ・詳しい操作は[使い方と困ったとき](USER_GUIDE.md)へ。[Asteriaの紹介へ戻る](../README.md)。
