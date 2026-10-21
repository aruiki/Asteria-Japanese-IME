<p align="center">
  <img src="https://raw.githubusercontent.com/aruiki/Asteria-Japanese-IME/main/docs/marketing/assets/social.png" alt="Asteria — GPUで考える、日本語入力。文脈を読むAIも、あなたのPCの中に。" width="100%">
</p>

<h2 align="center">
  <a href="https://github.com/aruiki/Asteria-Japanese-IME/releases/download/v1.1.4-rc.4/Asteria64.msi">↓ Asteria64.msiをダウンロード</a>
</h2>

<p align="center"><strong>Windows 10 / 11・64bit</strong><br>v1.1.4-rc.4 · リリース候補 · 2026-10-21 · 約1.66 GB · 未署名</p>

**GPUで考える、日本語入力。** 文脈を読むAIも、あなたのPCの中に。

Asteriaは、Mozcの辞書・文節処理・学習と、ローカルAIの候補生成・採点を組み合わせた日本語入力です。変換する文章を外部サーバーへ送らず、オフラインで使えます。無料・オープンソースです。

## RC4で変わったこと

- AIが生成・採点した別候補を、読みと文節が合う場合に候補一覧へ残すようにしました。
- 予測の時間上限付近でも採点済み候補を保持します。
- 弱い予測が先頭の本文を置き換えないようにしました。

通常のAI経路では、最初の生成・採点結果が完成してから候補を表示します。その間も読みの表示・編集・取消を続けられます。AIの準備中に未評価の候補を先に表示する動作には戻していません。

## 使い始めるには

1. 上のリンクからMSIをダウンロードして開き、インストーラーの案内に従います。
2. 管理者の確認が出たら内容を確認します。OSが再起動を求める場合は、その案内に従ってください。
3. **Windows + Space → Asteria日本語入力**を選択します。設定はデスクトップやスタートメニューの「Asteria」から開けます。

モデル・実行環境は同梱済みです。GPU AIはVulkan対応の専用GPU（VRAM 3 GiB以上）を使用。GPUのないPCはCPU向けのLow設定で動作します。内蔵GPUはAIに使いません。メモリ8 GB以上（16 GB推奨）、ディスクに約1.7 GBの空き容量が必要です。

<details>
<summary>SmartScreenで「WindowsによってPCが保護されました」と表示されたら</summary>

配布MSIは未署名のため、SmartScreenが表示される場合があります。SmartScreenはファイルや発行元の評判も確認します。この画面だけでマルウェアが検出されたとは判断できませんが、安全性を保証する説明でもありません。[Microsoft公式の説明](https://learn.microsoft.com/ja-jp/windows/apps/package-and-deploy/smartscreen-reputation)。

この公式GitHub Releaseから取得した `Asteria64.msi` であることを確認し、続行する場合は **詳細情報 → 表示内容を確認 → 実行**を選びます。「実行」がない管理対象PCや別の脅威検出には、この手順を適用しないでください。SmartScreenやMicrosoft Defenderを無効にする操作は不要です。

[詳しい導入ガイドとファイル照合](https://github.com/aruiki/Asteria-Japanese-IME/blob/main/docs/INSTALL.md)

</details>

## 確認した範囲

同じMSIの導入・再導入・入力・削除と、EDIT/RichEditでの候補操作8項目を確認しています。固定した開発評価では978出力の第一候補が変更前と全一致し、正解候補の包含件数が増えました。第一候補の精度や一般的な速度の向上を示す更新ではありません。

実アプリのGPU AI ON/OFF寄与、複数アプリ、低性能実機、保護欄内部のAI/学習、旧版からの更新、再起動、独立した製品完成判定は未検証です。製品全体の完成宣言ではありません。

[配布条件と検証範囲（distribution-info.json）](https://github.com/aruiki/Asteria-Japanese-IME/releases/download/v1.1.4-rc.4/distribution-info.json) ／ [ソースとの対応（source-verification.json）](https://github.com/aruiki/Asteria-Japanese-IME/releases/download/v1.1.4-rc.4/source-verification.json) ／ [公開・検証記録](https://github.com/aruiki/Asteria-Japanese-IME/blob/main/docs/verification/release-v1.1.4-rc.4-20261021.md)

MSI ProductVersion: 3.34.6440.100。SHA-256:

~~~text
2b37f6fbdcb069251e820e2009b096111cc2253e79ffac65d973fa11ce18e301
~~~

[Asteriaの紹介・使い方](https://github.com/aruiki/Asteria-Japanese-IME#readme) ／ [これまでの更新履歴](https://github.com/aruiki/Asteria-Japanese-IME/releases) ／ [不具合・誤変換を報告](https://github.com/aruiki/Asteria-Japanese-IME/issues/new/choose)
