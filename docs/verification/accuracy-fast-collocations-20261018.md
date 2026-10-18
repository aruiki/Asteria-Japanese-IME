# 初期連語と通常変換の検証（2026-10-18）

元のOSS連語40件に、固定したWikipedia学習区間から8,405件を追加した。既存MozcのCollocationRewriterとコンパイル済みBloom filterを使い、実行時のモデル・通信・ファイル読取を追加しない。ユーザー辞書/変更禁止/コマンド/AI選択済みの上位候補を連語補正で上書きしない。

固定した4開発セット・3モードの978出力で、AIなしAJIMEEは102→103/200（改善1、改悪0）。他のAIなし3セット、AI準備後の前文なし/ありの全652出力は変更なし。データの増量を大幅な精度改善とはしない。Google日本語入力との直接比較ではなく、通常操作・AI準備前の全品質・実TSFの完成を意味しない。

## 比較対象

| 対象 | 識別 |
| --- | --- |
| 変更前Mozc | 3d1dfdee6ac1b410db4d1d1a7fff6af75f56f000 |
| 連語・候補保護のMozc | 4bf39c458b1807180c5ddc26d46536788594adf1 |
| 比較時の製品 | 3b440cfc15361f6366297584600621838de6e30d |
| converter SHA-256 | 9d62b233ae2eb1a0d9aba5032f3cba9a34ae829ed2ca07ca8b0a7fafdd6c5de1 |
| session SHA-256 | 901b84051bc0f1da5a40915080823d4a5ed0f6bfc284d8101ecd04b4775cda74 |
| 新連語 | 8,445行、99,282bytes、SHA f576ffc696ad81f244fb5791a2571ac1edb320f2a2f20d0773ea6a044f01b29c |
| MSI同梱の後続ソース | 9e4a9b256c2899fd6a07fbd88bc235e900dcbb6e（変換C++と連語の内容は4bfと同じ） |

原票は製品ツリー`target/accuracy-guide/fast-collocations-candidate-v1/identity.json`、`fast-collocations-quality-v1/{plan,summary}.json`と各モードのJSON/manifest/stderr。変更前は`contextless-baseline-v1/`。同じ4参照集合、既定Standard係数、同じ4モデルと最終OOM修正版runtimeを使い、各実行は新規プロファイル。全12実行exit0。同期評価用PRELOAD=0であり、初回の打鍵/即Spaceではない。

## 精度比較

| 固定セット | AIなし・前文なし 旧→新 | AI準備後・前文なし 旧→新 | AI準備後・前文あり 旧→新 |
| --- | --- | --- | --- |
| AJIMEE 200 | 102→103 | 175→175 | 188→188 |
| 慣用句25 | 22→22 | 24→24 | 24→24 |
| ニュアンス20 | 11→11 | 17→17 | 17→17 |
| 日常81 | 65→65 | 79→79 | 79→79 |

唯一の出力変更は既存AJIMEE ID1545。「シカイヲツトメ、ジシンノブログデモソノチョウカ」が「視界を努め、自身のブログでもその釣果」から「司会を務め、自身のブログでもその釣果」へ変わり、固定参照に一致した。結果後の語句の追加/削除、正解集合変更、係数変更は行っていない。

## 生成・配布経路

[固定取得/生成の手順](../../training/collocations/README.md)。revision 7d9c9ea36347b638a627c1d264da48ebc7eb38aaのWikipedia学習区間01〜07（各1MiB）のみを使う。dev00/Common Crawlを使わず、評価読み/近似を除外し、評価の正解表記を抽出へ渡さない。900読みの除外と、外部5例も除外した初期試作は同じ辞書SHAを再現した。

既存原票を使う`fast-collocations-stage-v1`と、新しく公開元からHTTP Rangeで取得する`fast-collocations-download-v1`の両方がexit0で同じ辞書SHAを再現した。HTTP 200全体応答、異なる範囲/長さ/SHA、異なる既存成果物を拒否する。31,208文を正規化したsample SHAはf886d411a7565171733edc9a3e6aa5f659affcc030eae277e5a6213165564255。旧workspaceの全コーパスへの依存を除いた。

CIは生成後に本文/manifest/NOTICEをBazel入力へ置く。MSIのビルドと`check_release.py --require-collocations`の両方で必須同梱・固定結果SHA・出典/学習区間・ライセンス表示を検査する。生成物は`make_patches.sh`で除外し、生の辞書/コーパスをGitへ追加しない。使用許諾とlicensesも更新した。

## 実行した検査と残る範囲

