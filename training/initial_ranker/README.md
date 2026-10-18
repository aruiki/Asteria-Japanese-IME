# 初期順位の文字統計実験

[ADR0065](../../docs/adr/0065-initial-statistical-ranking.md)のオフライン実験。製品へ未採用。既存Mozcの全文候補とコストを使う。Google日本語入力との比較や実TSFの証明ではない。

元データは固定zenz-v2.5-datasetのWikipedia学習区間01〜07だけ。取得は[連語の手順](../collocations/README.md)と同じ。Wikipedia日本語版の執筆者・Keita Miwaに帰属し、加工した統計モデルもCC BY-SA 4.0。Common Crawl/dev00を使わず、本文/候補原票/統計モデルをGitへ追加しない。

01〜06を学習、07の256文だけを係数選択へ使う。評価入力/近似・同じ文/読みを除外する。評価の正解表記を学習や係数選択へ渡さない。文字1〜3gram・平滑化・係数候補・選択規則はADRで結果前に固定している。

```powershell
python -X utf8 -m unittest discover -s training/initial_ranker -p test_*.py -v
python -X utf8 training/initial_ranker/prepare.py --corpus <固定sample.jsonl> --ajimee <固定AJIMEE> --out <新規モデル出力>
python -X utf8 training/initial_ranker/collect.py --exe <診断converter_main.exe> --source-commit <commit> --data <モデル出力>/calibration.jsonl --out <新規原票出力>
python -X utf8 training/initial_ranker/calibrate.py --model <モデル出力> --calibration <較正原票> --out <新規係数出力>
```

診断converterの`--kotori_lattice_trace`は通常false。明示的に指定したCLIだけで、候補の読み/表記をUTF-8のhexと元コストで出す。収集器は欠落・重複・かな/数字が違う読みを拒否する。Composerが英数/括弧の後で揃える句読点のASCII表記だけは同じ読みとして扱う。参照表記の採点は変えない。較正収集v1はこの句読点差2件で失敗し原票を保持、陰性回帰を追加してv2を作成した。
