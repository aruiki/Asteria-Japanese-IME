# 0081. 入力中に候補の窓を出さない設定(既定オフ)

- 状態: 採用
- 日付: 2026-11-02
- 関係: docs/adr/0039(手が止まった後の取り直し)、0067(AI の結果を待って候補を出す)、0071(先回りの変換)、0077(ライブ変換)、Issue #205

## 背景

Issue #205(v4.1.0、RTX 3060、Standard): 文章を考えながら打つと、手を止めるたびに 1 秒ほどで入力中の候補の窓が出て邪魔になる。
「サジェスト」タブの 3 項目(入力履歴・システム辞書・リアルタイム変換)をすべて切っても、取り直し(`KOTORI_REFRESH_SUGGESTION`)が
AI の変換結果を入力中の候補の先頭に入れる(`EngineConverter::KotoriPreviewConversion`)ため、窓が出る。
`KOTORI_LM_LIVE=0` では窓は消えるが、先回りの変換と補正も止まり、Space の待ちが延びる。
Google 日本語入力はサジェストを、Microsoft IME は予測入力を切ると、入力中の窓は出ない。

## 決めたこと

- 設定 `kotori_hide_suggestion_window`(既定 false)。設定画面の「AI 変換」に「入力中に候補の窓を出さない」。
- オンのとき、`EngineConverter::Suggest` は Mozc のサジェストを切ったときと同じく入力中の候補を作らない(打鍵ごとの候補も出ない)。
- 取り直しは続ける。`KotoriPreviewConversion` は AI の変換(補正を含む)を用意して覚えるが、入力中の候補には入れない。
  Space を押すと、用意した結果をそのまま出す。
- ライブ変換(0077)は未確定の文字の表示なので、この設定がオンでも動く。
- Tab や ↓ など、利用者が候補を求めたときは今までどおり窓を出す。
- モバイルの方式(`mixed_conversion`)には使わない。

## 確認

- `//engine:engine_converter_test` の `KotoriHideSuggestionWindowKeepsAiForSpace`: オフでは入力中の窓の 1 番目が AI の結果、
  オンでは入力中の窓がなく(Mozc のサジェストも呼ばない)、Space の 1 番目は同じ AI の結果。
