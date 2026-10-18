# 候補のprefix比較改善（2026-10-17）

AJIMEEの固定200問・既存正解集合で187→188/200（94.0%）、改善1・改悪0。学習済み追加生成モデルを含む従来と同じモデル同梱構成を使用した。ID2002の末尾「翌月の初」が既存候補「翌月の初め」へ改善。導入済みIME・公開版は更新していない。

## 実装と比較

通常の合算採点で選ばれた候補と、その文字列を先頭に含む長い候補だけを対象に、既存合算スコアをLLMトークン数で割って比較する。無関係な候補、非有限スコア、トークン数0は除外。既存候補から選び、追加推論や候補文の書き換えは行わない。LLMなし・時間切れでは適用しない。補正の可否判定には従来の生スコアを維持する。

ID2002では実トークン列も20トークンの厳密なprefixと21トークンの拡張だった。合算値-85.40/-88.06を正規化すると-4.270/-4.193になり拡張が選ばれる。全200問の旧選択結果で、より長いprefix候補が存在したのはこの1件。実装に単語や評価IDの条件は含めていない。文字列prefix一般で常にトークンprefixになることは仮定しない。

| 構成 | 変更前 | 変更後 | 改善/改悪 | 出力変更数 |
| --- | --- | --- | --- | --- |
| 学習済み追加生成モデル同梱 | 187/200 | 188/200 | 1/0 | 1 |
| 学習モデル非同梱 | 186/200 | 187/200 | 1/0 | 1 |

原票は `target/accuracy-guide/ajimee-prefix-{bundle,missing}-v1/` のranking.json、manifest、comparison.json、traces、run.log。比較元は `ajimee-learned-default-{bundle,missing}-v1/`。追加生成・LLM採点数の環境変数は外した既定設定。同梱時のモデル処理中央値164ms・p95 513ms、非同梱95ms・265ms。入力表示遅延の測定ではない。

先に試したEOS追加採点は187/200のまま改善0・改悪0で、中央値163→228ms、p95 512→651msとなり不採用。原票 `ajimee-eos-{off,on}-v1/`、実験ソース64da6abeedd3d5d1ab3b2b2c3803c93a17d5aa37は別枝に保存し、今回の採用パッチへ含めていない。

## 固定ソースと確認

- 専用ソース `C:/Users/aruik/ag16`、枝 `codex/prefix-normalization-20261017`、commit `fb6de138432e18deb6d45f2e336cce1fd573128f`。
- converter SHA256 `71b9d1fe92645df758b6ee039a9ddd465a951de110c04f7213c150dc720cd053`。
- LmRewriter試験62件成功、失敗0・エラー0。`target/accuracy-guide/prefix-normalization-test.xml`。追加3件はprefix限定、低評価・無関係候補の維持、異常入力の維持を確認。release build成功。
- Bazel停止後に製品パッチを再生成。独立cloneへ適用した131ファイルと固定ソースが一致し、適用後diffのwhitespace検査成功。`target/accuracy-guide/prefix-source-reconstruction.json`。
- 0002パッチSHA256 `3dedbf14bc10d99a94e219d779cee95d8a023e417481d5c933d33e16198a8d6c`。

次は残る12問を同じAJIMEE基準で改善する。独立評価・実TSF・低資源・配布・独立verifierを含む製品完成条件は未達。
