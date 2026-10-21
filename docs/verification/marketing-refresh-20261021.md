# 紹介・告知刷新の確認（2026-10-21）

対象はREADME、利用者向け文書、紹介画像と告知コピー。製品のビルドは実行していない。本体の挙動・精度・速度に変更を加えた作業ではない。

## 内容

- 「GPUで考える、日本語入力。」「文脈を読むAIも、あなたのPCの中に。」を軸にREADMEを再構成。
- 採用済みADR0056「月の余白」のロゴをそのまま使い、hero 1600×720、social 1200×630、portrait 1080×1350を制作。HTML/CSSの文字組みをEdgeで描画した。製品ICO・設定・インストーラーの画像入力は変更していない。
- 添付された紹介文のダウンロード案内、オフライン表示、SmartScreen、配布元確認と変更履歴を導入ガイド・Release原稿へ反映。旧v1.1.1〜v1.1.3の出来事をRC4の新機能へ混入させていない。第三者のSmartScreen画像は転載せず、Microsoft一次資料を参照した。
- READMEと利用ガイドに残っていた旧来の「準備中はMozc候補を先出し」の説明を現行のAI初回結果待ちへ修正。文書の説明を直したもので、ソース変更ではない。
- 94.0%などの測定値は条件・測定時点を維持。精度データJSON・過去図表・実入力GIFは変更していない。
- 現在の紹介先はRC4の明示URL。既存Latestが指すRC2の公開区分を変更しない。

## ローカル確認

`node docs/marketing/render.mjs` を既存Edgeで実行。3アートボードの全テキストがキャンバス内に入り、ページ例外0。3画像を目視した。

READMEとRelease原稿をGitHub Markdown APIのGFMでHTML化し、レスポンシブな確認用HTMLでEdge表示した。1440pxと390pxの各幅、折り畳みの開閉状態を撮影。全4条件で画像欠落0、横はみ出し0、ページ例外0。配布リンクはすべてRC4。README冒頭をPC・スマートフォンの実サイズで目視した。GitHub本体の公開画面は統合後に別途確認する。

紹介に関わる文書のローカル参照78件と見出しリンク15件が解決。`git diff --check` 成功。

| 原票（target/accuracy-guide/） | SHA-256 |
| --- | --- |
| marketing-render-v2.json | fe69b7ed3f6087ce5b6ba6194e327b34a4e8a6ae1719a2cd192e79578885cddb |
| marketing-browser-qa-v1.json | 60b202b2593e831e39404c5653b8e2303ea3277c3e56a3664caf2e3aea2ab145 |
| marketing-integrity-v1.json | 01eb2eb7d16c7632f1b378cc32e5ef0e44b2a003d8d09318e3e90fbaecd3f297 |

## 本体・配布物の固定

- 基点main: d7bf7c1b5b69c8ecaee37d95c8048cab3204f480
- 製品公開tag: v1.1.4-rc.4 → feaeddc3ee40a9efc3ab4049166e042fef1d69fa
- 公開時の固定表に対し、98 build入力blobと2 native harness blobが一致。HEAD内のblobに加え、作業ファイルもGitフィルター適用/生バイトを区別して照合。
- 最初の照合では、Git内にもCRLFが保存されているfirst_display_medical.jsonをGitの改行フィルターでLF化した値と比べ、1件の差が出た。公開commitと現HEADのblobは同じ211a1498…、実ファイルの生バイトも同blobと一致。ファイルを書き換えず、フィルター適用値と生値を記録して判別した。
- 実MSI: 1,662,828,544 bytes。全バイトを読み、SHA-256 **2b37f6fbdcb069251e820e2009b096111cc2253e79ffac65d973fa11ce18e301** が固定値・GitHub公開asset digestと一致。署名状態は既存公開記録のNotSignedを引き継ぎ、新たな署名操作は行っていない。
- 公開4資産のID・size・digestをmarketing-release-before.jsonへ保存。今回更新する公開情報は紹介文とRelease本文で、MSI・添付JSON・SHAファイル・tag・Latest区分を固定する。

## 公開反映

この記録時点ではローカル確認が完了。PRの全適用CI成功後にmainへ統合し、公開README・画像・RC4本文と配布物不変を確認する。マーケティング刷新の完了はこの確認後に判断する。

[素材一覧](../marketing/README.md)、[主張と出典](../marketing/claims.md)、[作業カード82](../tasks/82-marketing-refresh.md)。実アプリGPU寄与などの製品検証残件は[RC4公開記録](release-v1.1.4-rc.4-20261021.md)から引き継ぎ、今回の画面確認で代用しない。