連語のnative buildは17.861秒、C++は連語8+LM66+session190=264件成功（59.690秒、うちLMはキャッシュ）。保護対象が左右どちらの文節にある場合も、元の連語補正が動かないことを確認した。生成・取得のPython9件、同梱検査の回帰を含むmozc/tools全101件が成功。実ビルドの検査関数でも、実生成物の成功、改変辞書/欠落NOTICE/変更revisionの拒否4条件を確認した。

通常変換は[必要条件](../NORMAL_CONVERSION.md)に従い、実辞書の通常例/操作を追加検証する。AI準備前「あいであきょく」、広い語彙/人名/専門語、実TSFの表示と候補操作、保護入力、インストーラー導入/入力/削除と独立verifierは別に残る。旧705 MSIの低メモリ不合格を上書きしない。公開/CIの実行済みとはしない。

## 通常変換の実セッション

結果を見る前に製品6f1128935d3db1c040dd16395efa959974a0aad5で入力例と検査を固定。`normal-conversion-native-v1/summary.json`とresults.jsonlへ28の別新規プロファイル/PIDと全原応答を保存した。AIを無効にし、モデル/DLLの配置先も空にして、実OSS辞書を埋め込んだ固定sessionで実行した。28/28成功、固定した読み・参照・表示/確定の不一致なし。最大単一応答43.48ms（事前上限3,000ms）。前後の速度改善や低性能機の証明にはしない。

- 19通常例: 基本語彙、動詞/形容詞の活用、助詞/接続、外来語、地名、人名の複数解、数字/助数詞/金額/年月日、句読点。
- 9操作: 候補循環/指定選択/確定選択の再利用、文節伸縮/移動、取消/Backspace、F6〜F10、隔離ユーザー辞書の候補包含/選択/確定。

基礎部品も追加で実行。OSS DataManager1、数字37、日付72、ユーザー辞書17、文節履歴38、計165件・5対象がすべて成功（20.110秒、全5対象実行）。`normal-foundation-test-xml-v1/`へXMLを保存した。これらを全語彙・全キー配列・再変換・確定取消・再起動後の履歴・実TSF表示/フォーカス/保護欄の全受け入れとはしない。

## MSI・低メモリ・初回の境界

source9e4a9b256からMSIを613.170秒で生成し、`release-candidate-1.1.4-rc.1-collocations/Asteria64.msi`へ固定。SHA e4456158a90b7c735c7eaf94c8a3ba1d7315ea9b98774cde572ac6be94cc0977、1,662,418,944bytes、ProductVersion3.34.6339.100。管理展開のcheck_release（require-generator/require-collocations）成功。4モデル、追加連語/manifest/NOTICEの固定検査、版の一致、ggml-rpc不在を確認した。サーバーSHA a38dd36667da340e0be2b26e4032e6c894e4ffdaabe8805e58de7257b927b1daは実ビルド出力と一致した。確認途中は通常のbazel-bin/serverを参照してパス不在となり、transition別の実出力`mozc_server.exe.exe`で照合を完了した。失敗を製品障害に数えない。

MSI内ggml-base.dllはOOM修正済みe23b0ff25f9e0d57e66fcfe12578461a275677c0a571baf02b3d8a97fde39bd4。新辞書sessionと実MSIモデル/DLLで同じ256MiB process commit制限を再検証し、対照3/実AI失敗時3文とも入力継続成功（peak8.50/226.00MiB）。同PIDのAI errorを確認しており、実AIが準備できた合格とはしない。物理RAM256MiBの実機試験ではない。原票`fast-collocations-memory-v1/`。

同じMSI資産の初回確認は「あいであきょく」→初回「アイデア局」、準備後「アイデア曲」で不合格を保持した。`fast-collocations-idea-startup-v1/receipt.json`のimmediate.ok=false/ready.ok=true。連語追加でこの既知の課題を解決したとはしない。

パッチを固定source9eから再生成し、素のMozcへ全3パッチの適用成功と全137ファイル一致を確認。生成した辞書/manifest/NOTICEはパッチから除外されている。適用後ソースのwhitespace検査も成功。製品側のpatchファイル自体へのgit diff --checkは、追加されたunified diffの空行コンテキスト（1スペース）2箇所を指摘するが、適用後の追加ソースの空白違反ではない。パッチを手編集したり検査基準を緩めていない。原票`fast-collocations-patch-reconstruction-v1.json`。

このMSIは未導入・未公開。実TSF・導入/削除・署名・独立verifierと、未公開モデルを含むGitHub送信の明示許可は残る。過去の公開版/失敗MSIの記録を変更していない。
