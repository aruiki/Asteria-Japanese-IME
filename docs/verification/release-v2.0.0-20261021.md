# v2.0.0正式公開記録（2026-10-21）

[正式v2.0.0](https://github.com/aruiki/Asteria-Japanese-IME/releases/tag/v2.0.0)を2026-10-21T14:42:54Zに公開。prerelease=false、draft=false、Latest=true。利用者の明示指定により未署名で公開した。二重入力を修正して再ビルドした新MSIを使用し、既存RC4を版名だけ変更した配布ではない。正式公開と製品全体の完成判定を区別する。

## 固定したソースと配布物

- 公開tag / PR180 merge commit: 298bee913ac4e1cd4608487494c4800f7be9dfd3
- 製品build head: 34763f4c2474de6e773ca207e4ce1510ccc9d191
- 実CI checkout: 9ea0b4f9283ea9dd71c05658f84a255653373700
- 上記3commitのtree: 40563d265a4cb226c3770612a43d1fb7304b8f48。GitHub commit API・jobログ・ローカルGitで一致。
- Mozc source: 785bd83a7f236b49925f7a83764d5beadf3bb378、上流a069a88d4cb5c011de0f9aebb6c149a1c808d904
- Windows [run39582216741](https://github.com/aruiki/Asteria-Japanese-IME/actions/runs/39582216741)、job119162299302。PR180の適用全10チェックが成功した後に統合。
- MSI ProductVersion: 3.34.6442.100、1,662,754,816 bytes
- MSI SHA-256: 1bc531b10b25cc46fbc60a8b7b052187f2b69c9065e8a1651acc23e89bd1b72a
- Authenticode: NotSigned（実ファイルで確認）

ADR0044に従い、同runで検証したMSIを改変せず4資産（MSI、SHA、distribution-info.json、source-verification.json）として公開した。draftで全資産のdigest/sizeを確認してから正式Latestへ切替。公開tagの実commitを照合し、全4資産を新しい保存先へ再取得して全バイト一致を確認した。公開MSIの同梱・アイコン検査も成功。

98 build入力blob、7受入れharness blob、3patch再構成140 source、4モデルのSHAと実CI checkoutを添付source-verification.jsonへ固定した。生成されるsrc/version.bzlのBUILD6442はjobログの実出力と照合した。6440から先頭3項目で版が上がっている。

## 二重入力の再現と修正

導入済み旧6438だけでなく最新RC4/6440でも、AI待機中のSpace直後Enterで不具合が再現した。EDITでは確定resultと同文preeditが同時に残り、Edge textareaでは「ゴールに設定してゴールに設定して」と実際に二重表示された。[修正前の失敗・原因・回帰検査](duplicate-commit-20261021.md)。

非同期変換でOutputへ書いたpreeditを、保留確定時にClearしてからCommitを作り直す。後続キーの新しいpreeditと意図した同文2回は保持。ランキング・辞書・モデルは変更していない。修正前の2回帰は失敗、修正後C++371件が成功。実辞書/モデルの隔離SessionもGPU/CPU指定各4条件が成功した。指定デバイスの試験を導入済みTSFのGPU寄与確認とはしない。

## 同じ新MSIの実Windows受入れ

使い捨てGitHub Windows runnerへこのMSIを導入し、実SendInput/候補クリックで入力した。利用者の実デスクトップは操作していない。

- 導入・入力・同一版再導入・削除: true、stage=finished、errors=[]。
- 既存EDIT/RichEdit候補操作: 8/8成功。
- 二重入力のEDIT/RichEdit: 28/28成功。Space直後/30/250ms後Enter、通常Enter、候補選択、フォーカス移動、候補クリック、意図した同文2回を含む。各確定結果の合計とホスト本文が一致、確定後preedit空。
- Edge textarea/contenteditable: 14/14成功。各compositionendの合計と最終DOMが一致。native SendInputを使い、DevToolsによる文字入力はしていない。
- 導入先TIP・server・rendererを識別し、管理展開した実binary SHAが導入原票と一致。

本体runのEdge試験はnative28の後なので、先行試験のかな確定による履歴もある。これだけを同一漢字候補の前後比較にしなかった。追加の[元順序run39614663634](https://github.com/aruiki/Asteria-Japanese-IME/actions/runs/39614663634)で同じMSI SHA/導入binaryと同じ7harness blobを使い、既存native8→Edge14を実施。14/14成功し、元のtextarea/Space直後Enterも「ゴールに設定して」1回、compositionend1回となった。再ビルドした別MSIではない。

再導入は同一版・同じboot。旧公開版からの更新や再起動を検証したとはしない。install exit0、reinstall exit0、uninstall exit3010、reboot_required=true。

check_release.py --local --previous-local（実RC4 MSI）--require-generator --require-collocationsはexit0。固定4モデル全バイト、連語、出典/ライセンス、通信DLL不在、server版を確認。TIP32/64の製品アイコン群102と設定toolの各8フレームはソースICOと全一致。

## 測定値と残る範囲

RC4の固定開発評価（Standard AJIMEE前文あり188/200、94.0%等）は履歴として保持する。converter_mainはRC4評価時と全バイト同一。今回の修正から第一候補の精度向上や速度向上を主張しない。

未検証は、導入済みTSFのGPU AI ON/OFF寄与、Officeなど今回対象外のアプリ、低性能実機、保護欄内部のAI/学習、旧公開版更新、再起動、独立verifier。添付配布情報の7項目をnot_verified、product_complete=falseとし、過去の独立FAILと.goal-complete不作成を保持する。

原票: target/accuracy-guide/ci-artifact-39582216741/、edge-fixed-original-order-39614663634/、release-v2-assets-v1/、public-v2-check-v1/。初回ローカルpreflightは同梱検査成功後にPowerShell5/7のmodule混在で署名コマンドが停止した。失敗ログを保持し、完了済み検査を再実行せず適切なpwshで署名照合以降を再開して成功した。

## 公開案内

README・導入ガイド・紹介3画像・Release本文を正式版の実URLとSHAへ揃える。公式7ページと公開表示の最終確認は別の完了記録へ追記する。
