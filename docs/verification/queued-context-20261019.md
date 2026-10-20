# 確定待ち後の入力へ文脈を引き継ぐ（2026-10-19）

## 問題と修正

AI結果を待つEnterの後に次の文字を打つと、後続キーはホストが最初の確定を反映する前の周囲文を持っている。再生時にそのまま次の合成を始めると、確定本文が前文から欠落する。LmRewriterは明示的な前文を変換器の履歴より優先するため、古い前文が残っている場合にも問題になる。

source `8af4797f92689474635dd49cf8163cef7aedc198` → `ee3fa5f45824c6ade7573de704e23a9488acadbb`。変更はsession.cc/.h/_test.ccの3ファイル、81追加/3削除。Commit直前のclient_contextを保持し、末尾確定（cursor_offset=0）の本文を再生Inputの前文へ足す。後文と部分Contextを保持し、周囲文が不明なら従来の履歴を使う。非末尾カーソルの結果集約は対象外。

## 回帰と固定source

旧実装の回帰は4組で6assert失敗。原票queued-context-red-v1.log/.xmlを保持。修正後の`SessionTest.KotoriQueuedTypingKeepsCommittedPrecedingContext`は前文あり/明示空 × 後続Contextなし/全項目/部分の6組で、StartPredictionへ渡るConversionRequestを検査する。前文は元の前文＋確定本文、後文は元のままであることを確認。文脈不明の既存検査も維持し、Session全197件成功（queued-context-session-v1.log、8 shards）。

固定SessionHandlerはSHA `66ffb19000026a095305c55e5f2bbe8c76aaafe35d5bd95318b34e25bbedc7aa`。ビルド時の8af→ee3 diff SHA `ceabdcd24d503f7ad1f305bbb0d53986a0e774b45e99847c0bacef61d7d3e6f1`は197件を検証したコミット差分と一致。原票queued-context-native-candidate-v1/identity.jsonとverification.jsonに固定した。

## 実モデルの前後比較

公開MSI6438から管理展開した4モデル/DLL、同じ読み表、RTX3060を使う。モデル・DLL・読み表のSHAが全一致することをassertした。

| 確認 | 8af | ee3 |
| --- | --- | --- |
| 明示的な前後文＋Space待ち/Enter保留/次の打鍵/次の変換と確定 | 成功、30イベント | 成功、32イベント |
| 最初の確定 | 明日の会議は１０時からです | 同じ |
| 次の確定 | その会議は東京です | 同じ |
| 共通する採点対象 | 17候補 | うち後続文7候補の採点が変化 |

元実装もこの2文の表面出力は正しかった。正確な前文の修復はC++ request回帰で確認し、実モデルでも採点が変わることと入力継続を確認した。1組の比較を全体の精度改善や速度比較へ広げない。別のTab/保留Enter/編集/取消/AIoff/本文保持はee3で52イベント成功。

実モデル検査は`mozc/tools/queued_context_acceptance.py`。`--exe --identity --install --proto --romanji-table --out`を指定し、GPUを固定する場合は`--require-device "NVIDIA GeForce RTX 3060"`を付ける。identity JSONは固定時のsource_commitとsession_handler_sha256を持ち、EXE SHAが違えば起動前に失敗する。モデル/実行環境・工具自身のSHAとraw eventsを保存する。最終statusの計算カウンターが0だったため、その値だけで稼働を判定せず、stderrのzenz/llm実採点18行も確認した。

原票はqueued-context-gpu-v1/、queued-context-surrounding-gpu-v2/、queued-context-surrounding-baseline-v1/、queued-context-real-model-comparison-v1.json。初期の単発プローブと失敗したLLVM path構成のログも保持する。検査は隔離SessionHandlerであり、実TSFや通常PCでの導入確認ではない。

## 製品パッチとWindows

Bazel停止後、make_patches.shで再生成。3patchの再適用が成功し、140 sourceが正規化バイト一致。0002 SHA `fd8dff08c908cfb199686d081ecfb1e44606f5ee9318bc76e63ef66405aa88ff`。0001/0003は既存SHAと同一（queued-context-patch-generation-v1.log / queued-context-patch-receipt-v1.json）。モデル・閾値・converter関係のsourceは変更していない。RC2の978出力/予測29例/heldout300例を、この変更後の新測定とはしない。

Windows workflowは同じrunで生成するMSIに`installer_smoke.py --acceptance --reinstall`を適用する。既存の追加8項目・同一MSI再導入/入力/削除を本体buildへ統合し、旧版MSIの成功を新版へ読み替えない。receiptの空/部分成功・違うTIPを拒否するPython2件も成功。新しいMSIの実行結果はCIで確認してから成功とする。

製品全体の独立verifier、実TSFのGPU AI ON/OFF寄与、低性能実機、内部privacy、旧版からの更新/再起動は残る。公開済みRC2の配布物とは区別する。
