# AI が選んだ数字を NumberRewriter に戻されないようにする(2026-11-24)

判定用セットの見直しで見つけた誤変換「ななじでいいよ」→「なな時でいいよ」(前の文「明日って何時集合だっけ」)。
AI(文全体の採点)は「7|時でいいよ」を選んで先頭に置いていたが、その「7」は文節の候補に無い値なので、AI は先頭の候補
(「なな」、数字でない品詞)を写して作っていた。NumberRewriter はこれを、辞書の数字の候補(基準)より上にある同じ表記と
みなして消し、基準の候補の表記を書き換えていたため、先頭が「なな」に戻っていた。

直し方(`rewriter/number_rewriter.cc`): 先頭にあって `kotori_ai_evaluated` の候補は、消す対象から外し、別の表記へ
書き換えない。七・７ などの表記の候補は、これまでどおりその下に並ぶ。AI が選んでいない候補の扱いは変えない。
単体テスト `NumberRewriterTest.KeepsAiChosenNumberOnTop` を足し、CI のテストの一覧に `//rewriter:number_rewriter_test` を加えた。

## 条件

- PC: RTX 3060、Windows 11。モデルはインストール先。`ab_suite.py`(GPU が空いていることを確かめて 1 つずつ)。
- main: `D:\dev\bins\main-f5512a6`(main #240 の時点)。fix: `D:\dev\bins\fix-num2`(main + この修正)。
- 原票: `D:\dev\ab\fix-num2`、`fix-num2-session`、`number-probe`、`number-probe-session`。

## 結果

既定のセット(7 セット 1,033 問):

| 経路 | main | fix | 入れ替わり | 時間 |
| --- | ---: | ---: | --- | --- |
| converter_main | 906 | 906 | なし | AI の計算 中央値 164 → 166 ms、p95 371 → 368 ms |
| session(製品と同じ) | 898 | 898 | なし | Space → 候補 中央値 0 / p95 16 ms(同じ) |

数字の読みの問(`eval/sets/asteria-number-probe-20261124.json`、8 問。この修正のために作った):

| 経路 | main | fix |
| --- | ---: | ---: |
| converter_main | 7/8 | 8/8 |
| session | 7/8 | 8/8 |

直った問: np-1「ななじでいいよ、えきでまちあわせしよ」 main「なな時でいいよ…」→ fix「７時でいいよ…」。外れた問はない。
全角の「７」になるのは、これまでの数字の表記の設定どおり(この修正では変えていない)。
