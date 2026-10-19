# 現在の引き継ぎ（2026-10-19、GPU AIを待つ候補表示）

この文書は現在の状況と次の作業を更新する。過去の作業指示は[履歴](HANDOFF_HISTORY_20261012.md)、測定は個別verificationを参照し、履歴の「現在」を再実行指示にしない。

v1.1.4-rc.1を公開済み。次版は利用者の明示指示によりGPUのAIを主体とし、最初の結果を待って候補を表示する。[ADR0067](adr/0067-ai-first-candidate-publication.md)。通常Spaceと入力中候補の非同期待機を実装し、[固定バイナリの前後比較](verification/ai-first-publication-20261019.md)で旧版失敗・新版成功、C++365件、品質978出力一致を確認。製品/Mozc枝はcodex/gpu-ai-first-candidates-20261019、Mozc source bba6fb37e。新MSI・Windows CIとTabの明示予測経路は継続する。以下の公開RCの検証は新バイナリの検証とは区別する。

## 公開記録と開発対象

最新RCは[v1.1.4-rc.1](https://github.com/aruiki/Asteria-Japanese-IME/releases/tag/v1.1.4-rc.1)、tag71300bff41d14b81a9c2f5d12830bfeeb7670fe0、ProductVersion3.34.6435.100、SHA0f12eb1db8726e7a3c49186626098a100c64c36ffd9cab97c9dbe526ffcdf57a。Latestは正式版v1.1.3のまま。利用者の「送信してPR・Windows CIへ進む」の明示許可後、追加モデル資産の公開・固定URL再取得/SHA照合も完了。利用者からRC導入済みの報告がある。今回の開発試験は隔離セッションで行い、普段の導入済みバイナリの版は再確認していない。

ユーザーの「普通の変換に必要なものを欠かさずに開発」を[必要条件](NORMAL_CONVERSION.md)で追跡する。製品枝はcodex/contextless-usage-collocations-20261018、専用MozcはC:/Users/aruik/ag16の同名枝。最新ソースb5b144bd9ca717d2768dee7d52fa904ec7a4a0f8（449828から追加モデルの公開状態metadataだけを更新）。ローカルMSIのビルド対象は449828のまま。既存mzsupport/mzcontext16の作業は保持する。

## 完了した実装と検証

[用例連語](verification/accuracy-usage-collocations-20261018.md): 元40+固定Wikipedia8,405件を保持し、既存Usage Dictionaryの引用例/活用から26,216件を追加、計34,661件。固定再取得/生成・出典/使用許諾・MSI同梱検査へ接続。AIなし日常65→69/81（4改善/改悪0）、AI準備後652出力は旧版と完全一致。生の辞書/コーパス/モデルはコミットしない。

[通常変換](verification/accuracy-normal-conversion-20261019.md): AI待機中に通常候補を消す処理と、文字種メニュー非表示時のページ先頭移動を修正した。旧実装でのredを保持。C++379件、固定実辞書の通常28項目/保存等7項目、Python111件（更新gateの追加4件を含む）、CLI入力file3対照が成功。保存検査は終了時cleanupに頼らず別PIDの実再起動と非学習/cleanup対照を使う。JISかな、選択文字の再変換、確定取消も対象。

修正後の固定4開発セット×3条件・978出力は用例連語版と全て一致。AI準備後Standardは前文なし175/200・前文あり188/200。CPU/GPUの初回・準備後各30入力、同じ256MiB process commit制限の対照3/AI失敗時3入力も成功。起動/TSF/実低性能機やGoogle版との直接比較をこの結果で代替しない。

Bazel shutdown後に生成パッチを再作成し、基点cloneへ適用した全139ソースが一致。基点bareはtarget/accuracy-guide/mozc-base.git、scratchは同target/patch-scratch。生成patchを手編集しない。source/model/runtime/dataの識別と原票はverificationから辿る。

ローカル比較用6432は`target/accuracy-guide/release-candidate-1.1.4-rc.1-upgrade/Asteria64.msi`、SHA068ed5c56cadd644f45557593b943f2ec2997b838ca21a4df0341e8df2598c28。公開6435とは別。実管理展開・ファイル一致・通常28/保存等7/CLI3対照の原票を保持する。

公開6435はPR168の全8CI成功・main71300のtree一致を確認した同一MSI。追加自動受入れPR169も全9チェック成功、mainc5473b2へ統合済み。Windows run39183067546でEDIT/RichEditの8項目と同一MSI再導入・削除が成功。[固定候補と自動受入れ](verification/candidate-automatic-acceptance-20261019.md)。

用例連語の旧MSI ab54f88e...（3.34.6339.100、未署名、未導入・未公開）は同梱検査成功。ただし今回の通常候補/ページ修正を含まない旧ビルド候補で、配布可能にしない。過去のe445/f576、低メモリ不合格の705/cf0d8f...と混同しない。

## 次の作業と残る条件

1. RC公開・自動入力確認・公開ファイル再取得照合は完了。配布情報・SHAをReleaseへ添付済み。次版の変更は新しいソース・バイナリで検証する。
2. 新しい通常Space経路はAIの初回結果を待ち、GPU指定の隔離試験で「アイデア曲」を最初から確定できた。新MSI・実TSFでは継続確認する。CPU指定の参照不一致は残る。文字3gram順位案や個別語句の分岐で通した変更ではない。
3. EDIT/RichEditの実入力・取消・欄移動・保護欄の見える挙動・同一MSI再導入・削除はCIで成功。AI寄与・保護欄内部のAI/学習・複数アプリ・低性能実機・旧版更新/再起動復旧・独立verifierは残る。[実アプリ条件](ACCEPTANCE.md)の全完了とはしない。
4. 追加依頼のブランディングはREADME・図版・プレビューへ反映済み。[測定条件](BENCHMARKS.md)と[更新検証](verification/release-preparation-v1.1.4-20261019.md)。旧Kotori88.4%は198問の過去実入力比較、開発版94.0%はStandard・AI準備後・前文ありのオフライン200問。異なる測定条件を混ぜて他IMEより高い精度と主張しない。

規約は[AGENTS.md](../AGENTS.md)、コマンドは[DEVELOPMENT.md](DEVELOPMENT.md)、作業票は[カード81](tasks/81-accuracy-guide.md)。独立FAIL記録をPASSへ書き換えず、.goal-completeを作らない。
