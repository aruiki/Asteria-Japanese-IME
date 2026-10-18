# 現在の引き継ぎ（2026-10-19、通常候補・ページ操作の修正）

この文書は現在の状況と次の作業を更新する。過去の作業指示は[履歴](HANDOFF_HISTORY_20261012.md)、測定は個別verificationを参照し、履歴の「現在」を再実行指示にしない。

## 公開記録と開発対象

前回公開記録は[Asteria v1.1.3](verification/release-v1.1.3-20261012.md)、commit dc9323ce1b96f20090290fbd7153515353235bcd、未署名。今回GitHubを更新していない。導入済み3.34.6337.100も変更していない。

ユーザーの「普通の変換に必要なものを欠かさずに開発」を[必要条件](NORMAL_CONVERSION.md)で追跡する。製品枝はcodex/contextless-usage-collocations-20261018、専用MozcはC:/Users/aruik/ag16の同名枝。最新ソース449828059f1b9b3c0f6d0eb6e66211e23c953500（8439からBUILDを6432へ変更した1ファイルのみ）。既存mzsupport/mzcontext16の作業は保持する。

## 完了した実装と検証

[用例連語](verification/accuracy-usage-collocations-20261018.md): 元40+固定Wikipedia8,405件を保持し、既存Usage Dictionaryの引用例/活用から26,216件を追加、計34,661件。固定再取得/生成・出典/使用許諾・MSI同梱検査へ接続。AIなし日常65→69/81（4改善/改悪0）、AI準備後652出力は旧版と完全一致。生の辞書/コーパス/モデルはコミットしない。

[通常変換](verification/accuracy-normal-conversion-20261019.md): AI待機中に通常候補を消す処理と、文字種メニュー非表示時のページ先頭移動を修正した。旧実装でのredを保持。C++379件、固定実辞書の通常28項目/保存等7項目、Python111件（更新gateの追加4件を含む）、CLI入力file3対照が成功。保存検査は終了時cleanupに頼らず別PIDの実再起動と非学習/cleanup対照を使う。JISかな、選択文字の再変換、確定取消も対象。

修正後の固定4開発セット×3条件・978出力は用例連語版と全て一致。AI準備後Standardは前文なし175/200・前文あり188/200。CPU/GPUの初回・準備後各30入力、同じ256MiB process commit制限の対照3/AI失敗時3入力も成功。起動/TSF/実低性能機やGoogle版との直接比較をこの結果で代替しない。

Bazel shutdown後に生成パッチを再作成し、基点cloneへ適用した全139ソースが一致。基点bareはtarget/accuracy-guide/mozc-base.git、scratchは同target/patch-scratch。生成patchを手編集しない。source/model/runtime/dataの識別と原票はverificationから辿る。

最新6432候補は`target/accuracy-guide/release-candidate-1.1.4-rc.1-upgrade/Asteria64.msi`、SHA068ed5c56cadd644f45557593b943f2ec2997b838ca21a4df0341e8df2598c28、1,662,558,208bytes、未署名。実管理展開、公開6431との版/製品識別子、8ビルド元ファイル/全30 DLLの一致と、新6432バイナリの通常28/保存等7/CLI3対照が成功。実導入・実入力・削除は未確認。

用例連語の旧MSI ab54f88e...（3.34.6339.100、未署名、未導入・未公開）は同梱検査成功。ただし今回の通常候補/ページ修正を含まない旧ビルド候補で、配布可能にしない。過去のe445/f576、低メモリ不合格の705/cf0d8f...と混同しない。

## 次の作業と残る条件

1. ユーザーのリリース質問後、コードと追加モデル約99MBのGitHub送信・PR/Windows CIについて明示許可を質問済み。回答待ちの間は送信しない。許可後にモデル資産の公開/固定SHAでの再取得、PR/Windows CI、同一CI成果物の実導入/入力/削除へ進む。
2. 初回「あいであきょく」→「アイデア局」は未解決。準備後の「アイデア曲」と混同せず、語彙・頻度・候補包含/順位/分節を分けて改善する。文字3gram順位案は日常の回帰で不採用、個別語句の分岐/参照変更で通さない。
3. 新MSIの実導入・実アプリの入力/保護欄/フォーカス・削除、署名/配布条件、独立verifierは未完了。[実アプリ条件](ACCEPTANCE.md)をCLIや展開成功で代替しない。
4. GitHubコード/モデル送信は自動承認審査が人間の明示許可不足として拒否した。許可なしで再試行しない。明示許可後にモデル資産の公開/再取得、PR/Windows CIを行い、未公開モデルURLのままmainへ統合しない。

規約は[AGENTS.md](../AGENTS.md)、コマンドは[DEVELOPMENT.md](DEVELOPMENT.md)、作業票は[カード81](tasks/81-accuracy-guide.md)。独立FAIL記録をPASSへ書き換えず、.goal-completeを作らない。
