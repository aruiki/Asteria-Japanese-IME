# 初期候補用の連語データ（実験）

Mozcの既存CollocationRewriterへ渡す短い連語を作る。実行時モデルを増やす変更ではない。辞書本文/コーパスはGitへ追加しない。採否は[ADR0064](../../docs/adr/0064-contextless-collocations.md)の同条件比較後に決める。

元データは[Miwa-Keita/zenz-v2.5-dataset](https://huggingface.co/datasets/Miwa-Keita/zenz-v2.5-dataset)、revision `7d9c9ea36347b638a627c1d264da48ebc7eb38aa`。Wikipedia部分のCC BY-SA 4.0だけを使用し、元のWikipedia執筆者とデータ加工者Keita Miwaを出典とする。Common Crawl部分は使用しない。追加連語もCC BY-SA 4.0で提供する。Mozcの元40件はGoogleによるBSD-3-Clauseのデータとして保持する。

現在は以前取得した`context-core/corpus/sample.jsonl`と取得manifestを入力にする。sample SHA `3d47371a7704fa359d8baeed03b25f4d820e68b299fb75f3fe348bb583878552`。gen scriptがrevision/SHA/学習区間とライセンス識別を検査し、既存dev区間00を排除する。取得済み原票を上書きしない。

```powershell
python -m unittest discover -s mozc/tools -p test_fast_collocations.py -v
python mozc/tools/build_fast_collocations.py --corpus ../context-core/corpus/sample.jsonl --corpus-manifest ../context-core/corpus/sample.manifest.json --baseline C:/Users/aruik/ag16/src/data/dictionary_oss/collocation.txt --exclude eval/sets/kotori-daily.json --exclude <他の固定評価JSON> --out target/accuracy-guide/fast-collocations-data-v1
```

実行原票の`fast_collocations.json`に、実際に除外した全評価ファイルのSHAと生成条件・結果SHAを保存する。v1は905の評価読みを除外対象とし、近似一致1件、重複文285件を排除。Wikipedia学習文29,326件から抽出し、参照表記を使用していない。

当面のデータ配置は実験専用Mozcの`src/data/kotori/fast_collocations.txt`。生の辞書はコミットしない。CIでの取得/生成・NOTICE/同梱の接続は採否判定後に実装する。未接続状態をリリース可能としない。
