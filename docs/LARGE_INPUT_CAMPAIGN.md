# 万単位の入力探索

`large_input_cases.py` と `large_input_campaign.py` は、実際のMozc辞書と同梱モデルを使う隔離SessionHandlerへローマ字キー・Space・Enterを送る。Windows TSFの実アプリ操作とは別の検査である。利用者のIMEプロセス、設定、辞書、履歴には接続しない。

入力種類数は重複のない読みで数え、待機や候補操作の再試験を別の文章として加算しない。読みから機械的に打鍵を生成するため、主試験は高速打鍵である。自然な打鍵速度や一般的なローマ字の別表記は追加試験に分ける。主試験後の追加操作は同じ隔離プロファイルの履歴/キャッシュを引き継いだ再入力であり、独立した初回入力の精度比較は新規プロファイルで行う。

## 準備

先に専用Mozcツリーへリポジトリのパッチを適用し、ローカルで `//session:session_handler_main` と `@com_google_protobuf//:protoc` をビルドする。実行中はBazelでバイナリを差し替えない。BazelサーバーやGPUを共有する試験は同時実行しない。

PythonにはMozcのprotocが生成するスキーマと互換性のある `protobuf` が必要。2026-10-12の検査はprotobuf 7.36.1で実行した。スキーマ・原データ・原票・モデルをGitへ追加しない。

```powershell
python -B -X utf8 mozc/tools/large_input_proto.py --protoc C:/Users/aruik/mzsupport/src/bazel-bin/external/protobuf+/protoc.exe --mozc C:/Users/aruik/mzsupport/src --out target/large-input-campaign/proto
python -B -X utf8 mozc/tools/large_input_cases.py --mozc C:/Users/aruik/mzsupport/src --out target/large-input-campaign/cases.jsonl --cpu-out target/large-input-campaign/cpu-cases.jsonl
python -B -X utf8 mozc/tools/large_input_campaign.py --exe C:/Users/aruik/mzsupport/src/bazel-bin/session/session_handler_main.exe --cases target/large-input-campaign/cases.jsonl --proto target/large-input-campaign/proto --out target/large-input-campaign/gpu-standard --interaction-samples 200 --settle 2.4 --key-interval .025 --warm 12
python -B -X utf8 mozc/tools/large_input_summary.py --cases target/large-input-campaign/cases.jsonl --run target/large-input-campaign/gpu-standard --out target/large-input-campaign/gpu-analysis
```

実際の作業ディレクトリ/モデル保存先に合わせて引数を指定する。既存の出力先を再利用するのは `--resume` と同じEXE/データ/モデル/条件のときだけ。隔離プロファイルには所有情報を保存する。途中までの結果は完走と報告しない。

## 採点と原典

| 分野 | 判定する内容 | 判定しない内容 |
| --- | --- | --- |
| 既存の調整セット | 固定された期待表記と一致するか | 表記不一致を自動で意味の誤りと呼ばない |
| 業務文 | 40対象/属性テンプレート群の組合せで期待表記を比較 | 2,560文を2,560独立の意味構造とは呼ばない |
| 上流回帰 | `Conversion Match`/`Not Match`の全文条件 | 複数文節にわたる候補順位を単一文節のUIから採点しない |
| 姓 | 原典の各期待候補が今回の応答で観測できるか。全未観測と一部未観測を分ける | 未観測を辞書欠落と断定しない。原典のDESKTOP条件とOSS辞書の差も残る |
| 上流自然文の読み | 読み保持、Spaceの非空応答、確定一致、待ち時間 | 意味の正解率。原文との自動対応は存在しない |

上流の自然文ストレスデータ、回帰、姓、ローマ字表のSHAを生成manifestへ記録する。最終評価用heldoutは生成器に含めない。重複、160文字超、対応しないローマ字入力を除外し、除外数を残す。主試験は撥音を `n'`、促音を `xtu` に固定して未確定キーの曖昧さを避ける。この条件だけで通常の打鍵全般を検証したとは扱わない。

## 疑義の再現

`large_input_replay.py` は選択したIDをAIモードと打鍵条件ごとに新規隔離プロファイルを作ってAI OFF/ONに通す。前の条件のメモ化結果を次の条件に再利用しない。高速打鍵・待機・通常速度（既定0.15秒/キー）を比較する。意味の再現ではEnterを送らず、検査した誤変換を次の試験へ学習させない。確定の検証を実施したという印は付けない。

```powershell
python -B -X utf8 mozc/tools/large_input_replay.py --exe C:/Users/aruik/mzsupport/src/bazel-bin/session/session_handler_main.exe --cases target/large-input-campaign/cases.jsonl --proto target/large-input-campaign/proto --out target/large-input-campaign/meaning-replay --selected target/large-input-campaign/meaning-replay-ids.json --key-interval .15 --settle 2.4
```

姓は `--candidate-only` でTabとSpaceの最初の4ページを調べる。全候補応答も保存するが、文節をまたぐ全文の候補順位とは区別する。`large_input_operations.py` は通常ローマ字・末尾n・編集・取消・文字種変更を辞書対照で実行する。AI ONの候補操作は主campaignの追加条件で検査する。

`large_input_cold.py` は予熱なしの最初の20入力とモデル読み込み後を比較する。各入力の前後のモデル状態を記録し、読み込み中のMozcフォールバックをモデル稼働後の精度と混同しない。

CPUのみの試験は `--cpu` を付けて別の出力先へ保存する。GPUマシンでCPUへ強制した結果を、実際の低性能ノートPCの受入れ検証とは呼ばない。

## 記録

原票 `results.jsonl`、圧縮した各段階のprotobuf応答、stderr、進捗、完走summary、EXE/モデル/データのハッシュを保持する。集計器は重複操作・読み/期待値の食い違い・欠測を拒否し、意味判定付きの数と総入力数を分ける。厳密な期待表記は変更せず、表記揺れ、曖昧な文脈、誤入力補正、候補への到達、遅延に再分類する。修正の前後には同じ条件で再現する。
