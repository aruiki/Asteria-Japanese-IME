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

[PR178](https://github.com/aruiki/Asteria-Japanese-IME/pull/178)を全7CI（run39533952262）成功後にmain 9054d35860008c63e180e63917de4c9554b783f3へ統合。公開README、導入ガイド、3画像をHTTP200で再取得し、コミットの生バイトと一致。GitHub紹介文とRC4本文も更新し、原稿と一致した。公開GitHub画面のREADME・RC4本文を1440/390pxの4条件で確認し、全画像が読み込まれた。目視でも確認した。

RC4の4配布資産のID・作成/更新時刻・size・digest・stateがすべて変更前と一致。tag feaeddc3も不変。RC4のprerelease=true、既存RC2のLatest/prerelease=falseを維持。main内の100 build入力blobが固定表と一致した。実MSIの再ビルド・再梱包・再アップロード・新たな版タグは作成していない。

## 公式紹介サイト

既存の[公式サイト](https://aruiki.github.io/kanai/)も刷新。日本語6ページ・英語1ページ、導入/FAQ/設計/公開状況、OG画像を統一した。旧比較・CSVと出典は日付付きの過去資料へ保持。紹介先をRC4の明示URLへ固定し、Latestから版表示だけが自動で変わる処理を停止した。本文・構造化データ・SHA・配布区分を揃えた。

[aruiki/kanaiのPR8](https://github.com/aruiki/kanai/pull/8)をmain 05befd9a4900cc1680b48333e114125e5f4eefa5へ統合。変更はsite-assets/pages、サイト用検査のreleaseVersion、運用記録のみ。このリポジトリにPR用CIはなく、既存の静的検査で14 HTML、264ローカル参照、16アンカーが成功。外部178リンクは構文確認であり、全外部リンクの実取得検査ではない。

gh-pagesの既存履歴を保ち、20配備ファイルをmainの生バイトから配置。削除0、配備commit 6afbd5a2487a8c660e6c26a280d1aa7bdca55d97。[Pages run39535159315](https://github.com/aruiki/kanai/actions/runs/39535159315)成功、Pages buildも同commitでbuilt。公開URLから全20ファイルを実取得し、HTTP200・SHA-256が配備元と全一致した。

公開サイト全7ページ×1440/390pxの14条件で、画像欠落・横はみ出し・ページ例外0、h1各1、RC4表示、スキップリンクへのキーボード到達を確認。FAQ/過去評価を開いた表示も撮影し、公開トップをPC・スマートフォンで目視した。アプリの受入れ検証ではない。

| 公開確認の原票（target/accuracy-guide/） | SHA-256 |
| --- | --- |
| marketing-public-v1.json | 2ab7239f4a1950f8f4238a23d44e0502c868bc46e280d0d1740d622994f4ac86 |
| marketing-public-browser-v1/summary.json | c424905360faac394d6d34bd69f4b810b6a1e3770d13ab67c694874bd9dd1f25 |
| marketing-site-public-v1.json | 0a934512a8c200a5730cf86c879bb4d776df2b488ef1632eb44c79baeeca1e90 |
| marketing-site-public-browser-v1/summary.json | 71134496f0af8a6aabd3fd200ca5a46b59e252feaefdf502ca5ebdf366df8946 |

[素材一覧](../marketing/README.md)、[主張と出典](../marketing/claims.md)、[作業カード82](../tasks/done/82-marketing-refresh.md)。実アプリGPU寄与などの製品検証残件は[RC4公開記録](release-v1.1.4-rc.4-20261021.md)から引き継ぎ、今回の画面確認で代用しない。
