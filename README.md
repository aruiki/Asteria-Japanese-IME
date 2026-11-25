<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="docs/images/crescent.png">
    <img src="docs/images/asteria-icon-240.png" alt="Asteria" width="88">
  </picture>
</p>

<h1 align="center">Asteria Japanese Input</h1>

<p align="center">
  <strong>Japanese input that understands context.</strong><br>
  A Windows IME that picks the right kanji from what you have already written.<br>
  Its AI runs entirely on your PC.
</p>

<p align="center">
  <a href="https://github.com/aruiki/Asteria-Japanese-IME/releases/latest"><img alt="Latest release" src="https://img.shields.io/github/v/release/aruiki/Asteria-Japanese-IME?label=Release&color=111C2D"></a>
  <img alt="Pure Rust Core" src="https://img.shields.io/badge/Core-100%25%20Rust-B7410E?logo=rust&logoColor=white">
  <img alt="Windows TSF" src="https://img.shields.io/badge/Windows-10%20%2F%2011%20x64-111C2D?logo=windows">
  <img alt="GPU AI" src="https://img.shields.io/badge/Local%20AI-Vulkan%20%2B%20CUDA-2E4A62">
  <img alt="Privacy" src="https://img.shields.io/badge/Telemetry-Zero%20%2F%20Offline-1F6F5B">
  <img alt="License" src="https://img.shields.io/badge/License-Apache--2.0%20OR%20MIT-536779">
</p>


<h3 align="center">
  <a href="https://github.com/aruiki/Asteria-Japanese-IME/releases/latest/download/Asteria-Setup.exe">Download Asteria-Setup.exe</a>
</h3>

<p align="center"><sub>v5.5.1 · 単一インストーラ（Vulkan 標準 + NVIDIA CUDA 自動導入）· アカウント登録なし · 100% ローカル処理</sub></p>

---

## The right kanji for the sentence

世の中のオープンソース日本語 IME の多くは、巨大なレガシー C++ コードベースにパッチを重ねた派生品です。  
**Asteria v5.5.1 は異なります。** Windows の TSF（Text Services Framework）DLL から、ダブル配列トライ辞書、Viterbi ラティス探索、読み整合バリデータ、文脈学習、IPC サーバー、WebView2 設定 UI、無停止アップグレーダに至るまで、**19 クレートのコア基盤を Rust で新規設計**しました。

| You type | One press of Space gives | Meaning |
| :--- | :--- | :--- |
| しょうひんのひんしつをほしょうします | 商品の品質を**保証**します | We *guarantee* the product's quality |
| こくみんのせいかつをほしょうする | 国民の生活を**保障**する | To *secure* people's livelihoods |
| じこのそんがいをほしょうする | 事故の損害を**補償**する | To *compensate* for accident damage |

<details>
<summary>システムアーキテクチャを見る（19 Pure-Rust Crates）</summary>

```mermaid
flowchart LR
    Key["打鍵入力 (Windows TSF)"] --> TIP["asteria-tip-win<br/>(x64 / x86 Rust DLL)"]
    TIP <-->|Named Pipe IPC + ACL| Server["asteria-server<br/>(tokio / State Machine)"]
    Server --> Lattice["asteria-dict + asteria-lattice<br/>(Double-Array Trie + Viterbi K-best)"]
    Lattice --> Validate["MatchesReading + 連濁補完<br/>+ 数詞正規化"]
    Validate --> LM["asteria-lm<br/>(Genmai-310M Drafted Beam 6<br/>+ TinySwallow-1.5B Judge + copyガード)"]
    LM --> Learn["asteria-learn<br/>(文脈学習 + 同音語辞書<br/>+ パスワード欄遮断)"]
    Learn --> UI["asteria-renderer / asteria-config<br/>(DirectWrite 候補窓 / WebView2 UI)"]
```

- **辞書データと推論カーネルの分離**: 変換・探索・学習エンジンは 100% 純 Rust 実装です。システム辞書の語彙素材としてオープンソース辞書コーパスを独自コンパイラ `asteria-dictc` で mmap バイナリ辞書へコンパイルし、GGUF 推論カーネルのみ `llama.cpp`（Vulkan / CUDA）を利用しています。

