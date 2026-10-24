<p align="center">
  <img src="docs/marketing/assets/hero.png?v=2.1" alt="Asteria — a Japanese input method with highly accurate conversion. Free, open source, and works offline." width="100%">
</p>

<h1 align="center">Asteria</h1>

<p align="center">
  <strong>A Japanese input method (IME) for Windows with highly accurate conversion.</strong><br>
  It reads the sentence you just wrote to pick the word you mean.<br>
  The AI runs on your own PC. What you type never leaves it.
</p>

<p align="center">
  <a href="https://github.com/aruiki/Asteria-Japanese-IME/releases/latest"><img alt="Latest release" src="https://img.shields.io/github/v/release/aruiki/Asteria-Japanese-IME?color=111C2D"></a>
  <img alt="Windows 10 / 11 x64" src="https://img.shields.io/badge/Windows-10%20%2F%2011%20x64-111C2D">
  <img alt="Free and open source" src="https://img.shields.io/badge/free-open%20source-536779">
  <img alt="Offline AI" src="https://img.shields.io/badge/AI-offline-536779">
</p>

<h2 align="center">
  <a href="https://github.com/aruiki/Asteria-Japanese-IME/releases/latest/download/Asteria64.msi">↓ Download for Windows</a>
</h2>

<p align="center">
  Free · about 1.7 GB · <a href="docs/INSTALL.md">Install guide (Japanese)</a> · <a href="README.md">日本語</a>
</p>

<p align="center">
  <img src="docs/images/typing-demo.gif?v=one-space" alt="A real typing recording: “onsei ninshiki no seido ga agatta” converted to 音声認識の精度が上がった with a single Space" width="100%">
</p>

## It reads the previous sentence

Japanese has many words that sound the same. Asteria uses the sentence you wrote just before to choose between them.

| Preceding text | Reading | Asteria | Google Japanese Input |
| --- | --- | --- | --- |
| …2被告にはいづれも懲役3年（ (…a three-year prison term for both defendants) | きゅうけいは | **求刑は** (the sentence sought) | 休憩は (the break) |
| …芝生に寝そべりながら草競馬の (…lying on the grass, the local horse race) | かんせんに | **観戦に** (to watch) | 感染に (to infection) |
| …同町が10月26日に内示していた担当課長らの (…the section chiefs the town had notified on Oct. 26) | いどうに | **異動に** (to the reassignment) | 移動に (to the move) |
| …東金市内の資材置き場の (…at a materials yard in Tōgane) | そっこうに | **側溝に** (into the gutter) | 速攻に (to a quick attack) |

On 300 such questions taken from real news and encyclopedia text, where the right word cannot be chosen without the
preceding sentence, Asteria answered **274** correctly and Google Japanese Input **133** ([how we measured](#by-the-numbers)).

## Natural spelling for everyday writing

- **Everyday spelling**: prefers the forms people actually write, such as ちょうどいい, ください, あとで, based on usage counted in public conversation corpora.
- **Numbers and symbols**: 「90ぱーせんと」 becomes 90%; the comma in 「賛成96、反対3」 stays Japanese.
- **Katakana to English**: アーキテクチャ also offers *architecture* (about 36,000 pairs).
- **Familiar IME behavior**: built on [Mozc](https://github.com/google/mozc), the open-source core of Google Japanese Input, so segment editing, learning and the user dictionary work as you expect.

## No waiting

The AI loads when you sign in and stays ready. It starts converting the moment you pause, so with normal typing
**candidates appear almost as soon as you press Space** (on an RTX 3060, 95% within 0.03 s when you press Space after a short pause).
If the AI cannot run, typing continues with Mozc's conversion.

## By the numbers

Measured on the same PC (RTX 3060) by typing into each IME. Spelling variants (見たり / 観たり) count as the same.

| Evaluation | Asteria v2.1.0 | Google Japanese Input 3.34 |
| --- | --- | --- |
| Everyday chat sentences, 500 | **461** (92.2%) | 434 (86.8%) |
| Homophones that need the preceding sentence, 300 | **274** (91.3%) | 133 (44.3%) |

The test sentences come from public corpora (everyday conversation, Wikinews, Wikipedia articles created since 2024) and
were held out from all tuning. Per-domain results (documents, technical terms, proper nouns), the latency breakdown,
and the full procedure and raw outputs are in the [verification record](docs/verification/release-v2.1.0-20261023.md) (Japanese).
Asteria still makes mistakes; proper nouns such as personal names are its weakest area.

## Getting started

1. Download and open [Asteria64.msi](https://github.com/aruiki/Asteria-Japanese-IME/releases/latest/download/Asteria64.msi). Windows asks for administrator approval.
2. Press **Windows + Space** and choose “Asteria日本語入力”.
3. Open the settings from “Asteria” in the Start menu.

The installer is not code-signed yet, so Windows SmartScreen may warn you. The [install guide](docs/INSTALL.md) explains how to check the source and the SHA-256.

| Requirements | |
| --- | --- |
| OS | Windows 10 (1809 or later) / Windows 11, 64-bit |
| GPU | A dedicated GPU with Vulkan (3 GB VRAM or more; NVIDIA / AMD / Intel Arc) is recommended |
| Without a GPU | Runs with a CPU setting; conversion is slower and less accurate than on a GPU |
| Memory / disk | 8 GB RAM (16 GB recommended), about 1.7 GB of free space |

Dictionaries, AI models and the runtime are all inside the installer. No Python or CUDA setup is needed.

## Privacy

Nothing you type is sent to the internet for conversion, and no account is needed. What Asteria learns stays on your PC
and can be cleared from the settings.

## How it works

| Component | Role |
| --- | --- |
| [Mozc](https://github.com/google/mozc) | Dictionaries, segmentation, the candidate list, learning, Windows integration |
| [zenz-v2.5](https://huggingface.co/Miwa-Keita/zenz-v2.5-small) | A conversion model that generates and scores sentences from the reading and the preceding text |
| [TinySwallow-1.5B](https://huggingface.co/SakanaAI/TinySwallow-1.5B) | A Japanese language model that scores how natural a sentence is and fixes misconversions |
| [llama.cpp](https://github.com/ggml-org/llama.cpp) | Runs the models on the GPU (Vulkan) or the CPU |

Design decisions are recorded with their reasons and measurements in the [architecture decision records](docs/adr/) (Japanese).

## Get involved

- [Report a misconversion, bug or idea](https://github.com/aruiki/Asteria-Japanese-IME/issues/new/choose) — misconversion reports are easiest to fix with the reading and the preceding sentence. English is welcome.
- [Development guide](docs/DEVELOPMENT.md) / [specification](docs/SPEC.md) / [contributor rules](AGENTS.md) (Japanese)
- [Support development (Stripe)](https://buy.stripe.com/cNi14m7jRdq005f0781ZS08) — every feature is available with or without support

## License

Asteria is free and open-source software. The code is licensed under [Apache License 2.0](LICENSE-APACHE) / [MIT License](LICENSE-MIT),
and the changes to Mozc under BSD-3-Clause. Licenses of the bundled models, dictionaries and libraries are listed in
[docs/licenses.md](docs/licenses.md). To report a vulnerability, see the [security policy](SECURITY.md).
