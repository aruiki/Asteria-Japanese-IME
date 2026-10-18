# 実変換候補からの対比学習

公開コーパスの参照表記と、実converterが出した同じ読みの候補を対比較へ変換する。参照以外の表記をすべて意味的な誤りとは呼ばず、コーパスの表記選好として扱う。学習・モデルは専用の `target/accuracy-guide/` へ出力し、導入済みモデルを上書きしない。

`native_pairs.py` は取得原票のSHAを照合し、AJIMEE等の除外入力は読みだけで照合する。近似する読みと、train/dev間の同じ読み・同じコーパス取得区間も拒否する。通常文を混ぜ、モデル選択では通常文・実候補の両方の開発正解数が基準から下がらないことを要求する。開発指標だけで製品へ採用しない。

```powershell
python -m unittest discover -s training/contrastive -p test_native_pairs.py -v
python training/contrastive/native_pairs.py --observed target/accuracy-guide/hard-candidates-v1 --general ../context-core/pairs/pairs-context-v2.jsonl --source-root ../.. --out target/accuracy-guide/native-contrastive-v1/pairs.jsonl
```

`train.py` はローカルHFモデルとCUDAでLoRA学習し、ソース・seed・データ・モデルSHAと選択stepを保存する。元コードは既存の `typing-demo/product-worktree/training/contrastive` を再利用し、通常文を維持する選択条件を追加した。他担当の元ツリーは変更しない。

`native_pairs.py` は実候補の有限なLLMスコア差も保存する。`train.py --joint-llm-weight 1.2` では順位損失と開発正解数へこの固定差を加え、製品と同じzenz+LLMの識別を学習する。通常文のNLLはzenz単体のまま保持する。これは記録された候補間の固定LLMスコアによる学習であり、生成候補・採点対象・補正も変わる実converterの評価とは区別する。

`merge_adapter.py` は選択adapterを統合し、統合前後のロジット一致を検査する。`export_gguf.py` は `--llama-root` で既存llama.cppを指定して書き出す。Torch・transformers・gguf依存のあるPythonを使う。量子化後のGGUFは実converterのAJIMEEで比較し、改善・改悪とモデルSHAを保存する。学習データ・モデル重みはGitへ追加しない。

今回のモデルは主生成・採点の置換では悪化したため、追加生成だけに使う。元のzenzとTinySwallowで全候補を採点するとAJIMEE186→187/200、改悪0だった。元モデル3件を保持したローカル同梱候補は次のコマンドで作る。出力済みのフォルダと導入済みIMEは変更しない。

```powershell
python training/contrastive/stage_generator.py --base-dir 'C:/Program Files (x86)/Kotori' --generator target/accuracy-guide/native-joint-512-v1/native-q8_0.gguf --out target/accuracy-guide/learned-generator-bundle-v1
```

モデル識別は `native_generator.json`。`distribution_status` はローカル候補であり、公開済みモデルではない。


リリース候補では `fetch_generator.py --out target/kotori` が固定URLから取得し、固定SHAと一致するモデルだけを配置する。ローカル候補では `--source <GGUF>` を指定できる。既存の不一致モデルを上書きしない。モデルと `native_generator.json` はMSIへ含め、WiX実行前とMSI展開後に照合する。

2026-10-18時点で取得URLのモデル資産は公開前。ローカルのMSIビルド・内容検証を先に行う。GitHubへの送信許可後にモデル資産を公開し、取得試験を通してからCI・PRを進める。未公開URLのままmainへ統合しない。
