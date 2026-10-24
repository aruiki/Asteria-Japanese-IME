<p align="center">
  <img src="docs/marketing/assets/hero.png?v=2.1" alt="Asteria — 変換精度の高い日本語IME。無料・オープンソース。オフラインで使えます。" width="100%">
</p>

<h1 align="center">Asteria — アステリア</h1>

<p align="center">
  <strong>変換精度の高い日本語IME。</strong><br>
  前の文まで読んで、意図した言葉に変換します。<br>
  AIはあなたのPCの中で動き、入力した文章を外へ送りません。
</p>

<p align="center">
  <a href="https://github.com/aruiki/Asteria-Japanese-IME/releases/latest"><img alt="最新版" src="https://img.shields.io/github/v/release/aruiki/Asteria-Japanese-IME?label=%E6%9C%80%E6%96%B0%E7%89%88&color=111C2D"></a>
  <img alt="Windows 10 / 11・64bit" src="https://img.shields.io/badge/Windows-10%20%2F%2011%20x64-111C2D">
  <img alt="無料・オープンソース" src="https://img.shields.io/badge/無料-オープンソース-536779">
  <img alt="オフライン" src="https://img.shields.io/badge/AI-オフライン-536779">
</p>

<h2 align="center">
  <a href="https://github.com/aruiki/Asteria-Japanese-IME/releases/latest/download/Asteria64.msi">↓ Windows版をダウンロード</a>
</h2>

<p align="center">
  無料 · 約1.7 GB · <a href="docs/INSTALL.md">導入ガイド</a> · <a href="README.en.md">English</a>
</p>

<p align="center">
  <img src="docs/images/typing-demo.gif?v=one-space" alt="「おんせいにんしきのせいどがあがった」をSpace 1回で「音声認識の精度が上がった」に変換した実際の入力の録画" width="100%">
</p>

## 前の文を読んで、言葉を選ぶ

日本語には同じ読みの言葉がたくさんあります。Asteriaは、直前に書いた文を手がかりに候補を選び直します。

| 直前の文 | 読み | Asteria | Google日本語入力 |
| --- | --- | --- | --- |
| …2被告にはいづれも懲役3年（ | きゅうけいは | **求刑は** | 休憩は |
| …芝生に寝そべりながら草競馬の | かんせんに | **観戦に** | 感染に |
| …同町が10月26日に内示していた担当課長らの | いどうに | **異動に** | 移動に |
| …東金市内の資材置き場の | そっこうに | **側溝に** | 速攻に |

