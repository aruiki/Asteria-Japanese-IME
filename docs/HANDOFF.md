# 現在の引き継ぎ（2026-10-22、変換精度を中心にした紹介）

利用者は「変換精度の高い日本語IME」を価値の中心にし、単語・短文の辞書/頻度/連語強化と紹介方針の見直しをゴールに指定した。今回の専用枝codex/quality-copy-20261022はREADME/Release原稿/3画像の紹介変更だけを扱う。[対象と公開前確認](verification/accuracy-copy-20261022.md)。本体・モデル・MSI・版と評価条件は維持し、開発中の辞書を公開v2.0.0へ採用済みとは案内しない。

公式サイトはkanai PR12/13、製品の紹介はPR183/全7CI成功→main05d70edで公開済み。README/Releaseの4配置、サイト14配置と20ファイルSHA、公開4資産不変を確認。GitHub Aboutの概要欄も精度中心へ統一した。紹介変更の詳細は上記記録を参照。辞書/順位の改善はカード81の別枝で継続中。GPU AI主体の設計、製品未検証7範囲と独立FAILは維持する。

---

以下はv2.0.0公開時点の履歴。

# 履歴（2026-10-21、正式v2.0.0公開）

[正式v2.0.0](https://github.com/aruiki/Asteria-Japanese-IME/releases/tag/v2.0.0)を署名なし/Latestとして公開。利用者が古い版による二重入力の可能性を指摘したため最新RC4も実Windowsで検査し、Space直後Enterで実二重表示が残ることを確認した。Outputに古いpreeditを残す原因を修正し、新MSIへ反映した。[公開記録](verification/release-v2.0.0-20261021.md)、[修正と前後証拠](verification/duplicate-commit-20261021.md)。

PR180は全10CI成功後main298bee9へ統合。build head34763f4/実checkout9ea0b4fと全tree一致。MSI3.34.6442.100、SHA1bc531b10b25cc46fbc60a8b7b052187f2b69c9065e8a1651acc23e89bd1b72a。同じMSIで導入/再導入/入力/削除、既存8、native28、Edge14が成功。元の順序の追加Edge14も同一MSIで成功。公開4資産を再取得して全バイト・同梱・4モデル・アイコン一致を確認した。

README・INSTALL・3画像・Release本文と公式7ページを正式版へ更新済み。製品PR181/全7CI統合後、公式サイトPR10→配備3f68e6c/Pages run39620353670が成功し、公開20ファイルのHTTP/SHA、サイト14画面とGitHub4画面、総合監査が成功。カード83はdoneへ移動し、今回の公開ゴールは完了。製品全体の未検証7範囲は保持し、.goal-completeは作成しない。カード81など別の製品課題を完了扱いにしない。

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
