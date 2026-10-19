# GPU AI主体の候補表示：新MSIと追加受入れ（2026-10-19）

## 固定した対象

- 製品PR171 head: `76aa7e32f9cebac5eafdbacff6c1738f556a7853`
- CI checkout: `16c3a3ea62ba7dfe4a9b5a82030240d48938a5f8`
- main統合: `039886e8f5377318512fdc8bff8750e7f13c906e`。上記3commitのtreeは同一。
- Mozc source: `8af4797f92689474635dd49cf8163cef7aedc198`
- Windows run: [39213557813](https://github.com/aruiki/Asteria-Japanese-IME/actions/runs/39213557813)、job117932472079、head76aa7e3
- MSI: `Asteria64.msi`、ProductVersion `3.34.6438.100`、SHA-256 `c21972edab537e1ea1473f63312f188b5b533baa700aa77fa9c3111364818c32`

PR171の全9チェックが成功後、merge commitで統合。Windowsの同じrunでbuild・同梱モデル/版・C++テスト・Qt応援画面・インストーラー導入/日本語入力/削除がすべて成功した。installer-smokeのresult.jsonはsource_commit16c3/run39213557813とMSI SHAを記録し、install/input/uninstall=true、stage=finished、errors=[]。「日本語」「東京」をネイティブ入力で確定した。削除exit3010でreboot_required=true、再起動は検査していない。

導入されたserver SHA `9569772ec8d5856dc6b8931c21b5b2422155af26d720fab9c99daf9ac0b3e11b`、TIP SHA `0ae798b3a4fdf1787208b4572259475eeeb2eea6a203b509eef730495b0b7b7b`を原票に保持する。

## 追加候補操作と再導入

`Candidate installer acceptance`をこのrun/head/MSI SHAへ更新した。固定した成功runのartifactだけを取得し、SHAが違えば失敗する。製品MSIを再build・編集しない。`installer_smoke.py --acceptance --reinstall`でEDIT/RichEdit、候補操作、取消・編集・文字種、フォーカス/保護欄の見える挙動、同一MSI再導入・再入力・削除を使い捨てWindows上で実行する。

追加8項目と再導入は、この更新後のCI結果を確認してから成功とする。旧run39213557622は公開6435で修正ハーネスを検査した原票であり、今回の6438の結果ではない。実TSFでGPU AIの寄与、内部privacy、複数実アプリ、旧版からの更新、低性能実機、独立検証をこの受入れだけで完了扱いしない。

## ソース評価と比較

詳細は[候補表示の実装比較](ai-first-publication-20261019.md)。開発978出力・予測29件の先頭表示は従来と一致。最終評価は`kotori-heldout.json`の300例で290/300、旧6432候補と全出力一致。データSHA `4b7bdcfe74aa3c186fa4e215b09e0a80d512f6aba47f415fae2271e8f11c0051`。旧候補との4モデルは同じバイトだがruntime DLL11件が異なり、純粋な単一変更の効果や速度向上を主張しない。最終評価は調整に使っていない。

原票は`target/accuracy-guide/ci-artifact-39213557813/`、`ai-first-heldout-v5/`、`ai-first-release-inputs-76aa7e3.json`。公開時には新MSIの管理展開検査、追加受入れのrun、build入力の一致、公開後再取得のSHAを公開provenanceへ記録する。次の別枝ee3fa5fの文脈修正はこのMSIへ含めていない。以前の失敗記録を成功へ書き換えない。

## 追加受入れと公開の確定結果

run39225823304で同じSHA c219の6438 MSIを実導入し、追加8項目・再導入・入力・削除が全成功。resultはinstall/input/uninstall/reinstall=true、stage=finished、errors=[]。acceptance.complete=trueで8件全passed、実際の導入TIPのpathとserver/renderer/TIPのSHAも一致した。PR172は一般7件を含む全8チェック成功後、main3da3672へ統合した。

v1.1.4-rc.2のタグはPR171のmerge039886e（CIとtree完全同一）へ固定して公開。公開バイトの再取得検査も成功した。[公開記録と範囲](release-v1.1.4-rc.2-20261019.md)。上記の「確認してから成功とする」は追加CI前の記録であり、この結果で新MSIについて確認できた。再起動や内部privacyの未確認範囲は引き続き残る。
