# 91: 回帰ゴールデンの運用(直した誤変換を二度と戻さない)

- 仕様: SPEC 14.1(回帰ゴールデン、REQ-14-1・REQ-14-2)
- 前提: なし
- 規模の目安: 道具 2 本・テスト 1 本・セット 3 問・README と CI の追記

## 目的
実際に使っていて見つけた誤変換を、直した時点で評価セットへ残し、以後の変更で戻らないようにする。
利用者の文章を送らずに、誤変換を 1 問ずつデータへ変える仕組みの土台にする。

## 読むもの
`eval/README.md`(評価セット)、`mozc/eval_baseline.py`(形式と `--min-acc`)、
`docs/verification/v501-consistency-20261104.md`(初期 3 問の根拠)、SPEC 14.1。

## 手順
1. `eval/sets/asteria-regression-golden.json` を作る。初期値は v5.1.0 で直ったことを確認した問題だけ。
2. `mozc/tools/regression_add.py` に追加・検査・一覧を入れる(index は rg- 連番、重複は足さない、原子的に書く)。
3. `mozc/tools/eval_regression.sh` に全件一致のゲートを入れる(`--min-acc 100`、外れた問題を表示)。
4. `eval/README.md` に運用(足すのは直った後、前の文を残す、調整に使わない)を書く。
5. CI に `test_regression_add.py` を足す。

## テスト
`mozc/tools/test_regression_add.py`: 形式の検査、重複、カタカナ・ひらがなの同一視、連番、原子的な書き込み、
コミットしたセットが検査を通ること、`--check` と追加の終了コード。

## 完了条件
- [ ] `python -X utf8 -m unittest discover -s mozc/tools -p test_regression_add.py -v` が通る
- [ ] 変換器のある環境で `mozc/tools/eval_regression.sh` が全件一致(この作業環境には変換器のビルドが無いため未実施)
- [ ] CI が緑
- [ ] 直した誤変換を足す手順が `eval/README.md` に書かれている

## 注意
- 直す前の赤い問題をセットへ足さない。ゲートが常に失敗する。
- 前の文が要る問題は `context_text` を落とさない(前の文なしでは再現しない)。
- セットは調整に使わない(重み・閾値を選ぶと、最終評価の意味がなくなる)。
- 評価の並列実行はしない(過去に結果が混ざった)。
