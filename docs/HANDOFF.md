# 現在の引き継ぎ（2026-10-19、RC2公開完了・後続入力の文脈改善）

現在の指示と次の作業を記す。過去の経緯はgit履歴と各verificationを参照する。

[v1.1.4-rc.2を公開済み](https://github.com/aruiki/Asteria-Japanese-IME/releases/tag/v1.1.4-rc.2)。公開MSI・SHA・配布情報・ソース検証情報を取得し直し、同梱検査とアイコン資源まで一致を確認した。[公開記録](verification/release-v1.1.4-rc.2-20261019.md)。

## 公開版のソースとMSI

利用者の指示どおり、GPUのAIの最初の結果を待ってSpace・入力中候補・Tabの候補を表示する。[ADR0067](adr/0067-ai-first-candidate-publication.md)。入力・編集・取消は続けられ、保留Enterと後続打鍵を順に処理する。

PR171は全9チェック成功後、main `039886e8f5377318512fdc8bff8750e7f13c906e`へmerge commitで統合済み。製品head `76aa7e32f9cebac5eafdbacff6c1738f556a7853`、CI checkout `16c3a3ea62ba7dfe4a9b5a82030240d48938a5f8`、統合commitのtreeは同一。Mozc sourceは`8af4797f92689474635dd49cf8163cef7aedc198`。

[Windows run39213557813](https://github.com/aruiki/Asteria-Japanese-IME/actions/runs/39213557813)でMSIのbuild・同梱検査・C++テスト・Qt画面検査・実導入・日本語入力・削除が成功。ProductVersion `3.34.6438.100`、SHA-256 `c21972edab537e1ea1473f63312f188b5b533baa700aa77fa9c3111364818c32`。[新MSIの出所と追加受入れ](verification/ai-first-installer-20261019.md)。

## 検証の範囲

[固定バイナリの前後比較](verification/ai-first-publication-20261019.md): C++534件・Python119件、通常28項目・保存等7項目成功。開発4セット×3条件978出力と予測29件の先頭表示が従来と一致。Standard AJIMEEは前文あり188/200、なし175/200を維持。最終評価300例も290/300、旧6432候補と全出力一致。ただし旧版とのruntime DLL差があり、同一DLLの性能比較ではない。

実GPUの隔離SessionHandlerでSpace・Tab・保留Enter・後続入力・編集/取消を確認した。これは実TSFでのAI寄与の証明ではない。冷起動Tabの語義選択の参照不一致は検証記録に保持し、解消したとしない。新MSIの基本実入力は「日本語」「東京」の2件。さらにrun39225823304で追加8項目・同一MSI再導入・入力・削除が全成功し、PR172も全8チェック成功後main3da3672へ統合した。

## 開発中の文脈引継ぎ

[修正と検証](verification/queued-context-20261019.md)。確定待ち中の後続打鍵を再生するとき、既知の周囲文へ確定本文を追加し、次の合成へ渡す。元の前文が空へ戻る・古いままになる経路を修正した。Mozc source `ee3fa5f45824c6ade7573de704e23a9488acadbb`、末尾確定が対象。公開RC2の8af/MSI6438には含めていない。

- 元実装での6assert失敗を保持し、修正後はC++ Session全197件が成功。新回帰は前文あり/明示空と後続Contextなし/全項目/部分の6組で実際のConversionRequestを検査する。
- 固定SessionHandlerと公開6438のモデル/DLL、RTX3060でTab等52イベントと、明示的前後文付き32イベントを確認。前後比較は同じモデル/DLL/読み表を使い、2つの確定文は一致。共通17候補のうち後続文7候補の採点が変化した。全体の精度や速度向上の数値ではない。
- `make_patches.sh`で生成、3patch適用・140 source一致。0002 SHA `fd8dff08c908cfb199686d081ecfb1e44606f5ee9318bc76e63ef66405aa88ff`。0001/0003・モデル・閾値は変更していない。
- 実モデルの確認は`mozc/tools/queued_context_acceptance.py`で再現する。固定EXEのSHAとsource identityを照合し、候補採点の実ログも必要とする。端末プロセスが起動しただけの成功にはしない。

## 次の具体作業

1. 新headの全適用CI成功と新MSIのSHA/版を確認する。Windows buildの同じrunで`installer_smoke.py --acceptance --reinstall`を実行し、新MSIの候補8項目・再導入・入力・削除を検証するよう更新した。旧6438を固定したCandidate workflowの結果を新MSIへ流用しない。
2. 新Windowsの結果を確認してから統合・配布する。追加検証を同じbuild runへ統合したため、成功した場合はMSI受入れのためだけの固定対象更新PRは不要。実際の失敗を無視したり条件を省略したりしない。
3. 利用者の公開・CI許可は取得済み。個別語句や署名の説明を逐一リリースノートへ列記しない指示を守る。


実アプリのAI ON/OFF寄与、複数アプリ・低性能実機・旧版更新/再起動・保護欄内部のAI/学習・独立verifierは残る。再導入の3010を再起動確認にしない。goalはactive、製品完成や`.goal-complete`にしない。

[通常変換の必要条件](NORMAL_CONVERSION.md)、[精度改善カード81](tasks/81-accuracy-guide.md)、[紹介情報の条件](BENCHMARKS.md)、[RC2の公開記録](verification/release-v1.1.4-rc.2-20261019.md)。規約は[AGENTS.md](../AGENTS.md)、コマンドは[DEVELOPMENT.md](DEVELOPMENT.md)。
