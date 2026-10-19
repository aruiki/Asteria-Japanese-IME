## 次のv1.1.4リリース候補の変更内容

- GPUで動くAIの最初の結果を待って、変換・予測候補を表示するようにしました。
- 待機中も文字の入力・編集・取消を続けられ、Enterでの確定と続けて打った文字を順番に処理します。
- 既存の変換品質・予測候補を維持し、候補操作・履歴・かな入力・再変換の動作を確認しています。

---

## v1.1.4-rc.1までの変更内容

- 変換候補の探索と順位付けを改善し、Standard向けの追加生成モデルを同梱します。追加モデルは候補生成だけに使い、元のzenzとTinySwallowで採点します。
- 同じAJIMEE200問・前文付き・モデル準備後の開発比較で188/200（94.0%）を維持。短い入力と似た読みが続く文の誤変換を修正し、開発326問で306→308正解（2改善・改悪0）。未使用データや実アプリ全般の精度を表す数値ではありません。
- 文脈やAIに依存しない初期変換へ用例の連語を追加しました。同じ日常81例のAIなし比較で65→69正解（4改善・改悪0）。他の開発セットの品質を維持しています。
- AIの準備中も通常の変換候補を表示します。文字種候補を隠す設定で、前後のページ移動がページ先頭へ移らない不具合も修正しました。F6〜F10の直接変換は維持しています。
- AI起動時のメモリ確保失敗で入力プロセスが終了する問題に対処しました。限定した低メモリ制限の検査は成功していますが、低性能ノートPC全般での検証は継続中です。
- モデルはMSIへ同梱し、実行時のダウンロードは不要です。モデルの固定SHAと派生元・変更点を同梱します。
- 起動直後の内部キー処理でAI実行環境の初期化を待たないようにしました。CPU/GPUの隔離セッションでは、最初のキー処理で約100msあった待ちを削減しました。Windows実アプリの表示応答は別途確認します。

---

GitHub Releaseページ下部の **Assets** から、このリリースの **Asteria64.msi** と **Asteria64.msi.sha256** を取得してください。

<p align="center">
  <strong>Windows 10 / 11・64bit</strong>
</p>

<p align="center">
  <a href="https://github.com/aruiki/Asteria-Japanese-IME/releases/latest">
    <img alt="最新版" src="https://img.shields.io/github/v/release/aruiki/Asteria-Japanese-IME?label=%E6%9C%80%E6%96%B0%E7%89%88&color=242C5C">
  </a>
  <a href="https://github.com/aruiki/Asteria-Japanese-IME/releases">
    <img alt="ダウンロード数" src="https://img.shields.io/github/downloads/aruiki/Asteria-Japanese-IME/total?label=%E3%83%80%E3%82%A6%E3%83%B3%E3%83%AD%E3%83%BC%E3%83%89&color=242C5C">
  </a>
  <img alt="Windows 10 / 11" src="https://img.shields.io/badge/Windows-10%20%2F%2011-242C5C?logo=windows">
  <img alt="オフライン動作" src="https://img.shields.io/badge/AI-%E3%82%AA%E3%83%95%E3%83%A9%E3%82%A4%E3%83%B3-F25C2E">
</p>

> [!NOTE]
> `Asteria64.msi` の実行時に Microsoft Defender SmartScreen の<br>
> **「WindowsによってPCが保護されました」**  
> という画面が表示される場合があります。

---

## SmartScreenが表示された場合

<p align="center">
  <img
    src="https://www.sanwa.co.jp/static/images/seihin_joho/windows11_protection/img_1-1.png"
    alt="WindowsによってPCが保護されました と表示されたSmartScreen画面"
    width="500">
</p>

<p align="center">
  <sub>
    SmartScreenの日本語表示例 —
    <a href="https://www.sanwa.co.jp/seihin_joho/windows11_protection/index.html">
      出典：サンワサプライ
    </a>
  </sub>
</p>

この表示だけで、Asteriaからウイルスやマルウェアが検出されたという意味ではありません。

Microsoft Defender SmartScreenは、危険なファイルの検出だけでなく、
インターネットからダウンロードしたアプリや発行元について
**Windows上で十分な「評判（reputation）」が確立されているか**も確認しています。

Microsoft公式ドキュメントでも、**署名されていないアプリでは
「Windows が PC を保護しました」という警告が表示される**ことが説明されています。

