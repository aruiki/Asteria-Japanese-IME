# 新リリースに向けたモデル比較・起動時の誤変換（2026-10-18）

利用者のモデル変更優先指示を受け、3〜4B級モデルを実converterで比較した。同じ読みの「アイデア局→アイデア曲」がAI準備前後で変わる現象も隔離converterで再現した。製品モデルの採用変更・インストール・新リリース公開はまだ行っていない。

## 採点モデルの単純交換

同じAJIMEE200問・正解集合・文脈、同じzenz・学習済み追加生成・生成幅8・LLM採点上限32を使用。`KOTORI_LLM_MODEL` だけを交換した。元の候補プールの生成処理は変更していない。モデル処理の時間であり、実TSFの入力表示遅延ではない。

| 採点モデル | 正解 | 現行188に対する改善/改悪 | モデル処理中央値/p95 | 採否 |
| --- | --- | --- | --- | --- |
| TinySwallow-1.5B Q5_K_M | 188/200 | 基準 | 164/513ms | 維持 |
| Qwen3-4B Q8_0 | 173/200 | 4/19 | 196/583ms | 単純交換は不採用 |
| Sarashina2.2-3B Q5_K_M | 161/200 | 3/30 | 155/441ms | 単純交換は不採用 |
| Qwen3.5-4B-Base Q5_K_M | 181/200 | 4/11 | 465/1019ms | 単純交換は不採用 |

全モデルの実行終了コード0。使用converterはsource `fb6de138432e18deb6d45f2e336cce1fd573128f`、SHA256 `71b9d1fe92645df758b6ee039a9ddd465a951de110c04f7213c150dc720cd053`。原票 `target/accuracy-guide/ajimee-{qwen3-4b,sarashina3b,qwen35-base}-v1/` のranking.json、manifest、comparison.json、要求別traces、run.logに保存した。

Qwen3.5は先に4件の互換性試験で実採点を確認し、その後200問を実行した。最初の4件の初回モデル処理は読み込み込み6698ms、200問では最大5595ms。単に読み込めるだけで低遅延の製品採用条件を満たすとはしない。大きいモデルには改善した入力もあるが、現方式で一律置換すると改悪が多い。

各モデルの量子化方式も異なり、パラメータ数のみを変えた厳密な因果比較ではない。現採点はチャット応答の生成ではなく、生文章の `log P(候補|前文)` とzenzの合算である。Qwen3-4Bの丸めログで重み0〜1.2を再計算しても基準を超えなかった（199件を再現、1件はprefix比較のため除外、NFKC照合を使った診断値）。`qwen3-weight-replay.json`。この結果だけで大きいモデルの能力が低いとは断定しない。

### モデルの固定

- Qwen3-4B: `Qwen/Qwen3-4B-GGUF`、revision `bc640142c66e1fdd12af0bd68f40445458f3869b`、`Qwen3-4B-Q8_0.gguf`、SHA256 `8c2f07f26af9747e41988551106f149b03eb9b5cb6df636027b6bf6278473300`。既存 `.local/context-core/models/` を読み取りだけで再利用。
- Sarashina: `mradermacher/sarashina2.2-3b-GGUF`、revision `85fa5d035098abdb173064741beaa136a2d737e7`、`sarashina2.2-3b.Q5_K_M.gguf`、SHA256 `b5881890d5ec15a79540e3ad57e4024f9ae69527b197f4e9e968c9f7e6d4797f`。
- Qwen3.5: `mradermacher/Qwen3.5-4B-Base-GGUF`、revision `4fb54fe7d3a51951fddba9fab026259f83d577fc`、`Qwen3.5-4B-Base.Q5_K_M.gguf`、SHA256 `f3da1af8ec9e7c40f57a29a3c43a061443deb23697bf703d2c60c7ec69ea81d8`。

