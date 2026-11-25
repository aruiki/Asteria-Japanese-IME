<h2 align="center">
  <a href="https://github.com/aruiki/Asteria-Japanese-IME/releases/download/v5.5.0/Asteria-Setup.exe">
    ⬇ Asteria-Setup.exe をダウンロード（v5.5.0・推奨）
  </a>
</h2>

<p align="center">
  <strong>Windows 10 / 11（64-bit）</strong> · 純 Rust 製ネイティブエンジン × ローカル GPU AI · 完全無料 / オープンソース
</p>

---

## Asteria v5.5.0 — Pure Rust アーキテクチャ統合 ＆ 単一インストーラ正式版

既存の C++ ベース IME に依存しない **19 クレートの純 Rust 日本語入力エンジン** へとコアを全面刷新し、単一インストーラ **`Asteria-Setup.exe`**（Vulkan 標準＋NVIDIA CUDA 自動導入）および精度・速度の大幅強化を収録した正式リリースです。

### 主なハイライト

1. **Pure Rust TSF ＆ 変換エンジン (`asteria-tip-win` / `asteria-dict` / `asteria-lattice` / `asteria-server`)**
   - 入力管理・ダブル配列トライ辞書・Viterbi K-best 探索・文脈学習・IPC サーバー・WebView2 設定 UI まで Rust で一新（基盤クレート `#![deny(unsafe_code)]`）。
   - 版付き TSF DLL 管理と独自 MSI カスタムアクション（`asteria-msi-action`）により、**作業中のアプリを閉じたり Windows を再起動したりせずに上書きアップデート**が可能です。
2. **「残り 2% の誤変換」をゼロへ近づける精度・文脈ガード強化**
   - **`MatchesReading` 読み完全検証 ＋ 複合語連濁フォールバック**: AI が生成した候補の読み整合性を辞書レベルで照合し、文字化けや別語化を排除しつつ「夜駆け」「支援試合」等の連濁語も正確に通過。
   - **孤立再採点 `[copy]` ＋ 既定辞書引きずり防止**: 前の文で使われた同音語へ引きずられる誤変換を抑制（「口座を開設しました。」に続く「にほんごをかいせつ」→「日本語を解説」等）。
   - **2 段階ローカル AI (`Genmai-310M` Drafted Beam 6 ＋ `TinySwallow-1.5B` Judge)** と **`llama.cpp` CUDA (`Flash Attention`) 自動判別** を統合。
3. **実測ベンチマーク (`Standard` / NVIDIA GeForce RTX 3060)**
   - **実用マルチ正解 (`usability-multi`, 120問)**: **100.0%** (`CER 0.00%`)
   - **同音異義語 (`homophone`, 40問) / 慣用句 (`idioms`, 25問) / 報告回帰 (`reports`, 6問) / ゴールデン (`golden`, 3問)**: **100.0%**
   - **日常文 (`daily`, 81問)**: **CUDA 100.0%** / **Vulkan 98.8%**（`Acc@10 100.0%`）
   - **未調整ホールドアウト (`heldout`, 300問)**: **99.0%**（`Acc@10 100.0%`, `CER 0.14%`）
   - **難文 (`ajimee`, 200問)**: **95.5%**（`Acc@10 98.5%`, `CER 0.40%`）
   - **E2E 応答時間**: 従来 Google 日本語入力 / Mozc (`v3.34`) 比で `daily` 合計 **-32%** (`10.63s` vs `15.66s`)、`ajimee` 合計 **-18%** (`43.83s` vs `53.35s`)、最大遅延も半分以下 (`167.5ms` vs `847ms`)。

---

## インストールとファイル検証

1. 上の **[Asteria-Setup.exe](https://github.com/aruiki/Asteria-Japanese-IME/releases/download/v5.5.0/Asteria-Setup.exe)** をダウンロードして実行します（NVIDIA GPU 環境では CUDA アドオンも自動導入されます。MSI 単体が必要な場合は `Asteria64.msi` / `AsteriaCuda64.msi` をご利用ください）。
2. 未署名インストーラのため、**「Windows によって PC が保護されました」** と表示された場合は **「詳細情報」→「実行」** を選択してください。
3. 完了後、**Windows + Space** で「**Asteria 日本語入力**」へ切り替えます。

### 配布ファイル SHA-256

| ファイル | サイズ (Bytes) | SHA-256 |
| :--- | ---: | :--- |
| **`Asteria-Setup.exe`**（推奨・単一バンドル） | `2,108,216,824` | `30a48f63117c600cb9097e02d019edd08a6c3d52d8b87cbed35c1b5fb14fd30e` |
| `Asteria64.msi`（本体単体 MSI） | `1,572,483,072` | `21ec2fdadcf895017df19902934aca6108f274e0023d098fc647258d5bb565ac` |
| `AsteriaCuda64.msi`（NVIDIA CUDA アドオン MSI） | `536,281,088` | `15532c9a995cf4e739a78f84785ee32594d31f772ab9298ac36d148874e1ad96` |

> PowerShell で検証: `Get-FileHash -Algorithm SHA256 .\Asteria-Setup.exe`

---

[README（製品概要・アーキテクチャ）](https://github.com/aruiki/Asteria-Japanese-IME#readme) · [使い方ガイド](https://github.com/aruiki/Asteria-Japanese-IME/blob/main/docs/USER_GUIDE.md) · [不具合・誤変換の報告](https://github.com/aruiki/Asteria-Japanese-IME/issues/new/choose)
