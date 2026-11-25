# Asteria v5.5.1 告知・紹介原稿

対象: **v5.5.1（正式版・単一インストーラ `Asteria-Setup.exe`・未署名）**

## 短文（SNS / 紹介欄向け）

レガシー C++ IME の改造ではない、**19 クレートの純 Rust 製 Windows 日本語入力「Asteria v5.5.1」** を公開しました。
前文脈を読むローカル GPU AI（Vulkan / CUDA 自動判定）と読み完全検証により、実用評価 100%・通信ゼロ。単一インストーラで導入・無停止更新できます。

https://github.com/aruiki/Asteria-Japanese-IME/releases/tag/v5.5.1

## 詳細紹介文（ブログ / 配布ページ / 外部サイト向け）

Asteria は、Windows TSF DLL から辞書コンパイラ・Viterbi 探索・読み検証・文脈学習・IPC サーバー・WebView2 設定画面まで **Rust でゼロから設計し直した** オープンソース日本語 IME です。

- **純 Rust 19 クレート基盤**: メモリ安全性と軽量な応答性を両立し、版付き TSF DLL による再起動不要のアップデートに対応。
- **2 段階ローカル GPU AI**: `Genmai-310M`（Drafted Beam 6）と `TinySwallow-1.5B` をローカル GPU（Vulkan 標準＋NVIDIA 環境は CUDA 自動導入）で実行し、入力テキストは一切外部送信しません。
- **精度と速度の実測（v5.5.1, Standard, RTX 3060）**:
  - `usability-multi` 120問 **100.0%** / 同音異義語 40問 **100.0%** / 慣用句 25問 **100.0%** / 報告回帰 6問 **100.0%**
  - `daily` 81問 **100.0% (CUDA)** / **98.8% (Vulkan)**、`heldout` 300問 **99.0%**、`ajimee` 200問 **95.5%**
  - 従来 IME（v3.34）比で `daily` 合計時間 **32% 短縮**、`ajimee` 合計時間 **18% 短縮**

[リリースページ・SHA-256 照合・導入手順](https://github.com/aruiki/Asteria-Japanese-IME/releases/tag/v5.5.1)
