<h2 align="center">
  <a href="https://github.com/aruiki/Asteria-Japanese-IME/releases/latest/download/Asteria64.msi">
    ⬇ Asteria64.msi をダウンロード
  </a>
</h2>

<p align="center">
  <strong>Windows 10 / 11・64bit</strong>　·　約1.7 GB　·　無料・オープンソース
</p>

<p align="center">
  <a href="https://github.com/aruiki/Asteria-Japanese-IME/releases/latest">
    <img alt="最新版" src="https://img.shields.io/github/v/release/aruiki/Asteria-Japanese-IME?label=%E6%9C%80%E6%96%B0%E7%89%88&color=111C2D">
  </a>
  <a href="https://github.com/aruiki/Asteria-Japanese-IME/releases">
    <img alt="ダウンロード数" src="https://img.shields.io/github/downloads/aruiki/Asteria-Japanese-IME/total?label=%E3%83%80%E3%82%A6%E3%83%B3%E3%83%AD%E3%83%BC%E3%83%89&color=111C2D">
  </a>
  <img alt="Windows 10 / 11" src="https://img.shields.io/badge/Windows-10%20%2F%2011-111C2D?logo=windows">
  <img alt="オフライン動作" src="https://img.shields.io/badge/AI-%E3%82%AA%E3%83%95%E3%83%A9%E3%82%A4%E3%83%B3-536779">
</p>

> [!NOTE]
> `Asteria64.msi` の実行時に Microsoft Defender SmartScreen の<br>
> **「WindowsによってPCが保護されました」**
> という画面が表示される場合があります。下の手順で続けられます。

---

## インストールの流れ

1. 上の **Asteria64.msi** をダウンロードして開きます。
2. SmartScreen が出たら、**詳細情報 → 実行**（下の説明）。
3. **「インストール」**（入っている場合は「更新」）を押し、Windows の確認（UAC）を許可します。完了の画面が出たら終わりです。
4. **Windows + Space** で「Asteria日本語入力」を選ぶと使えます。

「開いているアプリを閉じてください」と求めることはありません。v3.0.0 以降の更新は再起動もいりません。
**v2.0.0 以前から更新したときだけ、最後に Windows を再起動してください**（再起動まで、更新前から開いていたアプリでは日本語に変換できません）。設定・学習・ユーザー辞書は引き継ぎます。

---

## SmartScreenが表示された場合

> [!IMPORTANT]
> このGitHub Releaseからダウンロードした
> **`Asteria64.msi`** であることを確認してください。

**1. <kbd>詳細情報</kbd> をクリック**

<p align="center"><img src="https://raw.githubusercontent.com/aruiki/Asteria-Japanese-IME/main/docs/images/smartscreen-1.png" alt="WindowsによってPCが保護されました の画面で、詳細情報を押す" width="500"></p>

**2. アプリ名が `Asteria64.msi` であることを確かめて、<kbd>実行</kbd> をクリック**

<p align="center"><img src="https://raw.githubusercontent.com/aruiki/Asteria-Japanese-IME/main/docs/images/smartscreen-2.png" alt="詳細情報を開いた画面で、実行を押す" width="500"></p>

これで通常のAsteriaインストーラーが起動します。
**SmartScreenやMicrosoft Defenderを無効にする必要はありません。**

### なぜこの警告が出るの？

SmartScreenは、ウイルスの検査だけでなく、ダウンロードしたアプリや発行元に
**Windows上で十分な「評判（reputation）」があるか**も見ています。
Asteriaのインストーラーはコード署名をしていないため、この画面が出ることがあります。

**「WindowsによってPCが保護されました」＝「マルウェアが検出された」ではありません。**
[Microsoft公式：SmartScreenの評判について](https://learn.microsoft.com/ja-jp/windows/apps/package-and-deploy/smartscreen-reputation)

<details>
<summary><strong>安全性をさらに確認する</strong></summary>

<br>

- [ソースコード](https://github.com/aruiki/Asteria-Japanese-IME)
- [Security Policy](https://github.com/aruiki/Asteria-Japanese-IME/security/policy)
- ファイルの照合: PowerShell で `Get-FileHash -Algorithm SHA256 .\Asteria64.msi` を実行し、添付の `Asteria64.msi.sha256` と同じか確かめます。

AsteriaのAIモデル・辞書・日本語変換はPCの中で動き、
**入力した文章をAsteriaのサーバーや外部AIサービスへ送信しません。**

</details>

> [!WARNING]
> SmartScreenの「認識されないアプリ」という警告ではなく、
> Microsoft Defenderなどから**具体的な脅威名やマルウェア名**が表示された場合は実行しないでください。

---

## 動作環境

- Windows 10 1809 以降 / Windows 11（64bit）、メモリ 8 GB 以上（16 GB 推奨）、ディスク約 1.7 GB
- AI は Vulkan 対応の専用 GPU（VRAM 3 GiB 以上）で動きます。GPU のない PC は CPU 向けの設定で動きます。

[使い方と紹介（README）](https://github.com/aruiki/Asteria-Japanese-IME#readme) ／ [不具合・誤変換を報告](https://github.com/aruiki/Asteria-Japanese-IME/issues/new/choose)
