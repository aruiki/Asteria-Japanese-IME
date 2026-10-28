# AGENTS.md

Asteria日本語入力を開発する AI エージェント(Claude Code、Codex など)向けの作業規約。人間の開発者にも当てはまる。

**最初に読む(この順)**: `docs/HANDOFF.md`(現状・次の作業)→ `docs/DEVELOPMENT.md`(環境・コマンド・リリース)→
`docs/tasks/README.md`(作業カード)→ 作業に関係する `docs/adr/`。仕様の全体は `docs/SPEC.md`。

## いまの製品と方針

- 2026-10-19の利用者指示: GPUのAIを主体とし、正常な準備・推論中は読みの表示と編集を続け、候補一覧とSpaceによる変換表示はAIの初回結果を待つ。Mozc候補の先出しへ戻さない。[ADR0067](docs/adr/0067-ai-first-candidate-publication.md)を参照する。

- 製品は **Mozc をベースにした Windows の IME**(docs/adr/0012)。google/mozc をコミットで固定して取得し、
  `mozc/patches/` のパッチで AI の変換(zenz-v2.5 の生成・採点、TinySwallow-1.5B の採点、入力中の予測)と
  設定画面、インストーラを加える。配布は GitHub Releases の `Asteria64.msi`。
- `crates/`・`frontends/`・`justfile` の Rust のエンジンと自前の TSF TIP は **旧 Rust 版**。評価の基準として残すが、
  機能は足さない(CI が緑のままであればよい)。
- 目標は「高品質版 Google 日本語入力」(docs/adr/0014)。利用者の2026-10-12指示により、内部の変換精度と実入力上の失敗発見を優先する。精度(Standard で
  AJIMEE-Bench 90% 以上)を保ち、万単位の入力シナリオで回帰・待ち時間・使い勝手も確かめる(docs/adr/0024〜0029)。
  ハイエンド(GPU)向けの Standard / High / Unreal の中身は保ち、GPU のない PC は Low(CPU)で動かす。
- **メンテナは判断を任せている**。細かい確認は取らずに進め、結果と数値を報告する。止めるのは、仕様との矛盾、
  取り消せない操作、権限の都合で自分ではできない操作のときだけ。

## 作業の手順(毎回これに従う)

1. 利用者の現在の依頼と `docs/HANDOFF.md` を優先し、関連するカードの「読むもの」を読む。独立した新しい仕事は短いカードで追跡する。既存カード内の修正・検証のたびにカードを増やさない。
2. `main` から枝を切る(`git switch -c <種類>/<名前> origin/main`)。1 つの PR は 1 つの目的に絞る。
3. Mozc 版の変更は、Mozc の作業ツリー(`docs/DEVELOPMENT.md` の「Mozc 版」)で書いてビルドし、評価してから、
   `mozc/tools/make_patches.sh` でパッチを作り直してコミットする。パッチを手で編集しない。
4. 確かめる(下の「確かめ方」)。数値が変わる変更は、**PR 本文に前後の比較を貼る**(REQ-14-2)。
5. 設計・配布方針が変わる判断は ADR に理由を書く。通常の不具合修正や検証の追記に新しい ADR は不要。測定結果と失敗原票への参照は verification にまとめ、カード・ADR・HANDOFF ではリンクする。
6. PR は `.github/pull_request_template.md` に沿い、変更に該当する確認だけを記載する。コミットは Conventional Commits、本文に関係する REQ ID。
7. CI が緑ならマージする(マージコミット。下の「はまりどころ」)。カードを `docs/tasks/done/` に移し、
   `docs/HANDOFF.md` の現在の状況を更新する。過去の作業中段落を積み重ねない。
8. 通常のリリースは Actions の「Mozc (Windows)」を main で手動実行する(版は自動で上がる。`docs/DEVELOPMENT.md`)。
   検証済みMSIの再利用は ADR0044 の全条件を満たす場合だけ許可する。CIの省略やMSIの改変はしない。

## 確かめ方(Mozc 版)

| 何を | 道具 | 基準 |
| --- | --- | --- |
| 精度(調整・回帰用) | `mozc/tools/eval_all.sh`(AJIMEE・慣用句・ニュアンス・日常) | 下がらないこと。Mozc より悪くなった問題を見る |
| 精度(最終評価用) | `eval_baseline.py --data eval/sets/kotori-heldout.json` | **調整に使わない**。報告にだけ使う(eval/README.md) |
| GPU の負荷 | `mozc/tools/cost_bench.py`(AI の稼働率) | GPU の使用率は他のアプリで揺れるので、計算の時間で比べる |
| Space の待ち・ノート PC | `mozc/tools/space_latency.py KOTORI_LM_DEVICE=cpu` | 固まらない。正解数も出る |
| 落ちない・待たせない | `mozc/tools/stress_test.py`(GPU と `KOTORI_LM_DEVICE=cpu`) | 異常終了なし、変換の最大が上限内 |
| 予測 | `mozc/eval_predict.py --warm 1`(`--suggest` で入力中の候補) | 当たり・節約文字数・外れ候補の割合 |
| 単体テスト | `bazelisk test //rewriter:lm_rewriter_test //base:kotori_diagnostics_test //session:session_test` | 通る(CI でも回る) |
| 設定画面 | `mozc/tools/capture_window.py`(MSI を展開した mozc_tool) | 崩れない。倍率・ダークモード |

