# 現在の引き継ぎ（2026-10-22、短文の語彙・順位を改善）

利用者は製品の価値を「変換精度の高い日本語IME」とし、単語・短文・前文なしの辞書対策と紹介方針の見直しを新ゴールに指定した。カード81を継続し、[ADR0068](adr/0068-short-input-lexicon.md)へ比較前の条件を固定した。製品枝codex/short-input-quality-20261022はmain769ed9eから開始。

SudachiDict v20261015の全288万行を公式ZIP/LEGAL/ライセンスのSHAで固定し、取得・名詞抽出・Mozc品詞への対応・共通語のコスト分布による候補生成をtraining/lexiconへ実装。取得破損/品詞/読み/コストの6試験が成功。最初の候補は165万語だが製品採用前。コスト較正を同一品詞の共通語に限定したv2を生成している。変換結果を見る前に、開発120/最終報告120の短文評価をSHA固定した。最終報告用は調整へ使わない。

基準Mozc785bd83aのEXE/辞書をtarget/accuracy-guide/short-input-baseline-native-v1へ複製し、converter SHA14284c10...、session SHA81f85a90...を確認。ソース差分は空。これから基準の開発短文を測り、隔離した追加辞書をコンパイルして候補欠落・順位・改悪を比較する。データ量だけで採用しない。大きなコーパス由来の頻度/連語、Google同条件比較、実TSF・配布候補、コピー/画像/サイトの変更も残る。

公開v2.0.0は署名なし/Latestのまま保持。[実配布と対応ソース](verification/release-v2.0.0-20261021.md)。新辞書の未検証をv2.0.0の成果へ混ぜない。AI主体の非同期候補表示、保護欄、ユーザー辞書、二重入力修正と、製品全体の未検証7範囲・過去の独立FAILは維持する。

---

以下はRC4時点と二重入力調査開始時点の履歴。

# 現在の引き継ぎ（2026-10-21、最新候補の二重入力確認→v2.0.0）

最新依頼は署名なしの正式v2.0.0公開。利用者の二重入力報告を受け、修正・再ビルドも許可された。ただし導入済み3.34.6438.100が古く、現在は解消している可能性も指摘されたため、まず最新RC4/6440で再現確認する。再現した原因は修正し、再現しない場合は憶測で本体を変更しない。[カード83](tasks/done/83-v2-stable-and-duplicate-input.md)。実デスクトップは操作せず、固定MSIの使い捨てWindows CIで確定イベントとホスト文字列を記録する。まだv2.0.0は公開していない。

比較run39575173488で両版のSpace直後Enterに、確定結果と同じpreeditが残った。非同期変換と確定が同じOutputへ追記される原因を修正した。source785bd83a7f236b49925f7a83764d5beadf3bb378、修正前のSpace/Tab2回帰失敗→修正後C++371件成功、GPU/CPUの隔離Session各4条件成功、再生成patch140ソース一致。[検証記録](verification/duplicate-commit-20261021.md)。固定RC4ではEdge textareaの実二重表示も再現した。実一覧を開く手順へ試験を補正し、RC4のnative28条件は既知のSpace直後Enterだけが失敗、残る27件が成功した。補正Edgeも全14件中13件成功、同じSpace直後Enterの実二重だけが失敗。新MSI用の全42条件の判定手順を確認できた。次は新MSIの実TSF28条件とEdge14条件、全CI、版/SHA/対応ソースを確認してv2.0.0公開へ進む。

直前の販促刷新は完了。README、月ロゴによる3画像、添付文を活かした導入ガイド・RC4本文を公開した。[素材](marketing/README.md)、[確認記録](verification/marketing-refresh-20261021.md)、[カード82](tasks/done/82-marketing-refresh.md)。100 build入力と実MSIのハッシュ一致を確認。PR178/179とkanai PR8/9を統合し、公式サイト全7ページも配備6afbd5a/Pages run39535159315で公開した。20ファイルのHTTP200/SHA一致、公開画面7ページ×PC/390pxを確認。この時点では本体や版を変更していなかった。

