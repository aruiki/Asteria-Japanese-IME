# 0081. 入力中に候補の窓を出さない設定(既定オン)

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

- 設定 `kotori_hide_suggestion_window`(既定 true)。設定画面の「AI 変換」に「入力中に候補の窓を出さない」。
  最初は既定オフで作ったが、利用者の指示(2026-11-02「デフォルトで表示されないようにしてほしい」)で既定をオンにした。
  既存の利用者も、設定を保存していなければ(proto の既定値が使われるので)次の版から窓が出なくなる。
- 既定がオンなので、「サジェスト」タブの入力履歴・システム辞書の候補も入力中には出ない。入力中の候補がほしい人は
  この項目を外す。
- オンのとき、入力中の候補は今までどおり作り(`EngineConverter::Suggest`、取り直しの `KotoriPreviewConversion`)、
  窓にだけ出さない(`EngineConverter::FillOutput` が入力中の候補を出力に入れない)。Space を押すと、裏で用意した結果を出す。
- 最初は「入力中の候補を作らない」(Mozc のサジェストを切ったときと同じ)で作ったが、比べると Space の結果が変わった
  (下の「確認」)。打鍵ごとの候補作りが `LmRewriter::RewriteSuggestion` で AI の先回りの変換(打ち間違いの補正を含む)を
  始めさせていたため。作るのをやめると補正の準備が遅れる。
- 候補は作っても見えないので、キーは候補がないときと同じに扱う(`Session::KotoriShowsSuggestion`)。Shift+Enter・
  Ctrl+Enter の「1 番目の候補で確定」で見えない候補を確定しない(Ctrl+Enter は打った読みのまま確定する)。
- ライブ変換(0077)は未確定の文字の表示なので、この設定がオンでも動く。
- Tab や ↓ など、利用者が候補を求めたときは今までどおり窓を出す。
- モバイルの方式(`mixed_conversion`)には使わない。

## 確認

- 実際の IME と同じセッションの経路(`mozc/tools/session_eval.py --context`、RTX 3060、Standard)で、窓あり・なしを
  手を止めて Space・最後の打鍵から 0.15 秒・すぐ Space の 3 通りで比べた。
  - 「作らない」版: 打ち間違いのセットで手を止めたとき 23→22。「らいねんのけいかう」が 来年の計画 → 来年の経過う(4 回とも)。
    また起動して最初の問だけ Space が約 0.18 秒待った(取り直しの時間 `kotori::g_live_first_ms` が打鍵ごとの変換の要求で
    決まっていたため。モデルの読み込みのときにも決めるようにした)。
  - 「作って出さない」版(採用): 結果は `docs/verification/hide-suggestion-window-20261102.md`。
- Mozc の既存のテスト(engine_converter_test・session_test)は入力中の候補を確かめるものなので、テストの初期化でこの項目を
  外して今までどおり確かめる(文字種メニューを隠す設定 `kotori_hide_character_types` と同じ扱い)。
- `//session:session_test` の `KotoriDefaultHidesSuggestionWindowButKeepsAiForSpace`: 既定の設定のまま、打鍵・取り直しで
  窓が出ず、未確定の文字は読みのまま、Shift+Enter で確定せず、Space で AI の結果を出す。
  `KotoriHiddenSuggestionIsNotCommittedByKeys`: 窓を出さないときの Ctrl+Enter は打った読みを確定する。
- `//engine:engine_converter_test` の `KotoriHideSuggestionWindowKeepsAiForSpace`: オフでは入力中の窓の 1 番目が AI の結果、
  オンでは入力中の候補を作っても窓と候補の一覧を出力せず、Space の 1 番目は同じ AI の結果。
