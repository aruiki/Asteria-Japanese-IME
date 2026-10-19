# GPU AIを待つ初回候補表示（2026-10-19）

利用者の明示指示に基づく[ADR0067](../adr/0067-ai-first-candidate-publication.md)の通常Space・入力中候補・明示Tabの実装記録。公開済みv1.1.4-rc.1を改変した記録ではない。

## 最新: Tabとシナリオ配送の修正

Mozc source `8af4797f92689474635dd49cf8163cef7aedc198`。Tabも候補素材と要求を所有するワーカーで生成・採点し、入力/モデル世代・設定・文脈・候補素材が一致した結果だけを公開する。Spaceと同じ保留Enter・後続入力・取消を使う。評価済み本文の全半角英数表記とNO_LEARNINGを継ぎ、予測で延長した候補へ本文だけの補正読みを流用しない。mixed_conversionでも明示的なAI待機の配送callbackを継続する。

固定v5 (`ai-first-tab-native-candidate-v5`) のsession SHAは`763d0184545ce7163dfee7b4f0e5b6b73c602622fbf4e6c15d94905ba90deb6f`、converter SHAは`66ae75a21889d7ef085a5124ce0f39775715774d4f739ab86f7b5c272fefe505`。ビルド時diffとbba6からのコミットdiffはSHA`f4aaa29683d217f0d50ffc2107389dae916b0f093428ba46e918f74d6c62779c`で一致した。

- Windows CIと同じ13対象のC++534件成功。Engine92/LM70/Session196/Scenario81を含む。`ai-first-all-ci-tests-v1.log`と`ai-first-tab-test-counts.json`。Python119件成功、`ai-first-tab-python-v3.log`。
- GPU/実モデルのSpace→保留Enter→後続入力の検査成功、`ai-first-tab-space-regression-v5`。明示Tabの同じ操作、取消・編集・AI-off、通常変換で評価した本文とTabの表記/確定一致も成功、`ai-first-tab-queue-gpu-v5`。後者の冷起動入力は既存予測データの「おせわに」、期待prefix「お世話に」。RTX3060を選択。最大キー/配送応答はそれぞれ36.23/35.73msだが、AI結果到着時間や反復性能比較ではない。
- 固定予測29件の前後比較 (`ai-first-tab-prediction-quality-v2`) は先頭表示29件すべて一致、1位12/29・3位以内20/29を維持、表示と確定の不一致0。データSHA`e0eacca8d1a68c03342dbb2e226743cfc2994753480c44c602ac1c8b9ae2bb79`。調整用予測prefix一致であり、通常変換の正解率や独立評価ではない。
- 最終v5の通常変換4セット×3条件978出力が基準と完全一致 (`ai-first-tab-quality-v3`)。Standard AJIMEE前文あり188/200、なし175/200を維持。AI-offの通常28項目/保存等7項目も成功 (`ai-first-tab-normal-basics-v5` / `ai-first-tab-normal-lifecycle-v5`)。
- Bazelをsrcからshutdown後にパッチ再生成・3パッチ再適用を実施。140ソースの正規化バイト一致、`ai-first-tab-patch-generation-v1.log` / `ai-first-tab-patch-receipt.json`。モデル/生コーパスは含めていない。
- インストール後の8項目のC#ハーネスは、Space後の固定500ms待機から、期待する未確定/確定結果を15秒以内に観測する方式へ更新。期待文字列・全8項目・TIP識別・候補操作は保持。ローカルはコンパイルのみ成功し、開発PCでは実行していない。実行は使い捨てWindowsランナーで行う。
- 初回Tabの前に通常変換を強制したv3案は3位以内20→13/29で不採用。既に評価済みの本文だけを参照する修正後に20/29へ戻った。モデルや正解集合は変更していない。失敗原票`ai-first-tab-prediction-quality-v1`を保持する。
- 冷起動Tabで「あいであきょく」から「アイデア曲」を期待した検査は、実際には「アイデア局長に就任した」と補完して参照不一致、`ai-first-tab-real-gpu-v5`を失敗のまま保持。初回Tabには評価済み通常変換本文が存在しないため、本文保持の検査と混同しない。通常Spaceでの「アイデア曲」と、Space後の本文保持を別に検査した。予測自体の語義選択の改善を達成したとは主張しない。
- PR171の初回Windows run39187992825はMSI構築後の旧シナリオ7/8 shardsで失敗し、実入力/成果物公開へ進まなかった。旧シナリオが非同期配送前に候補を検査していたため、試験ハーネスでcallback処理を有効化し、AI有効・モデルなしへ環境を固定した。全81シナリオ成功。AI無効化や期待値削除で通していない。次回CIには`--test_output=errors`を追加した。

