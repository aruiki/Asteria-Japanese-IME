# 固定ソースと要求IDによる精度基準測定（2026-10-17）

判定: **オフラインの基準測定と要求対応は検証済み。製品完成・実TSFは未達**。カード81の継続。既存の学習配分は不採用のままで、新しいモデルや重みを既定へ入れていない。

## ソース・成果物の固定

専用Mozcツリー `C:/Users/aruik/ag16` を上流 `a069a88d4cb5c011de0f9aebb6c149a1c808d904` から作り、製品パッチと既存のe11eaa6採点漏れ修正、評価CLIの要求ID診断を適用した。ローカルのソースsnapshot commitは `5a750cd579b36c690714be02fd0fd64cc4ecb84c`。旧 `mzsupport` と `mzcontext16` のソースは変更していない。

ビルド済み `converter_main.exe` のSHA-256は `c4542107433cc417c1b54449a73678b19763838463771d0d2672bcb5046a867f`。辞書SHAは `720d0cb42651fe35692578ea5f3c752940b8357fc24efc46230c904d2f5d5d70` で従来の測定と一致。モデル・runtime・評価器・データ・環境・CPU/GPUは各manifestへ保存した。

最初のビルドはLLVMへのjunction参照と組み込みヘッダーの実体パスの不一致で失敗した。検査は無効化せず、次のrepo環境変数で実体パスを明示した。成功したbuildは371.9秒。固定ソースの整形後は同じ指定で変換器と関連試験を再ビルドした。

```text
bazelisk build //converter:converter_main --config release_build --jobs=6 --repo_env=BAZEL_LLVM=C:/Users/aruik/mz/src/third_party/llvm --shell_executable=C:/Program Files/Git/bin/bash.exe
bazelisk test //converter:converter_main //rewriter:lm_rewriter_test //base:kotori_diagnostics_test //session:session_test --config release_build --jobs=6 --repo_env=BAZEL_LLVM=C:/Users/aruik/mz/src/third_party/llvm --test_output=errors
```

shell_executableの空白を含む引数は実行シェルに合わせて引用する。コマンド原票は `target/accuracy-guide/native-build*.log` と `native-tests.log`。

## 要求IDの実装と確認

評価CLIの `--evaluation_trace` は既定で無効。入力本文を追加せず、start要求のbegin/endと連番をstderrへ出す。`eval_baseline.py --request-traces` がID欠落・順序違反・入れ子・境界外の採点を拒否し、要求の原票をバッチ全体のstdout/stderr SHAへ結び付ける。単に隣のログへ当てはめたり、未対応変換器へ黙ってフォールバックしたりしない。

固定7診断例で診断OFF/ONの最終候補がすべて一致し、7要求のIDリンクを確認。AI OFF1/7、ON4/7（改善3/改悪0）は従来と同じ。これはモデル起動だけでなく**オフライン変換器の最終候補への寄与**の証拠であり、Windows TSFの入力・表示・Space・確定への寄与ではない。原票 `native-request-v1/receipt.json` と `request-traces/`。

## 同一辞書・モデルでの前後比較

旧固定変換器SHA e9110624…からの比較。旧成果物はソース対応未確認なので、変更の効果をこの比較だけで特定のコードへ断定しない。e11eaa6はすでに別作業で検証されていた修正を前提として取り込んだもので、今回新たに学習したモデルの効果ではない。

| 調整・回帰用 | 旧固定変換器 | 今回の固定ソース | 改善 | 改悪 |
| --- | ---: | ---: | ---: | ---: |
| AJIMEE | 183/200 | 184/200 | 1 | 0 |
| 慣用句 | 24/25 | 24/25 | 0 | 0 |
| ニュアンス | 17/20 | 17/20 | 0 | 0 |
| 日常 | 79/81 | 79/81 | 0 | 0 |
| 計 | 303/326 | 304/326 | 1 | 0 |

今回のソースでMozc対照も再取得した。Mozc200/326に対してAI ON304/326、改善106・改悪2。Mozcが正しかった入力をAIが壊す2件は残課題である。候補表示・表記揺れのレビューを別途行い、期待表記を後から広げていない。

独立300問は採用した前提修正の最終報告として一度測定した。289/300=96.3%、Wilson95%区間93.6〜97.9%。work50/50、chat47/50、tech50/50、name47/50、num45/50、news50/50。この結果を重み・モデル・閾値の選択へ使っていない。原票は `native-regression-v1/`、コマンドは `native-request-v1/run_regression.py` と `final_baseline.py`。

## 学習用の実候補への接続

同じtrain64/dev32を固定ソースの要求IDで再取得し、96要求すべてで完全な候補ログを収集できた。trainは参照一致45/64、内部参照あり55/64、devは27/32・31/32。参照選好として保存し、非参照を意味的誤りとは自動ラベル付けしない。以前の別プロセス方式の原票は保持した。原票 `native-preferences-v1/`。これは収集経路の改善であり、入力性能やモデル精度の向上とは呼ばない。

## 試験とパッチ

- Python試験80件成功（実行時のTEMP/TMPは作業ツリーのtest-tmp）。
- C++は3対象すべて成功。XMLから集計した実行件数はdiagnostics5、lm_rewriter59、session190、計254、失敗0。
- パッチは手で編集せず固定ソースから再生成。素の上流へ0001〜0003を順次適用し、ビルド済みソースの変更131ファイルと照合して相違0。
- パッチ生成器のAND連結で、適用失敗後も処理と出力上書きが続く問題を修正した。隔離fixtureで元コードは失敗後に出力を上書き、修正版は最初のapply失敗で停止し出力を保持することを確認。基点はソースとは独立したclone/bare cloneを要求し、reset/cleanは絶対パス確認済みの専用scratch内に限定する。

再生成時、同じツリーをソースと基点にしたコマンドは自動承認審査で拒否され未実行。独立bare cloneへ変更して審査を通し、安全な構成で実行した。以前の利用上限による停止も解消済み。

生成0002のSHAは `f28752a0ee5168861d7ae1e989795c9cf03713d0a170e91e3bca8af4c45bd8ef`。再構成・失敗時検証の原票は `source-reconstruction.json`、`patch-generator-red-green.json`、`patch-regeneration.log`。パッチ本文に新規で含まれる必須の空文脈行3行はrawのgit diff --checkが末尾空白として検出する。通常コードのcheckと、パッチ適用後の実ソース全体のcheckは成功。検査を無効化したり結果を書き換えたりしていない。

## 残る完成条件

要求IDは評価CLIの範囲。実TSFの世代・候補表示・Space・確定・期限内反映率は未確認。残る候補欠落と順位誤りの中核修正、重大誤変換、較正・個人適応、CPU/低メモリ/初回/即時Space/障害時・保護フィールド、Windows CI/MSIと独立verifierの製品完成判定は未完了。未採用の配分を復活させず、この固定基準から次の改善を比較する。
