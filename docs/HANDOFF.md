# 現在の引き継ぎ（2026-10-19、通常候補・ページ操作の修正）

この文書は現在の状況と次の作業を更新する。過去の作業指示は[履歴](HANDOFF_HISTORY_20261012.md)、測定は個別verificationを参照し、履歴の「現在」を再実行指示にしない。

最新の方針は、固定CI候補をRCとして先に公開し、以後も開発を続けること。PR168は全8CI成功でmain71300へ統合済み。同CIのMSI 3.34.6435.100を取得し、SHA0f12eb1d...、モデル/連語/更新metadata/未署名を実バイトで確認した。[固定候補と自動受入れ](verification/candidate-automatic-acceptance-20261019.md)。追加の入力・取消・再導入はWindows CIで実行する。初回「アイデア局」は未解決としてRCノートへ明記し、AI準備後の正解と混同せず後続開発で修正する。

## 公開記録と開発対象

前回公開記録は[Asteria v1.1.3](verification/release-v1.1.3-20261012.md)、commit dc9323ce1b96f20090290fbd7153515353235bcd、未署名。利用者の「送信してPR・Windows CIへ進む」の明示許可後、開発枝をpushし、追加モデル資産を公開・固定URLからの再取得/SHA照合まで完了した。製品リリースは未公開。導入済み3.34.6337.100も変更していない。

ユーザーの「普通の変換に必要なものを欠かさずに開発」を[必要条件](NORMAL_CONVERSION.md)で追跡する。製品枝はcodex/contextless-usage-collocations-20261018、専用MozcはC:/Users/aruik/ag16の同名枝。最新ソースb5b144bd9ca717d2768dee7d52fa904ec7a4a0f8（449828から追加モデルの公開状態metadataだけを更新）。ローカルMSIのビルド対象は449828のまま。既存mzsupport/mzcontext16の作業は保持する。

## 完了した実装と検証

[用例連語](verification/accuracy-usage-collocations-20261018.md): 元40+固定Wikipedia8,405件を保持し、既存Usage Dictionaryの引用例/活用から26,216件を追加、計34,661件。固定再取得/生成・出典/使用許諾・MSI同梱検査へ接続。AIなし日常65→69/81（4改善/改悪0）、AI準備後652出力は旧版と完全一致。生の辞書/コーパス/モデルはコミットしない。

[通常変換](verification/accuracy-normal-conversion-20261019.md): AI待機中に通常候補を消す処理と、文字種メニュー非表示時のページ先頭移動を修正した。旧実装でのredを保持。C++379件、固定実辞書の通常28項目/保存等7項目、Python111件（更新gateの追加4件を含む）、CLI入力file3対照が成功。保存検査は終了時cleanupに頼らず別PIDの実再起動と非学習/cleanup対照を使う。JISかな、選択文字の再変換、確定取消も対象。

修正後の固定4開発セット×3条件・978出力は用例連語版と全て一致。AI準備後Standardは前文なし175/200・前文あり188/200。CPU/GPUの初回・準備後各30入力、同じ256MiB process commit制限の対照3/AI失敗時3入力も成功。起動/TSF/実低性能機やGoogle版との直接比較をこの結果で代替しない。

Bazel shutdown後に生成パッチを再作成し、基点cloneへ適用した全139ソースが一致。基点bareはtarget/accuracy-guide/mozc-base.git、scratchは同target/patch-scratch。生成patchを手編集しない。source/model/runtime/dataの識別と原票はverificationから辿る。

最新6432候補は`target/accuracy-guide/release-candidate-1.1.4-rc.1-upgrade/Asteria64.msi`、SHA068ed5c56cadd644f45557593b943f2ec2997b838ca21a4df0341e8df2598c28、1,662,558,208bytes、未署名。実管理展開、公開6431との版/製品識別子、8ビルド元ファイル/全30 DLLの一致と、新6432バイナリの通常28/保存等7/CLI3対照が成功。実導入・実入力・削除は未確認。

用例連語の旧MSI ab54f88e...（3.34.6339.100、未署名、未導入・未公開）は同梱検査成功。ただし今回の通常候補/ページ修正を含まない旧ビルド候補で、配布可能にしない。過去のe445/f576、低メモリ不合格の705/cf0d8f...と混同しない。

## 次の作業と残る条件

1. 追加モデル資産model-native-generator-20261017はprerelease/latest=falseで公開済み。4資産のsize/digestと固定URLから再取得したGGUFのSHAが一致。draft PR/Windows CI、同一CI成果物の実導入/入力/削除へ進む。
2. 初回「あいであきょく」→「アイデア局」は未解決。準備後の「アイデア曲」と混同せず、語彙・頻度・候補包含/順位/分節を分けて改善する。文字3gram順位案は日常の回帰で不採用、個別語句の分岐/参照変更で通さない。
3. 新MSIの実導入・実アプリの入力/保護欄/フォーカス・削除、署名/配布条件、独立verifierは未完了。[実アプリ条件](ACCEPTANCE.md)をCLIや展開成功で代替しない。
4. 追加依頼のブランディングはREADME・図版・プレビューへ反映済み。[測定条件](BENCHMARKS.md)と[更新検証](verification/release-preparation-v1.1.4-20261019.md)。旧Kotori88.4%は198問の過去実入力比較、開発版94.0%はStandard・AI準備後・前文ありのオフライン200問。異なる測定条件を混ぜて他IMEより高い精度と主張しない。

規約は[AGENTS.md](../AGENTS.md)、コマンドは[DEVELOPMENT.md](DEVELOPMENT.md)、作業票は[カード81](tasks/81-accuracy-guide.md)。独立FAIL記録をPASSへ書き換えず、.goal-completeを作らない。
