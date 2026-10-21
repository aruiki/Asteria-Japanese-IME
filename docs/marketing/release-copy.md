<p align="center"><img src="https://raw.githubusercontent.com/aruiki/Asteria-Japanese-IME/0dae1d19c047c18e3af9767f6a1421459903b61c/docs/marketing/assets/social.png" alt="Asteria — GPUで考える、日本語入力。v2.0.0正式版。" width="100%"></p>

<h2 align="center"><a href="https://github.com/aruiki/Asteria-Japanese-IME/releases/download/v2.0.0/Asteria64.msi">↓ Asteria v2.0.0をダウンロード</a></h2>

<p align="center">Windows 10 / 11・64bit · 正式版 · 2026-10-21 · 1.66 GB · 未署名</p>

## Asteria v2.0.0 — 正式版

**GPUで考える、日本語入力。** 文脈を読むAIも、あなたのPCの中に。

AsteriaはMozcの辞書・文節処理・学習と、ローカルAIの候補生成・採点を組み合わせたWindows日本語入力です。文章を外部サーバーへ送らず、モデル・実行環境を同梱してオフラインで動作します。

### v1.1.3からの主な更新

- 通常のAI経路では、最初の生成・採点結果が完成してから変換・予測候補を表示します。AIの準備中も、読みの表示・編集・取消を続けられます。
- Enterで確定を待ちながら続けて打った文字を順番に処理し、直前の確定文を次のAI文脈へ引き継ぎます。
- Standard向けの追加生成モデルを同梱し、候補の探索・順位付けと通常変換の連語を改善しました。AIが採点した別候補を、読みと文節が合う場合に一覧へ残します。
- 予測の期限付近でも採点済み候補を保持し、弱い予測が先頭の本文を置き換えないようにしました。
- AI待機中のSpace／Tabの後にEnterで確定したとき、確定済みの文が未確定表示にも残る原因を修正しました。同じ文を意図して2回入力する動作は保持します。

v2.0.0はこれらの累積更新をまとめた正式版です。変換の第一候補の精度や一般的な速度が、RC4から向上したことを示す更新ではありません。

### ダウンロードと導入

このページ下部の **Assets → Asteria64.msi** を取得して開き、インストーラーの案内に従ってください。導入後は **Windows + Space → Asteria日本語入力** を選択します。設定はスタートメニューやデスクトップの「Asteria」から開けます。

Windows 10 / 11・64bit向け。GPU AIはVulkan対応の専用GPU（VRAM 3 GiB以上）を使用します。GPUのないPCはCPU向けのLow設定で動作します。内蔵GPUはAIに使いません。メモリ8 GB以上（16 GB推奨）、ディスクに約1.7 GBの空き容量が必要です。

モデルの手動配置やコマンド入力は不要です。OSによる管理者確認や再起動要求が出る場合は、表示に従ってください。

### 署名とSmartScreen

**配布MSIは未署名です。** SmartScreenの「WindowsによってPCが保護されました」が表示される場合があります。公式Releaseのファイルであることと、添付のSHA-256ファイルを確認してください。続行する場合は **詳細情報 → 表示内容を確認 → 実行** を選択します。セキュリティ機能を無効にする操作は不要です。

[導入・ファイル照合の手順](https://github.com/aruiki/Asteria-Japanese-IME/blob/main/docs/INSTALL.md) ／ [Microsoft公式のSmartScreen説明](https://learn.microsoft.com/ja-jp/windows/apps/package-and-deploy/smartscreen-reputation)

### 検証の範囲

実装・修正前の失敗と修正後の検証は[二重入力の検証記録](https://github.com/aruiki/Asteria-Japanese-IME/blob/main/docs/verification/duplicate-commit-20261021.md)へ保存しています。配布MSIの版・SHA-256・対応ソース・実入力検査の結果は、Releaseへ添付する配布情報とソース対応情報を参照してください。

固定した開発評価の94.0%（AJIMEE Standard、前文あり188/200）はRC4の測定値で、全アプリ・全入力の正解率ではありません。今回の修正はランキング・辞書・モデルを変更していません。

導入済みTSFのGPU AI ON/OFF寄与、低性能実機、保護欄内部のAI/学習、旧公開版からの更新、再起動、独立した製品完成判定は未検証です。正式版としての公開は、これらをすべて検証済みとする宣言ではありません。

[紹介・使い方](https://github.com/aruiki/Asteria-Japanese-IME#readme) ／ [更新履歴](https://github.com/aruiki/Asteria-Japanese-IME/releases) ／ [不具合・誤変換を報告](https://github.com/aruiki/Asteria-Japanese-IME/issues/new/choose)

MSI ProductVersion: 3.34.6442.100。SHA-256: 1bc531b10b25cc46fbc60a8b7b052187f2b69c9065e8a1651acc23e89bd1b72a。

新MSIの導入・再導入・実入力・削除、既存8操作、EDIT/RichEditの二重入力28条件とEdgeの14条件を確認しました。受入れ範囲は添付の[配布情報](https://github.com/aruiki/Asteria-Japanese-IME/releases/download/v2.0.0/distribution-info.json)、ソースは[対応情報](https://github.com/aruiki/Asteria-Japanese-IME/releases/download/v2.0.0/source-verification.json)をご覧ください。
