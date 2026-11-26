<h2 align="center">
  <a href="https://github.com/aruiki/Asteria-Japanese-IME/releases/download/v5.5.2/Asteria-Setup.exe">
    ⬇ Download Asteria-Setup.exe (v5.5.2)
  </a>
</h2>

<p align="center">
  <strong>Windows 10 / 11 (64-bit)</strong> · About 1.5 MB Web Setup · Free &amp; Open Source · 100% Offline AI
</p>

---

## What's New in Asteria v5.5.2

**Asteria** is a clean-slate Japanese IME written across **19 Rust crates** with local GPU AI (`Genmai-310M` + `TinySwallow-1.5B`, Vulkan + automatic CUDA acceleration). `v5.5.2` fixes critical live-usage issues reported in `v5.5.1`:

### Fixes in v5.5.2
- **Live Japanese Conversion (`ITfKeystrokeMgr::AdviseKeyEventSink`) & Taskbar `あ` / `A` Icon Fix**:
  - Registered `ITfKeyEventSink` via `ITfKeystrokeMgr::AdviseKeyEventSink` in `ActivateEx` (and `UnadviseKeyEventSink` on deactivate) so TSF actually delivers keystrokes to Asteria for romaji-to-kana/kanji conversion instead of passing raw ASCII through.
  - Stopped `ModeItem::GetStatus()` from hiding the taskbar mode indicator with `TF_LBI_STATUS_HIDDEN`, synchronized `GUID_COMPARTMENT_KEYBOARD_INPUTMODE_CONVERSION` for Windows 10/11 Modern taskbar display, premultiplied `HICON` alpha, and added `VK_IME_ON` / `VK_IME_OFF` / `VK_DBE_HIRAGANA` handling while keeping `Win+Space` and standalone modifiers passed through.
- **Settings App (`asteria-config.exe`) Startup Crash Fix**:
  - Cached runtime GPU detection (`probe_runtime_gpus`) via `OnceLock` and kept `ggml*.dll` loaded in-process, preventing repeated `LoadLibrary`/`FreeLibrary` cycles from triggering `GGML_ASSERT(prev != ggml_uncaught_exception)` (`0xc0000409`) inside `ucrtbase.dll`.
  - Switched `asteria-config.exe` to the Windows GUI subsystem so no console window flashes on launch.
- **Legacy Input Entry Cleanup**:
  - Extended `InputList.Ensure()` to clean orphaned legacy entries from `HKCU\Control Panel\International\User Profile\ja` alongside `InstallLayoutOrTip`.

<details>
<summary>Earlier fixes included from v5.5.1</summary>

- Unified 64-bit install path (`-arch x64`, `C:\Program Files\Asteria`) + automatic `InstallLayoutOrTip` registration (`InputList.Ensure()` & `EnsureUserTipProfile`).
- WebView2 `<script>` DOM ordering fix and automatic `asteria-server.exe` launch in `asteria-config.exe`.
- Compact **~1.5 MB** `Asteria-Setup.exe` (`PlatformTarget x86`, STA thread, hidden `/quiet` apply window, crescent app icon, `aruiki/asteria-downloads` Web Setup).

</details>

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

1. Download and run **[Asteria-Setup.exe](https://github.com/aruiki/Asteria-Japanese-IME/releases/download/v5.5.2/Asteria-Setup.exe)** (about 1.5 MB). During setup it downloads Asteria itself (about 1.5 GB), plus the CUDA add-on (about 0.5 GB) on PCs with a supported NVIDIA GPU. An internet connection is needed only while installing (for offline PCs, see [INSTALL.md](https://github.com/aruiki/Asteria-Japanese-IME/blob/main/docs/INSTALL.md)).
2. The installer is not code-signed, so Windows may show "Windows protected your PC". Click **More info**, then **Run anyway**.
3. Press **Windows + Space** and choose **Asteria**.

`checksums.txt` is optional and lets you verify with PowerShell (`Get-FileHash -Algorithm SHA256 .\Asteria-Setup.exe`) that the download was not damaged or altered.

---

[README](https://github.com/aruiki/Asteria-Japanese-IME#readme) · [User guide](https://github.com/aruiki/Asteria-Japanese-IME/blob/main/docs/USER_GUIDE.md) · [Report a bug or a wrong conversion](https://github.com/aruiki/Asteria-Japanese-IME/issues/new/choose)