後者2モデルは `target/accuracy-guide/model-upgrade-20261018/` へ取得し、公開メタデータのサイズ・SHAと照合した。取得receiptを同じフォルダへ保存。重みをGitへ追加せず、導入済みモデルも変更していない。[Sarashina提供元](https://huggingface.co/sbintuitions/sarashina2.2-3b)、[Qwen3.5提供元](https://huggingface.co/Qwen/Qwen3.5-4B-Base)。

## 報告例の再現

同じプロセス、同じ読み「あいであきょく」、確定や学習をせずresetを挟んで比較した。起動直後は「アイデア局」、状態ファイルで `state=llm` を確認した後は「アイデア曲」。前者の要求にはAI採点のtimeログがなく、後者にはzenz・LLM採点がある。AI準備前のMozc結果という仮説を裏付ける。ただし導入済みIME・実アプリでの再現証明や修正完了ではない。

`target/accuracy-guide/idea-startup-v3/` と、保存用ツールの実行 `idea-startup-tool-v1/` に原票。後者は初回193ms・準備後144ms。採点済みAJIMEEではPRELOAD=0でモデルを待つため、この差を表していなかった。

再現ツール `mozc/tools/check_startup_quality.py` は既存のモデルを隔離プロファイルで使い、同じ読みの初回と `state=llm/zenz` 到達後を比較する。期待と不一致があれば成功扱いにしない。現在は初回不正解のため不一致として終了する。試験プロファイルと原票を保持し、導入済みIMEへ入力・学習しない。

```powershell
python mozc/tools/check_startup_quality.py --converter C:/Users/aruik/ag16/src/bazel-bin/converter/converter_main.exe --model-dir target/accuracy-guide/learned-generator-bundle-v1 --runtime-dir 'C:/Program Files (x86)/Kotori' --reading あいであきょく --expected アイデア曲 --out target/accuracy-guide/idea-startup-next
```

最初の試験は日本語を含むプロファイル引数で状態を取得できず終了。次は準備完了を誤って `state=ready` と想定してタイムアウトした。実装を読み、実出力の `state=llm/zenz` へ修正して再現できた。失敗原票 `idea-startup-v1/v2` も保持する。

## 読みと文脈を明示する採点実験

専用ソース枝 `codex/conversion-score-prompt-20261018`、commit `b4f29c1af9af8bb269ac16cabc27e8a2f7acdd08` に、`KOTORI_LLM_SCORE_PROMPT=qwen3` のときだけ変換指示・読み・前文を公式チャットテンプレートへ渡して既存候補の尤度を計算する実験を実装した。Qwen3の空の思考区間を閉じたassistant prefixを用い、任意文の生成や候補差し替えは行わない。未指定なら従来の採点処理を維持する。制御トークンと過大な入力、テンプレート非対応は拒否する。

converter build成功18.706秒、SHA256 `96b15ecdca1a76eb71fc6fff02814a46c6d2d18c283e076cd6e18a1932d8ba51`。最初のbuildはBazelをsrcの親から呼び失敗し、srcから実行して成功した。ログを別名で保持。新バイナリの実験OFFは188/200、従来188版との出力変更0。`target/accuracy-guide/ajimee-prompt-off-v1/`。

ONは104/200、改善4・改悪88、出力変更112。モデル処理中央値221ms・p95 645ms。`target/accuracy-guide/ajimee-qwen3-prompt-v1/`。指示を付けた文章生成の尤度も変換候補の正しさを測る目的に合わず、不採用とする。製品の0002パッチへ実験コードを入れていない。実験ソースと原票は別枝に保持する。

前回の自動承認審査の上限は今回解除されており、現在の実行上のブロッカーではない。新リリース候補は188/200構成を維持するが、起動時不一致の解消・モデル変更の有効な適用・MSI・実TSF・配布条件は未完了。次は大きいモデルに少数の既存候補を選ばせる方式と、準備前後の品質差を減らす実装を検討する。指示付き文章の生尤度が悪かった結果を、候補選択モデル全般の不可能性とは扱わない。
