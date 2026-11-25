<h2 align="center">
  <a href="https://github.com/aruiki/Asteria-Japanese-IME/releases/download/v5.5.0/Asteria-Setup.exe">Download Asteria-Setup.exe</a>
</h2>

<p align="center">Windows 10 / 11 (64-bit) · free and open source · runs fully offline</p>

---

## What's new

- **A new engine written in Rust.** The text service, dictionary, conversion search, learning, IPC server and settings UI are now written in Rust, across 19 crates. The foundation crates use `#![deny(unsafe_code)]`.
- **One small installer.** `Asteria-Setup.exe` is about 1 MB and downloads only what your PC needs. On PCs with an NVIDIA GPU, it adds the CUDA add-on automatically.
- **Updates without restarting.** New versions install without closing your apps or restarting Windows.
- **Fewer wrong conversions.**
  - Every AI candidate is checked against what you typed, and any whose reading does not match is never shown. Compound words whose sounds change when joined, such as 夜駆け and 支援試合, still come through correctly.
  - Asteria is less easily misled by the previous sentence. After 「口座を開設しました。」 ("I opened an account"), typing にほんごをかいせつ now gives 日本語を解説 ("explain Japanese").
- **Two-stage local AI.** `Genmai-310M` proposes candidates and `TinySwallow-1.5B` picks the one that fits the context. On NVIDIA GPUs, CUDA with Flash Attention is used automatically.

## Accuracy and speed

Measured on the Standard setting with an NVIDIA GeForce RTX 3060.

| Test set | Questions | Top candidate correct |
| :--- | ---: | ---: |
| Held-out set, never used for tuning (`heldout`) | 300 | 99.0% |
| Long sentences with compounds and rare readings (`ajimee`) | 200 | 95.5% |
| Short everyday sentences (`daily`) | 81 | 100.0% (CUDA) · 98.8% (Vulkan) |
| Practical writing, spelling variants accepted (`usability-multi`) | 120 | 100.0% |
| Homophones (`homophone`) | 40 | 100.0% |
| Idioms and set phrases (`idioms`) | 25 | 100.0% |

Against Google Japanese Input / Mozc (v3.34) under the same procedure, total time fell from 15.66 s to 10.63 s on `daily` and from 53.35 s to 43.83 s on `ajimee`. The slowest single conversion on `daily` fell from 847 ms to 167.5 ms.

## Install

1. Download and run [Asteria-Setup.exe](https://github.com/aruiki/Asteria-Japanese-IME/releases/download/v5.5.0/Asteria-Setup.exe). It is the only file you need (about 1 MB). During setup it downloads Asteria itself (about 1.6 GB), plus the CUDA add-on (about 0.5 GB) on PCs with a supported NVIDIA GPU. An internet connection is needed only while installing.
2. The installer is not code-signed, so Windows may show "Windows protected your PC". Click **More info**, then **Run anyway**.
3. Press **Windows + Space** and choose **Asteria 日本語入力**.

`checksums.txt` is optional. It lets you confirm that the download was not damaged or altered.

---

[README](https://github.com/aruiki/Asteria-Japanese-IME#readme) · [User guide](https://github.com/aruiki/Asteria-Japanese-IME/blob/main/docs/USER_GUIDE.md) · [Report a bug or a wrong conversion](https://github.com/aruiki/Asteria-Japanese-IME/issues/new/choose)
