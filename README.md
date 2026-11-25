<p align="center">
  <img src="docs/images/asteria-icon-240.png" alt="Asteria" width="96">
</p>

<h1 align="center">Asteria日本語入力</h1>

<p align="center">
  前の文まで読む AI が、あなたの PC の中で変換を選ぶ Windows の日本語入力です。<br>
  打ち方は今までと同じ。Space を 1 回押すだけです。
</p>

<p align="center">
  <a href="https://github.com/aruiki/Asteria-Japanese-IME/releases/latest/download/Asteria-Setup.exe"><strong>Asteria-Setup.exe をダウンロード</strong></a><br>
  <sub>Windows 10 / 11（64bit）　無料・オープンソース</sub>
</p>

<p align="center">
  <a href="https://github.com/aruiki/Asteria-Japanese-IME/blob/main/docs/marketing/assets/asteria-promo.mp4">
    <img src="docs/images/promo-preview.gif" alt="メモ帳でローマ字を続けて打ち、Space を 1 回押すと文全体が正しく変換される実際の録画" width="100%">
  </a>
</p>

## 文脈で選ぶ

同じ読みでも、前の文によって正しい漢字は変わります。Asteria は直前の文を読み、文全体を一度に変換します。

| 同じ PC・同じ打ち方で、Space で出た最初の候補が正しかった割合 | Asteria | Google日本語入力 |
| --- | ---: | ---: |
| 前の文で正解が決まる同音語（300 問） | **91%** | 45% |
| 日常の短い文（500 問） | **95%** | 87% |

<sub>調整に使っていない問題。Asteria 開発版 3.34.6464、Google日本語入力 3.34.6260、2026 年 11 月。[測り方と全結果](docs/verification/accuracy-realinput-20261111.md)</sub>

## いつもの打ち方のまま

- 変換するのは Space を押したときだけです。打っている途中で文字が勝手に変わることはありません（ライブ変換は設定でオンにできます）。
- Tab を押すと、AI が文の続きを予測します。
- 覚えた言葉やよく選ぶ候補は PC の中に保存され、設定の「辞書」からいつでも消せます。

## 打った文章は PC の外へ出ない

AI のモデルも辞書も PC の中にあり、変換にインターネットを使いません。打った文章や使い方の情報を、どこかへ送ることもありません。
パスワード欄では日本語入力にならず、学習もしません。

## インストール

1. **[Asteria-Setup.exe](https://github.com/aruiki/Asteria-Japanese-IME/releases/latest/download/Asteria-Setup.exe)** を開き、**インストール** を押します。
2. 終わったら **Windows + Space** で「Asteria日本語入力」を選びます。

<p align="center"><img src="docs/images/installer.png" alt="Asteria のインストーラーの画面" width="360"></p>

NVIDIA の GPU がある PC では、変換を速くする部品も自動で入ります。

「WindowsによってPCが保護されました」と出たら、**詳細情報 → 実行** で続けられます。インストーラーにコード署名をしていないために出る画面で、
ウイルスが見つかったという意味ではありません。[詳しい手順](https://github.com/aruiki/Asteria-Japanese-IME/releases/latest)

<details>
<summary>動作環境</summary>

| 項目 | 条件 |
| --- | --- |
| OS | Windows 10 1809 以降 / Windows 11（64bit） |
| メモリ | 8 GB 以上（16 GB 推奨） |
| ディスク | 約 1.7 GB |
| AI | Vulkan に対応した専用 GPU（VRAM 3 GB 以上）。GPU のない PC では CPU 向けの設定で動きます |

</details>

## 仕組み

1. [Mozc](https://github.com/google/mozc)（Google日本語入力のもとになったオープンソースの変換エンジン）が、辞書から候補の素材を作ります。
2. genmai-310M が読みから文の候補を書き起こし、TinySwallow-1.5B が前の文と合わせて自然さを採点します。どちらも [llama.cpp](https://github.com/ggml-org/llama.cpp) で GPU の上で動きます。
3. いちばん自然な文を最初の候補にします。AI が使えないときは、Mozc の候補で変換を続けます。

---

<p align="center"><sub>
  <a href="docs/USER_GUIDE.md">使い方と困ったとき</a>　
  <a href="https://github.com/aruiki/Asteria-Japanese-IME/issues/new/choose">不具合・誤変換の報告</a>　
  <a href="docs/licenses.md">ライセンス</a>　
  <a href="https://aruiki.github.io/kanai/">公式サイト</a>
</sub></p>

<p align="center"><sub>by aruiki　<a href="https://stemstudio.jp/">stemstudio.jp</a></sub></p>