</details>

---

## 変換精度と速度の実測値

`v5.5.1`（Standard: `Genmai-310M-q8_0` + `TinySwallow-1.5B-q5_k_m`、NVIDIA GeForce RTX 3060）で、8 種類の評価セットすべてを実測した成績です。

| 評価ベンチマーク | 問題数 | Acc@1 (正答率) | Acc@10 (候補内) | CER | 内容 |
| :--- | ---: | ---: | ---: | ---: | :--- |
| **実用マルチ正解 (`usability-multi`)** | 120 | **100.0%** | 100.0% | 0.00% | 日常〜長文・表記ゆれ許容の実入力評価 |
| **同音異義語 (`homophone`)** | 40 | **100.0%** | 100.0% | 0.00% | 文脈依存の同音語選択 |
| **慣用句・表現 (`idioms`)** | 25 | **100.0%** | 100.0% | 0.00% | 誤変換しやすい慣用句・定型表現 |
| **報告回帰 (`user-reports`)** | 6 | **100.0%** | 100.0% | 0.00% | 実ユーザーから報告された誤変換ケース |
| **ゴールデン (`golden`)** | 3 | **100.0%** | 100.0% | 0.00% | 必須維持テストケース |
| **日常文 (`daily`)** | 81 | **100.0% (CUDA)** / 98.8% (Vulkan) | 100.0% | 0.00% / 0.27% | 標準的な日常短文 |
| **未調整ホールドアウト (`heldout`)** | 300 | **99.0%** | 100.0% | 0.14% | 調整に使っていない独立テスト |
| **難文・複合語 (`ajimee`)** | 200 | **95.5%** | 98.5% | 0.40% | 連濁・難読語を含む長文ベンチ |

### 応答速度の比較 (`converter-runner` E2E / RTX 3060)

- **従来 Google 日本語入力 / Mozc (`v3.34`) 比**:
  - `daily`（81 問）合計処理時間: **`10.63s`**（従来 `15.66s` 比 **32% 短縮**、最大遅延 `167.5ms` vs `847ms`）
  - `ajimee`（200 問）合計処理時間: **`43.83s`**（従来 `53.35s` 比 **18% 短縮**、最大遅延 `551.2ms` vs `918ms`）

---

## 実入力イメージ

<p align="center">
  <a href="https://aruiki.github.io/kanai/asteria-v3-promo.mp4?v=3.0.0">
    <img src="docs/images/promo-preview.gif" alt="Typing 107 characters of romaji and converting them in context with a single press of Space" width="100%">
  </a>
</p>

## Features

- **Works the way you already type.** Text never changes on its own while you type. Space converts, Enter confirms, and Shift+←/→ adjusts segment boundaries. Live conversion is available as an option.
- **Everything stays on your PC.** The dictionary and the AI models run locally. Nothing you type, and no usage data, is ever sent anywhere. Password fields switch to direct input automatically, and nothing is learned in private browsing windows.
- **Fast with a GPU, fine without one.** The AI runs on any Vulkan-capable GPU and uses CUDA automatically on NVIDIA cards. On PCs without a GPU, Asteria switches to a lighter CPU setting.
- **Updates without interruption.** New versions install without closing your apps or restarting Windows.
- **Thoughtful details.** Typo correction, and English spellings such as `github` → `GitHub` (more than 31,000 words).

## Accuracy and speed

Measured with v5.5.0 on the Standard setting, using an NVIDIA GeForce RTX 3060.

| Test set | Questions | Top candidate correct |
| :--- | ---: | ---: |
| Held-out set, never used for tuning (`heldout`) | 300 | 99.0% |
| Long sentences with compounds and rare readings (`ajimee`) | 200 | 95.5% |
| Short everyday sentences (`daily`) | 81 | 100.0% (CUDA) · 98.8% (Vulkan) |
| Practical writing, spelling variants accepted (`usability-multi`) | 120 | 100.0% |
| Homophones (`homophone`) | 40 | 100.0% |
| Idioms and set phrases (`idioms`) | 25 | 100.0% |

