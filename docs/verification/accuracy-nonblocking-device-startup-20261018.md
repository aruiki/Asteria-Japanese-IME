# 初回キーのruntime初期化待ちを除く（2026-10-18）

初回の`Touch`→`HasGpu`が`EnsureLlamaRuntime`を呼び、DLL/backendのstatic初期化を同期で待っていた。通常入力ではワーカーが判定したGPUの有無をatomicで読むように変更し、モデルロード前のワーカーで実デバイスを判定する。未判定の入力は待たずに既存のMozc結果を返す。

評価専用の`KOTORI_LM_PRELOAD=0`は従来の同期判定を維持。順位付け・モデル・係数・生成幅・保護語/読み/候補の検査を変更していない。初回のAI未準備による誤変換が解消したとはしない。

## 固定ソースとビルド

- Mozc source `705e64361c61708a68716954bc8969046cb842b4`、専用枝`codex/release-quality-calibration-20261018`。
- `bazelisk build //converter:converter_main //session:session_handler_main --config release_build --repo_env=BAZEL_LLVM=C:/Users/aruik/mz/src/third_party/llvm`成功214.965秒。LM66・診断5・session190、計261件の実XMLも成功、test全体100.441秒。ビルド後、正しいag16/srcからBazel shutdown。
- converter SHA `be82fdad2ecb92ce9ededefcbd930f1081f8473263d82d37cdefff40c39bb193`、session SHA `02c71e455cbf0f1514de12f109e327485a977f0a4087f2e24308dbdc7449d661`。辞書SHAは既存`720d0cb42651fe35692578ea5f3c752940b8357fc24efc46230c904d2f5d5d70`と一致。SessionHandlerの辞書はexeへ埋め込み。
- 実バイナリ、辞書/runfiles、テストXMLを`target/accuracy-guide/nonblocking-device-candidate-v1/`へ固定コピー。runfiles manifestもコピー先を指し、可変のBazel出力へ戻らない。Bazelと測定を並列実行していない。
- make_patches.shから再生成して0001〜0003の適用成功。ビルド元との実ソース132ファイル不一致0、適用後のwhitespace検査成功。`nonblocking-device-source-reconstruction.json`。0002 SHA `959f92d66d62ad94332e9ec0621d8ac9f0bdc00d5d0fc71ed54fc23c360f550a`、0003は既存`948746771343aadc20a7f7a5eb32cd8d94655877a4b694113697194b4d56ddde`のまま。

## 実SessionHandlerでの前後比較

旧407a449f5/新705e64361をCPU/GPUそれぞれ3組、順番を交互にして実行。各実行は準備なし/準備後で別PID・別新規profile、各15件を1打鍵ずつ送って即Space/Enter。元の同じ日常15文、同じ4モデル/DLL/生成pb2を固定し、開始と終了のSHAを照合した。AIのCPUスレッド数2、キー間待機0秒。モデルや順位の環境変数上書きなし。

Ryzen7 5700X、RTX3060。CPU側は製品自動Low/zenz-only、GPU側は実際にllm状態/非CPUデバイスであることを同じPIDの状態から確認した。単にzenzが起動しただけでGPU Standard成功とはしていない。開始前のSHA計算でファイルを読むためOSキャッシュcoldではなく、低性能/低メモリ実機でもない。

| 条件 | 旧: 最初のキー3回 | 新: 最初のキー3回 | 中央値 旧→新 |
| --- | --- | --- | --- |
| CPU | 122.45 / 106.28 / 111.85ms | 0.83 / 0.69 / 0.66ms | 111.85→0.69ms |
| GPU | 122.31 / 106.77 / 123.81ms | 0.83 / 0.63 / 0.77ms | 122.31→0.77ms |

これは最初の1文字をSessionHandlerが処理した応答時間。IME選択・TSF描画・アプリ全体の遅延ではない。初回DLL待ちを除いた変更に対する同条件の測定結果であり、一般的な変換速度や初回AI候補の表示速度の向上へ拡大しない。

各version/device90件、全360件で読み・非空の変換・確定との一致、事前の時間上限（キー1000ms/Space1500ms）の欠陥0。Space最大はCPU旧905.66/新582.16ms、GPU旧238.53/新231.83ms。Spaceの速度向上は主張しない。

| 条件 | 旧: 参照表記一致 準備なし/準備後 | 新: 同左 |
| --- | --- | --- |
| CPU | 39/45・44/45 | 39/45・45/45 |
| GPU | 39/45・45/45 | 39/45・45/45 |

少数の非同期試行の表記差を一般精度向上としない。準備なし側の誤変換は残る。各phaseは15回のコールド起動ではなく、1プロセスで15文を続けたもの。新旧いずれもphase間で学習を渡さない。