実際のニュースや百科事典の文から、前の文がないと決められない同音語を集めた300問で比べると、Asteriaは
**274問**、Google日本語入力は **133問** 正解しました（[測定方法](#数字で見る)）。

## 毎日の文章も、自然な表記で

- **日常の表記**: 「ちょうどいい」「ください」「あとで」のように、普段よく使われる書き方を選びます。根拠は公開の会話コーパスで数えた使用比率です。
- **数字と記号**: 「90ぱーせんと」は「90%」に、「賛成96、反対3」の読点は日本語の「、」のままにします。
- **カタカナ語から英語**: 「アーキテクチャ」から architecture のように、英語の綴りも候補に出ます（約3.6万組）。
- **いつものIMEの操作**: Mozc（Google日本語入力のオープンソース版）がもとなので、文節の区切り直し、学習、ユーザー辞書をそのまま使えます。

## 待たせない

AIはログイン時に読み込まれ、常に準備ができています。打つ手が止まった瞬間から裏で変換を始めるので、
普段の打ち方なら **Spaceを押すと、ほぼすぐに候補が出ます**（RTX 3060で、手を止めてから押した場合、95%が0.03秒以内）。
AIが使えない状況でも、Mozcの変換で入力を続けられます。

## 数字で見る

同じPC（RTX 3060）で、実際のIMEにキー入力して比べました。正解は、表記ゆれ（「見たり／観たり」など）を同じとみなして数えています。

| 評価 | Asteria v2.1.0 | Google日本語入力 3.34 |
| --- | --- | --- |
| 日常の短文・チャット 500問 | **461**（92.2%） | 434（86.8%） |
| 前の文が必要な同音語 300問 | **274**（91.3%） | 133（44.3%） |

評価文は公開コーパス（日常会話・ウィキニュース・2024年以降のWikipedia）から作り、調整には使っていない判定用のものです。
分野別の結果（文書・技術・固有名詞を含む）、速さの内訳、測定の手順と原票は[検証記録](docs/verification/release-v2.1.0-20261023.md)にあります。
誤変換が起こらないわけではありません。人名などの固有名詞は、ほかの分野より誤りが多めです。

## 使い始める

1. [Asteria64.msi](https://github.com/aruiki/Asteria-Japanese-IME/releases/latest/download/Asteria64.msi) をダウンロードして開きます。管理者の確認が表示されます。
2. **Windows + Space** で「Asteria日本語入力」を選びます。
3. 設定は、スタートメニューの「Asteria」から開けます。

配布ファイルはまだ署名されていないため、Windowsの警告（SmartScreen）が表示されることがあります。配布元とファイルの確かめ方は[導入ガイド](docs/INSTALL.md#smartscreenが表示されたとき)をご覧ください。

| 動作環境 | |
| --- | --- |
| OS | Windows 10（1809以降）/ Windows 11、64bit |
| GPU | Vulkan対応の専用GPU（VRAM 3 GB以上。NVIDIA / AMD / Intel Arc）を推奨 |
| GPUがないPC | CPU向けの設定で動きます。GPUより変換の速さと精度は下がります |
| メモリ・空き容量 | メモリ8 GB以上（16 GB推奨）、空き容量 約1.7 GB |

辞書・AIモデル・実行環境はすべてインストーラーに入っています。PythonやCUDAの準備は要りません。

## プライバシー

入力した文章は、変換のためにインターネットへ送られません。アカウント登録も不要です。
学習した内容はこのPCの中にだけ保存され、設定画面から消せます。詳しくは[使い方とデータの扱い](docs/USER_GUIDE.md#データとプライバシー)へ。

## しくみ

| 部分 | 役割 |
| --- | --- |
| [Mozc](https://github.com/google/mozc) | 辞書、文節の区切り、候補の一覧、学習、Windowsとの連携 |
| [zenz-v2.5](https://huggingface.co/Miwa-Keita/zenz-v2.5-small) | 読みと前の文から、変換文を生成・採点する専用のAI |
| [TinySwallow-1.5B](https://huggingface.co/SakanaAI/TinySwallow-1.5B) | 文章としての自然さを採点し、誤変換を補正する日本語のAI |
| [llama.cpp](https://github.com/ggml-org/llama.cpp) | AIをGPU（Vulkan）またはCPUで動かす |

設計の判断は[設計記録（ADR）](docs/adr/)に、理由と測定結果と一緒に残しています。

## 一緒につくる

- [使い方と困ったとき](docs/USER_GUIDE.md) — キー操作、更新、アンインストール
- [誤変換・不具合・要望を報告する](https://github.com/aruiki/Asteria-Japanese-IME/issues/new/choose) — 誤変換は、読みと前の文があると直しやすくなります
- [開発の始め方](docs/DEVELOPMENT.md) ／ [仕様](docs/SPEC.md) ／ [開発の約束ごと](AGENTS.md)
- [開発を支援する（Stripe）](https://buy.stripe.com/cNi14m7jRdq005f0781ZS08) — 支援の有無で機能は変わりません

## ライセンス

Asteriaは無料のオープンソースソフトウェアです。コードは [Apache License 2.0](LICENSE-APACHE) / [MIT License](LICENSE-MIT)、
Mozcへの変更は BSD-3-Clause です。同梱するモデル・辞書・ライブラリのライセンスは[一覧](docs/licenses.md)にまとめています。
脆弱性の報告は[セキュリティ方針](SECURITY.md)へ。
