# Asteria「月の余白」反映の検証 — 2026-10-11

## 対象

利用者によるAsteria改名、比較10案から05の採用、READMEの図版・文章統一、進行画面改善の依頼。
ブランチ codex/asteria-brand、基点 de7a7fda9b0953b3d141cce8012b698038d0b29b。
Mozc upstream a069a88d4cb5c011de0f9aebb6c149a1c808d904、専用作業ツリー C:/Users/aruik/mzsupport。
本記録はローカルのソース・単体テスト・MSI内容確認。実機導入の合格記録ではない。

## 実装

- 表示名をAsteriaに統一。内部Kotori名、保存先、インストール先、GUID、MSI UpgradeCodeを保持。
- 製品アイコンとREADMEヒーローを05の月・墨・濃紺・生成りに展開。内蔵image_genで制作。
- 設定・辞書ツールの6アイコンとツール用従来製品アイコンは旧版と同一SHA。
- 16 / 20 / 24 / 32 / 48 / 64 / 128 / 256pxの製品ICOを検査。PNGからの縮小画像とICO内の画素が一致。
- 配布用TIP32/64に月の全8フレームが入り、配布用mozc_toolに従来の全8フレームが入ることを資源バイト列で照合。
- READMEの比較図・変換例・特長・仕組みなど7枚をデータから再描画。実入力比較は公開CSVを直接使用。
- 文章は利用場面と動作を簡潔に説明する調子へ統一。READMEの外部URLはバッジ配色以外同一。
- 88.4 / 59.6 / 58.1%と2026-10-01・beta.8・Unrealを保持。現在版の再測定として扱わない。
- mozc/release-notes.mdの差分はゼロ。公開中のLatestはAPIでv1.1.1を確認。
- MSI ActionTextに短い日本語12行を定義。Template空文字、[1]等を含まない。状態ラベルは「進行状況」。

## 検証結果

| 確認 | 結果 |
| --- | --- |
| SystemUtilTest（保存先等） | 2件PASS |
| KotoriDashboardTest（設定画面・表示名） | 4件PASS |
| SupportReminderTest（再表示・保存） | 7件PASS |
| 月の最終MSIビルド | exit 0、629.508秒 |
| MSI ProductName | Asteria日本語入力 |
| MSI ProductVersion | 3.34.6337.100 |
| MSI UpgradeCode | {45FB8DEB-0CFE-4924-96FB-9022888112E1}、旧版と同一 |
| MSI SHA-256 | c5e688de8a7f6dead5d7a12728a416e161e843a364915ad485e43aeddcad357f |
| MSIサイズ | 1,569,370,112 bytes |
| MSI署名 | NotSigned |
| 配布用mozc_tool metadata | Asteria / 3.34.6337.100 |
| 再生成パッチ | 0001〜0003適用成功 |
|  fresh upstream + patches とビルド入力 | 127ファイル一致（テキストのCRのみ正規化） |

Qtテストの初回はDLL探索により開始前にexit -1073741515。Qtのbin / pluginsを明示して再実行すると全11件成功。
QtテストはQT_QPA_PLATFORM=offscreen。実際のデスクトップ操作とは区別する。
PNG7枚と生成画像を目視確認。図版プログラムはテキストの幅を確認し、長い注記を用紙内へ収める。
パッチの空の文脈行に由来するgit diff --checkの空白検出はある。パッチ以外の同チェックは成功。
パッチは手編集せず、make_patches.shで作成して適用・入力一致を確認した。

## 保存先と再現

- ローカル候補MSI: target/asteria-brand/Kotori64.msi（上記SHAと一致）。
- 比較プレビュー: docs/design/2026-10-asteria/brand-preview.html
- プロンプト: docs/design/2026-10-asteria/generation.md
- 図版: python docs/images/gen_readme_images.py
- ログ: target/asteria-brand/moon-tests.log / moon-qt-tests.log / moon-package.log / moon-patches.log
- 検査JSON: target/asteria-brand/asset-verification.json / binary-icons.json / msi-verification.json / build-inputs.json
- テスト: bazelisk test //base:system_util_test --config release_build
- Qt: bazelisk test //gui/config_dialog:kotori_dashboard_test //gui/tool:kotori_support_reminder_test --config release_build、
  PATHにthird_party/qt/bin、QT_PLUGIN_PATHにthird_party/qt/plugins、QT_QPA_PLATFORM=offscreen。
- MSI: bazelisk build package --config release_build --repo_env=BAZEL_LLVM=C:/Users/aruik/mz/src/third_party/llvm

## 未実施

新MSIの実アップグレード、進行中の実画面、更新版TIPでの日本語入力、アンインストール。
GitHubへのこのブランチの公開・CI・新リリース。
v1.1.1での実入力記録を6337の実機受入れへ流用しない。

前回の実表示テストで作ったsupport-reminder.iniは、利用者の明示許可後の削除が完了。今回も存在しないことを確認。
