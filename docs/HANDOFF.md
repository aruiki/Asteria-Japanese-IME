# 現在の引き継ぎ（2026-10-18、通常変換の基礎と新MSI）

この文書は現在の状況と次の作業だけを更新する。過去の指示・測定値は[履歴](HANDOFF_HISTORY_20261012.md)へ保存した。履歴の「現在」「次」は記録当時の意味であり、作業指示として再実行しない。

## 公開版

[Asteria v1.1.3](https://github.com/aruiki/Asteria-Japanese-IME/releases/tag/v1.1.3)を公開済み。公開commitはdc9323ce1b96f20090290fbd7153515353235bcd。MSIは未署名。導入・日本語入力・削除と公開後再取得を検証した。成果物の版・SHA・検証境界は[リリース検証記録](verification/release-v1.1.3-20261012.md)を参照する。

## 開発中

ユーザーの最新指示は「普通の変換に必要なものを欠かさずに開発」。[通常変換の必要条件](NORMAL_CONVERSION.md)で、辞書/頻度/連語/活用/接続/文節/数字・記号・英数/固有名詞/ユーザー辞書/履歴/候補操作/確定と、AI不在・保護入力・実TSFを追跡する。AI準備後の平均精度で初回候補を代表しない。

製品枝は`codex/contextless-collocations-20261018`。専用Mozcは`C:/Users/aruik/ag16`の同名枝、source9e4a9b256c2899fd6a07fbd88bc235e900dcbb6e。[初期連語の実装・検証](verification/accuracy-fast-collocations-20261018.md)で、固定Wikipedia学習区間から8,405件を追加し、元40件を保持。AI/ユーザー辞書の選択を後段の連語補正で上書きしない。実行時モデル/I/O/通信は追加しない。固定公開区間からの取得・生成、CI配置、MSI同梱とSHA/出典検査まで接続し、パッチ再構成137ファイルが実ソースと一致した。生の辞書/コーパスはコミットしていない。

同じ4開発セット×3モード・978出力の比較は、AIなしAJIMEE102→103/200（1改善/改悪0）。他のAIなし出力と、AI準備後の前文なし/あり全652出力は完全一致した。前文ありStandard188/200を維持。小さい基礎候補の改善であり、Google日本語入力との直接比較や初回即Spaceの完成ではない。元の[前文なし基準](verification/accuracy-contextless-baseline-20261018.md)と、[較正/独立評価の境界](verification/accuracy-confidence-calibration-20261018.md)は保持する。

AIなしの実辞書セッション28項目（通常例19/操作9）が成功。活用・助詞・固有名詞・数字/年月日/句読点、候補選択/履歴・文節伸縮/移動・取消/Backspace・F6〜F10・隔離ユーザー辞書を確認。C++は連語/LM/session264件に加え、OSSデータ/数字/日付/ユーザー辞書/履歴165件が成功。mozc/tools Python101件、生成/取得9件も成功。28項目の合格を全語彙・全通常操作・実画面の完成とはしない。

新MSIは`target/accuracy-guide/release-candidate-1.1.4-rc.1-collocations/Asteria64.msi`、SHA e4456158a90b7c735c7eaf94c8a3ba1d7315ea9b98774cde572ac6be94cc0977、3.34.6339.100、未署名・未導入・未公開。管理展開と固定4モデル/連語/出典/サーバーのビルド元一致を確認。OOM修正済みbase e23b0ff...を実同梱し、同じ256MiB process commit上限で対照3/AI失敗時3文の入力継続が成功した。[低メモリ修正の経緯](verification/accuracy-low-memory-fallback-20261018.md)と旧705 MSIの不合格を保持する。物理256MiB実機の証明ではない。

新MSI資産でも「あいであきょく」は初回「アイデア局」/準備後「アイデア曲」が残る。初回の誤変換を合格扱いにしない。[初回キーのDLL待ち解消](verification/accuracy-nonblocking-device-startup-20261018.md)は継承しているが、準備前の候補品質と実TSF/新MSI導入・入力・削除は未達。導入済み6337は変更していない。GitHubコード/モデル送信は過去に自動承認審査が明示許可なしとして拒否したため、人間の明示許可待ちで再試行していない。未公開モデルURLのままmainへ統合しない。

## 次の作業

1. 前文なし・AI準備前の候補品質を改善する。通常変換の辞書/語彙/頻度・候補包含/順位/分節を実出力から分けて調べ、語句の個別分岐や参照変更で合格させない。「アイデア局」の初回課題、既出の改悪1件、未使用評価を分けて保持する。
2. 通常操作の残る範囲（JISかな、再変換/確定取消、未確定/取消の非学習、履歴の再起動保持、保護欄/フォーカス）と[実アプリの確認](ACCEPTANCE.md)を進める。今回の28項目やC++の成功で実TSFを代替しない。
3. 新MSIの導入/入力/削除・署名/配布段階・独立verifierを満たす。GitHubコード/モデル送信の明示許可後にモデル資産公開・取得確認・PR/Windows CIを進める。旧705の低メモリ不合格MSIを再利用しない。

## 作業環境と記録の入口

- 製品リポジトリ: Asteria（旧KotoriIME）。既存 `mzsupport` と `mzcontext16` は別作業の所有物として保持する。専用ソースは `C:/Users/aruik/ag16`（9e4a9b256、build/test/MSI内容照合済み）。LLVMはrepo_envで実体パスを指定する。基点bareは `target/accuracy-guide/mozc-base.git`、生成scratchは同targetのpatch-scratch。Bazelは生成前にshutdown済み。全137ファイルの復元一致を確認。
- 規約: [AGENTS.md](../AGENTS.md)。コマンド: [DEVELOPMENT.md](DEVELOPMENT.md)。作業一覧: [tasks/README.md](tasks/README.md)。
- 原票: `target/release-v1.1.3`、`target/accuracy-999`。結果の根拠は各verification文書、設計判断はADRに置く。同じ結果を全書類へ転記しない。
