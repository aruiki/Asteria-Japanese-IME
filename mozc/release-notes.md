<h2 align="center">
  <a href="https://github.com/aruiki/Kotori-AI-Japanese-IME/releases/latest/download/Kotori64.msi">
    ⬇ Kotori64.msi をダウンロード
  </a>
</h2>

<p align="center">
  <strong>Windows 10 / 11・64bit</strong>
</p>

<p align="center">
  <a href="https://github.com/aruiki/Kotori-AI-Japanese-IME/releases/latest">
    <img alt="最新版" src="https://img.shields.io/github/v/release/aruiki/Kotori-AI-Japanese-IME?label=%E6%9C%80%E6%96%B0%E7%89%88&color=242C5C">
  </a>
  <a href="https://github.com/aruiki/Kotori-AI-Japanese-IME/releases">
    <img alt="ダウンロード数" src="https://img.shields.io/github/downloads/aruiki/Kotori-AI-Japanese-IME/total?label=%E3%83%80%E3%82%A6%E3%83%B3%E3%83%AD%E3%83%BC%E3%83%89&color=242C5C">
  </a>
  <img alt="Windows 10 / 11" src="https://img.shields.io/badge/Windows-10%20%2F%2011-242C5C?logo=windows">
  <img alt="オフライン動作" src="https://img.shields.io/badge/AI-%E3%82%AA%E3%83%95%E3%83%A9%E3%82%A4%E3%83%B3-F25C2E">
</p>

> [!NOTE]
> `Kotori64.msi` の実行時に Microsoft Defender SmartScreen の  
> **「WindowsによってPCが保護されました」**  
> という画面が表示される場合があります。

---

## SmartScreenが表示された場合

<p align="center">
  <img
    src="https://www.sanwa.co.jp/static/images/seihin_joho/windows11_protection/img_1-1.png"
    alt="WindowsによってPCが保護されました と表示されたSmartScreen画面"
    width="500">
</p>

<p align="center">
  <sub>
    SmartScreenの日本語表示例 —
    <a href="https://www.sanwa.co.jp/seihin_joho/windows11_protection/index.html">
      出典：サンワサプライ
    </a>
  </sub>
</p>

この表示だけで、Kotoriからウイルスやマルウェアが検出されたという意味ではありません。

Microsoft Defender SmartScreenは、危険なファイルの検出だけでなく、
インターネットからダウンロードしたアプリや発行元について
**Windows上で十分な「評判（reputation）」が確立されているか**も確認しています。

Microsoft公式ドキュメントでも、**署名されていないアプリでは
「Windows が PC を保護しました」という警告が表示される**ことが説明されています。

[Microsoft公式：SmartScreenの評判について](https://learn.microsoft.com/ja-jp/windows/apps/package-and-deploy/smartscreen-reputation)

---

## インストールを続けるには

> [!IMPORTANT]
> このGitHub Releaseからダウンロードした  
> **`Kotori64.msi`** であることを確認してください。

1. <kbd>詳細情報</kbd> をクリック
2. 表示内容を確認
3. <kbd>実行</kbd> をクリック

これで通常のKotoriインストーラーが起動します。

**SmartScreenやMicrosoft Defenderを無効にする必要はありません。**

---

## なぜこの警告が出るの？

SmartScreenは単純なウイルス検査だけではなく、
ファイルや発行元の**評判**も利用して判断しています。

そのため、

**「WindowsによってPCが保護されました」  
＝「マルウェアが検出された」**

ではありません。

Kotoriはソースコードを公開しているオープンソースソフトウェアです。

<details>
<summary><strong>安全性についてさらに確認する</strong></summary>

<br>

- [ソースコード](https://github.com/aruiki/Kotori-AI-Japanese-IME)
- [最新Release](https://github.com/aruiki/Kotori-AI-Japanese-IME/releases/latest)
- [Security Policy](https://github.com/aruiki/Kotori-AI-Japanese-IME/security/policy)
- [Microsoft公式：SmartScreenの仕組み](https://learn.microsoft.com/ja-jp/windows/apps/package-and-deploy/smartscreen-reputation)

KotoriのAIモデル・辞書・日本語変換処理はPC内で動作し、
**入力した文章をKotoriのサーバーや外部AIサービスへ送信しません。**

</details>

---

> [!WARNING]
> SmartScreenの「認識されていないアプリ」という警告ではなく、
> Microsoft Defenderなどから**具体的な脅威名やマルウェア名**が表示された場合は実行しないでください。

---

## 今回の更新（v1.1.1）

Kotoriを気に入っていただけた方へ、GitHubの ⭐ Starで応援をお願いする案内を追加しました。

- 「GitHubで応援する」でページを開くと、以後このお願いは表示されません。
- 「いいえ」または閉じる操作では、1週間後に再度表示します。
- 入力処理とは別に確認し、操作が落ち着いたタイミングで表示します。入力内容は収集しません。
- 停止・延期の状態はPC内に保存し、アップデート後も引き継ぎます。

