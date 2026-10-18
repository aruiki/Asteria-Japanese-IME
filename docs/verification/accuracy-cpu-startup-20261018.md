# CPU・初回入力と即時Space（2026-10-18）

較正済みソース407a449f56e3291e1c6d7ac3e38a882d74ef3427の実Mozcセッションで、CPUを強制し、1打鍵ずつ入力して直後のSpace・Enterを確認した。入力経路の検査は30/30成功、参照表記は28/30一致。AI準備前の誤変換2件を保持する。Windows実アプリのTSFや低性能PCを検証済みとはしない。

## 測定条件

- Ryzen 7 5700X上でGPUを使用せず、AIのCPUスレッド数を2に指定。低メモリ制限やノートPC実機ではない。
- `session_handler_main.exe`を上記ソースから新規build。protocも同じBazel依存関係から生成し、実プロトコルのOutputを解析した。build成功87.687秒。`@protobuf`を指定した初回buildはリポジトリ別名の誤りで失敗、`@com_google_protobuf`で修正した。失敗ログを保持。
- SET_CONFIGはStandardだが、CPUでは製品の自動選択でLow相当のzenz-only動作。状態ファイルの実測も`device=CPU / state=zenz / llm calls=0`。GPU Standardの94%と同じ品質条件とはしない。
- 評価済みMSIから展開したモデル全4件とDLLを指定。ランキング・生成幅などの実験用環境変数を除去。モデル・実行ファイル・生成したpb2・入力データのSHAを開始前と終了後で比較し、一致を確認した。
- 開始前のSHA計算でモデルファイルを読むため、OSのファイルキャッシュが空の状態の試験ではない。ここでの初回は新プロセス・新プロファイルの初回で、OS再起動直後やディスクの低速条件は未確認。
- 既存の日常データ先頭15件を使用。新規の未使用評価ではない。読みから実Mozcローマ字表で入力列を作成し、各文字を個別に送信。キー間の待機0秒、Space前の待機なし。
- 準備なし/準備後は別プロセス・別の新規隔離プロファイル。各phase内では通常の確定学習を有効にするが、準備なし側の学習を準備後側へ渡さない。導入済みIMEのユーザープロファイルに接続しない。
- 準備後は自身のPIDとCPUデバイスのready状態を確認する。古い別プロセスの状態ファイルをreadyとしない。入力上限は1打鍵1000ms、Space1500msと事前指定。

## 結果

| 段階 | 参照表記一致 | 読み・変換・確定の欠落/不一致 | 最大キー応答 | 最大Space応答 |
| --- | --- | --- | --- | --- |
| 事前準備なし | 13/15 | 0 | 114.0ms | 490.7ms |
| CPU準備確認後 | 15/15 | 0 | 1.7ms | 420.7ms |

プロセスはPID424/13592、プロファイルは別々。事前準備なし側は1プロセスで15件を連続入力したもので、15回のコールド起動ではない。開始時の状態は空、その後loadingからzenzへ移行した。準備後側は最初の入力前から同じPIDのzenz状態を確認できた。

114.0msのキー応答は最初のd01の1文字目`a`。その他のキーの最大は1.85ms未満だった。初回の遅延を隠すために最初のキーを集計から除外していない。遅延の原因をモデルロードだけとは断定しない。

誤変換はどちらも準備なし側のloading中だった。d01「あしたのかいぎはじゅうじからです」→「明日の会議は従事からです」、d06「ひるごはんなにたべる？」→「昼ご飯名に食べる？」。参照表記や条件を変えて合格にしていない。準備後の別プロファイルでは両方が正解集合と一致した。初回の品質差が残ることを示すもので、30件の入力経路成功を製品精度の合格には使わない。

`large_input_cold.py`は読み一致、変換の非空、Enter確定と変換文字列の一致、時間上限を確認する。参照表記の不一致は別に数え、`--require-exact`なら非成功終了にする。今回のexit0は`input_path_checks_passed=true`に対する判定。未使用評価・初回品質・製品完成のPASSではない。空の結果、途中終了、応答拒否、異なるPID/デバイスを成功扱いしない回帰試験4件も成功。

測定時間は隔離セッションのコマンド応答であり、TSFの候補描画までの時間ではない。変更前との同条件比較をしていないため、性能改善とは主張しない。

## 再現と原票

製品リポジトリで実行（Bazelを停止し、他の変換器・推論を並列実行しない）:

```powershell
python -X utf8 mozc/tools/large_input_cold.py `
  --exe C:/Users/aruik/ag16/src/bazel-bin/session/session_handler_main.exe `
  --proto target/accuracy-guide/session-cpu-acceptance-proto `
  --cases target/accuracy-guide/session-cpu-acceptance-cases.jsonl `
  --install target/accuracy-guide/generator-package-check-v1/ext/PFiles/Kotori `
  --source-commit 407a449f56e3291e1c6d7ac3e38a882d74ef3427 `
  --cpu --threads 2 --count 15 --key-interval 0 `
  --max-key-ms 1000 --max-space-ms 1500 `
  --out target/accuracy-guide/session-cpu-immediate-v2
```

出力先は新規であること。原票は`target/accuracy-guide/session-cpu-immediate-v2/`のsummary.json、results.jsonl、各phaseのstderr。初回のv1は外部辞書を要求する検査ツールの誤認で、セッション起動前に失敗した。SessionHandlerのOSS辞書は実行ファイルへ埋め込まれるため、実行ファイル全体のSHAで固定するよう修正し、v1のログも保持した。

| 対象 | SHA-256 |
| --- | --- |
| session実行ファイル（埋め込み辞書を含む） | e58d2ed1b3bfe25fa6cbf9d896daa204b1c9013b63babd9c0d6404bbfddc1f82 |
| cases.jsonl | 8d6d2116cbee7084c2392d5bea6b8300bce94186edbbf7c43498a8c3a4519ca1 |
| summary.json | 699ecfa46fbc68a8e909ed7f06e649ad0077e53a495ecfc62cab90406e87043b |
| results.jsonl | 7ae8a52d0b0ed0907758f4fba946dfab5cd346f7863d26d4e4291f3505aa612f |

実行ファイルとrunfilesも同原票内の`evaluation-session/`へ固定コピーし、コピー前後のSHA一致を確認した。`snapshot.json`にソース・実ファイル・埋め込み辞書の対応を保持する。新しい回帰試験は既存CIの`test_*.py`探索対象に含まれ、CI設定へ個別の実行例外を追加しない。

## 残る範囲

初回AI準備前の品質、実TSFのキー入力・候補描画・確定、低性能/低メモリ実機、モデル障害時、実MSIの導入/削除、未使用データ、署名、独立verifierは残る。[較正の精度検証](accuracy-confidence-calibration-20261018.md)と合わせても製品完成には達していない。モデルロードやAI待機を入力同期処理へ追加して初回品質を取り繕わない。

初回キーの次の調査箇所: `Touch`→`HasGpu`が打鍵側で`EnsureLlamaRuntime`を呼ぶ。Windowsの`LoadLibraryExW`/backend初期化の初回呼び出しはstatic初期化で同期するため、非同期ロード中でも打鍵が待つ可能性がある。ソースの読み取りで見つけた経路で、114msの原因を特定した実行トレースではない。次はデバイス判定をロード用ワーカーへ寄せ、打鍵側は準備済み状態だけを参照する変更を、GPU/CPU・起動/復帰・AI不在・精度回帰を含めて確認する。この変更はまだ実装していない。
