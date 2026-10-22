# 変換精度を中心にした紹介への変更（2026-10-22）

利用者が求める価値を「変換精度の高い日本語IME」とし、GPUという実装手段を主見出しにしたコピーを変更した。辞書/頻度/連語の改善は別枝で検証中であり、この紹介変更を本体の精度向上として扱わない。

## 変更対象と維持したもの

main769ed9eからcodex/quality-copy-20261022で作業。README、紹介原稿、Release本文原稿、3画像と代替文を変更。画像は既存の月を保持し、画像commit `db070f42662b63a000c0b3168cfb74c466d78c6f` の不変URLをRelease本文で使用する。

製品ソース、Bazelパッチ、モデル、ビルド設定、MSI、版番号は変更していない。v2.0.0/Latest、未署名、MSI3.34.6442.100を紹介し、公開4資産のサイズ/digestを作業開始時に記録した。MSI SHA `1bc531b10b25cc46fbc60a8b7b052187f2b69c9065e8a1651acc23e89bd1b72a`。同じ配布物の新しい導入検査を行ったという意味ではない。

前文あり94%の測定条件、前文なしの値、実TSFなどの未検証範囲、実録GIFと旧比較、支援、動作環境の記述を保持。Google日本語入力との最新比較や、短文の大規模辞書対策の完了は主張しない。GPU AI主体の非同期候補表示の設計は変えない。

## 公開前確認

- 変更したMarkdown5件の参照71件を解析し、ローカルのファイル参照が解決することを確認。
- HTML原稿から生成済みのhero/social/portraitが原稿内へ収まり、3画像を目視。紹介専用枝へ同じ画像blobを移した。
- README/Releaseのローカルプレビュー×1440px/390pxの4配置で、新見出し、版、ダウンロードURL、画像、横はみ出しと例外を検査し全成功。4配置を目視。
- 初回のRelease390pxプレビューは既存の長いSHAがはみ出した。プレビュー用CSSの折り返し指定不足であり、失敗JSON/画像を保存して折り返しを補った。公開GitHub上の表示は別途検査する。

原票は紹介専用worktreeの`target/quality-copy-v1/`。`source-check.json`、`preview-check-before-wrap.json`、`preview-check.json`、`release-before.json`と画像を保持する。製品のMSI・実入力・精度の新規検証として数えない。

## 公開確認

[PR183](https://github.com/aruiki/Asteria-Japanese-IME/pull/183)は全7チェック成功後、main `05d70ed443baa1b761221c44520a956f6a5f5689`へmerge commitで統合。CI run39697621925。コミット済み原稿からv2.0.0のRelease本文を更新し、改行表現を除く本文一致を確認した。

公開前後のRelease ID・tag・公開日時・Latest/正式区分と、4資産のID/名前/サイズ/digestはすべて不変。公開README/Release×1440px/390pxの4配置でHTTP200、新見出し、v2.0.0、MSIリンク、全画像の読込、横はみ出しなし、例外なしを確認した。Release画像とREADME本文を目視した。原票は`ci-before-merge.json`、`published.json`、`public-browser.json`。

公開画面の目視で、GitHub Aboutの概要欄だけに旧コピーが残っていた。本文検査の対象外だったため、概要欄を「変換精度の高い日本語IME。」へ更新し、APIの全文一致とhomepage不変を確認。原票`repository-description.json`。見出しの更新を本体の精度向上やGoogleとの最新比較として扱わない。

公式サイトはkanai PR12/13、gh-pages33a823f、Pages39693693438で新コピーを公開し、20ファイルSHA一致/14配置を確認済み。

短文の変換精度改善、未使用評価、Google同条件比較、導入済みGPU AIの寄与などの製品ゴールは継続中。過去の独立FAILと製品全体の未検証7範囲を保持し、製品完成とは扱わない。
