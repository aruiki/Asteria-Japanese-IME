<h2 align="center">
  <a href="https://github.com/aruiki/Asteria-Japanese-IME/releases/download/v5.5.1/Asteria-Setup.exe">
    ⬇ Download Asteria-Setup.exe (v5.5.1)
  </a>
</h2>

<p align="center">
  <strong>Windows 10 / 11 (64-bit)</strong> · About 1.5 MB Web Setup · Free &amp; Open Source · 100% Offline AI
</p>

---

## What's New in Asteria v5.5.1

**Asteria** is a clean-slate Japanese IME written across **19 Rust crates** with local GPU AI (`Genmai-310M` + `TinySwallow-1.5B`, Vulkan + automatic CUDA acceleration). `v5.5.1` resolves two user-session issues reported after `v5.5.0` while delivering the full Web Setup architecture (`ADR 0092`):

### Fixes in v5.5.1
- **Automatic Windows + Space Input List Registration & 64-bit Path Unification**:
  - Built `Asteria64.msi` with `-arch x64` so the core installs to `C:\Program Files\Asteria` alongside `C:\Program Files\Asteria\cuda`.
  - Added per-user `InstallLayoutOrTip` registration in both the setup bootstrapper (`InputList.Ensure()`) and MSI deferred user custom action (`EnsureUserTipProfile`, `Impersonate="yes"`), automatically adding Asteria to the active user's input list and removing any orphaned legacy TIP entries.
- **Settings App (`asteria-config.exe`) Rendering & Auto-Start Fix**:
  - Moved the inlined `<script>` in `bundled_html()` from `<head>` to right before `</body>` so the WebView2 UI no longer stalls on a blank / loading screen.
  - Configured `WinHost` to auto-launch and wait for the sibling `asteria-server.exe`, read `LocalLow\Asteria` profile state and install-directory GPU info, and unify the window & Start Menu title to **Asteria**.
- **Web Setup Fixes (PR #256)**:
  - Compact **~1.5 MB** `Asteria-Setup.exe` (`PlatformTarget x86`, STA UI thread, hidden Apply window for `/quiet`, crescent app icon, live download progress UI, and offline MSI folder fallback).

### Accuracy & Speed (`Standard`, NVIDIA GeForce RTX 3060)

| Evaluation set | Questions | Acc@1 | Acc@10 | CER |
| :--- | ---: | ---: | ---: | ---: |
| Practical writing, variants accepted (`usability-multi`) | 120 | **100.0%** | 100.0% | 0.00% |
| Homophones (`homophone`) | 40 | **100.0%** | 100.0% | 0.00% |
| Idioms and set phrases (`idioms`) | 25 | **100.0%** | 100.0% | 0.00% |
| User-reported regressions (`reports`) | 6 | **100.0%** | 100.0% | 0.00% |
| Regression golden (`golden`) | 3 | **100.0%** | 100.0% | 0.00% |
| Short everyday sentences (`daily`) | 81 | **100.0% (CUDA)** · 98.8% (Vulkan) | 100.0% | 0.00% / 0.27% |
| Held-out set (`heldout`) | 300 | **99.0%** | 100.0% | 0.14% |
| Long compounds &amp; rare readings (`ajimee`) | 200 | **95.5%** | 98.5% | 0.40% |

Compared with Google Japanese Input / Mozc (`v3.34`) under the same procedure, total time fell from `15.66 s` to **`10.63 s`** on `daily` (`-32%`, max latency `167.5 ms` vs `847 ms`) and from `53.35 s` to **`43.83 s`** on `ajimee` (`-18%`).

---

## Install

1. Download and run **[Asteria-Setup.exe](https://github.com/aruiki/Asteria-Japanese-IME/releases/download/v5.5.1/Asteria-Setup.exe)** (about 1.5 MB). During setup it downloads Asteria itself (about 1.5 GB), plus the CUDA add-on (about 0.5 GB) on PCs with a supported NVIDIA GPU. An internet connection is needed only while installing (for offline PCs, see [INSTALL.md](https://github.com/aruiki/Asteria-Japanese-IME/blob/main/docs/INSTALL.md)).
2. The installer is not code-signed, so Windows may show "Windows protected your PC". Click **More info**, then **Run anyway**.
3. Press **Windows + Space** and choose **Asteria**.

`checksums.txt` is optional and lets you verify with PowerShell (`Get-FileHash -Algorithm SHA256 .\Asteria-Setup.exe`) that the download was not damaged or altered.

---

[README](https://github.com/aruiki/Asteria-Japanese-IME#readme) · [User guide](https://github.com/aruiki/Asteria-Japanese-IME/blob/main/docs/USER_GUIDE.md) · [Report a bug or a wrong conversion](https://github.com/aruiki/Asteria-Japanese-IME/issues/new/choose)
