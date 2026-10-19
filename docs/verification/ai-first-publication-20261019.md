# GPU AIを待つ初回候補表示（2026-10-19）

利用者の明示指示に基づく[ADR0067](../adr/0067-ai-first-candidate-publication.md)の通常Space・入力中候補の実装記録。公開済みv1.1.4-rc.1を改変した記録ではない。

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

Windows CI・新MSIの実入力と、Tabの明示予測経路を引き続き確認する。今回の変更は通常Space・入力中の候補表示が対象で、Tabの同期予測自体は変更していない。新MSIでの結果を得る前に公開RCの実アプリ確認を引き継がない。製品完成・独立verifier・`.goal-complete`の判定ではない。