[Microsoft公式：SmartScreenの評判について](https://learn.microsoft.com/ja-jp/windows/apps/package-and-deploy/smartscreen-reputation)

---

## インストールを続けるには

> [!IMPORTANT]
> このGitHub Releaseからダウンロードした  
> **`Asteria64.msi`** であることを確認してください。

1. <kbd>詳細情報</kbd> をクリック
2. 表示内容を確認
3. <kbd>実行</kbd> をクリック

これで通常のAsteriaインストーラーが起動します。

**SmartScreenやMicrosoft Defenderを無効にする必要はありません。**

---

## なぜこの警告が出るの？

SmartScreenは単純なウイルス検査だけではなく、
ファイルや発行元の**評判**も利用して判断しています。

そのため、

**「WindowsによってPCが保護されました」  
＝「マルウェアが検出された」**

ではありません。

Asteriaはソースコードを公開しているオープンソースソフトウェアです。

<details>
<summary><strong>安全性についてさらに確認する</strong></summary>

<br>

- [ソースコード](https://github.com/aruiki/Asteria-Japanese-IME)
- [最新Release](https://github.com/aruiki/Asteria-Japanese-IME/releases/latest)
- [Security Policy](https://github.com/aruiki/Asteria-Japanese-IME/security/policy)
- [Microsoft公式：SmartScreenの仕組み](https://learn.microsoft.com/ja-jp/windows/apps/package-and-deploy/smartscreen-reputation)

AsteriaのAIモデル・辞書・日本語変換処理はPC内で動作し、
**入力した文章をAsteriaのサーバーや外部AIサービスへ送信しません。**

</details>

---

> [!WARNING]
> SmartScreenの「認識されていないアプリ」という警告ではなく、
> Microsoft Defenderなどから**具体的な脅威名やマルウェア名**が表示された場合は実行しないでください。

---

## 今回の更新（v1.1.1）

Kotoriを気に入っていただけた方へ、GitHubの ⭐ Starで応援をお願いする案内を追加しました。

- 「GitHubで応援する」でページを開くと、以後このお願いは表示されません。
- 「いいえ」または閉じる操作では、1週間後に再度表示します。
- 入力処理とは別に確認し、操作が落ち着いたタイミングで表示します。入力内容は収集しません。
- 停止・延期の状態はPC内に保存し、アップデート後も引き継ぎます。

インストーラーの内部版は **3.34.6336.100** です。コード署名は引き続きありません。
Windowsで案内の7テスト・関連9テスト対象と、100%・200%の画面表示を確認しました。
今回のMSIをこのPCへ導入しての実入力・アンインストール確認は未実施です。
詳細は[検証記録](https://github.com/aruiki/Asteria-Japanese-IME/blob/v1.1.1/docs/verification/github-support-reminder-20261011.md)をご覧ください。

---

## 今回の更新（v1.1.2）

日本語入力の表示名を **Asteria — アステリア** に変更しました。READMEと図版、製品アイコン、インストーラーの見た目を「月の余白」のデザインへ揃えています。設定・辞書ツールのアイコンと、これまでの設定・辞書・履歴の保存先は引き継ぎます。

GitHubでの応援をお願いする案内を、Windows標準の通知へ変更しました。

- 「GitHubで応援する」でページを開くと、以後このお願いは表示されません。
- 「いいえ（1週間後に再表示）」または閉じる・応答しない場合は、1週間延期します。
- 通知音は鳴らさず、Windowsの通知設定に従います。停止・延期の記録は更新後も引き継ぎます。
- インストーラーの進行状況を、短く読みやすい日本語へ変更しました。

インストーラーの内部版は **3.34.6338.100** です。コード署名は引き続きありません。
ローカルのWindowsビルド、通知の8テスト、実行ファイルのCOM応答処理、MSIの通知登録と版番号を確認しました。
今回のMSIを導入してのWindows通知の実表示・通知センターからのボタン操作・日本語入力・アンインストール確認は未実施です。
詳細は[通知の検証記録](https://github.com/aruiki/Asteria-Japanese-IME/blob/v1.1.2/docs/verification/windows-support-toast-20261011.md)と[デザインの検証記録](https://github.com/aruiki/Asteria-Japanese-IME/blob/v1.1.2/docs/verification/asteria-moon-20261011.md)をご覧ください。


---

## 今回の更新（v1.1.3）

日本語の入力補正とAI変換を改善しました。

- 日本語を含むモデル・推論環境・文脈ファイルの保存先を正しく扱い、AIが意図せず無効になる問題を修正しました。
- AI生成候補で「電気を消してね」が「電気を消して寝」のように文末の文法語を変えてしまう問題を修正しました。
- Mozcが補正した先頭候補をAIが取り消す問題を修正しました。「こんいちは→こんにちは」「りょうしゅしょをください→領収書をください」を、高速・待機・通常打鍵の3条件で確認しています。

StandardのAJIMEEは **183/200＝91.5%** を維持し、今回の補正保護の前後で調整用386問の出力が一致しました。隔離した変換セッションで万単位の入力探索を行い、CPUのみの1,000例＋90操作、通常キー操作、モデル読み込み前後の検査も実施しました。

意味選択や誤入力補正には改善の余地があります。低性能ノートPC、複数のWindowsアプリでの入力、長時間利用の受入れは未確認です。コード署名は引き続きありません。Windows CIで当該MSIの導入、「日本語」「東京」のTSF実入力、削除を確認しました。削除では再起動要求（終了コード3010）がありました。

インストーラーの内部版は **3.34.6431.100**。公開ファイルは検証済みCI成果物と同一です。最終評価用300問は **289/300＝96.3%** を維持しています。版・SHA-256・検証条件は[公開用MSIの検証記録](https://github.com/aruiki/Asteria-Japanese-IME/blob/v1.1.3/docs/verification/release-v1.1.3-20261012.md)をご覧ください。

詳細は[大規模探索と補正保護の検証記録](https://github.com/aruiki/Asteria-Japanese-IME/blob/v1.1.3/docs/verification/large-input-campaign-20261012.md)、[文末保護の検証記録](https://github.com/aruiki/Asteria-Japanese-IME/blob/v1.1.3/docs/verification/generated-grammar-ending-20261012.md)をご覧ください。
