# 現在の引き継ぎ（2026-10-18、新リリースと初回応答）

この文書は現在の状況と次の作業だけを更新する。過去の指示・測定値は[履歴](HANDOFF_HISTORY_20261012.md)へ保存した。履歴の「現在」「次」は記録当時の意味であり、作業指示として再実行しない。

## 公開版

[Asteria v1.1.3](https://github.com/aruiki/Asteria-Japanese-IME/releases/tag/v1.1.3)を公開済み。公開commitはdc9323ce1b96f20090290fbd7153515353235bcd。MSIは未署名。導入・日本語入力・削除と公開後再取得を検証した。成果物の版・SHA・検証境界は[リリース検証記録](verification/release-v1.1.3-20261012.md)を参照する。

## 開発中

2026-10-18: 新リリース版を開発。現在の製品枝は `codex/release-quality-calibration-20261018`、Mozcソースは `C:/Users/aruik/ag16` の同名枝、commit705e64361c61708a68716954bc8969046cb842b4。[初回キーのDLL待ち](verification/accuracy-nonblocking-device-startup-20261018.md)を非同期化。CPU/GPU各3組で最初のキー中央値111.85→0.69ms/122.31→0.77ms、全360件の入力経路欠陥0。AI不在/休止復帰も12件成功、C++261件成功、パッチ再構成132ファイル一致。新ソースもAJIMEE188/200、慣用句24/25、ニュアンス17/20、日常79/81、全326件の出力が直前版と一致した。

前段の較正ソース407a449f5は既定値でAJIMEE188/200を維持し、日常78→79/81・ニュアンス16→17/20、開発合計306→308/326（2改善/改悪0）。既出300問の回帰は286→290/300（4改善/改悪0）、古い289基準比には1件改悪が残る。未使用評価の代わりにはしない。[較正と限界](verification/accuracy-confidence-calibration-20261018.md)。

追加モデルのBazel/WiX同梱・固定SHA検査とCIを接続し、較正前ソース3b29ddd6fで実MSI生成・展開・モデル全4件一致・版3.34.6339.100・未署名を確認した。[MSI同梱の記録](verification/accuracy-release-generator-20261018.md)。この旧MSIに新しい較正は含まれない。新ソースはC++261件成功・製品パッチ再構成132ファイル一致。現在の変換器と辞書は `target/accuracy-guide/confidence-default-native-v1/evaluation-converter/` に固定コピー済み。

後続の[較正済みMSI](verification/accuracy-calibrated-installer-20261018.md)もローカル生成・管理展開・モデル全4件のSHA・サーバーのビルド元一致が成功。SHA e09bffa5479ea44b95afd4d2a14a89ba900be95ada0c8ba512dc15a74ae530fb、版3.34.6339.100、未署名。`release-candidate-1.1.4-rc.1-calibrated/`へ旧候補と別に保存し、未導入・未公開。実TSF/導入/削除は未確認。

係数固定後に作成・事前コミットした[実用40文の補助比較](verification/accuracy-release-probe-20261018.md)は旧/新とも38/40一致、出力変更0。1件は評価側の読み作成ミスで元JSONと結果を保持し、製品誤りや独立合格判定には使わない。もう1件「容量を開けます」は残る。結果後の係数変更・再学習なし。

「あいであきょく」は初回「アイデア局」/AI準備後「アイデア曲」が引き続き再現する。[CPU・即時Space](verification/accuracy-cpu-startup-20261018.md)は2スレッド・各打鍵を個別送信し、入力経路30/30成功、参照表記28/30一致。誤変換2件はloading中、別プロファイルの準備後15件は一致した。CPU自動Lowの隔離セッションであり、低性能実機/実TSFの証明ではない。初回課題、既出回帰の誤変換1件、新しい未使用評価、低資源、実TSF・新MSI導入/削除は残る。モデルURLの資産は未公開、GitHubへのコード/モデル送信は過去に自動承認審査で明示許可なしとして拒否され、今回もpush/PR/公開を再試行していない。送信を人間が明示許可した後にモデル資産公開・取得確認・PR/Windows CIを進める。未公開モデルURLのままmainへ統合しない。

以下の学習・比較は経緯の記録。

追加の実候補学習を `codex/native-contrastive-20261017` で進めた。`training/contrastive/` に実候補作成・通常文を保つモデル選択・固定LLMスコアを含む順位損失・統合/GGUF書き出しを接続し、GPUで400/600ステップを完了。新モデルはAJIMEE184/200と182/200のため不採用。境界候補を最大2件補うソース実験は186/200を維持するが製品へ未採用。[学習原票](verification/accuracy-native-training-20261017.md)。製品パッチと採用モデルは186/200版を保持。次は学習用の実候補を512文・開発128文へ増やし、同じAJIMEE基準で改善・改悪を見る。

最新: ユーザーの指示によりAJIMEE改善へ集中。専用製品・ソースは `codex/ajimee-ranking-20261017`。順位付けのLLM重み1.2、補正は従来0.7へ分離、Standardの生成8/LLM採点4、不正UTF-8生成の除外を採用。既定設定の実converterで184→186/200（93.0%）、改善2・改悪0。前回185/200比は改善1・改悪0。ソース9252fb8f0、パッチ再構成131ファイル一致。[測定記録](verification/accuracy-ajimee-ranking-20261017.md)を参照。導入・公開は未実施。次は残る14問を改善し、診断やベンチマークの拡大を先行させない。

追加比較ではmedium生成は186/200で効果なし、一語辞書候補の追加は180/200（改善0/改悪6）で不採用。[失敗原票と実装](verification/accuracy-local-candidates-20261017.md)を保存。実験枝 `codex/local-homophones-20261017` を保持し、採用済みパッチは変更していない。残る誤答の8/14件は正解候補が採点されても選べないため、次は既存候補の識別学習を優先する。

以下の基準測定・公開情報は記載時点の履歴。

利用者が `AI-NihongoIME/docs/AI_ACCURACY_IMPROVEMENT_GUIDE.md` を開発ゴールへ指定。[カード81](tasks/81-accuracy-guide.md)を `codex/accuracy-guide-20261016` で進める。評価器の文脈衝突を修正し、観測候補包含と内部AI採点の診断・結果SHAつき比較台帳を実装した。[評価仕様](ACCURACY_EVALUATION.md)を参照。基準測定は従来バイナリで行い、新しい製品精度改善とは呼ばない。原票 `target/accuracy-guide/`。

2026-10-17: 採点配分は不採用のまま。審査復旧後、専用Mozcソース5a750cd57からのnative build、C++254件・Python80件、要求IDと診断ON/OFF一致を確認した。全調整層304/326（旧固定比1改善/0改悪）、独立289/300。素のMozcへのパッチ適用で131ファイル一致。[固定ソースと要求ID検証](verification/accuracy-request-trace-20261017.md)を参照。実TSF・MSI・製品完成は未達。学習の失敗と比較上の注意は[移行検証](verification/accuracy-ranking-transfer-20261017.md)に保存した。

既存のリリース準備は別ツリー `.local/accuracy-release`、カード80の実験は `.local/typing-demo/product-worktree` に保持。これらの未コミット変更や未採用モデルをこの枝へ混ぜない。以下のPR/公開版の状態はmainを取得した時点の履歴であり、公開・統合前には再確認する。

[PR #162](https://github.com/aruiki/Asteria-Japanese-IME/pull/162)、codex/accuracy-999-ranking、カード78。辞書候補のLLM採点取りこぼしを修正した。AJIMEE184/200、独立評価289/300で99.9%は未達。重み変更の失敗実験は採用していない。詳細と原票の場所はPRの検証記録を参照する。修正は公開済みMSIに含まれない。

一般CI7件は成功。Windows MSIビルドは確認時点で実行中（run38258134869）。現在の状態はPRで再確認し、成功するまで統合しない。

## 次の作業

1. ソース705の4精度セットは維持を確認。新ソースのMSI・実TSF・導入/削除へ進める。既存6339MSIは407で、初回キーの修正を含まない。GitHubコード/モデル送信の明示許可が必要。モデル専用releaseを前の製品MSIと誤認する公開検査も修正した。既出データの個別誤答を独立評価の調整へ流用しない。初回AI準備と短い生成語の読み検査は、開発用の別データで修正する。
2. 実TSF・未使用評価・CPU/低資源/障害時・配布条件は製品完成の未達条件として保持する。現在の改善作業では診断拡大を先行させない。

## 作業環境と記録の入口

- 製品リポジトリ: Asteria（旧KotoriIME）。既存 `mzsupport` と `mzcontext16` は別作業の所有物として保持する。専用ソースは `C:/Users/aruik/ag16`（fb6de1384、build/test済み）。LLVMはrepo_envで実体パスを指定する。基点bareは `target/accuracy-guide/mozc-base.git`、生成scratchは同targetのpatch-scratch。Bazelは生成前にshutdown済み。
- 規約: [AGENTS.md](../AGENTS.md)。コマンド: [DEVELOPMENT.md](DEVELOPMENT.md)。作業一覧: [tasks/README.md](tasks/README.md)。
- 原票: `target/release-v1.1.3`、`target/accuracy-999`。結果の根拠は各verification文書、設計判断はADRに置く。同じ結果を全書類へ転記しない。
