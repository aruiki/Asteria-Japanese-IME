# 短文向け語彙候補

2026-10-22の利用者指定により、単語・短文・前文なしの変換精度を改善する。頻度/POS下限を修正したv4を配布準備候補に固定。[生成と採否の条件](../../docs/adr/0068-short-input-lexicon.md)、[最終比較](../../docs/verification/short-input-lexicon-20261022.md)。公開v2.0.0には追加していない。

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

初期の比較では`--define=asteria_short_lexicon=true`だけで有効にした。現在の配布準備パッチはv4を既定辞書へ加える。`false`は辞書を外した回帰比較専用であり、配布MSIには使わない。既存システム辞書のコンパイルへ入り、実行時にCSVや外部解析エンジンを起動しない。元のユーザー辞書や履歴の形式を変更しない。

評価は`eval/short-input-20261022.json`で開発120件と最終報告120件を分離しSHA固定した。最終報告用はv4を固定後に一度だけ実行済みで、その結果による調整は行わない。前文なしのAI無効/Standardを分け、既存4セットの回帰も確認した。基準EXEとrunfiles内の辞書を別ディレクトリへ実コピーし、ビルドによる差し替えを防いだ。

## 比較済みデータの配置と更新

`package.json`はTSV・原生成記録・全帰属のサイズ/SHA/配布URLを固定する。`stage_package.py`は取得元、上限、全SHA、TSV形式と件数、原生成記録の出典・評価参照未使用を確認する。既存の不一致ファイルや失敗の.partialは上書きしない。原生成記録の`product_adopted=false`は生成時の事実として保持し、現在の公開状態は検証記録で区別する。

```text
python -X utf8 training/lexicon/stage_package.py --source <比較済みv4の3ファイル> --out <Mozc>/src/data/kotori
python -X utf8 mozc/tools/check_release.py --local <候補MSI> --previous-local <公開MSI> --require-generator --require-collocations --require-short-lexicon --work <新規出力先>
```

将来の固定データタグは`data-short-lexicon-20261022`。GitHubアカウント停止HTTP403のため、現時点ではこのデータタグ/資産は未公開。既定のネットワーク取得とWindows CIはデータ資産公開まで成功しない。ローカル配置は`--source`を使用し、検査を省略しない。データタグを公開しても製品Latestにはしない。

更新時は出典/解析器/規則を新しい固定版に変更し、取得→全量集計→`build_lexicon.py`→`build_frequency_lexicon.py`を新規出力先で再実行する。集計には全評価の読み/近重複除外を渡す。生成TSVのSHAで再現を照合し、実行秒数等を含む新生成記録を保存する。旧JSONの時間や旧評価の結果を書き換えない。比較前に採否条件を固定し、既存/新未使用評価、配布manifest、ネイティブのビルド前SHAと同梱検査を一緒に更新する。今回開いた未使用120件を次の版の未使用評価へ再利用しない。

## コーパス頻度と連語

初案は既存変換に改悪があり[不採用](../../docs/verification/short-input-lexicon-20261022.md)。`corpus_sources.json`のWikipedia全量4.13GBをSHA固定して使う。Common Crawl部分は含めない。取得と集計は別で、件数だけで品質改善を扱わない。

```text
python -X utf8 training/lexicon/fetch_sources.py --config training/lexicon/corpus_sources.json --out target/short-frequency/source
python -X utf8 training/lexicon/collect_frequency.py --source target/short-frequency/source --runtime target/short-frequency/runtime --out target/short-frequency/counts --exclude <評価JSON> --exclude <別の評価JSON> --shards 8
python -X utf8 -m unittest discover -s training/lexicon -p "test_*.py" -v
```

`--exclude`には全評価JSONと固定AJIMEEを指定する。参照表記は集計へ渡さず、読みと三文字集合Jaccard0.8以上を除外する。旧dev00は`frequency_policy.json`の固定区間SHAから完全行の入力を読み、その近重複を他の区間からも除外する。未使用評価の正解を開発へ渡さない。

学習用のSudachiPy0.7.0とfull辞書は`tokenizer_sources.json`で固定し、`stage_tokenizer.py`で私有のruntimeディレクトリへ展開する。グローバルインストールとIMEの実行時依存は増やさない。既存の不一致ファイルや途中の集計を上書きしない。

NFKC本文hashで分割し、重複本文を同じworkerで除去してから形態素を数える。Counterと重複集合には上限があり、SQLiteへ分割保存した結果をソートmergeする。語頻度、2〜3形態素の連語、表記頻度を出力。列は語頻度が「読み/表記/Mozc品詞/出現数/異なる本文数/hash区分bitmask」、連語と表記頻度が「表記/出現数/異なる本文数/bitmask」。読みは解析器の優先読みであり、発音照合済みの頻度ではない。hashの16区分は独立サイト/記事ではない。

`--limit-bytes`は処理量の試走用。採用候補には使わない。全量処理では開始前と読み出し中の両方で本文SHAを照合し、出典/規則/評価/解析器/出力のSHAとCC-BY-SA4.0帰属を保存する。集計結果を使う選択・コスト規則と実変換の採否は、次の段階で別に固定する。

## 追加語彙の探索用診断

`build_diagnostic.py`は追加辞書と頻度から、8〜24文字の読みを持つ語を品詞ごと最大50件、固定hash順で選ぶ。既存評価の読み/近重複を除外し、同じ読みの既存Mozc名詞も許容表記へ加える。参照は辞書由来であり、NFKC/文字幅の許容も最初の出力を見る前に決める。元の開発120件の参照や判定は変えない。

これは追加語の候補欠落を調べる探索用の機能診断であり、独立評価でも一般精度の推定でもない。辞書にある表記が、その読みで利用者が常に意図する表記とは扱わない。読みの正確さも独立した人手検証ではない。本文はGitへ入れず、[規則と304件のSHA](../../eval/lexical-diagnostic-20261022.json)を保存した。元の開発120件の厳密参照や最終120件の判定は変更していない。

```text
python -X utf8 training/lexicon/build_diagnostic.py --candidate <固定候補> --frequency <固定全量頻度> --mozc <Mozcツリー> --exclude <既存評価JSON> --out <新規診断出力先>
```
