# 現在の引き継ぎ（2026-11-11、v5.2.0 を公開）

**v5.2.0(正式版、Latest)を公開した**(Mozc (Windows) run 42953371781、main 16a2c3f、MSI 3.34.6503.100、
SHA-256 9787f473…、1,668,571,136 bytes。`check_release.py v5.2.0` はすべて OK)。NVIDIA 向けの追加パッケージ
AsteriaCuda64.msi(536,281,088 bytes)も添付。中身は入力モードの保持(#218)、パスワード欄の AI・学習の停止(#213・#219)、
AI の準備の短縮(#214)、CUDA の追加パッケージ(#217)、誤変換の報告(#212)。紹介素材・紹介動画・公式サイトも v5.2.0 に
そろえた(#222・#223、kanai)。

- 精度(実入力、同じ PC、Google日本語入力 3.34.6260): 同音語 300 問 Asteria 91%・Google 45%、日常の短文 500 問 95%・87%
  ([記録](verification/accuracy-realinput-20261111.md))。紹介動画のグラフと設定画面の場面も差し替えた
  ([記録](verification/promo-graph-20261111.md))。
- 残り(安定化): 設定画面の累計の回数が食い違う(別に起動した設定画面で 134、利用者の画面で 122。`asteria_usage.txt` は
  11-10 のまま更新されていなかった。未調査)。6464 の mozc_server の abort 1 件(アンインストールの最中と重なる、未確認)。
  Office・ブラウザーの実入力、低性能機、旧版からの上書き・再起動、24 時間の試験(カード 45)。日常の「あついほん → 厚い本」が
  Standard で外れる。古い開いたままの PR(#113〜#216)の整理。サイトの validate-pages.mjs が v3.0.0 を前提にしたまま。

---

以下は過去の引き継ぎ（履歴）。

# 現在の引き継ぎ（2026-11-11、入力モードの保持・README の精度の測り直し）

利用者の依頼は「アプリを移ると英字に戻るのをやめる」と「README と動画用にフルテストして根拠を作る」。
並行して走っていたセッションはなく、残っていた作りかけ(パスワード欄の受入れ試験)はこのセッションが引き継いだ。

- #217(CUDA の追加パッケージ)・#218(入力モードの保持、カード 84 は done)・#220(精度の測り直し)は main へ統合済み。
  #218 は、全体の入力モードを利用者の切り替えだけで書き、窓・入力欄の移動で当てはめ、IMM 側もそろえる。実機で
  引き継ぎを確かめ、実キーの 半角/全角 も利用者が確認した([記録](verification/input-mode-retention-20261110.md))。
- #219: 導入した MSI で、IS_PASSWORD の欄では学習(使用量)が増えないことを CI の受入れ試験で確かめる。固定の旧 MSI
  (2026-10-19、使用量を記録しない)の検査では `--legacy-candidate` でこの確認だけを飛ばす。
- 精度(実入力、同じ PC、Google日本語入力 3.34.6260): 日常の短文 500問の表記ゆれ一致 Asteria 474(94.8%)・Google 434、
  前文が必要な同音語 300問 Asteria 273(91.0%)・Google 135(45.0%)。README の表を差し替えた。オフラインでは
  AJIMEE 189/200(94.5%)([記録](verification/accuracy-realinput-20261111.md))。
- 残り: 紹介動画のグラフの数字の差し替え(動画の再書き出し)。日常の「あついほん → 厚い本」が Standard で外れる。
  6464 の mozc_server の abort 1 件(アンインストールの最中と重なる、未確認)。古い開いたままの PR(#113〜#216)の整理。
  main の作業ツリー(`D:\dev\Asteria-Japanese-IME`)は改行コードだけの差分が 400 ファイル残っている。

---

以下は過去の引き継ぎ（履歴）。

# 現在の引き継ぎ（2026-11-05、誤変換の報告と回帰ゴールデンを main へ統合）

利用者の依頼「誤変換をデータにできない」への対策。**#211(回帰ゴールデン)と #212(誤変換の報告)は
main へ統合済み**(#211 merge 021f292、#212 merge 88fc30b)。CI の MSI は 3.34.6494.100 で
installer_smoke も成功。

- 91: `eval/sets/asteria-regression-golden.json`、`mozc/tools/regression_add.py`、`mozc/tools/eval_regression.sh`。
  初期 3 問を実変換器で 3/3([記録](verification/regression-golden-20261105.md))。
- 92: 設定「プライバシー → 誤変換の報告」(既定オフ)。直近 20 件をこの PC に残し、設定画面から確認・
  書き出し・消去。パスワード欄・シークレットでは記録しない。`session_handler_main` の実経路で、
  オン=1 件、オフ・シークレット・パスワード=0 件。抽出した MSI の実 UI で表示・消去も確認
  ([記録](verification/recent-conversions-20261105.md))。
- CI: PR の全チェック成功。MSI 3.34.6494.100(1,665,924,089 bytes、SHA-256 9f520a99…c9bd)で
  installer_smoke の install/input/uninstall が成功(native_tsf、8 checks、errors 0)。
- 導入確認: 同じコードのローカル MSI 3.34.6494.100 を実機へ導入し、実 TSF で
  `くうかんこうへんちょうき → 空間光変調器` が記録されることを確認(設定はオフに戻し、記録は消去)。
- 残り: 書き出しの保存ダイアログの実保存、保護欄の AI/学習(CI でも未検証
  `secure_field_internal_ai_or_learning_verified: false`)。導入したのは公開リリースではないローカルビルド。

---

以下は過去の引き継ぎ（履歴）。

# 現在の引き継ぎ（2026-11-04、v5.1.0 一貫性・堅牢性・Issue #208・Unreal の判定役）

利用者の依頼は「堅牢性と一貫性の目標を設定して開発」「Issue に基づく一貫性・堅牢性」「High・Unreal の大きなモデルの
仕組み」。方針(利用者): 多少待っても正しい答え。Mozc の答えへ戻したり、AI の計算を省いたりしない(カード 89)。

- 利用者の PC で 1 回目だけ外れた原因は、Kotori 時代の区切りの学習(boundary.db)。AI がオンなら覚えた区切りで切り直さない。
- 時間の上限で AI の結果を捨てない(GPU の上限 2 → 5 秒)。Space を待っていても打ち間違いの補正を同じ深さで探す。
  辞書にある 4 文字以下の読みは直さない。文を作り直す LLM の補正は変換で使わない(正解が増えず待ちだけ延びた)。
- #208: 別案は AI の点の順、辞書にない作文は Mozc の候補の後ろ、英字表記は一覧の後ろ。
- High・Unreal: 大きな LLM を判定役(点差の小さい意味の選び分けだけ採点し直す)にした。Llama-3.1-Swallow-8B-v0.5 を Unreal に
  載せた(判定用の文脈の選択 225 → 228)。High は同じモデルの Q3_K_M(225 → 229)。置き場は aruiki/Llama-3.1-Swallow-8B-v0.5-GGUF。
- 道具: `consistency_check.py`(押し方・学習の有無で 1 番目が変わらないか)、`oracle_check.py`(外れを作る側/採点に分ける)。
  記録: docs/verification/v501-consistency-20261104.md。Mozc の作業ツリーは `~/ag16` の枝 `asteria/candidate-order`。
- 残り: 文脈の選択の外れ 62 のうち 44 は候補にあるのに点で負けた(合わせ方の改善)。この PC の Claude アプリの入力欄で
  前の文が IME に届いているかは未確認。Word の実入力。

---

以下は過去の引き継ぎ（履歴）。

# 現在の引き継ぎ（2026-11-03、v5.0.0 genmai・新しい設定画面・Eco・追加の AI）

利用者の依頼は「すべてまとめて 5.0.0 を作る」。要件は利用者と決めた(docs/adr/0085、0086、カード 86・87)。

- 変換の中心の zenz を genmai-310M にした(判定用: 普段使い 116 → 118/120、文脈の選択 214 → 225/297)。打ち間違いの
  補正は zenz-small で判定し、genmai のときは Space の後の補正を 100 ms までにした(重い文の待ちを抑える)。
- 段階: Eco(zenz-small だけ、LLM なし)・Standard(genmai と TinySwallow)。High・Unreal は設定画面からダウンロードする
  追加の AI で採点する形にしたが、候補(Qwen3-4B・Qwen3-8B・sarashina2.2-3b)がどれも Standard を上回らなかったので
  表は空(選べない)。仕様に「設定画面のダウンロードだけは通信してよい」例外を足した。
- 設定画面は HTML(WebView2、docs/adr/0084)。記録: docs/verification/v5-models-20261103.md、web-settings-20261103.md。
- Hugging Face の CLI(hf)でアカウント aruiki にログイン済み(OAuth、2026-12-03 まで)。追加の AI を置くときに使う。
- 残り: 上回る大きい AI を探す、Word の実入力(この PC の Office はライセンス切れ)。

---

以下は過去の引き継ぎ（履歴）。

# 現在の引き継ぎ（2026-11-03、HTML の設定画面）

利用者の依頼「設定ソフトの質を格違いに」に、設定画面を HTML で作り Edge の表示部品(WebView2)で開く形にした
([ADR0084](adr/0084-web-settings.md)、カード 86)。v4.2.0 の公開の後の枝 `feat/web-settings`。

- 設定は変えたらすぐ保存。画面と mozc_tool の間は JSON(`KotoriSettingsBridge`、載せた項目だけ・型と範囲を確かめる)。
  WebView2 がない PC と `ASTERIA_CLASSIC_SETTINGS=1` では今までの Qt の画面。キーの割り当て・ローマ字の表・ユーザー辞書は
  Qt のまま濃紺の Fluent の見た目にそろえた。WebView2 SDK 1.0.3179.45(BSD 相当)を静的リンク(使用許諾に追加)。
- 検証: [記録](verification/web-settings-20261103.md)。テスト 14 件、実機で開く・保存が届く・戻せる。
- 見本のデータで画面を見る: `python -m http.server --directory ~/ag16/src/gui/config_dialog/web` を開く(WebView2 の外では見本で動く)。
- 残り: WebView2 のない PC・ハイコントラストの実機。Mozc の作業ツリーは `~/ag16` の枝 `asteria/web-settings`。

---

以下は過去の引き継ぎ（履歴）。

# 現在の引き継ぎ（2026-11-03、v4.2.0 入力中の窓・同じ間違いを二度しない・設定画面）

利用者の依頼は Issue #205(手を止めるたびに候補の窓が出る)への対応と、「使用感が自然で、普段使いの精度を上げる」目標、
設定画面の質の向上。要件は利用者と対話で決めた(カード 85)。

- 入力中の窓は続きの予測だけ、AI を待たずにすぐ出す([ADR0081](adr/0081-hide-suggestion-window.md))。窓は出力で隠し、候補作りは
  続ける(止めると Space の結果が変わる)。同じ文の選び直しを文ごと覚える・英字は半角から・濁点の付け忘れ([ADR0082](adr/0082-same-mistake-never-twice.md))。
  設定画面の作り直し([ADR0083](adr/0083-settings-redesign.md))。応援のお願いの通知(0x80070490 を失敗と扱っていた)を直した。
- 検証: [記録](verification/hide-suggestion-window-20261102.md)。普段使いの判定用 115 → 117/120、回帰 0、同じ間違い 22/22。
  利用者の PC(3.34.6467.100)で受入れ 8/8・Edge 12/12。
- 残り: 普段使いの判定用 99% と打ち間違いの補正の伸びは、同梱のモデルでは届かない(High も Standard と同じ)。大きいモデルを試すには
  利用者の許可が要る。Word の実入力は `local_install_check.py word` で回せるが、この PC は Office のライセンス切れの案内で打てない。
- Mozc の作業ツリーは `~/ag16` の枝 `asteria/hide-suggestion-window`。

---

以下は過去の引き継ぎ（履歴）。

# 現在の引き継ぎ（2026-10-28、v4.0.0 選べる新機能の版）

**v4.0.0 は 2026-10-28 に Latest で公開し、利用者の PC に公開した MSI(3.34.6462.100)を導入して確かめた。** 公式サイトと
stemstudio.jp/asteria も更新済み。次の版の候補: 英語の名前をローマ字のまま打てる機能(PR #203、[ADR0080](adr/0080-romaji-english-names.md)、既定オフ)。

利用者の依頼は「ライブ変換及びその他の最新機能を追加した v4.0.0 を開発・公開。内蔵 GPU への対応も同時に」。方針(2026-10-28):
精度は頭打ちになりつつあるので、精度の高さを土台に、使う人が選べるひねった機能を足す。新機能は既定オフで、オフのときの
精度と速さを変えない。

- ライブ変換([ADR0077](adr/0077-live-conversion.md))、同音語のヒント([ADR0079](adr/0079-homophone-hints.md))、
  内蔵 GPU([ADR0078](adr/0078-integrated-gpu.md)、試験的・実機未検証)。打ち間違いの補正の事後確率版(ADR0076、PR #201)を含む。
- 検証: [記録](verification/v4-features-20261028.md)。オフでは既存の評価の正解数が同じ(交互の A/B で確認)。オンの未確定の表示は
  Space の 1 番目と全セットで一致。利用者の PC に 3.34.6461.100 を導入し、受入れ 8/8・二重入力 26/26・Edge 12、ライブ変換の実入力 4/4。
- 残り: 内蔵 GPU の実機、候補のクリック、Office などのアプリ。Mozc の作業ツリーは `~/ag16` の枝 `asteria/v4`。
- 評価の道具: `session_eval.py --live`(ライブ変換の表示)、`live_conversion_check.py`(導入済みの IME で、設定を一時的にオンにして実入力)。

---

以下は過去の引き継ぎ（履歴）。

# 現在の引き継ぎ（2026-10-28、打ち間違いの補正を事後確率で決める）

利用者の指摘「補正の仕組みが論理的でない」を受け、打ち間違いの補正の決め方を「P(打ったキー | 意図した読み) × P(文 | 前の文脈)」の事後確率(境目 2/3)に置き換えた([ADR0076](adr/0076-typo-posterior.md))。打ち間違いの率は論文の数値(Dhakal 2018、Grudin 1983。データは同梱しない)。点の差 5、3 文字以下は補正しない、ず/づ だけ緩める などの決まりは外した。未公開(MSI は作っていない)。

- 同じ日に v3.2.0 と続けて測った全セット・全タイミングで、v3.2.0 から外れた問は 0。打ち間違いは 24・16・15 で同じ、ふつうの文の Space の p95 は 1〜2 ms の差。専門+ご は「語」のまま。[検証記録](verification/typo-posterior-20261028.md)。
- 残り: しんかんせで のように手を止めた 0.7 秒にぎりぎりの問は揺れる(今日は v3.2.0 でも 3 回中 1 回)。調整用の choice-dev は 221 → 219(補正の書き換えではなく LLM の状態の揺れ)。正解数そのものは増えていない。
- 評価は `mozc/tools/regression_matrix.py`(全セット × 3 タイミングを 1 つずつ順に測って比べる)。

---

以下は過去の引き継ぎ（履歴）。

# 現在の引き継ぎ（2026-10-27、v3.2.0 学習と安定化の版）

**v3.2.0 は 2026-10-28 に Latest で公開し、利用者の PC に導入して実入力で確かめた(3.34.6460.100、[記録](verification/context-choice-20261027.md))。**

利用者の依頼は「文脈に合った語を選べるようにして v3.2.0 として公開」と、途中で加わった「製品としての安定化」。判定用のセットで精度が上がらなかったため、利用者の指示(2026-10-27)で「精度は v3.1.0 と同じ」と明記して、学習と安定化の版として公開する。

- 文脈つきの学習([ADR0075](adr/0075-context-aware-learning.md)): 選び直した語を、近くの語を手がかりに覚え、同じ手がかりのときだけ効かせる。学習の流れ 5/5(起動し直し・学習履歴のクリアも)。既定の手がかり(同音の語 325 組)と、漢字の語の入れ替えだけ LLM の重みを上げる比べ直しも入れた。
- 判定用のセット(文脈の選択・文章)は v3.1.0 と同じで改善なし。既存のセットの回帰は全タイミングで 0、同音語 38 → 39、Space の待ちは同じ。連続操作 4500 回で落ちない。[検証記録](verification/context-choice-20261027.md)。
- 残り: Google 日本語入力との文章の比較(実入力が要る)、調整中に 1 度だけ評価のサーバーが終わった原因(再現せず。`session_eval.py` が記録を残すようにした)、精度の改善(LLM 自身が外す文が多く、モデルを変えない範囲では動かせなかった)。
- Mozc の作業ツリーは `~/ag16`。評価の道具: `session_eval.py`、`score_context_choice.py`、`compare_runs.py`、`learning_flow.py`、`session_soak.py`。評価を並べて走らせない(止めた評価の子のプロセスが残り、結果が混ざった)。

---

以下は過去の引き継ぎ（履歴）。

# 現在の引き継ぎ（2026-10-27、v3.1.0 安定版）

利用者の依頼は「変換精度が下がる不具合が起きない安定版 v3.1.0」。IME の最適化は「精度が下がる不具合や、たまに外れる原因をなくす」ことを優先する。v3.1.0 は v3.0.0 に打ち間違いの補正の強化([ADR0074](adr/0074-key-level-typo-correction.md))と、点検で見つけた不具合の修正を入れた版。モデル・辞書・AI の結果を待って候補を出す方式(ADR0067)・インストーラーは v3.0.0 のまま。

- 打ち間違い 40 問: 手を止めて Space 17 → 24、すぐ Space 4 → 16、0.15 秒後 4 → 15。日常・短文・同音語・AJIMEE は全タイミングで正解数が同じで、v3.0.0 で正しかった文が外れたものは 0。[検証記録](verification/key-typo-correction-20261026.md)。
- 点検で見つけて直した不具合: その場の変換で深い探索が Space を待たせる、すぐ Space を押すと打ち間違いが直らない(v3.0.0 から。打ち直すと直る原因の 1 つ)、自然な文を別の語に変える、時間切れで見つけた直しを捨てる。
- 直さず記録したもの: AI を通すとかえって外れる 4 文(厚い本→熱い本 など、v3.0.0 から。モデルの好み)。利用者の 5 例は 3 例だけ直る。打ち間違いのある文は Space の待ちが延びる。
- Mozc の作業ツリーは `~/ag16` の枝 `asteria/key-typo-correction`。ビルドは `target/accuracy-guide/run_ag16_bazel_v1.py`(`MSYS_NO_PATHCONV=1`)。Qt のテストは CI と同じく `QT_PLUGIN_PATH`・`PATH`・`QT_QPA_PLATFORM=offscreen` を渡す。

---

以下は過去の引き継ぎ（履歴）。

# 現在の引き継ぎ（2026-10-22、短文語彙v4を配布準備へ）

利用者指定は「変換精度の高い日本語IME」。単語/短文/前文なしの辞書・頻度・連語と紹介の改善は[カード81](tasks/81-accuracy-guide.md)、[ADR0068](adr/0068-short-input-lexicon.md)、[全比較](verification/short-input-lexicon-20261022.md)へ記録した。

全量Wikipedia4.13GB/重複等除外15,896,922本文で選んだ115,318語のv4を配布準備候補に固定。開発短文はAIなし111→113、Standard118→118。未使用短文120件を規則固定後に一度だけ開き、AIなし106→109・Standard113→115、各改悪0、候補包含116→118。辞書由来304件の探索診断はStandard286→293だが、一般精度へ読み替えない。この最終結果を使ったコスト/参照の追加調整は行わない。

連語v5は意味の改悪があり不採用、AI専用素材v6も追加の正解数改善0なので今回の配布へ含めない。元のR66f4d665枝/原票は保持し、語彙だけのsource7702a998cからパッチ/同梱/CIへ接続する。AI主体の非同期候補表示・既存34,661連語・モデル・二重入力修正を維持する。

紹介は製品PR183/全7CI→main05d70edとkanai PR12/13で公開済み。README/Release4配置、サイト14配置/20ファイルSHA、Release資産不変を確認。Aboutも更新。記録PR184作成後、GitHub APIがアカウント停止HTTP403を返したため公開/CIは停止中。同時刻の非認証閲覧でもRelease/公式サイト各404を確認し、公開済みという履歴と現在の閲覧不能を区別する。ローカルはsourcec80d4ebc/実MSI3.34.6443.100のbuild・同梱・評価済み辞書のサーバー埋込み・通常Session28/追加13・C++299・再構成140ソースが成功。MSI SHA2b42bc58b32a21910c277701430d9e8a4206e6b517bd9ab1b6530fdd9ba0d446、NotSigned。[固定原票](verification/artifacts/short-input-quality-20261022.json)。次はGitHub回復後に公開先の旧4資産を再確認し、用意したデータ資産を公開→新PR/全Windows CI→同じMSIの実導入/入力/削除とGoogle比較へ進む。公開v2.0.0にはまだ新辞書を含めていない。

Google現行版同条件比較、新MSI/active TIP・実アプリAI寄与、低性能実機、保護欄内部、旧版更新、再起動、独立verifierは未完了。製品完成ではなく、独立FAILと.goal-complete不作成を維持。

---

以下はRC4時点と二重入力調査開始時点の履歴。

# 現在の引き継ぎ（2026-10-21、最新候補の二重入力確認→v2.0.0）

最新依頼は署名なしの正式v2.0.0公開。利用者の二重入力報告を受け、修正・再ビルドも許可された。ただし導入済み3.34.6438.100が古く、現在は解消している可能性も指摘されたため、まず最新RC4/6440で再現確認する。再現した原因は修正し、再現しない場合は憶測で本体を変更しない。[カード83](tasks/done/83-v2-stable-and-duplicate-input.md)。実デスクトップは操作せず、固定MSIの使い捨てWindows CIで確定イベントとホスト文字列を記録する。まだv2.0.0は公開していない。

比較run39575173488で両版のSpace直後Enterに、確定結果と同じpreeditが残った。非同期変換と確定が同じOutputへ追記される原因を修正した。source785bd83a7f236b49925f7a83764d5beadf3bb378、修正前のSpace/Tab2回帰失敗→修正後C++371件成功、GPU/CPUの隔離Session各4条件成功、再生成patch140ソース一致。[検証記録](verification/duplicate-commit-20261021.md)。固定RC4ではEdge textareaの実二重表示も再現した。実一覧を開く手順へ試験を補正し、RC4のnative28条件は既知のSpace直後Enterだけが失敗、残る27件が成功した。補正Edgeも全14件中13件成功、同じSpace直後Enterの実二重だけが失敗。新MSI用の全42条件の判定手順を確認できた。次は新MSIの実TSF28条件とEdge14条件、全CI、版/SHA/対応ソースを確認してv2.0.0公開へ進む。

直前の販促刷新は完了。README、月ロゴによる3画像、添付文を活かした導入ガイド・RC4本文を公開した。[素材](marketing/README.md)、[確認記録](verification/marketing-refresh-20261021.md)、[カード82](tasks/done/82-marketing-refresh.md)。100 build入力と実MSIのハッシュ一致を確認。PR178/179とkanai PR8/9を統合し、公式サイト全7ページも配備6afbd5a/Pages run39535159315で公開した。20ファイルのHTTP200/SHA一致、公開画面7ページ×PC/390pxを確認。この時点では本体や版を変更していなかった。

[v1.1.4-rc.4を公開済み](https://github.com/aruiki/Asteria-Japanese-IME/releases/tag/v1.1.4-rc.4)。採点済み候補保持と予測期限の修正を製品パッチ/新MSIへ反映した。公開4資産を再取得し、全バイト・同梱・アイコンが一致。[公開記録](verification/release-v1.1.4-rc.4-20261021.md)。

## 公開ソースとMSI

PR176は全8CI成功後、main feaeddc3ee40a9efc3ab4049166e042fef1d69faへmerge commitで統合。製品head 813e78aa568826b1cf68eeeab494616d82b3b8a0、CI checkout ed86426e326770f837401069f58e41bf75cdc993、統合commitのtreeは同一。Mozc source039a7251fa16a77a91cb67f5158e20073a3f3762、140 source再構成一致、98+2入力blobを固定。

[Windows run39433685950](https://github.com/aruiki/Asteria-Japanese-IME/actions/runs/39433685950)で同じMSIのbuild・C++・Qt・同梱検査・候補8項目・導入/再導入/入力/削除が成功。版3.34.6440.100、SHA-256 2b37f6fbdcb069251e820e2009b096111cc2253e79ffac65d973fa11ce18e301。導入server/renderer/TIPと管理展開した実バイトが一致した。

RC4はprerelease=true/Latest=false、既存RC2のLatest区分は不変。AuthenticodeはNotSigned。同一MSI/同じbootの再導入を旧版更新/再起動の検証へ読み替えない。

## 実装と固定実モデル検証

[採点済み候補保持](verification/scored-alternatives-20261020.md)は有限採点済みの別仮説を最大8件、読みと既存文節keyが合う場合だけ保持。第一候補/ユーザー辞書を保ち、新規代替はNO_LEARNING。700ms予算・閾値・モデルは不変。期限後の部分集合を通常cacheへ保存せず、弱い続きは本文の先頭を置き換えない。

最終ソースでC++445件・工具2件成功。同じモデル/DLL・辞書・データ・評価器で978の第一候補が全一致、Standard AJIMEE前文あり188/200・なし175/200を維持。候補包含は前文なし189→193/200、あり193→195/200、改悪0。最終300例も290/300、全出力一致。予測29例は先頭一致12→12、上位3一致19→20、改悪0・確定差0。第一候補精度や速度の改善とはしない。

固定EXEの隔離Sessionと実GPUで欠落2候補の実ID選択・表示・確定、AI待機/編集/取消/保留Enter/後続打鍵、Tab、[文脈引継ぎ](verification/queued-context-20261019.md)、通常変換/保存を確認。[AI主体の候補表示](adr/0067-ai-first-candidate-publication.md)を維持する。ローカル詳細検査と新MSIのEDIT/RichEdit受入れは別の証拠。

推論DLLの追加監査で、4モデルと他の推論DLLは評価時と全バイト一致。ggml-base.dllのSHA差はCOFF時刻欄の3バイトだけで、その他全バイトは一致した。新MSIの実モデル/DLLでも実GPUの欠落2候補の選択・表示・確定が成功。元の不一致原票と差の全バイト検査を[公開記録](verification/release-v1.1.4-rc.4-20261021.md)に残した。

## 残る製品検証

実アプリGPU AI ON/OFF寄与、複数アプリ、低性能実機、保護欄内部のAI/学習、旧公開版更新、再起動、独立verifierは残る。配布情報のremaining_verificationへ7範囲をnot_verified、product_complete=falseとして保存。製品完成や.goal-completeにしない。

次の製品検証は、導入済みMSIとactive TIPのSHAを固定し、実アプリでGPU AI ON/OFFの候補差と表示/Space/確定の一致を確認する。複数アプリ・低性能・内部privacy・旧版更新/再起動・独立verifierもそれぞれの条件で確認する。今回の限定受入れを代用しない。

[通常変換の条件](NORMAL_CONVERSION.md)、[精度改善カード81](tasks/81-accuracy-guide.md)、[測定値の紹介条件](BENCHMARKS.md)。規約は[AGENTS.md](../AGENTS.md)、コマンドは[DEVELOPMENT.md](DEVELOPMENT.md)。
