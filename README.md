<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="docs/images/crescent.png">
    <img src="docs/images/asteria-icon-240.png" alt="Asteria" width="96">
  </picture>
</p>

<h1 align="center">打ちたい言葉が、Space 1回で。</h1>

<p align="center">
  前の文まで読むAIが、あなたのPCの中で変換を選ぶ Windows の日本語入力。<br>
  完全無料・オープンソースです。
</p>

<p align="center">
  <a href="https://github.com/aruiki/Asteria-Japanese-IME/releases/latest"><img alt="最新版" src="https://img.shields.io/github/v/release/aruiki/Asteria-Japanese-IME?label=%E6%9C%80%E6%96%B0%E7%89%88&color=111C2D"></a>
  <a href="https://github.com/aruiki/Asteria-Japanese-IME/releases"><img alt="ダウンロード数" src="https://img.shields.io/github/downloads/aruiki/Asteria-Japanese-IME/total?label=%E3%83%80%E3%82%A6%E3%83%B3%E3%83%AD%E3%83%BC%E3%83%89&color=111C2D"></a>
</p>

<p align="center">
  <a href="https://aruiki.github.io/kanai/asteria-v3-promo.mp4?v=3.0.0">
    <img src="docs/images/promo-preview.gif" alt="Wordで107文字をローマ字で続けて打ち、Spaceを1回押すと「来月から始まる新しいプロジェクトの計画について部長と相談した結果…」と正しく変換される実際の録画と、Google日本語入力との比較グラフ" width="100%">
  </a>
</p>

<h3 align="center">
  <a href="https://github.com/aruiki/Asteria-Japanese-IME/releases/latest/download/Asteria64.msi">↓ 無料でダウンロード（Windows 10 / 11）</a>
</h3>

<p align="center"><sub>v3.2.0 · 約1.7 GB（AIモデルと辞書を含む） · アカウント登録なし</sub></p>

---

変換した漢字が違っていて、Space を何度も押したり、文節を区切り直したりしたことはありませんか。

Asteria（アステリア）は、変換の正確さを何より優先した日本語入力です。PC の GPU で AI を動かし、直前の文まで読んで、文脈に合う漢字を最初の候補に出します。打ち方は今までと同じで、Space で変換し、Enter で確定します。

## 前の文で決まる同音語に強い

同じ PC・同じ打ち方で、調整に使っていない問題を変換し、Space で出た最初の候補が正しかった割合を比べました。

| | Asteria | Google日本語入力 |
| --- | ---: | ---: |
| 前の文で正解が決まる同音語（300問） | **91%** | 44% |
| 日常の短い文（500問） | **92%** | 87% |

<sub>Asteria v3.0.0、Google日本語入力 3.34.6260、2026年10月。<a href="docs/verification/release-v3.0.0-20261024.md#google日本語入力との比較の測り直し紹介動画のグラフ2026-10-25">測り方と全結果</a></sub>

## 打ち方は、いつものまま

- 入力中に文字が勝手に変わるライブ変換はありません。変換するのは Space を押したときだけです。
- 文の続きの予測は、Tab を押したときだけ選べます。打っている途中に割り込みません。
- 候補は、AI の結果ができてから出します。準備中の候補を先に出して、あとから差し替えることはしません。

## 打った文章は、PC の外へ出ない

- AI のモデルも辞書も PC の中にあり、変換にインターネットを使いません。
- 打った文章を、Asteria のサーバーやほかの AI サービスへ送ることはありません。利用状況の送信や、自動の不具合報告もありません。
- AI が前の文を読むために覚えておく内容は、メモリの中だけに置きます。学習（よく選ぶ候補など）は PC の中に保存し、設定の「辞書」タブからいつでも消せます。

## 完全無料・オープンソース

有料版も課金もありません。ソースはすべてこのリポジトリにあり、誰でも中身を確かめられます。

## 仕組み

Windows の入力の仕組み（TSF）の上で、次の順に変換します。

1. Mozc（Google日本語入力のもとになったオープンソースの変換エンジン）が、辞書から候補の素材を作ります。
2. GPU の上の 2 つの AI モデルが、文を書き起こして採点します。zenz-v2.5 が読みから文の候補を作り、TinySwallow-1.5B が前の文と合わせて自然さを採点します。llama.cpp と Vulkan で動き、モデルは PC にログインしたときに読み込んでおきます。
3. いちばん自然な文を、最初の候補にします。

GPU がない PC では、CPU で動く軽い設定で変換します。AI が動かないとき（モデルの読み込みに失敗した、時間の上限を超えたなど）は、Mozc の候補で変換を続けます。

## インストール

1. **[Asteria64.msi をダウンロード](https://github.com/aruiki/Asteria-Japanese-IME/releases/latest/download/Asteria64.msi)** して開きます。
2. 「インストール」を押し、Windows の確認を許可します。完了の画面が出たら終わりです。
3. **Windows + Space** で「Asteria日本語入力」を選びます。

インストールの途中で、開いているアプリを閉じるよう求めることはありません。v3.0.0 からの更新は、Windows の再起動もいりません。

「WindowsによってPCが保護されました」という画面が出たら、「詳細情報」→「実行」で続けられます。インストーラーにコード署名をしていないために出るもので、ウイルスが見つかったという意味ではありません。添付の `Asteria64.msi.sha256` と、PowerShell の `Get-FileHash -Algorithm SHA256 .\Asteria64.msi` の結果が同じなら、配布したファイルのままです。

<details>
<summary>動作環境</summary>

| 項目 | 条件 |
| --- | --- |
| OS | Windows 10 1809 以降 / Windows 11（64bit） |
| メモリ | 8 GB 以上 |
| ディスク | 約 1.7 GB |
| AI | Vulkan に対応した専用 GPU（VRAM 3 GB 以上）で動かします。それ以外の PC は CPU で動く軽い設定になります |

</details>

---

<p align="center"><sub>
  <a href="docs/USER_GUIDE.md">使い方と困ったとき</a>　|　
  <a href="https://aruiki.github.io/kanai/">公式サイト</a>　|　
  <a href="https://github.com/aruiki/Asteria-Japanese-IME/issues/new/choose">不具合・誤変換の報告</a>　|　
  <a href="docs/licenses.md">ライセンス</a>
</sub></p>

<p align="center"><sub>紹介動画の音楽: 「流るる時」（YouTube オーディオライブラリ）。打鍵音: NCPRIME（Pixabay）。</sub></p>
