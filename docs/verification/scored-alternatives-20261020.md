# 採点済み候補を一覧と確定まで保持する（2026-10-20）

## 問題と変更

LmRewriterは文全体を生成・採点しても、Bestへ最良の1文だけを保存していた。採点済みの正解が別仮説にあっても、最終候補一覧から欠落する場合があった。最良の文と同じ読み・同じ文節keyへ対応する別仮説を、有限スコア順に最大8件保持する。文節境界をまたぐ対応、欠落、不正UTF8、重複を拒否し、第一候補とユーザー辞書先頭は維持する。新規代替はNO_LEARNINGとし、補正読みを別候補へ継承しない。

予測では、700msの予算直前にモデル採点が終わっても、終了後の時刻だけで結果を全破棄する境界を修正した。有限採点済み・現在の読みと整合する候補だけを保持する。先頭に残す元の候補も同じ要求で有限のLLM採点が必要。弱い続きは選べる候補へ残し、強い先頭の条件は既存のcan_lead/yomiを再利用する。期限後の部分集合を通常の予測cacheへ保存しない。時間予算・モデル・重み・閾値・正解集合は変更していない。

Mozc sourceはee3fa5f45824c6ade7573de704e23a9488acadbbから、8d395f464を経て039a7251fa16a77a91cb67f5158e20073a3f3762。変更はlm_rewriter.cc/.h/_test.cc。既存の入力世代・文脈・設定・モデル・保護欄の配送/補正検査は維持する。

## 回帰と固定バイナリ

旧ApplyBestFromのまま新回帰2件を実行して失敗を確認した。最終ソースではLM75・Session197・EngineConverter92・Scenario81の計445件が成功。source039aと凍結時差分、3ファイルのSHAを照合した。

- SessionHandler SHA-256: `092e58ed41cac3b4fead4394bc98620d72483b78282952c4fa95376b45b93305`
- converter SHA-256: `14284c10d8ef4193eb95dc37e052ce67de4b0457ac223ecdccab0adc3f2be4d2`
- 最終原票: `target/accuracy-guide/scored-alternatives-final-v1/`
- 凍結EXE/runfiles: `target/accuracy-guide/scored-alternatives-native-v4/`

初期案のコンパイル失敗と予測の改悪も保持している。v1/v2の予測は上位3候補に改悪が残り、v3/v4は先頭の1例が悪化したため採用していない。最終の予測比較v5は同じ旧ee3/新039a、同じ公開6438のモデル/DLL、同じ29例で先頭一致12→12、上位3一致19→20、改悪0、表示と確定の差0。旧版側にも時間上限付近の揺れがあり、普遍的な予測精度向上とはしない。

## 同じ条件の変換比較

旧8afの固定converterと新039aを、同じ公開MSI6438の4モデル/DLL・辞書・データ・評価器で順に実行した。RTX3060を使用し、実GPU/LLM採点のtime記録も確認。各組の辞書/モデル/runtime/データ/評価器SHAはcomparability-audit.jsonですべて一致した。開発4セット×3条件の全978出力で第一候補が一致し、第一候補精度の改善/改悪は0。

| AJIMEE200例 | 第一候補正解数 旧→新 | 正解候補包含 旧→新 |
| --- | --- | --- |
| AIなし・前文なし | 103→103 | 180→180 |
| Standard・前文なし | 175→175 | 189→193 |
| Standard・前文あり | 188→188 | 193→195 |

前文なしの包含改善はindex118/1329/2032/2340、包含改悪0。文脈は評価データが指定したもの。通常利用で常に同じ前文を得られるという意味ではない。

調整に使わない固定最終評価300例も、同じモデル/DLLで旧290→新290正解、全300の第一候補が一致した。測定はscored-alternatives-quality-v2/とfinal-v1/heldout-*へ保存。データの正解集合を変えて点数を上げていない。

## 実モデルの選択・確定と入力継続

旧SessionHandlerではindex1329/2032の正解候補がなく、両方の選択プローブが失敗した。新しい固定EXEでは対象文節へ移動し、実候補ID経由の選択・表示・Enter確定が両方成功。第一候補は従来どおりで、正解候補へ到達できる改善である。

同じ最終EXEでAI待機中の候補非公開・編集/取消・保留Enter/後続打鍵、Tab、文脈引継ぎ、通常変換の基礎と履歴/非学習/保存を確認した。候補選択工具はEXE/source identity・ケースを必須とし、開始前にSHA不一致を拒否する。GPUを要求した検査では実process statusのRTX3060と候補採点ログが必要。検証中にEXE・モデル・DLL・データ・工具が変われば失敗する。空ケース・不正index/制御文字・EXE不一致を拒否する2試験も成功。

再現工具はmozc/tools/scored_alternatives_acceptance.py。`--exe --identity --cases --install --proto --romanji-table --out`を指定し、`--require-device "NVIDIA GeForce RTX 3060"`で機器を検査する。identity JSONはsource_commit（完全SHA）とsession_handler_sha256、cases JSONはindex/reading/alternative/expectedの非空文字列を持つ配列。最終検査の入力と全ファイルSHAはfinal-v1/source-identity.jsonおよびcandidate-selection/summary.jsonへ固定した。

個別キー/callbackは実測最大約37ms、予測29例の結果到着最大は旧約914ms/新約1016ms。キー応答とAI到着までの時間を混同しない。これらの少数観測から速度改善を主張しない。

## 製品パッチと残る条件

Bazel停止後にmake_patches.shで再生成し、3patchの再適用と140 sourceの正規化バイト一致を確認した。0002 SHA-256は`586e5c0981be409398c1a7d128db175aa29f417b1c38a6c83e23345aa16a6e14`。0001/0003・モデル・閾値は既存と同一。原票はscored-alternatives-patch-generation-v1.logとpatch-receipt-v1.json。

これは固定したローカルEXEのオフライン/隔離Session検証である。公開RC3/MSI6439へ新挙動が入ったとはしない。新しいWindows MSIと全CI、同じ新MSIの実候補8項目・再導入/入力/削除が成功してから統合・配布へ進む。

実アプリのGPU AI ON/OFF寄与、複数アプリ、低性能実機、保護欄内部のAI/学習、旧版更新/再起動、独立verifierは未完了。製品全体の完成や.goal-completeの判定ではない。
