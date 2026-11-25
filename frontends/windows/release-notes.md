<h2 align="center">
  <a href="https://github.com/aruiki/Asteria-Japanese-IME/releases/download/v5.5.1/Asteria-Setup.exe">
    ⬇ Asteria-Setup.exe をダウンロード（v5.5.1・推奨）
  </a>
</h2>

<p align="center">Windows 10 / 11 (64-bit) · free and open source · runs fully offline</p>

---

## Asteria v5.5.1 — Pure Rust アーキテクチャ & 登録/設定UI 修正版

既存の C++ ベース IME に依存しない **19 クレートの純 Rust 日本語入力エンジン** へ刷新した `v5.5.0` に続き、実環境（ユーザーセッション）で報告されたインストーラ登録・設定画面不具合を根本修正した正式版です。

### v5.5.1 での主な修正

- **セットアップ後の入力方式自動登録（Windows + Space）**:
  - `Asteria64.msi` を 64-bit (`-arch x64`) ビルドへ修正し、本体 (`C:\Program Files\Asteria`) と CUDA アドオン (`C:\Program Files\Asteria\cuda`) の導入先を統一しました。
  - セットアップ UI (`InputList.Ensure()`) および MSI カスタムアクション (`EnsureUserTipProfile`, `Impersonate="yes"`) の両経路で `InstallLayoutOrTip` を実行し、インストール直後から現在ログイン中のユーザーの「入力方式一覧」へ **Asteria 日本語入力** が確実に追加されるよう修正しました（旧版 TIP の孤立レジストリ自動掃除も収録）。
- **設定アプリ (`asteria-config.exe`) の表示・起動修正**:
  - WebView2 インライン埋め込み `<script>` を `<head>` から `</body>` 直前へ移動し、UI が空白・読み込み中のまま止まる問題を修正しました。
  - `asteria-config` 起動時に同梱の `asteria-server.exe` を自動起動・待機接続し、`LocalLow\Asteria` データ領域および導入先 GPU 情報を正しく参照するよう修正しました。ウィンドウおよびスタートメニュー表示名も「Asteria」に統一しています。

### v5.5.0 からの主な特徴（Pure Rust × Local GPU AI）

1. **Pure Rust TSF ＆ 変換エンジン (`asteria-tip-win` / `asteria-dict` / `asteria-lattice` / `asteria-server`)**
   - 入力管理・ダブル配列トライ辞書・Viterbi K-best 探索・文脈学習・IPC サーバー・WebView2 設定 UI まで Rust で一新（基盤クレート `#![deny(unsafe_code)]`）。
   - 版付き TSF DLL 管理（`asteria_tip64-5.5.1.dll` / `x86\asteria_tip32-5.5.1.dll`）により、作業中のアプリを閉じず再起動不要で更新できます。
2. **高精度・高速ローカル AI パイプライン**
   - `MatchesReading` 読み完全照合（複合語連濁フォールバック対応）・孤立再採点 `[copy]`・`Genmai-310M` Drafted Beam 6 ＋ `TinySwallow-1.5B` Judge・CUDA (`Flash Attention`) 自動判別を搭載。
3. **実測ベンチマーク (`Standard` / NVIDIA GeForce RTX 3060)**
   - **実用マルチ正解 (`usability-multi`, 120問)**: **100.0%** (`CER 0.00%`)
   - **同音異義語 (`homophone`, 40問) / 慣用句 (`idioms`, 25問) / 報告回帰 (`reports`, 6問) / ゴールデン (`golden`, 3問)**: **100.0%**
   - **日常文 (`daily`, 81問)**: **CUDA 100.0%** / **Vulkan 98.8%**（`Acc@10 100.0%`）
   - **未調整ホールドアウト (`heldout`, 300問)**: **99.0%**（`Acc@10 100.0%`, `CER 0.14%`）
   - **難文 (`ajimee`, 200問)**: **95.5%**（`Acc@10 98.5%`, `CER 0.40%`）

---

## インストールとファイル検証

1. 上の **[Asteria-Setup.exe](https://github.com/aruiki/Asteria-Japanese-IME/releases/download/v5.5.1/Asteria-Setup.exe)** をダウンロードして実行します（`v5.5.0` からの上書きアップデートも可能です。NVIDIA GPU 環境では CUDA アドオンも自動導入されます）。
2. 未署名インストーラのため、**「Windows によって PC が保護されました」** と表示された場合は **「詳細情報」→「実行」** を選択してください。
3. 完了後、**Windows + Space** で「**Asteria**」へ切り替えます。

### 配布ファイル SHA-256

| ファイル | サイズ (Bytes) | SHA-256 |
| :--- | ---: | :--- |
| **`Asteria-Setup.exe`**（推奨・単一バンドル） | `2,108,226,490` | `5b105d1e72cc7e433cd0c6a012fd82954afcae98e4d9f8e76ea9003de5e82708` |
| `Asteria64.msi`（本体単体 MSI） | `1,572,544,512` | `ebbdcaab876420f13a5b6adbb523f9991c2a4d7bda07b317719102f6ac942580` |
| `AsteriaCuda64.msi`（NVIDIA CUDA アドオン MSI） | `536,281,088` | `aafbd3896c915db381cdc6c85c4c755770c4a1ac7524cc1d361d9405ec10a891` |

> PowerShell で検証: `Get-FileHash -Algorithm SHA256 .\Asteria-Setup.exe`

---

[README（製品概要・アーキテクチャ）](https://github.com/aruiki/Asteria-Japanese-IME#readme) · [使い方ガイド](https://github.com/aruiki/Asteria-Japanese-IME/blob/main/docs/USER_GUIDE.md) · [不具合・誤変換の報告](https://github.com/aruiki/Asteria-Japanese-IME/issues/new/choose)
