# 現在の引き継ぎ（2026-10-17、AJIMEE改善を優先）

この文書は現在の状況と次の作業だけを更新する。過去の指示・測定値は[履歴](HANDOFF_HISTORY_20261012.md)へ保存した。履歴の「現在」「次」は記録当時の意味であり、作業指示として再実行しない。

## 公開版

[Asteria v1.1.3](https://github.com/aruiki/Asteria-Japanese-IME/releases/tag/v1.1.3)を公開済み。公開commitはdc9323ce1b96f20090290fbd7153515353235bcd。MSIは未署名。導入・日本語入力・削除と公開後再取得を検証した。成果物の版・SHA・検証境界は[リリース検証記録](verification/release-v1.1.3-20261012.md)を参照する。

## 開発中

最新の採用候補: 枝 `codex/prefix-normalization-20261017`。既存候補のprefix群だけをトークン数で正規化して比較する変更によりAJIMEE187→188/200（94.0%）、改善1・改悪0。学習モデル非同梱でも186→187/200、改善1・改悪0。追加推論なし。C++62件成功、製品パッチ再構成131ファイル一致。[今回の実装と原票](verification/accuracy-prefix-normalization-20261017.md)。導入済みIME・公開版は未更新。次は残る12問を改善する。

2026-10-18: 新リリース開発へ継続。Qwen3-4Bは173/200、Sarashina3Bは161/200、Qwen3.5-4B-Baseは181/200で単純交換は不採用。読み・前文を明示したQwen3の採点実験も104/200で不採用。既定OFFは188/200・全出力一致。「アイデア局→曲」は同一プロセスのAI準備前後で再現し、mozc/tools/check_startup_quality.pyへ確認手順を保存した。[モデル比較と起動時品質](verification/accuracy-release-model-comparison-20261018.md)。審査上限は解除済み。現製品パッチ・導入済みIMEは変更せず、次は候補選択の方式と起動時の品質差を改善する。

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

1. ソースfb6de1384と既存モデル同梱構成のAJIMEE188/200から残る12問を改善する。正解集合を変えず改善/改悪で判断する。
2. 実TSF・未使用評価・CPU/低資源/障害時・配布条件は製品完成の未達条件として保持する。現在の改善作業では診断拡大を先行させない。

## 作業環境と記録の入口

- 製品リポジトリ: Asteria（旧KotoriIME）。既存 `mzsupport` と `mzcontext16` は別作業の所有物として保持する。専用ソースは `C:/Users/aruik/ag16`（fb6de1384、build/test済み）。LLVMはrepo_envで実体パスを指定する。基点bareは `target/accuracy-guide/mozc-base.git`、生成scratchは同targetのpatch-scratch。Bazelは生成前にshutdown済み。
- 規約: [AGENTS.md](../AGENTS.md)。コマンド: [DEVELOPMENT.md](DEVELOPMENT.md)。作業一覧: [tasks/README.md](tasks/README.md)。
- 原票: `target/release-v1.1.3`、`target/accuracy-999`。結果の根拠は各verification文書、設計判断はADRに置く。同じ結果を全書類へ転記しない。
