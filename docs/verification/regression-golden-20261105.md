# 回帰ゴールデンの初期実行(2026-11-05)

カード 91。`eval/sets/asteria-regression-golden.json` の初期 3 問を、実際の変換器とモデルで測った。
**これは 3 問の回帰の確認であり、精度の測定ではない**(95% 区間は 43.8〜100.0%)。
証拠の範囲は変換器の出力(`evidence_scope: offline_converter_not_installed_tsf`)。導入済み TSF の実入力ではない。

## 条件

- 変換器: `C:\Users\aruik\ag16\src\bazel-bin\converter\converter_main.exe`
  (実体 `...\bazel-out\x64_windows-opt\bin\converter\converter_main.exe`、7,410,688 bytes、
  2026-11-04 9:49、SHA-256 `3e418a3d…ed89`)。Mozc の作業ツリー `~/ag16` の枝 `asteria/candidate-order`、
  HEAD `d69de76`。ただし原票では `converter_source_commit: unverified` で、このバイナリが HEAD と
  同一のソースから作られたことは未検証(作業ツリーには未追跡の生成データがある)。
- モデル(インストール済み Asteria 3.34.6491.100 から): genmai-310M-q8_0 `89ab0a1d…fd2b`、
  tinyswallow-1.5b-q5_k_m `0aa55c79…ebc8`、zenz-v2.5-small-q8_0 `4edf2115…aa4c`、
  zenz-v2.5-native-generator-20261017-q8_0 `49112e8b…8924`。
- 実行環境: Windows 11 (10.0.26300)、16 論理 CPU、NVIDIA RTX 3060、ドライバ 617.14。
- データ: `eval/sets/asteria-regression-golden.json`(3 問)、SHA-256 `01c8893e…5e02`。
- 評価器: `mozc/eval_baseline.py`、`--context`、品質 Standard(製品の既定)、`KOTORI_LM_PRELOAD=0`。
- 実行時刻: 2026-11-05 06:56 JST。リポジトリのコミット `267614e`(この変更)。

## 実行

```bash
OUT=<一時フォルダ> mozc/tools/eval_regression.sh \
  "C:/Users/aruik/ag16/src/BAZEL-~1/CONVER~1/converter_main.exe" "C:/PROGRA~2/Asteria"
```

## 結果

Acc@1 **3/3 = 100.0%**(95% 区間 43.8〜100.0%)、10 秒。

| index | 読み | 前の文 | 第1候補 | 判定 |
| --- | --- | --- | --- | --- |
| rg-0001 | くうかんこうへんちょうき | (なし) | 空間光変調器 | 正解 |
| rg-0002 | くうかんこうへんちょうき | うーむ納得いきませんね。今は普通に出ます。 | 空間光変調器 | 正解 |
| rg-0003 | せいどのはなしなんかしていない | 上の文自体が精度の悪さを物語っています。文脈的に | 精度の話なんかしていない | 正解 |

原票: `standard_regression-golden.json` と `.manifest.json`(実行した PC の一時フォルダ。
`data`・`converter_main`・`models`・`runtime`・`dictionary` の SHA-256 を含む)。

## 意味と限界

- 初期 3 問は、v5.1.0 の記録(`v501-consistency-20261104.md`)で直ったことを確認した問題。この実行で、
  現在の変換器でも戻っていないことを確かめた。
- 3 問だけなので、精度の主張には使わない(区間 43.8〜100.0%)。SPEC 14.1 の 2,000 問へ足していく。
- 導入済み TSF・実アプリ・CPU(Low)の各段階は未検証。条件が変われば結果も変わりうる。
