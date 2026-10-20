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

## 開発中の採点済み代替候補保持

公開RC3/MSI6439には含めない。Mozcの別枝codex/scored-alternative-candidates-20261020で開発中。source8d395f464は、有限スコアの代替文を最大8件、読みと既存文節keyへ対応する場合だけ保持する。第一候補・ユーザー辞書先頭を保ち、新規代替はNO_LEARNINGとする。

- 旧反映処理で回帰2件が失敗し、修正後LM74件が成功。固定v1 EXEと同じ公開6438モデル/DLLで4セット×3条件を前後比較し、全978の第一候補が一致。AJIMEE候補包含は前文なし189→193/200、前文あり193→195/200、包含改悪0。
- 既知2例は旧SessionHandlerで候補欠落、修正後は実候補ID経由の選択・表示・確定まで成功。第一候補精度の改善とは報告しない。
- 予測29例の再比較で、700msの期限直前に採点が完了しても全破棄される境界差を発見。時間予算や採点閾値を変えず、有限採点済み・読み整合済みだけを保持する修正を追加。弱い続きは選択候補へ残し、先頭は既存の確信度条件で保護する。LM75件成功、予測の再比較中。初期の予測改悪原票は残し、採用済みとしない。
- 原票はtarget/accuracy-guide/scored-alternatives-*。実TSFや新MSIの検査ではない。Rの既存NOTICE差分と未追跡モデル/連語、固定したv1/v2/v3 EXEを保持する。

## 次の具体作業

1. 現行の予測再比較と最終ソースの識別を確認する。改悪が残れば原因を直し、成功に読み替えない。
2. 最終ソースで通常変換・AI待機/Tab/後続入力・最終評価・関連Session/Engineを確認し、製品パッチ再生成と全source照合へ進む。過去EXEの成功を新EXEへ移さない。
3. 全適用CIと同じ新MSIの受入れが成功した後に統合・次版公開・再取得照合まで進める。公開/CI許可は取得済み。

[通常変換の条件](NORMAL_CONVERSION.md)、[精度改善カード81](tasks/81-accuracy-guide.md)、[測定値の紹介条件](BENCHMARKS.md)。規約は[AGENTS.md](../AGENTS.md)、コマンドは[DEVELOPMENT.md](DEVELOPMENT.md)。
