# 現在の引き継ぎ（2026-10-12）

この文書は現在の状況と次の作業だけを更新する。過去の指示・測定値は[履歴](HANDOFF_HISTORY_20261012.md)へ保存した。履歴の「現在」「次」は記録当時の意味であり、作業指示として再実行しない。

## 公開版

[Asteria v1.1.3](https://github.com/aruiki/Asteria-Japanese-IME/releases/tag/v1.1.3)を公開済み。公開commitはdc9323ce1b96f20090290fbd7153515353235bcd。MSIは未署名。導入・日本語入力・削除と公開後再取得を検証した。成果物の版・SHA・検証境界は[リリース検証記録](verification/release-v1.1.3-20261012.md)を参照する。

## 開発中

[PR #162](https://github.com/aruiki/Asteria-Japanese-IME/pull/162)、codex/accuracy-999-ranking、カード78。辞書候補のLLM採点取りこぼしを修正した。AJIMEE184/200、独立評価289/300で99.9%は未達。重み変更の失敗実験は採用していない。詳細と原票の場所はPRの検証記録を参照する。修正は公開済みMSIに含まれない。

一般CI7件は成功。Windows MSIビルドは確認時点で実行中（run38258134869）。現在の状態はPRで再確認し、成功するまで統合しない。

## 次の作業

1. PR #162のWindows CI結果を確認し、失敗があればログから修正する。成功後にmainへ統合し、カード78をdoneへ移す。
2. 誤入力評価40問の残る24不一致を再現し、未知語・補正・意味選択の原因を分けて修正する。独立評価の問題を調整へ流用しない。
3. 99.9%を判定できる独立評価の規模と範囲を用意する。CLI評価と実アプリTSF受入れは別々に記録する。

## 作業環境と記録の入口

- 製品リポジトリ: KotoriIME。Mozc作業ツリー: `C:/Users/aruik/mzsupport`。ビルド・パッチ生成を同時実行しない。
- 規約: [AGENTS.md](../AGENTS.md)。コマンド: [DEVELOPMENT.md](DEVELOPMENT.md)。作業一覧: [tasks/README.md](tasks/README.md)。
- 原票: `target/release-v1.1.3`、`target/accuracy-999`。結果の根拠は各verification文書、設計判断はADRに置く。同じ結果を全書類へ転記しない。