原票`nonblocking-device-native-v{1,2}/`。v1はcpu-2-afterの準備後フェーズでPython PermissionErrorとなり15件で停止。具体的な発生箇所は当時のtraceback未保存で未確定。成功扱いにしない。v2は成功済み5測定のソース/完了/件数を確認して再利用し、不足7測定だけ追加してexit0。失敗フォルダと元スクリプト/ログを保持。

検証ツールの状態読取では、一時ロックのPermissionErrorを`status_read_error=permission_denied`というunknownとして扱い、PIDやreadyを補って合格にしない。永続する場合は従来のready期限で失敗する。以後の例外tracebackも保存。読み取り不能をreadyと誤認しない陰性試験を含むPython5件成功。入力/時間/完了条件や正解集合は変更していない。保存された成功caseの状態にunknownマーカーは0件。

再現スクリプトは原票ルートの`nonblocking-device-matrix.py`、`nonblocking-device-matrix-v2.py`。既存large_input_cold.pyを呼び、各条件とPID、モデル・データ・実バイナリのSHAを各summary.jsonへ保存した。

## AI不在と休止復帰

同じ新バイナリ・新規隔離profileで各3件、計12件の入力/Space/Enterと実状態遷移を確認。通常profileや登録済みIMEに接続していない。

| 条件 | 確認した状態 | 最大キー応答 |
| --- | --- | --- |
| runtimeフォルダ空 | 同PIDのerror/llama.dll、Mozc入力継続 | 1.75ms |
| modelフォルダ空 | 同PIDのerror、Mozc入力継続 | 1.56ms |
| CPU・idle2秒設定 | ready(zenz/CPU)→実sleep→入力→ready(zenz/CPU) | 2.27ms |
| GPU・idle2秒設定 | ready(llm/非CPU)→実sleep→入力→ready(llm/非CPU) | 1.79ms |

全12件の読み・変換非空・確定/時間上限の検査成功、exit0。idleの監視間隔は製品のままで、sleepまで状態をポーリングして確認した。DLL/モデル不在は、実行中モデルのクラッシュやプロセス終了の試験とは別。原票`nonblocking-device-lifecycle-v1/`、再現`nonblocking-device-lifecycle.py`。

## 精度・配布の境界

既定Standardの関連4セットを実測し、直前の407a449f5と全326件の出力が一致した。

| セット | 新705の正解数 | 直前版から改善/改悪/出力変更 |
| --- | --- | --- |
| AJIMEE | 188/200（94%） | 0/0/0 |
| 慣用句 | 24/25 | 0/0/0 |
| ニュアンス | 17/20 | 0/0/0 |
| 日常 | 79/81 | 0/0/0 |

原票`nonblocking-device-quality-v2/`。同じモデルと既定係数、前文あり、PRELOAD=0。全manifestは705の固定converter、cleanな製品コミットd16a190を記録。データは旧manifestの実パスとSHAを照合して専用フォルダへコピーした。v1はローカル既定パスにAJIMEEが存在せず、変換器起動前に失敗したログとして保持。実パスは隣接KotoriIMEの既存評価ファイルで、元ファイルを変更していない。

報告入力「あいであきょく」は初回39.5msで「アイデア局」、AI状態llm確認後138.4msで「アイデア曲」。check_startup_quality.pyは不一致でexit2。原票`nonblocking-device-idea-startup-v1/`。今回の変更で初回AI未準備の誤変換を解消したとはしない。

## 公開前のモデル資産と製品releaseの区別

追加モデルの取得URLは`model-native-generator-20261017`のモデル専用releaseを指す。従来のcheck_release.pyはrelease一覧で隣のタグをそのまま「前のMSI」として取得するため、モデル専用releaseが間に入ると公開後の版比較が失敗する。

製品の既存タグ形式vX.Y.Z/beta.N/rc.Nと公開済み状態で比較対象を選ぶよう修正。モデル/draftを飛ばすが、資産が欠けた製品タグは飛ばさず従来の資産検査で失敗させる。取得上限に達して前の製品が見つからない場合も、版検査を省略して通さない。関連Python11件成功（うち4件を追加）。ghの実JSONフィールドもread-onlyで確認した。公開済みLatestは今回の確認でもv1.1.3。モデル専用releaseは未公開のままで、実資産の送信はしていない。

その後、CIと同じmozc/toolsのunittest探索コマンドを実行してPython92件成功。`nonblocking-device-python-tests-v1.log`。GitHub Actionsの実行成功とは区別する。

既存の較正済みMSIはソース407a449f5で、この705の修正は含まれない。新ソースのMSI・実TSF・導入/削除・初回AI未準備の品質・低資源実機・独立verifier・公開は未完了。GitHub送信は以前の自動審査拒否に対する明示許可待ちを継続し、push/PR/CI/公開を再試行していない。
