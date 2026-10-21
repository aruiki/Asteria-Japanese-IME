# 現在の引き継ぎ（2026-10-20、RC3公開・採点済み候補保持の開発）

現在の指示と次の作業を記す。過去の経緯はgit履歴と各verificationを参照する。

[v1.1.4-rc.3を公開済み](https://github.com/aruiki/Asteria-Japanese-IME/releases/tag/v1.1.4-rc.3)。確定待ちの後続入力へ、直前の確定本文と既知の周囲文を渡す。公開MSI・SHA・配布情報・ソース情報を取得し直し、全バイト・同梱内容・アイコンが一致した。[公開記録](verification/release-v1.1.4-rc.3-20261020.md)。

## 公開版のソースとMSI

PR174は全8チェック成功後、main 5b60b68b1d6b6ce469896e719774f954774d9085へmerge commitで統合。製品head 0b98251c4842bfc574190f7cac5dfe922e2e1dab、CI checkout e1d07706df3b9ab679fef76b94cd82ee7789cfbf、統合commitのtreeは同一。Mozc sourceはee3fa5f45824c6ade7573de704e23a9488acadbb。

[Windows run39249434607](https://github.com/aruiki/Asteria-Japanese-IME/actions/runs/39249434607)で同じ新MSIのbuild・C++・Qt画面・同梱検査・候補操作8項目・導入/再導入/日本語入力/削除がすべて成功。ProductVersion 3.34.6439.100、SHA-256 606c5dc07bdf0c53b031f13ad91c4bd1545b280b4ec276146379157339c90045。導入server/renderer/TIPと管理展開した実バイトも一致した。

RC3はprerelease=true/Latest=false。公開時点のLatestは既存RC2で、その区分は変更していない。AuthenticodeはNotSigned、配布情報に実測値と受入れ範囲を保存した。削除3010を再起動確認にしない。

## 検証の範囲

GPUのAIの最初の生成・採点結果を待ってSpace・入力中候補・Tabを表示し、入力/編集/取消、保留Enterと後続打鍵を処理する。[ADR0067](adr/0067-ai-first-candidate-publication.md)。RC3の[文脈引継ぎ](verification/queued-context-20261019.md)は末尾確定が対象で、元実装の6assert失敗と修正後Session197件成功を保持する。実GPUの旧/新2文比較では確定文は一致し、後続文7候補の採点が変わった。全体の精度向上の数値ではない。

変換精度の既存の固定評価はStandard AJIMEE前文あり188/200、なし175/200。開発978出力・予測29件・最終評価290/300の[RC2までの固定検証](verification/ai-first-publication-20261019.md)を、新たなRC3/MSI測定とはしない。今回のconverter/model/閾値は変えていない。

実導入の受入れはWinForms EDIT/RichEdit限定。実TSFのGPU AI ON/OFF寄与、複数アプリ、低性能実機、保護欄内部のAI/学習、旧版からの更新/再起動、独立verifierは残る。goalはactive、製品完成や.goal-completeにしない。

## 次版の採点済み候補保持

[実装と固定検証](verification/scored-alternatives-20261020.md)。公開RC3/MSI6439には含めない。Mozc source039a7251fa16a77a91cb67f5158e20073a3f3762を製品パッチへ反映し、3patchの再適用と140 sourceの正規化バイト一致を確認した。

- 採点済みの別仮説を最大8件、読みと既存文節keyが一致する場合だけ保持する。第一候補・ユーザー辞書を保ち、新規代替はNO_LEARNING。元実装の回帰2件の失敗を保持し、最終ソースではC++445件と工具2件が成功。
- 同じ6438のモデル/DLL・辞書・データ・評価器で旧/新を前後比較。全978の第一候補一致、AJIMEEは前文あり188/200・なし175/200を維持。候補包含は前文なし189→193/200、あり193→195/200。最終評価300例も290/300、全出力一致。
- 固定SessionHandlerと実GPUで、欠落していた2候補の選択・表示・確定、AI待機/編集/取消/保留Enter/後続入力、Tab、文脈引継ぎ、通常変換/保存を確認した。実TSFの検査ではない。
- 予測の700ms予算と閾値は変えず、期限直前に有限の採点が済んだ候補を保持する。未採点・読み不足を拒否し、弱い続きは先頭本文を置き換えず、期限後の部分集合は通常cacheへ保存しない。初期案の予測改悪ログを保持し、修正後29例は先頭一致12→12、上位3一致19→20、改悪0、確定差0。速度改善は主張しない。
- 新MSIはまだ未検証。新しいWindows runと同じMSIの候補8項目・再導入/入力/削除を確認してから統合・次版公開へ進む。

## 次の具体作業

1. この変更の全適用CIと新Windows MSIを確認する。旧RC3の8項目成功を次MSIへ流用しない。
2. 成功後にMSIの実SHA/版・同梱物・導入binary・CI source/treeを照合し、ADR0044の条件を満たして統合・次版公開・再取得照合まで進める。
3. 実アプリGPU ON/OFF寄与・複数アプリ・低性能・内部privacy・旧版更新/再起動・独立verifierは引き続き残る。公開/CI許可は取得済み。

[通常変換の条件](NORMAL_CONVERSION.md)、[精度改善カード81](tasks/81-accuracy-guide.md)、[測定値の紹介条件](BENCHMARKS.md)。規約は[AGENTS.md](../AGENTS.md)、コマンドは[DEVELOPMENT.md](DEVELOPMENT.md)。
