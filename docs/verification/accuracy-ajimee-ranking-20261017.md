# AJIMEE順位改善 — 2026-10-17

ユーザーの指示により、当面の改善判断をAJIMEE-Benchへ絞る。TSF診断・追加データセットの拡大は後回しとする。

- 基準184/200から、新しい実converterで185/200（92.5%）。改善1・改悪0、出力変更5。残り15問。
- 改善例: ID2159「ことで、音声を取り戻し、」→「ことで、温情を取り戻し、」。正解・採点器・モデルは変更していない。
- 順位付けのLLM既定重みを0.7から1.2へ変更。補正採用のLLM重みは0.7に固定し、既存の補正閾値の尺度を保つ。
- 重みだけを変更した初回実験は184/200（1改善・1改悪）。ID2600「目論ん」の末尾を誤入力補正が削ったため、順位付けと補正採用を分離した。最終版ではこの改悪なし。
- ソース: `C:/Users/aruik/ag16`、commit `01f14741e9a80d2e10dc59f77d0b0a8888d6c74a`。converter SHA256 `3582825ffa919be45c5e76b38a9405a44f03034bd63aea97e0d9e37aacb86f14`。
- `bazelisk build //converter:converter_main --config release_build --jobs=6 --repo_env=BAZEL_LLVM=C:/Users/aruik/mz/src/third_party/llvm` 成功（72.627秒）。AJIMEE200問の評価成功（31秒）。実行条件・モデルSHA・要求原票は `target/accuracy-guide/ajimee-ranking-v1/`。
- Bazel停止後、`mozc/tools/make_patches.sh` で再生成。独立cloneへ0001〜0003を適用成功、実ビルド元131ファイルとの不一致0、適用後ソースの `git diff --check` 成功。
- 0002 SHA256 `5da8c645d91f16d96d9434c14538e84d7b396689bf345bb8626ce17e62c59da7`。

AJIMEEの200問を用いた調整結果であり、未知文の改善率は未測定。導入済みIMEの更新・公開は未実施。以前の診断作業は別枝に保持し、この修正へ混ぜていない。次は残る15問の候補欠落・順位誤りを改善する。
