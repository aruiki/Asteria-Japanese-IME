# 現在の引き継ぎ（2026-10-16、精度指南書対応）

この文書は現在の状況と次の作業だけを更新する。過去の指示・測定値は[履歴](HANDOFF_HISTORY_20261012.md)へ保存した。履歴の「現在」「次」は記録当時の意味であり、作業指示として再実行しない。

## 公開版

[Asteria v1.1.3](https://github.com/aruiki/Asteria-Japanese-IME/releases/tag/v1.1.3)を公開済み。公開commitはdc9323ce1b96f20090290fbd7153515353235bcd。MSIは未署名。導入・日本語入力・削除と公開後再取得を検証した。成果物の版・SHA・検証境界は[リリース検証記録](verification/release-v1.1.3-20261012.md)を参照する。

## 開発中

利用者が `AI-NihongoIME/docs/AI_ACCURACY_IMPROVEMENT_GUIDE.md` を開発ゴールへ指定。[カード81](tasks/81-accuracy-guide.md)を `codex/accuracy-guide-20261016` で進める。評価器の文脈衝突を修正し、観測候補包含と内部AI採点の診断・結果SHAつき比較台帳を実装した。[評価仕様](ACCURACY_EVALUATION.md)を参照。基準測定は従来バイナリで行い、新しい製品精度改善とは呼ばない。原票 `target/accuracy-guide/`。

2026-10-17: 採点配分は不採用のまま。審査復旧後、専用Mozcソース5a750cd57からのnative build、C++254件・Python80件、要求IDと診断ON/OFF一致を確認した。全調整層304/326（旧固定比1改善/0改悪）、独立289/300。素のMozcへのパッチ適用で131ファイル一致。[固定ソースと要求ID検証](verification/accuracy-request-trace-20261017.md)を参照。実TSF・MSI・製品完成は未達。学習の失敗と比較上の注意は[移行検証](verification/accuracy-ranking-transfer-20261017.md)に保存した。

既存のリリース準備は別ツリー `.local/accuracy-release`、カード80の実験は `.local/typing-demo/product-worktree` に保持。これらの未コミット変更や未採用モデルをこの枝へ混ぜない。以下のPR/公開版の状態はmainを取得した時点の履歴であり、公開・統合前には再確認する。

[PR #162](https://github.com/aruiki/Asteria-Japanese-IME/pull/162)、codex/accuracy-999-ranking、カード78。辞書候補のLLM採点取りこぼしを修正した。AJIMEE184/200、独立評価289/300で99.9%は未達。重み変更の失敗実験は採用していない。詳細と原票の場所はPRの検証記録を参照する。修正は公開済みMSIに含まれない。

一般CI7件は成功。Windows MSIビルドは確認時点で実行中（run38258134869）。現在の状態はPRで再確認し、成功するまで統合しない。

## 次の作業

1. 固定基準5a750cd57と要求ID原票を使い、残る内部候補の欠落と順位誤りへ中核変更を比較する。カード80の探索・モデル実験の失敗を踏まえ、配分だけの変更を繰り返さない。
2. 実TSFの入力世代→候補適用→表示→Space→確定の追跡と、入力同期処理・非同期AIの境界を確認する。評価CLIのIDで代用しない。
3. 新しい改善候補はStandard AJIMEE90%以上と全調整層、採用後の未使用評価、CPU/低資源/初回/即時Space/障害時・保護フィールドを確認する。

## 作業環境と記録の入口

- 製品リポジトリ: Asteria（旧KotoriIME）。既存 `mzsupport` と `mzcontext16` は別作業の所有物として保持する。専用ソースは `C:/Users/aruik/ag16`（5a750cd57、build/test済み）。LLVMはrepo_envで実体パスを指定する。基点bareは `target/accuracy-guide/mozc-base.git`、生成scratchは同targetのpatch-scratch。Bazelは生成前にshutdown済み。
- 規約: [AGENTS.md](../AGENTS.md)。コマンド: [DEVELOPMENT.md](DEVELOPMENT.md)。作業一覧: [tasks/README.md](tasks/README.md)。
- 原票: `target/release-v1.1.3`、`target/accuracy-999`。結果の根拠は各verification文書、設計判断はADRに置く。同じ結果を全書類へ転記しない。
