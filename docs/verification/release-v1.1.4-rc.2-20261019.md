# v1.1.4-rc.2公開記録（2026-10-19）

[リリース](https://github.com/aruiki/Asteria-Japanese-IME/releases/tag/v1.1.4-rc.2)を2026-10-19T11:41:57Zに公開し、公開ファイルを再取得して照合した。GPU AIの最初の生成・採点結果を待って候補を表示し、待機中の入力・編集・取消と保留Enter/後続入力を処理する。[実装比較](ai-first-publication-20261019.md)。

## ソースと配布物

- タグ/公開commit: `039886e8f5377318512fdc8bff8750e7f13c906e`（PR171 merge）
- build head: `76aa7e32f9cebac5eafdbacff6c1738f556a7853`、checkout: `16c3a3ea62ba7dfe4a9b5a82030240d48938a5f8`。3commitのtreeは完全一致。
- Mozc source: `8af4797f92689474635dd49cf8163cef7aedc198`
- [Windows run39213557813](https://github.com/aruiki/Asteria-Japanese-IME/actions/runs/39213557813)の同一MSIをADR0044で再利用。PR171全9チェック成功後に統合。
- ProductVersion: `3.34.6438.100`、1,662,767,104 bytes
- SHA-256: `c21972edab537e1ea1473f63312f188b5b533baa700aa77fa9c3111364818c32`

MSI・外部SHA・distribution-info.json・source-verification.jsonの4資産を公開。全アップロード資産のGitHub digest/sizeが実ファイルと一致することを確認してから公開した。タグの実commitも照合。RC2はprerelease=true、Latest=false。従来のrc.1がGitHub上Latest/prerelease=falseである状態は変更していない。READMEの導入リンクはRC2へ直接向けている。AuthenticodeはNotSigned、配布情報に実測値を保存した。

## 同じMSIで確認したこと

同じWindows runでbuild・C++・Qt画面・同梱モデル/版・実導入/「日本語」「東京」入力/削除が成功。さらに[run39225823304](https://github.com/aruiki/Asteria-Japanese-IME/actions/runs/39225823304)でEDIT/RichEditの追加8項目、同一MSI再導入・再入力・削除が成功。PR172全8チェック成功、main3da3672へ統合。[受入れ記録](ai-first-installer-20261019.md)。削除3010/reboot_required=trueは再起動確認ではない。

管理展開とcheck_release.pyで実公開6435から先頭3項目の版が上がること、固定4モデル・連語34,661件・出典/ライセンス・通信DLL不在・server版一致を確認した。TIP32/64の製品アイコン群102と設定toolの各8フレームもソースICOと完全一致。

公開後、verify_public_rc1142_v1.pyでMSI/SHA/配布情報/ソース情報を新しい保存先へ再取得。MSIは同じSHAとsize、メタデータ3件も公開前と全バイト一致。check_release.py --local --previous-local（実公開6435）--require-generator --require-collocationsはexit0。公開PEのアイコン資源も再照合した。

原票は`target/accuracy-guide/ci-artifact-39213557813/`、`candidate-acceptance-39225823304/`、`release-rc1142-assets-v1/`、`public-rc1142-check-v1/`。最初の公開資材stageは、内容/アイコン検査成功後にWindows PowerShellのSecurityモジュールautoloadだけ失敗した。既定shellで同じMSIの署名情報を正常取得し、成功済み検査を繰り返さずメタデータ作成から続行した。

## 品質と範囲

開発978出力と予測29件の先頭表示は従来と一致。Standard AJIMEEはAI準備後・前文あり188/200、なし175/200を維持。最終評価300例は290/300、旧6432と全出力一致。ただし旧候補とruntime DLL差があり、同一DLLの速度比較ではない。

実TSFのAI ON/OFF寄与・複数アプリ・低性能実機・旧版からの更新/再起動・内部privacy・独立verifierは未完了。次の文脈引継ぎ修正ee3fa5fはこのMSIへ含めていない。製品全体の完成や`.goal-complete`の判定ではない。