For speed, we ran the same procedure against Google Japanese Input / Mozc (v3.34). On the 81 everyday sentences, total time dropped from 15.66 s to 10.63 s, and the slowest single conversion from 847 ms to 167.5 ms.

## Install

1. Download and run [Asteria-Setup.exe](https://github.com/aruiki/Asteria-Japanese-IME/releases/latest/download/Asteria-Setup.exe) (about 1.5 MB). During setup it downloads Asteria itself (about 1.5 GB), plus the CUDA add-on (about 0.5 GB) if you have a supported NVIDIA GPU.
2. Press **Windows + Space** and choose **Asteria 日本語入力**.

> [!NOTE]
> The installer is not code-signed, so Windows may show "Windows protected your PC" the first time you run it. Click **More info**, then **Run anyway**. You can check the download against its SHA-256 hash; see the [installation guide](docs/INSTALL.md) (Japanese).

<details>
<summary>System requirements</summary>

| | |
| :--- | :--- |
| OS | Windows 10 (1809 or later) / Windows 11, 64-bit |
| Memory | 8 GB or more (16 GB recommended) |
| Disk space | About 2.0 GB, including the AI models and dictionary |
| GPU (recommended) | A Vulkan-capable GPU with 3 GiB of VRAM or more, or an NVIDIA GPU. Asteria also runs on the CPU alone. |

</details>

<details>
<summary>How it works (for developers)</summary>

Everything from the Windows TSF text service to the dictionary (a double-array trie), the conversion search (Viterbi), learning, the IPC server and the settings UI is written in Rust, across 19 crates. The foundation crates use `#![deny(unsafe_code)]`. Only AI inference uses `llama.cpp` (Vulkan / CUDA).

The AI works in two stages. `Genmai-310M` proposes candidates, and `TinySwallow-1.5B` picks the one that fits the preceding text (up to 256 characters). Every AI candidate is checked against what you typed, and any whose reading does not match is never shown.

```mermaid
flowchart LR
    Key["Keystrokes (Windows TSF)"] --> TIP["asteria-tip-win<br/>(x64 / x86 Rust DLL)"]
    TIP <-->|Named Pipe IPC + ACL| Server["asteria-server<br/>(tokio / state machine)"]
    Server --> Lattice["asteria-dict + asteria-lattice<br/>(double-array trie + Viterbi K-best)"]
    Lattice --> Validate["Reading match + rendaku fallback<br/>+ numeral normalization"]
    Validate --> LM["asteria-lm<br/>(Genmai-310M drafted beam 6<br/>+ TinySwallow-1.5B judge + copy guard)"]
    LM --> Learn["asteria-learn<br/>(context learning + homophone dictionary<br/>+ password-field guard)"]
    Learn --> UI["asteria-renderer / asteria-config<br/>(DirectWrite candidate window / WebView2 UI)"]
```

The system dictionary is built from open-source dictionary data, compiled into a memory-mapped binary by our own compiler, `asteria-dictc`.

### Build and test

```powershell
# Lint and run all tests
just ci

# Windows インストーラ（MSI + CUDA Addon + Burn 単一 Setup EXE）の一括ビルド
.\frontends\windows\build.ps1 -OutDir target\msi-dist -BuildMsi -BuildBundle -MsiVersion 5.5.1
```

</details>

---

<p align="center"><sub>
  <a href="docs/USER_GUIDE.md">User guide</a>　·　
  <a href="docs/INSTALL.md">Installation, SmartScreen and SHA-256</a>　·　
  <a href="https://aruiki.github.io/kanai/">Project page</a>　·　
  <a href="https://github.com/aruiki/Asteria-Japanese-IME/issues/new/choose">Report a bug or a wrong conversion</a>　·　
  <a href="docs/licenses.md">Licenses</a>
</sub></p>