評価と調整の環境変数は `mozc/README.md` の一覧。性能表は `docs/PERFORMANCE.md`。

## 守ること

- 辞書・モデル・データセットはコミットしない(取得スクリプトと SHA-256 を置く)。ライセンスは `docs/licenses.md` と
  インストーラの使用許諾(`mozc/tools/gen_assets.py`)に書き足す。
- 実行時にネットワークへ出ない(モデルと実行環境は MSI に同梱)。
- `docs/SPEC.md` の変更は、メンテナの承認がある PR でのみ行う。仕様と矛盾したら止めて Issue を立てる。
- Windows 固有の変更は、手元の Windows か CI の Windows ランナーで確かめてからマージする。
- テストを消したり無効にしたりして CI を緑にしない。

## はまりどころ(AI が実際にはまったこと)

- **Git Bash のヒアドキュメントで `\\` が崩れる**: `python - <<'EOF'` の中に `"\\n"` や `D:\\bazel` を書くと、
  改行や制御文字(`\b`)に化けることがある。C++ やパスを書き換えるスクリプトは、Write でファイルに書いてから
  `python ファイル` で実行する。書き換えた後は制御文字がないか `grep -P '[\x08\x0c]'` で確かめる。
- **Bazel のターゲットが MSYS に書き換えられる**: Git Bash では `MSYS_NO_PATHCONV=1 bazelisk build //converter:converter_main ...`。
- **実験中にビルドしない**: 走っている `converter_main.exe` を bazel が差し替えると、結果が混ざる。別のビルドが
  CPU を使っている間の時間の計測も揺れる(先に `tasklist` を見る)。
- **パッチを作り直す道具が Mozc の作業ツリーを壊した(2026-10-02)**: `make_patches.sh` が作業用の clone を作り直せず
  (別の作業ツリーで Bazel のサーバーが開いていた)、`&&` でつないだせいで止まらずに `~/mz` のまま続け、ファイルを自分自身に
  写して 13 個を空にした。今は 1 行ずつ確かめて止まる。`~/mz` 以外の所で Bazel を動かしたら `bazelisk shutdown` してから
  作り直す。壊れたときは、素の Mozc に main のパッチを当てた木から写して戻す(`git worktree add`)。
- **Bazel の出力は読み取り専用**: `bazel-bin` からコピーしたファイルを上書きするときは `chmod u+w` してから。
- **積み重ねた PR のマージ**: 下の PR を `--delete-branch` でマージすると、上の PR が付け替えられずに閉じることが
  ある。先に上の PR の base を main に変えてからマージするか、一番上のブランチからまとめて出し直す。
  squash は積み重ねた PR で衝突を起こすので、マージコミットを使う。
- **CI を待たずにマージした(#125)**: `gh pr checks` は、push の直後はチェックがまだ 1 つも出ていない
  (「no checks reported」)。「pending がない」だけを見て待つと、CI が始まる前にマージしてしまう。チェックが
  出そろい(Asteria64.msi を含む)、pending が 0、fail が 0 であることを確かめてからマージする。
- **MSI の版**: リリースごとに `version.bzl` の BUILD を上げている(カード 22、35)。同じ版の MSI を上書きしても
  ファイルは入れ替わらない。Mozc の `mozc_version.py` は `BUILD = <数>` の数しか読まない(足し算は無視される)。
  CI のログで書き換えが成功していても、**出来上がった MSI の ProductVersion で確かめる**(beta.5 で見落とした)。
- **内蔵 GPU は既定では AI に使わない**: 専用 GPU(VRAM 3 GiB 以上)だけを使う(docs/adr/0024)。設定「内蔵 GPU で AI を動かす(試験的)」がオンで専用 GPU がないときだけ、内蔵 GPU に zenz を載せ、品質や時間の上限は GPU のない PC と同じにする(docs/adr/0078)。
- 旧 Rust 版の CI でだけ落ちるテスト、CRLF、UNIX ソケットのパス長などは `docs/HANDOFF_HISTORY_20261012.md` の「はまりどころ」。

## コーディング規約

- Mozc のパッチ(C++): Mozc の書き方に合わせる(Google C++ スタイル、`absl`、`// Kotori:` で始まるコメントで
  Kotori の変更だと分かるようにする)。コメントと文書は日本語。
- 旧 Rust 版: edition 2021、MSRV 1.80、`rustfmt`、`clippy -D warnings`、`unsafe` は FFI 境界だけ(`// SAFETY:`)。
