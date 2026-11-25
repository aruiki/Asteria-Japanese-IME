# 98: 候補の窓を Rust + Direct2D にする(軽量化のロードマップ Phase 4-1)

- 仕様: SPEC の候補の窓の章(表示内容は今の mozc_renderer と同じ)
- 前提: ADR0090(軽量化のロードマップ)。利用者が 2026-11-25 に「候補の窓(4-1)から」を選んだ
- 規模の目安: 数週間。段ごとに小さな PR、設定で切り替えて既定は今の窓のまま

## 目的
GDI の mozc_renderer(C++ 約 6,400 行)を、Direct2D・DirectWrite で描く Rust の窓に置き換え、高 DPI・連打のときの
描画の引っかかりと常駐メモリを減らす。

## 段階
1. 受け口: Mozc の renderer の IPC(名前付きパイプ、`RendererCommand` の protobuf)を Rust で受ける。
   `kotori-proto` に renderer_command.proto を足し、`crates/kotori-renderer` を受け口にする。
2. 描画: 候補の一覧・注釈・フッター・選択の色(kotori_palette と同じ値)を Direct2D で描く。位置は
   `placement.rs`(マルチモニタ・DPI)。
3. 置き換え: 設定(試験的)で mozc_renderer の代わりに起動する。MSI に同梱、既定はオフ。
4. 比べる: 描画の時間(連打 30 文字)、常駐メモリ、表示の差(capture_window で画像を比べる)。
5. 既定にする: 差がなく速ければ既定にし、mozc_renderer を外す。

## 完了条件
- [ ] 1〜3 を設定で試せる
- [ ] 表示の差なし(倍率 100・150・200%、ダークモード)、常駐メモリ 10 MB 以下、描画 1 ms 以下
- [ ] 既定にする判断(ADR)

## 調べたこと(2026-11-25)
- 受け口の約束: mozc_renderer はプロファイルの `.renderer.ipc` に IPCPathInfo(鍵・pid・版)を書き、名前付きパイプ
  `<kIPCPrefix><鍵>.renderer` をメッセージ単位(`PIPE_TYPE_MESSAGE`、`PIPE_REJECT_REMOTE_CLIENTS`)、Mozc の
  サンドボックスのセキュリティ記述子(`WinSandbox::kSharablePipe`)で開く。1 往復 = protobuf 1 つ。
- 呼ぶ側(TIP)は `IsValidServer` でサーバーの実行ファイルのパスを確かめる(`ipc/ipc_path_manager.cc`)。Rust の窓は
  同じパスの mozc_renderer.exe として置くか、確かめる先を足す必要がある。
- 今の C++ の窓には Asteria の追加(kotori_palette の色、kotori_candidate_snapshot)がある。表示の差を比べる対象。
