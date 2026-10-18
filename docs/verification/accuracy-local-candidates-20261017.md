# 候補追加の比較 — 2026-10-17

採用済み版は186/200（93.0%）を維持。以下の2案は改善しなかったため製品へ入れない。正解集合・辞書・採点器は変更なし。

| 条件 | AJIMEE | 186基準比の改善/改悪 | モデル処理中央値/p95 |
| --- | --- | --- | --- |
| 採用済み生成8/LLM4 | 186/200 | — | 93/269ms |
| mediumから追加生成 | 186/200 | 0/0 | 203/648ms |
| 一語候補OFF | 186/200 | 0/0 | 92/269ms |
| 一語候補ON | 180/200 | 0/6 | 287/948ms |

一語候補は、生成文と読みの辞書対応から同音候補を作り、一文につき一語だけを変更する。既存の `IsSafeDictionaryCorrection` を通し、利用者辞書・保護語・読み不一致を拒否。種文2件×各16候補、取消対応、CPUでは無効。実験用環境変数を指定した場合のみ動作。

改悪6件は1015/1129/1287/1776/1992/2018。いずれも採点完了原票なし。候補追加が通常の生成時間枠を使い、`gen_over()`で既存の良い候補まで返せなくなった可能性が高い。元の誤答14件も改善しなかった。追加辞書探索を主処理の採点前に挿入する方式は不採用。候補追加OFFは採用済み版と全200問の出力一致。

実験ソースは `C:/Users/aruik/ag16` の枝 `codex/local-homophones-20261017`、commit `a2786cca44a9ce56e5399a8f7fa5bef9555532a8` に保持。関連C++62件成功、converter build成功。読み維持・取消・上限・利用者辞書の関連試験を追加した。製品パッチへ再生成していない。

終了時はソースを採用済み `codex/ajimee-ranking-20261017` の9252fb8f0へ戻し、再build成功（9.895秒）。再リンク後のSHAが変わったため、その実バイナリもAJIMEE200問で測定し186/200を確認した（29秒、モデル処理中央値91ms・p95 260ms）。SHA256 `ea1a9c241c4490f640c6296d52f2d4638466f0ec0af787efcbda5fee5173a06a`。原票 `target/accuracy-guide/ajimee-restored-186-v1/`。Bazel停止済み。導入済みIMEは変更していない。

原票は `target/accuracy-guide/ajimee-extra-medium-v1/`、`ajimee-local-off-v1/`、`ajimee-local-on-v1/`。OFF/ONの比較は `comparison-vs-off.json`。試験XMLは `target/accuracy-guide/local-candidates-test.xml`。各runnerとbuild/testログも同targetに保持。

残る14誤答のうち8件は正解候補が内部プールにあり採点されても選べない。残る6件は全文正解がない。単純な文字数加点とMozc先頭への編集距離の加点も机上比較したが、表記偏りまたは改悪が増えるため不採用（仮説原票 `length-bias-hypotheses.json`、`mozc-prior-hypotheses.json`。実バイナリの改善結果ではない）。次は候補追加を一律に増やすより、既存候補の識別学習を優先する。
