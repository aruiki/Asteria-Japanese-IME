# 短文向け語彙候補

2026-10-22の利用者指定により、単語・短文・前文なしの変換精度を改善する。現在は製品採用前。[生成と採否の条件](../../docs/adr/0068-short-input-lexicon.md)。

SudachiDict v20261015の公式small/core/notcore CSVから、Mozc辞書と重複しない読み/表記を持つ名詞の候補を生成する。公式ZIPとLEGAL/ライセンスのSHAを`sources.json`へ固定し、辞書本文をGitへ含めない。小規模Wikipedia抽出の増量だけで終えず、更新される語彙資源の取得と辞書への接続を検証する。

Sudachiは形態素解析用の辞書であり、Mozcと品詞・コストの体系が異なる。元の接続IDとコストをそのまま使わない。品詞の対応と、共通語彙のコスト分布による変換は別の監査情報として保存する。元の辞書/プロファイルを改変せず、隔離した候補を作って前後比較する。

同梱時はApache-2.0の全文と、LEGALにあるUniDic/NEologd等の帰属・条件、加工内容を配布情報へ接続する。現段階では取得・比較用であり、v2.0.0へ導入済みとは扱わない。

## 生成と比較

```text
python -X utf8 training/lexicon/fetch_sources.py --out target/short-lexicon/source
python -X utf8 training/lexicon/build_lexicon.py --source target/short-lexicon/source --mozc <Mozcツリー> --out target/short-lexicon/candidate
python -X utf8 -m unittest discover -s training/lexicon -p test_lexicon.py -v
```

`--cache`で既存の取得原票を検証して再利用できる。取得サイズ/ZIP内容/全SHAが合わない場合は停止し、異なる既存ファイルを上書きしない。途中失敗の.partialも保持するため、失敗後は原票を確認し新しい出力先を使う。

候補の`short_lexicon.tsv`をMozcの`src/data/kotori/`へ配置し、比較用ビルドだけで`--define=asteria_short_lexicon=true`を指定する。データは既存システム辞書のコンパイルへ入り、実行時にCSVや外部解析エンジンを起動しない。フラグを指定しない既定ビルドは従来の辞書のまま。比較結果を確認するまで製品採用へ切り替えない。

評価は`eval/short-input-20261022.json`で開発120件と最終報告120件を分離しSHA固定した。最終報告用は調整に使わない。前文なしのAI無効/Standardを分け、既存4セットの回帰も確認する。先に基準EXEとrunfiles内の辞書を別ディレクトリへ実コピーし、ビルドによる差し替えを防ぐ。
