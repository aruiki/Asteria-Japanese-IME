# 初期候補用の連語データ

Mozcの既存CollocationRewriterへ渡す短い連語を作る。実行時モデルを増やす変更ではない。辞書本文/コーパスはGitへ追加しない。採否は[ADR0064](../../docs/adr/0064-contextless-collocations.md)の同条件比較後に決める。

元データは[Miwa-Keita/zenz-v2.5-dataset](https://huggingface.co/datasets/Miwa-Keita/zenz-v2.5-dataset)、revision `7d9c9ea36347b638a627c1d264da48ebc7eb38aa`。Wikipedia部分のCC BY-SA 4.0だけを使用し、元のWikipedia執筆者とデータ加工者Keita Miwaを出典とする。Common Crawl部分は使用しない。追加連語もCC BY-SA 4.0で提供する。Mozcの元40件はGoogleによるBSD-3-Clauseのデータとして保持する。

`sources.json`で固定したWikipedia学習区間7つ（合計7MiB）だけをHTTP Rangeで取得する。200応答・範囲/長さ/SHA不一致を拒否し、既存の別データを上書きしない。31,208文の正規化sample SHAは`f886d411a7565171733edc9a3e6aa5f659affcc030eae277e5a6213165564255`。dev区間00を取得しない。従来の全体sample SHA `3d47371a7704fa359d8baeed03b25f4d820e68b299fb75f3fe348bb583878552`も低水準generatorだけで検証できるが、配布ビルドは専用の固定区間を使う。

```powershell
python -X utf8 -m unittest discover -s training/collocations -p test_stage.py -v
python -X utf8 -m unittest discover -s mozc/tools -p test_fast_collocations.py -v
python -X utf8 training/collocations/stage.py --baseline <Mozc>/src/data/dictionary_oss/collocation.txt --out target/kotori/fast-collocations-v1
Copy-Item target/kotori/fast-collocations-v1/fast_collocations.txt <Mozc>/src/data/kotori/
Copy-Item target/kotori/fast-collocations-v1/fast_collocations.json <Mozc>/src/data/kotori/
Copy-Item target/kotori/fast-collocations-v1/NOTICE-fast-collocations.txt <Mozc>/src/data/kotori/
```

`--out`は新しいディレクトリを指定する。`--cache-dir`で既存の取得原票を読み取り専用で再利用でき、`--ajimee`で固定SHAのAJIMEEを渡せる。既定は固定revisionから取得。除外は製品の`eval/sets/*.json`の読みと固定AJIMEEの読みだけで、参照表記を抽出へ渡さない。900の評価読みを除外対象とし、近似一致1件・重複文285件を排除。Wikipedia学習文29,326件から生成し、元40件+追加8,405件を保持する。外部の既存5例も除外した初期試作と結果SHAは同一だった。

結果SHAは`f576ffc696ad81f244fb5791a2571ac1edb320f2a2f20d0773ea6a044f01b29c`。CIがこのコマンドで生成し、Bazelが既存CollocationRewriter用のデータへコンパイルする。実行時の読取や通信は追加しない。再構築・出典確認用の本文/manifest/NOTICEもMSIへ同梱し、ビルド時と`check_release.py --require-collocations`の管理展開で欠落/異なるSHAを拒否する。生成物は`make_patches.sh`の対象外。本文をGitへ追加しない。

実測結果と限界は[連語の検証記録](../../docs/verification/accuracy-fast-collocations-20261018.md)へ保存する。データの件数を精度の代わりにせず、実TSF・初回入力・通常変換の確認は別に残す。