以下は先行するSpace実装時点の記録であり、現在の配布物の検証へ読み替えない。

## 実装と固定対象

Mozc source `bba6fb37ea754087d8ff0d790e2b7ee9befed801`。待機・評価済み・利用不能を分け、正常な待機中には候補一覧と全候補出力を抑える。Space要求を既存callbackで非同期に待ち、準備した文節をそのまま表示する。Enterの保留、Enter後の次の打鍵とBackspace、取消・編集による入力世代の失効に対応。モデルの読込と推論はワーカーで行う。

固定session EXEは`21431d4dec010018eed27903a1bf223a31dd3cc982ede0965d451b821109f05a`、converter EXEは`4abe5b9013d7218e6e85f46a8681f6d82c39b917542e3a11b9467ed83f5d8a8d`。原票は`target/accuracy-guide/ai-first-native-candidate-v2/`。ビルド時のdiffと後続コミットのdiffが同じSHA `0f509b528f13f41703ce8d347c8f98633ac4c4959d4f55089eb3484e6fe705a3`であることを照合した。source receiptに記録。

モデル・DLLは公開6435 MSIの展開物（`ci-artifact-39158232305/package-check/ext/PFiles/Kotori`）。4 GGUFとDLLのSHAを各試験summaryへ記録した。導入済みIMEや普段の履歴には接続していない。

## 前後比較

`mozc/tools/ai_first_acceptance.py`を同じモデル/DLL・空の隔離profileで実行した。

| 対象 | 起動直後の入力 | 即Space→Enter | ライブ候補とSpace・確定 |
| --- | --- | --- | --- |
| 旧6432 session（`a9b62952…`） | 未評価候補が公開される | AI前に確定し参照不一致 | 初回候補公開の条件に不適合 |
| 今回v1固定session（`cade2883…`） | 読みを表示、候補非公開 | AIの「アイデア曲」を待って確定 | 一致 |
| 今回v2固定session（`21431d4d…`） | 同上 | 待機中の次の入力・Backspaceも保持 | 一致 |

旧版原票`ai-first-old-source-red-v1`はexit1、6件の失敗を保持。新v1 `ai-first-real-gpu-v1`と新v2 `ai-first-typeahead-gpu-v2`はexit0。v2はRTX3060選択状態で34応答、最大39.02ms。起動から最初のAI確定到着まで約1.98秒であり、応答時間だけをAIの所要時間と呼ばない。これは速度改善の反復ベンチマークではない。実アプリTSFの確認でもない。

CPU指定の`ai-first-real-cpu-v1`では待機・配送・ライブ/Space/確定一致の条件は通ったが、先の参照は「アイデア局」で不一致、exit1を保持した。GPU経路での解消をCPUの精度改善に拡大しない。

## 回帰

- 関連C++4対象365件成功（LM67、Engine90、Session194、Segments14）。新しい待機・回数上限超過・保留Enter・編集/取消・次の打鍵・focus submit・失敗の世代分離を含む。最終ログ`ai-first-cpp-v5.log`。初回のLLVM参照不一致、型名エラー、mockの空文節での失敗もv1〜v3に保持し、修正後に再実行した。
- 固定した4開発セット×3条件、978出力が前の通常変換版と完全一致、12実行ともexit0。Standard AJIMEEは前文あり188/200（94.0%）、前文なし175/200（87.5%）。`ai-first-quality-v1`。調整専用の同期converterによる検査であり、実入力の正解率ではない。
- 新v2 sessionの通常変換28項目と保存・再起動・かな入力・再変換等7項目が成功。`ai-first-normal-basics-v1` / `ai-first-normal-lifecycle-v1`。AI-offの隔離セッションで、TSF受入れの代わりにはしない。
- Bazel shutdown後に`make_patches.sh`で再生成し、3パッチの再適用と139ソースの正規化バイト一致を確認。原票`ai-first-patch-generation-v1.log` / `ai-first-source-receipt.json`。生成パッチのcontext空行は外側git diffの空白検査から除き、ソースと文書を検査する。パッチを手編集していない。

## 次の確認

修正後のWindows CI・新MSIの実入力を引き続き確認する。新MSIでの結果を得る前に公開RCの実アプリ確認を引き継がない。製品完成・独立verifier・`.goal-complete`の判定ではない。
