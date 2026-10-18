# AI初期化のメモリ不足と入力継続（2026-10-18）

## 固定した対象

Mozcソース3d1dfdee6ac1b410db4d1d1a7fff6af75f56f000。元は705e64361c61708a68716954bc8969046cb842b4。Bazelの固定b11259ソースへ`src/bazel/llama_oom.patch`を当て、ggml-base.dllだけを再構築。他のllama/ggml/CPU/Vulkan DLLと4モデルは既存MSIの展開物を使用した。元の展開物を変更せず別runtimeフォルダへ保存した。

- session: 07f26f7b4d867569a5d17a60bb5bca1cc4c24af3876f0e94b15d6b25115b41cc
- converter: a9777c23d6ecc2ec59496dc7b5a87b82f79d3855d949051f08fa09d4e3bedce6
- 修正版base: e23b0ff25f9e0d57e66fcfe12578461a275677c0a571baf02b3d8a97fde39bd4
- 元base: 0396d453898eff5f105baf187cd7bfd0d83edba62ebfda0c76c5a2be40170b49

元の686 exportsを維持し、公開APIの署名は変更していない。`ggml_init`はbuffer不足でNULLを返す。schedulerの同じ確保失敗はstd::bad_allocとして、既存llama_init_from_modelのstd::exception境界へ戻す。C++例外を処理できる当該初期化経路を実プロセスで確認したもので、任意のGGML APIの全メモリ不足を保証する修正ではない。

## 失敗を含む切り分け

Windows Jobのprocess commit上限256MiBに、自分の子だけをsuspended状態で割り当ててから再開した。物理RAM256MiBの実機、導入済みTSF、AIを別プロセスへ隔離した試験とは異なる。

| 原票（target/accuracy-guide以下） | 結果 |
| --- | --- |
| nonblocking-device-memory-v1 | 元baseでAI準備中にggml.cのassert、exit0xC0000409。対照3文成功、AI入力0件 |
| ggml-oom-memory-v2 / lifecycle-v1 | base単独差替えで入力は続くが、通常メモリでもCPU backend未登録。合格にしない |
| ggml-oom-lifecycle-v2 | OpenMP明示ロード後、通常CPU/GPUと休止復帰・AI不在の12文成功 |
| ggml-oom-memory-v3 | 同じ256MiBでscheduler側の別abort、exit0xC0000409。未解決として保持 |
| ggml-oom-memory-v4 | 最終baseと明示ロードで対照3文/CPU-AI失敗時3文とも読み/変換/確定成功。peakは対照8.48MiB、CPU-AI226.17MiB |
| ggml-oom-normal-v1 | 最終baseで通常CPU実zenz/GPU実llmの同PID readyを確認し、各3文成功 |

公式baseはlibompを暗黙に読み込んでいた。新baseはその依存を持たないため、通常runtimeローダーが同梱libomp.dllを安全なLoadLibraryExWフラグで先に読む。最初の差替えでの「低メモリ成功」はこの差によりAIが普段から動かない結果だったため、最終修正の合格根拠に使わない。stderr、PID、状態、入力原票、各SHAを保存した。

## 検証と未完了

C++はWindows workflowと同じ10対象が成功（71.405秒、2対象を実行、8対象は同一入力のキャッシュ）。パッチの新規scratchへの適用と全134ファイルのソース一致・whitespace検査が成功。`ggml-oom-source-reconstruction.json`にソースcommitとパッチSHAを保存した。パッチ生成開始時にテスト終了待ちを完了できていなかったが、独立した基点cloneを使用し、固定commitとの全ファイル一致とテスト最終exit0・shutdownを後で確認した。ソース破損はなかった。

DLLのビルド失敗v1/2（宣言不足）、v4（patch区切り）、v5（公開関数の署名競合）もログを保持。公開ABIを変更する案は採用していない。最初のバイナリ固定コピーはreadonly MANIFESTで途中失敗し、新規`ggml-oom-candidate-v3`でrunfilesを含め固定し直した。

この記録時点では最終DLLの精度回帰も未実施だった。後続の[前文なし比較](accuracy-contextless-baseline-20261018.md)で、従来の前文あり4セット全326出力が705と一致した。MSIへの実同梱と展開照合、実TSF、導入/削除、独立verifierは未完了。この修正を既存MSIが含むとはしない。ユーザーの後続指摘により、次の精度改善は前文なしの初回変換を優先する。既存MSIを再生成しただけでこの品質課題の完了にしない。
