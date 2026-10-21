# v1.1.4-rc.4公開記録（2026-10-21）

[RC4](https://github.com/aruiki/Asteria-Japanese-IME/releases/tag/v1.1.4-rc.4)を2026-10-21T01:03:02Zに公開。公開4資産を新しい保存先へ再取得し、全バイト・同梱内容・アイコンを照合した。AIが生成・採点した別候補を、読みと文節が合う場合に候補一覧へ残す。予測期限の境界でも採点済み候補を保ち、弱い予測による先頭本文の置換を防ぐ。[実装と固定検証](scored-alternatives-20261020.md)。

## ソースと配布物

- 公開tag/PR176 merge commit: feaeddc3ee40a9efc3ab4049166e042fef1d69fa
- 製品head: 813e78aa568826b1cf68eeeab494616d82b3b8a0
- 実CI checkout: ed86426e326770f837401069f58e41bf75cdc993
- 3commitのtree: 8ad2df40974b805dccd511355aa2ed4b1680d1ee。jobログ・GitHub commit API・ローカルGitで一致。
- Mozc source: 039a7251fa16a77a91cb67f5158e20073a3f3762、上流a069a88d4cb5c011de0f9aebb6c149a1c808d904
- Windows run: [39433685950](https://github.com/aruiki/Asteria-Japanese-IME/actions/runs/39433685950)、job118665009927。一般CIと合わせ全8チェック成功後に統合。
- MSI ProductVersion: 3.34.6440.100、1,662,828,544 bytes
- SHA-256: 2b37f6fbdcb069251e820e2009b096111cc2253e79ffac65d973fa11ce18e301

ADR0044に従い、同runで検証したMSIを改変せず公開した。4資産はMSI・SHAファイル・distribution-info.json・source-verification.json。draftで実digest/sizeを確認してから公開し、tagの実commitも一致した。RC4はprerelease=true/Latest=false。既存RC2のLatest区分は変更していない。

実ファイルのAuthenticodeはNotSigned。配布情報には署名状態、導入binary SHA、未検証7範囲とproduct_complete=falseを保存した。

## 同じMSIの受入れ・内容確認

同じWindows runでbuild・C++・Qt画面・同梱検査が成功。installer_smoke.py --acceptance --reinstallの原票でinstall/input/uninstall/reinstall=true、stage=finished、errors=[]、acceptance.complete=true、候補8項目すべてpassedを確認した。

対象はWinForms EDIT/RichEdit、候補移動/ページ往復、取消/再開、Backspace、F6〜F10、通常欄間移動、保護欄の外形挙動。loaded TIPは導入先mozc_tip64.dllで、管理展開したserver/renderer/TIPのSHAが導入原票と全件一致した。

再導入は同一MSI・同じboot。旧版更新と再起動は未検証。削除exit3010・reboot_required=trueも再起動確認へ読み替えない。

check_release.py --local --previous-local（実公開RC3/MSI6439）--require-generator --require-collocationsはexit0。先頭3項目の版上昇、固定4モデル、追加連語、出典/ライセンス、通信DLL不在、server版一致を確認。TIP32/64の製品アイコン群102と設定toolの各8フレームはソースICOと全一致した。

公開後、全4資産を新しい保存先へ再取得して公開前と全バイト一致。公開MSIの同梱/アイコン検査も再実行して成功した。98 build入力blobと2 harness blob、3patch再構成140 sourceとの対応を固定した。

原票はtarget/accuracy-guide/ci-artifact-39433685950/、release-scored-assets-v1/、public-scored-check-v1/。配布source-verification.jsonに固定ソース/EXE・入力blob・原票SHA・実CI checkoutとMSIの対応を含む。

## 評価時と同梱推論DLLの対応

最終監査でggml-base.dllのSHA差を検出し、最初の不一致原票を保持した。4モデルと他の推論DLLは全バイト一致。ggml-base.dllは同じ787,968 bytesで、差はfile offset128/129/130の3バイトだけだった。[MicrosoftのPE仕様](https://learn.microsoft.com/en-us/windows/win32/debug/pe-format#coff-file-header-object-and-image)に従ってCOFF TimeDateStamp欄を特定し、その4バイト欄以外の全ファイルバイトが一致することを直接確認した。.text/.rdata/.data/.pdata/.tls/.relocの各SHAも全一致。DLLを改変してSHAを合わせた検証ではない。

- 評価時ggml-base SHA: 7c6a8384fb864bf60df96131e9b9f6063aa478bf5b0cb8f4c0b570663876abc1
- 新MSI ggml-base SHA: 3130988fad03c65c671553b093b6f6748cb79994e8b8948ed8e4fd6a9ed2f0f6
- 原票: ggml-base-6440-difference-v1.json、scored-quality-receipt-audit-v1.json（元の不一致）、scored-quality-receipt-audit-v2.json（全バイト差の分類と追加検査）

さらに新MSIから展開した実モデル/DLLを指定し、同じ固定source/EXEの隔離Sessionで実GPUの2候補を追加検査した。両方ともモデル採点の実ログ、実候補IDによる選択、表示とEnter確定の一致が成功した。原票scored-alternatives-ci6440-candidates-v1/summary.jsonのSHAはe98f023985f92751fb78eff4b5fbeca6dd1b24bdb106b7333aca301e262c0ede。導入済みTSFのGPU寄与を確認した検査とはしない。

## 実モデル比較と限界

同じ6438モデル/DLL・辞書・データ・評価器で旧/新978出力の第一候補が全一致。Standard AJIMEEは前文あり188/200（94.0%）、なし175/200（87.5%）を維持した。候補包含は前文なし189→193/200、あり193→195/200、包含改悪0。最終300例も290/300、全出力一致。第一候補精度改善とはしない。

最終ソースでC++445件・工具2件が成功。固定SessionHandlerと実GPUで欠落2候補の実ID選択・表示・確定、AI待機/入力/編集/取消/保留Enter/後続打鍵、Tab、文脈引継ぎ、通常変換/保存を確認。予測29例は先頭一致12→12、上位3一致19→20、改悪0、確定差0。初期案の改悪ログを保持する。個別キー/callback応答とAI結果到着時間を区別し、速度改善とはしない。

詳細なGPU比較は固定ローカルEXEのオフライン/隔離Session検査。同じMSIのEDIT/RichEdit受入れと区別し、導入済みTSFのGPU AI寄与を直接確認した検査とはしない。

未検証: 実アプリGPU AI ON/OFF寄与、複数アプリ、低性能実機、保護欄内部のAI/学習、旧公開版更新、再起動、独立verifier。製品全体の完成や.goal-completeの判定ではない。
