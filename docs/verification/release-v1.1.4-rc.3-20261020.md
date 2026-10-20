# v1.1.4-rc.3公開記録（2026-10-20）

[RC3](https://github.com/aruiki/Asteria-Japanese-IME/releases/tag/v1.1.4-rc.3)を2026-10-20T07:56:46Zに公開し、公開した4資産を新しいフォルダへ取得し直して照合した。確定待ちの後続打鍵へ、直前の確定本文と既知の周囲文を渡す。[実装と実モデルの比較](queued-context-20261019.md)。

## ソースと配布物

- 公開tag/PR174のmerge commit: `5b60b68b1d6b6ce469896e719774f954774d9085`
- 製品head: `0b98251c4842bfc574190f7cac5dfe922e2e1dab`
- CI checkout: `e1d07706df3b9ab679fef76b94cd82ee7789cfbf`
- 3commitのtree: `a7da87baaaf1c0730fe892b12d3b751888599da2`。実jobログ・GitHub commit API・ローカルGitで一致を確認。
- Mozc source: `ee3fa5f45824c6ade7573de704e23a9488acadbb`
- Windows run: [39249434607](https://github.com/aruiki/Asteria-Japanese-IME/actions/runs/39249434607)、job118049304071。一般CIを含む全8チェックが成功してから統合した。
- MSI ProductVersion: `3.34.6439.100`、1,662,738,432 bytes
- SHA-256: `606c5dc07bdf0c53b031f13ad91c4bd1545b280b4ec276146379157339c90045`

ADR0044に従い、同runで検証したMSIそのものを改変せず再利用した。公開4資産はMSI・SHAファイル・distribution-info.json・source-verification.json。draftの全資産の実digest/sizeを照合してから公開し、tagの実commitも確認した。RC3はprerelease=true、Latest=false。公開時点のLatestは既存RC2で、その公開区分は変更していない。

Authenticodeは実ファイルでNotSigned。配布情報へ署名状態、導入binary SHA、検証範囲と未確認事項を保存した。

## 同じMSIの実行・内容確認

同じWindows runでbuild・C++・Qt画面検査・同梱モデル/版検査が成功。`installer_smoke.py --acceptance --reinstall`の原票でinstall/input/uninstall/reinstall=true、errors=[]、stage=finished、acceptance.complete=true、追加8件すべてpassedを確認した。

検査はWinFormsのEDIT/RichEdit、候補移動/ページ往復・取消/再開・Backspace・F6〜F10・通常欄間移動・保護欄の外形挙動を対象とする。実loaded TIPは導入先mozc_tip64.dllで、管理展開したserver/renderer/TIPのSHAが導入原票と全件一致した。同一MSI・同じbootでの再導入であり、旧版からの更新や再起動確認ではない。削除3010/reboot_required=trueも再起動検証へ読み替えない。

check_release.py --local --previous-local（実公開RC2/6438）--require-generator --require-collocationsはexit0。先頭3項目の版上昇、固定4モデル、追加連語と出典/ライセンス、通信DLL不在、server版一致を確認した。TIP32/64の製品アイコン群102と設定toolの各8フレームもソースICOと全一致した。

公開後、全4資産を新しい保存先へ再取得し、MSI/SHA/メタデータの全バイトが公開前と一致。公開MSIで同じ内容検査とアイコン検査を再実行して成功した。公開用の96 build入力blobと2 harness blobも固定表と照合した。

原票は`target/accuracy-guide/ci-artifact-39249434607/`、`release-rc1143-assets-v1/`、`public-rc1143-check-v1/`。native受入れ原票は外側リポジトリの`target/status-review-20261020-39249434607/installer-smoke/`。最初のローカルpreflightは旧artifactのpackage/配置を仮定してFileNotFoundで停止した。今回のMSIはartifact直下にあり、実配置へ修正して全検査を実行した。失敗ログは保持している。

## 品質と未確認範囲

この変更はSessionの末尾確定時の文脈引継ぎで、モデル・閾値・converter sourceは変更していない。C++ Session197件と、同じ公開モデル/DLLで旧/新の実GPUイベントを確認した。既存のRC2の品質測定を新たな精度測定とはしない。

実TSFのGPU AI ON/OFF寄与、複数アプリ、低性能実機、保護欄内部のAI/学習、旧版からの更新/再起動、独立verifierは未完了。開発中の採点済み代替候補保持と予測期限の修正はこのMSIに含めていない。製品全体の完成や`.goal-complete`の判定ではない。