[v1.1.4-rc.4を公開済み](https://github.com/aruiki/Asteria-Japanese-IME/releases/tag/v1.1.4-rc.4)。採点済み候補保持と予測期限の修正を製品パッチ/新MSIへ反映した。公開4資産を再取得し、全バイト・同梱・アイコンが一致。[公開記録](verification/release-v1.1.4-rc.4-20261021.md)。

## 公開ソースとMSI

PR176は全8CI成功後、main feaeddc3ee40a9efc3ab4049166e042fef1d69faへmerge commitで統合。製品head 813e78aa568826b1cf68eeeab494616d82b3b8a0、CI checkout ed86426e326770f837401069f58e41bf75cdc993、統合commitのtreeは同一。Mozc source039a7251fa16a77a91cb67f5158e20073a3f3762、140 source再構成一致、98+2入力blobを固定。

[Windows run39433685950](https://github.com/aruiki/Asteria-Japanese-IME/actions/runs/39433685950)で同じMSIのbuild・C++・Qt・同梱検査・候補8項目・導入/再導入/入力/削除が成功。版3.34.6440.100、SHA-256 2b37f6fbdcb069251e820e2009b096111cc2253e79ffac65d973fa11ce18e301。導入server/renderer/TIPと管理展開した実バイトが一致した。

RC4はprerelease=true/Latest=false、既存RC2のLatest区分は不変。AuthenticodeはNotSigned。同一MSI/同じbootの再導入を旧版更新/再起動の検証へ読み替えない。

## 実装と固定実モデル検証

[採点済み候補保持](verification/scored-alternatives-20261020.md)は有限採点済みの別仮説を最大8件、読みと既存文節keyが合う場合だけ保持。第一候補/ユーザー辞書を保ち、新規代替はNO_LEARNING。700ms予算・閾値・モデルは不変。期限後の部分集合を通常cacheへ保存せず、弱い続きは本文の先頭を置き換えない。

最終ソースでC++445件・工具2件成功。同じモデル/DLL・辞書・データ・評価器で978の第一候補が全一致、Standard AJIMEE前文あり188/200・なし175/200を維持。候補包含は前文なし189→193/200、あり193→195/200、改悪0。最終300例も290/300、全出力一致。予測29例は先頭一致12→12、上位3一致19→20、改悪0・確定差0。第一候補精度や速度の改善とはしない。

固定EXEの隔離Sessionと実GPUで欠落2候補の実ID選択・表示・確定、AI待機/編集/取消/保留Enter/後続打鍵、Tab、[文脈引継ぎ](verification/queued-context-20261019.md)、通常変換/保存を確認。[AI主体の候補表示](adr/0067-ai-first-candidate-publication.md)を維持する。ローカル詳細検査と新MSIのEDIT/RichEdit受入れは別の証拠。

推論DLLの追加監査で、4モデルと他の推論DLLは評価時と全バイト一致。ggml-base.dllのSHA差はCOFF時刻欄の3バイトだけで、その他全バイトは一致した。新MSIの実モデル/DLLでも実GPUの欠落2候補の選択・表示・確定が成功。元の不一致原票と差の全バイト検査を[公開記録](verification/release-v1.1.4-rc.4-20261021.md)に残した。

## 残る製品検証

実アプリGPU AI ON/OFF寄与、複数アプリ、低性能実機、保護欄内部のAI/学習、旧公開版更新、再起動、独立verifierは残る。配布情報のremaining_verificationへ7範囲をnot_verified、product_complete=falseとして保存。製品完成や.goal-completeにしない。

次の製品検証は、導入済みMSIとactive TIPのSHAを固定し、実アプリでGPU AI ON/OFFの候補差と表示/Space/確定の一致を確認する。複数アプリ・低性能・内部privacy・旧版更新/再起動・独立verifierもそれぞれの条件で確認する。今回の限定受入れを代用しない。

[通常変換の条件](NORMAL_CONVERSION.md)、[精度改善カード81](tasks/81-accuracy-guide.md)、[測定値の紹介条件](BENCHMARKS.md)。規約は[AGENTS.md](../AGENTS.md)、コマンドは[DEVELOPMENT.md](DEVELOPMENT.md)。
