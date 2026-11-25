# Asteria 日本語入力 導入ガイド

Windows 10 (1809 以降) / Windows 11（64-bit）向けの純 Rust 製・ローカル AI 日本語 IME です。モデル・辞書・推論ランタイムはすべてインストーラに同梱されています。

## ダウンロードとインストール

1. [GitHub Releases 最新版](https://github.com/aruiki/Asteria-Japanese-IME/releases/latest) から **`Asteria-Setup.exe`** をダウンロードして実行します（管理者の確認で「はい」）。
   - 本体（`Asteria64.msi`）に加えて、NVIDIA GPU を検出した場合は高速化用の CUDA アドオン（`AsteriaCuda64.msi`）も自動導入されます。
2. 完了後、**Windows + Space** で「**Asteria 日本語入力**」に切り替えます。設定画面はスタートメニューまたはデスクトップの「Asteria」から開けます。

手動でのモデル配置や追加コマンドは不要です。旧版からの更新時も、版付き TSF DLL 機構により開いているアプリを閉じずにそのまま上書き更新できます。

- **動作環境**: メモリ 8 GB 以上（16 GB 推奨）、ディスク空き約 2.0 GB。
- **GPU / CPU 動作**: Vulkan 対応 GPU（VRAM 3 GiB 以上）または NVIDIA GPU（CUDA）でローカル AI が高速動作します。GPU 非搭載環境では自動で CPU 向け軽量設定で動作します。

## SmartScreen が表示されたとき

配布インストーラは**未署名**です。実行時に「Windows によって PC が保護されました」という SmartScreen 画面が表示される場合がありますが、これは新規・未署名のオープンソース配布物に対して表示される確認画面です（[Microsoft 公式説明](https://learn.microsoft.com/ja-jp/windows/apps/package-and-deploy/smartscreen-reputation)）。

配布元が `github.com/aruiki/Asteria-Japanese-IME` の Releases であることを確認したうえで、**詳細情報 → 実行** でインストールを進められます。なお、「実行」が許可されない管理対象 PC や、Microsoft Defender 等から具体的な脅威名が表示された場合は実行しないでください。

## SHA-256 によるファイル照合

各リリースには `Asteria-Setup.exe.sha256`・`Asteria64.msi.sha256`・`AsteriaCuda64.msi.sha256` を添付しています。PowerShell で改ざんや破損がないか確認できます：

```powershell
Get-FileHash -Algorithm SHA256 .\Asteria-Setup.exe
```

取得したハッシュ値がリリース同梱の `.sha256` と一致すれば、公開したファイルそのままです。

使い方・設定・アンインストール・データ管理は [使い方ガイド](USER_GUIDE.md) をご覧ください。[README へ戻る](../README.md)。
