# v1.1.3公開候補を全CIと実MSIで検証完了（2026-10-12）

ユーザーは「最新版リリース」を指示。PR160を全8チェック成功後にmain f519326へ統合。head f006fe5、実CI checkout4e841b2とtree一致。Windowsrun38235273294の未加工MSIをADR0044で公開する。
MSI3.34.6431.100、SHA b7dbaee7e99dcea16544b169cadb7ea1e0c5b66f1225cbc01a044ad4293688d5、1,569,517,568bytes。導入exit0、TSF「日本語」「東京」、削除3010（再起動要求）、原票install/input/uninstall true。公開前check_release全項目成功、アイコン/モデル一致。
詳細docs/verification/release-v1.1.3-20261012.md。取得に時間を要したがCLI成果物は検証原票とSHA一致。候補照合用スクリプトの文字コード名を修正後に全検査成功。元のMSIは改変しない。
公開ノートとこの記録など文書だけを追加し、draft assets/タグ照合→正式公開→公開後check_releaseで再取得まで進める。一般CI38235273342全7、mainCI38246133783もsuccess。ローカルRust147成功、最終heldout289/300=96.3%。
ユーザーの追加指示: リリースが終わったら変換精度99.9%を目標に開発を進める。公開と再取得確認を終えてから着手し、調整データと独立評価を分け、曖昧な表記を正解へ変更して率を上げない。現在の91.5%/96.3%を99.9%達成と扱わない。

以下は履歴。

# 中断した大規模探索を再開：入力補正保護のローカル検証完了（2026-10-12）

現在の対象はAsteria（KotoriIMEリポジトリ）、codex/large-input-campaign、開始main b23f0b8。前のチャット「GitHubのダウンロード数を確認」の最終作業を原票とソースで再確認。既存変更を保持した。通常execはMXC os error1326で起動不能だったが、許可された別実行経路で復旧。

Mozcの入力補正がAIに取り消される問題を仕上げ、文節/文全体へTYPING_CORRECTIONを伝播。先頭補正候補のキャッシュ/再採点を保護。実ラティスの文節確認テストも追加PASS。修正前4対象PASSと最終guard build/testのログを保存し、再開後は変更したテストだけを追加検証。
40例×3条件で「こんいちは→今日は」「りょうしゅしょをください→領主書を下さい」がそれぞれ「こんにちは」「領収書をください」へ。AI ON6出力改善/他114不変、OFF120不変。386問全出力不変、AJIMEE183/200=91.5%維持。heldout未使用。

既存GPU13,178読み+600操作を完走済みと確認。今回CPU1,000読み+90操作を328秒で完走、defect0、即Space中央値156.1ms/最大493.0ms。通常34操作defect0。初回ロード前後GPU/CPU各40操作defect0。利用者のIME/履歴/設定へ接続しない隔離CLIでありTSF受入れではない。
姓696読みの追加ON/OFF検査完了。ON501読みで少なくとも1表記観測、選択685/685一致。OFF502読み/選択782/782一致。全変種未観測や4ページ範囲外を辞書欠落と断定しない。元のGPU6.8秒長文遅延は再現せず、未解決として保持。

Python全37テストPASS。CPU抽出を正式生成器の--cpu-outへ追加し既存1,000例とバイト一致。Bazel shutdown後make_patches.sh成功、全3パッチ適用、130変更入力が検証済みソースと一致。0002のみ変更。ソース7ファイルSHAと前後比較をguard-paired-comparison.jsonへ固定。
記録: docs/verification/large-input-campaign-20261012.md、ADR0061/0062、docs/LARGE_INPUT_CAMPAIGN.md。原票はtarget/large-input-campaign（meaning-before/after、guard-eval、cpu-after、surname-after、cold-gpu-after/cold-cpu-after、各log）。
コマンド: python -B -X utf8 -m unittest discover -s mozc/tools -p test_*.py（exit0、37件）、large_input_campaign.py --cpu --interaction-samples30（exit0）、large_input_replay.py --candidate-only（exit0）、large_input_cold.py GPU/--cpu（両exit0）。詳細引数は上記文書と実行記録。

push/PR/一般CI/main統合/MSI作成・公開は今回未実施。既存の未コミット変更を含む開発ブランチの差分として保持。次は差分をレビューして一般CIと必要なWindowsビルドを通して統合する。その後「おふろわいたよ→お風呂湧いたよ」等の意味選択/誤入力補正を別の再現から開発する。利用者への完了報告で製品完成や導入版への反映とは言わない。

以下は履歴。過去の「実行中」PID/sessionは終了済み。

# 万単位の入力探索を実行中（2026-10-12）

利用者は小規模評価では不十分として万単位で入力者が躓く点を発見するよう指示。カード77、codex/large-input-campaign、main b23f0b8から開始。AGENTSの「精度は十分」という前提を改めた。
重複しない13,178読みをGPU Standardで実打鍵→即Space→Enterまで完了（4,285秒、ローマ字617,586打鍵）。Meaning oracleは2,910件。自然文には意味の正解率を作らない。heldout未使用。データはtargetのみ。
続く200サンプル×待機/Tab取消/F6再変換を実行中。exec session4959、検査専用PID34804。利用者のIME/履歴/設定には接続しない。全体完走後に追加試験を順番に実行する。GPUを使うnative試験は並列化しない。
初回主試験は読み欠落/確定差分なし、Space6,832.65msの1例（ログLLM採点6,682ms）。表記不一致67（業務33は念の為/念のためのみ）、上流Match3、姓の全未観測456。別途全変種も照合し696入力/889読み表記組が未観測、338組は辞書raw sourceに存在。未観測を辞書欠落と断定しない。
exe SHA387506290813c7988a0bbddd8a7d7f8eaf9aac3ec6a91064701fba51a96e1b0f、cases SHA66b0c30c06a80c70bb9d460595117d8d75ab49ee78f7d9518bebd7bd8f619828。out target/large-input-campaign/gpu-standard。start時のharness/generator/sessionソースをoutへ保持、source-identity.jsonへSHAを記録。
主試験のPythonは開始時に読み込んだ版であり、現在のmozc/toolsは原応答保存/姓の部分未観測/timeout診断等を追加した次版。source/native EXE/modelsは検査中に書き換え・buildしない。原票は変更せず、現在版のflagが過去結果にあったことにしない。原F6試験の最初のSpaceは後の再変換Spaceに置換される記録上の制約を次版で改善済み。
追加予定: large_input_replay.pyでmeaning-replay-ids.jsonの40例（条件毎に新規profile、OFF/ON×高速/待機/.15秒打鍵）、surname-all-missing-ids.jsonの696例（--candidate-only、窓/全候補/選択を区別）、cpu-cases.jsonlの層化1,000例（--cpu --interaction-samples30）、large_input_operations.pyの通常n/促音/編集/F7/F10/URL等32操作、large_input_cold.pyの予熱なし/モデルready後20例（GPU/CPU）。まだ実行したとは報告しない。
Protoを対応protocから生成するlarge_input_proto.pyを追加し生成byte一致を確認。protobuf7.36.1/Python3.13.15/Win11 26300/5700X/RTX3060。集計large_input_summary.pyは重複/期待値変更/欠測を拒否。工具36テスト成功。初期pilot失敗はpilot-v*へ保持。
次は4959完走→上記追加試験→原票集計・疑義分類・再現記録→必要な修正のみ前後比較→一般CI全7成功→PR統合。製品コード/MSI/READMEは今回変更していない。以下は履歴。

# 内部精度改善：生成候補の文末文法語を保護（2026-10-12）

PR158のUnicode修正はCI38192369025全7成功後にmain b1d73d8へ統合。続けて抽出例「電気を消してね→電気を消して寝」を修正。Mozcの右品詞/内部文節/辞書の一致で機能語を守り、生成候補だけを対象にする。
初案は単体テストPASSでも実例FAIL、複合候補のcontentに機能語を含むことを実ログで確認し辞書補完へ修正。失敗原票も保持。対象0/1→1/1、調整386問は表記58→59/60・他4セット不変・悪化0。Standard AJIMEE91.5%維持。
採用後のheldout最終比較は289/300→289/300、改善0/悪化0。調整に使わない。54ネイティブテスト成功、Bazel停止後パッチ再生成/3適用成功。生成なしの不要な辞書検索を省略。
カード76、ADR0060、verification/generated-grammar-ending-20261012.md。原票target/conversion-quality-debug/grammar-final、heldout-paired。公開MSIは変更なし、実TSFとCLI評価を区別。次は残る24不一致のうち「書く/描く」「うかがえる/伺える」の意味選択と曖昧な正解基準を調べる。一般CI成功を確認してmainへ統合する。以下は履歴。

# 内部変換デバッグ開始・日本語パスのAI無効化を修正（2026-10-12）

利用者は例の抽出も含む高速デバッグを指示。READMEの改名説明削除はPR157をCI全7成功後にmain7cf6239へ統合、公開READMEの一致確認済み。
専用mzsupportで現行converterをビルドし、5セット386問のMozc/Standardを比較。日本語runtimeパスでAIが動かずMozcへ戻る問題を再現し、Unicode環境取得とUTF-8ファイル入出力へ修正。日常65→79/81、AJIMEE102→183/200までAIが働く状態に回復。通常の英数字パス対照と全386出力一致、Standard91.5%維持。
環境回帰テストFAIL→PASS、lm_rewriter53件/工具18件成功。Bazel停止後パッチ再生成/全3適用成功。公開MSIは変更なし。CLIを実TSFの精度/受入れとは扱わない。
ADR0059、verification/conversion-quality-debug-20261012.md、カード75。quality_triage.pyで結果を問題ID/データSHA/文脈条件で照合し25件の不一致とMozc正解→AI不一致3件を抽出。元データ/期待表記は維持、heldout未使用。
次は「続編を書く→描く」「うかがえる→伺える」「消してね→消して寝」の候補選択を優先し、意味誤り/許容表記を分けて改善。原票はtarget/conversion-quality-debug。一般CI成功を確認してmainへ統合する。以下は履歴。

# README冒頭の改名説明を削除（2026-10-12）

利用者の指示で、Asteria/Kotoriの改名案内と付随するWindows表示変更の段落を削除。カード74。冒頭はバッジからダウンロードへ続く。差分/空白検査後、一般CI成功を確認してmain統合する。以下は履歴。

# Asteria公開名の移行（2026-10-12）

利用者の「すべて移行」に従い、公開リポジトリは https://github.com/aruiki/Asteria-Japanese-IME 。originも変更済み。
説明/Topicsと23件のリリース表示名、README/使い方/公開工具/ソース内応援URLを移行。READMEと公開本文の構成・旧版の測定値は維持。最新版のWindows表示説明もv1.1.2/Asteriaへ修正。
Latest v1.1.2へAsteria64.msi/sha256を追加。既存Kotori64.msi/sha256は互換リンク用に保持し、将来workflowも両名添付。全タグ/リリースID/公開日/既存添付ID/digest保持、旧URL2種類の転送と両名ダウンロードHTTP200確認。
再取得/展開check_release全項目OK、MSIは6338・SHA a968312279d33c67679436d071d78409e6c74bdebf1824e475895fe9266e420eで内容不変・未署名。
Python14テスト、応援通知8テスト成功。Mozc作業ツリーでURL変更後パッチを再生成し全3適用成功。公開MSIの応援リンクは旧URL転送、正規URLを組み込むのは次回ビルド。
内部Kotori識別子/保存先/UpgradeCode/タグはADR0056/0058により保持。利用者の実状態・インストールを変更していない。
カード73、ADR0058、verification/asteria-public-migration-20261012.md。一般CI成功後にPRをmainへ統合する。実機入力/通知の確認は既存の未実施境界のまま。
以下は履歴。
# README図版の視認性を修正（2026-10-12）

利用者の小さすぎる指摘を受け、図版7枚を横880→640の論理幅にし、主要文字を24〜38pxへ。変換結果は17→28px、500px表示時は約9.7→21.9px（2.26倍）。太字と明るい文字を使用し、横詰めを縦のカード/棒へ変更。
Asteriaの紺/生成り/紙の質感を維持。README本文/公開本文/MSIは変更なし。設定・アイコンも今回の対象外。
旧Kotoriの生成コードを参照。6図版の元データはAST比較で完全一致、実入力比較は既存CSVを維持。
再生成成功。500px/360pxへ実縮小したPNGを目視確認。変換例のカード下端のはみ出しを修正して再描画済み。コードでも文字幅/下端の境界を検査。
カード72。プレビュー/データ照合補助はtarget/readme-readable-figures。公開はREADME画像のmain統合のみで、MSI再リリースは不要。
以下は履歴。

# Asteria v1.1.2正式公開・Windows通知版6338（2026-10-11）

利用者の「そのまま公開」に従い、6338ローカルMSIを再ビルドせずLatestとして公開。
URL: https://github.com/aruiki/Kotori-AI-Japanese-IME/releases/tag/v1.1.2 。release409720573。
PR153をmainへmerge commit e94a9b23b891ccc498ed121f51c8a19cb9ef539fで統合、タグも同じ。公開用head55a9c37とtree一致。
一般CI run38147576524は7ジョブsuccess。ローカルビルド指定でMozcクラウドrun38147576662はcancelled。成功扱いしない。
旧リリース本文の全文/書式を保持しv1.1.2説明のみ末尾追加。公開後も本文、タグ、MSI digest/size一致。

公開MSI3.34.6338.100、SHA a968312279d33c67679436d071d78409e6c74bdebf1824e475895fe9266e420e、1,569,386,496bytes、48ファイル、未署名。
公開後check_release.py全項目成功。再取得SHA、先頭3項目6336→6338、同梱物/版/サイズ/除外物を確認。
モデル/推論runtime22ファイルは前版とSHA一致。実利用者の案内状態を変更・削除していない。
Asteriaの月のデザインとREADME/図版、読みやすいMSI進行状況、Windows標準の応援通知を含む。

実導入・通知の実画面/通知センターからの操作・IME実入力・削除は未実施。公開ノートに明記済み。
次の作業は更新版の実機確認。記録verification/windows-support-toast-20261011.md、カード70/71、ADR0056/0057。
以下は履歴。

# Windows標準の応援通知へ変更・6338ローカルMSI完成（2026-10-11）

利用者は独立ウィンドウではなくWindows通知を希望。ToastGeneric＋COM activatorへ変更。通知センターから後日操作可能な構成。
GitHub固定URLを開けた時だけ永久停止、いいえは7日延期、閉じる/無応答も送信前保存した7日延期を維持。無音。OSの通知設定に従い、独自ダイアログへの代替なし。
既存support-reminder.iniの停止/延期を継承。実利用者のファイルは変更・削除していない。README/リリース本文/設定アイコンは今回差分なし。

8単体テスト成功。製品exe6338の-Embedding起動と実COMコールバック取得・不正引数拒否を確認。実状態の前後SHA一致。
初回テストとBazel出力直起動のDLL探索失敗は記録に保持。Qt環境を揃えた試験は成功。
package exit0、694.102秒。MSI3.34.6338.100、48ファイル、1,569,386,496bytes、未署名。
SHA a968312279d33c67679436d071d78409e6c74bdebf1824e475895fe9266e420e。
MSIのAUMID/CLSID/HKLM LocalServer32、UpgradeCodeを照合。Bazel停止後パッチ再生成、全3パッチ適用/127入力一致。
ローカル候補target/windows-support-toast/Kotori64.msi。カード71、ADR0057、verification/windows-support-toast-20261011.md。

未実施: 新MSI導入、Windows通知の実画面、通知センターからの後日クリック、ブラウザー実起動、OS通知抑制、IME入力/削除、push/CI/新公開。
次の具体的作業: 6338候補を導入しWindows標準通知の実表示と両ボタンを確認。既存延期を解除する目的で実状態ファイルをリセットしない。
公開中/導入済み版への変更ではない。以下は履歴。

# Asteria「月の余白」を反映・6337ローカルMSI完成（2026-10-11）

利用者が10案から05の月を採用。内蔵image_genでアイコンと横長ヒーローを生成し、READMEの文章と図版7枚を統一。
既存の章順・ダウンロード導線・Stripeリンク・測定値/条件を保持。比較グラフは公開CSVから描画。現在版の再測定ではない。
表示名Asteria、内部Kotori名と保存先/インストール先/GUID/UpgradeCodeは保持。設定/辞書の6アイコンは旧版と同一SHA。
進行表示は短い日本語12行、MSI ActionText/Templateに生の置換記号が残らないことを確認。

専用mzsupportでSystemUtil2件、設定画面4件、応援案内7件PASS。Qtはoffscreen。初回のDLL探索失敗はログに保持。
月の最終packageはexit0、629.508秒。MSI3.34.6337.100、1,569,370,112bytes、未署名。
SHA c5e688de8a7f6dead5d7a12728a416e161e843a364915ad485e43aeddcad357f。
配布用TIP32/64に月の8フレーム、配布用mozc_toolに従来の8フレームを資源のバイト列で照合。
Bazelを停止して全3パッチを再生成/適用確認。fresh cloneと127ビルド入力一致。
詳細 [検証記録](verification/asteria-moon-20261011.md)、[プレビュー](design/2026-10-asteria/brand-preview.html)、カード70/ADR0056。

公開中のLatestはv1.1.1のまま（API確認）。公開本文mozc/release-notes.mdは無変更。
新MSIの実アップグレード/進行実画面/日本語入力/削除、今回のpush/PR/CI/新リリースは未実施。
次はこの候補の実機受入れと公開準備。ローカル候補target/asteria-brand/Kotori64.msi。
前回の案内状態support-reminder.iniは利用者の明示許可後に削除済み、今回も不存在を確認。

以下は履歴。

# GitHub応援案内の正式版1.1.1を公開（2026-10-11）

利用者の実装・正式公開依頼。PR151、専用mzsupportからパッチ生成。READMEは無変更、ユーザーの正式版公開本文を全文保持。
リンクを開けたら永久停止、いいえ/閉じる/Escは7日延期。通常クライアントから独立ツールを起動し、ツール内で週次タイマーを管理。
サーバーには子プロセス禁止があるので、サーバーからの起動案は修正済み。sandboxを緩めない。
WindowsネイティブQtの7テスト・実foreground維持・日本語描画、全3パッチ適用が成功。最終MSIと関連9テスト対象が成功。内部版6336、同梱48ファイル、未署名。モデル3件/runtime18件は前の公開版と同一SHA。
最新のローカルビルド指定を保持。MozcクラウドCIはキャンセル、一般CIは最終入力c5c773dで全7項目成功。キャンセルをPASSにしない。
正式版1.1.1はLatestとして公開済み（release409577311、PR151、source b5113b1582d8be125f5eca0a305a52c0edcc23a9）。公開APIのMSI digest/sizeとタグを照合済み。新MSIの導入/実入力/削除は未実施。詳細verification/github-support-reminder-20261011.md。
実表示テストで作成したLocalLow/Kotori/support-reminder.iniの削除はauto-reviewが拒否し、利用者の明示許可待ち。入力設定/辞書/履歴は対象外。
公開URL: https://github.com/aruiki/Kotori-AI-Japanese-IME/releases/tag/v1.1.1 。公開版のMSI SHAはf8c6db1394a273f60590d82198372a68f2b6c83462bfc25a6c55a35c1480dba5。README差分ゼロと旧公開本文の全文保持を公開直前にも照合。

以下は履歴。

# v1.1.0正式版公開・6334導入成功（2026-10-10）

利用者の「残りトークンが少ないので軽く確認したら正式版」を優先。全buildローカル、再build/大規模試験は繰り返さない。
6334導入は利用者UAC承認後84.21秒、3010成功・再起動要求、自動再起動なし。設定/利用者辞書は同一。
core/モデル/推論runtime/辞書等43ファイルSHA一致。5 Microsoft runtimeは既存のより新しい版14.51.36247.0。
新版server+旧起動メモ帳でSpace1ちょっと待ってください、Space2ちょっとまってください、Tabも補正文を確認。
予測のSpace前補正、再起動後の新版TIP、CPU/全アプリ/24時間/削除は未確認。詳細verification/native-6334-20261010。

v1.1.0をLatest正式版として公開済み（release408766172）。公開後digest/size/source tag一致、小さい添付2ファイル再取得一致。
source e6e35a1、MSI3.34.6334.100、SHA7136ea32e39092ecdc7069cd0834e09d7fc0c2053fb89f3586410d5c8a7a599a。
未署名と今回の受入れ範囲を明示する。全GOAL/全受入れPASSと扱わない。以前の取消し/FAIL原票は保持。

以下は履歴。

# 6334導入許可あり・UACの実承認待ち（2026-10-10）

利用者が6334のこちらでの導入/検証を許可。MSI SHAを再照合して起動したが、UAC段階で128.32秒後に1602（取消し）、Credential Request=0x800704C7。
更新前に終了し、導入版は6333のまま。core4 SHAがRC.5一致、config1.db/user_dictionary.dbが保存コピーと同一、自動再起動なし。
詳細verification/6334-install-attempt-20261010-1540.md/json。私的ログと設定コピーはinstalled-6334-debug-20261010。
MSIログの「インストールされました」だけを成功と扱わない。6334の導入成功/実入力/正式版公開はまだない。

次: 利用者へ、UACを今承認できるか/自分でMSIを起動するかをasyncで質問済み。返答後に新しいログ名で起動し、旧ログは保持。
インストール全体の許可は取得済み。UACは手元の利用者による操作が必要。未返答をUAC承認と扱わない。
6334のローカル正式版案内文formal-release-notes-draft.mdを準備したが、GitHub draft/tag/published releaseは作成していない。
対象MSI/ソース/検査結果は前段と同じ。次は更新版照合とメモ帳実入力、その後に公開判定。

以下は履歴。

# RC.5の実入力を確認・6334導入の確認待ち（2026-10-10）

メモ帳でComputer Useの実キー入力が動いた。利用者のKotori選択後、新しいタブは英数へ戻ったが、編集欄でF13（IMEOn）により日本語化。
あしをいためた: Space前/Tab/Space1が足を痛めた、Space2が脚を痛めた。
ちょとまってください: Space前/Tab/Space1がちょっと待ってください、Space2がちょ止まってください。不具合を実画面で再現。
いんでっくす: 入力中にindex/indices、Space2でindex。設定の適用UXは前回実画面で確認済み。
導入先は引き続き6333。メモ帳PID32188のKotori TIPとserver PID23700、core4 SHAのRC.5一致を再確認。
詳細verification/rc5-native-input-20261010.md/json。元文書/履歴/スクリーンショットは公開していない。

新しい6334 MSIは前段の通り検証済み。正式版はまだ公開せず、6334のインストールを利用者へasyncで確認中。
Computer Useのインストール直前確認規約に基づく。返答なしを許可と扱わない。
次: 返答後に6334を導入し、稼働binary/TIPを更新版と照合して補正後Space2を実画面確認。自動再起動しない。
現在の実入力は一台/メモ帳/温まったモデル/各キー約3秒。CPUのみ/高速/冷起動や全アプリを保証しない。
古い未確認/FAILは当時の履歴として保持。

以下は履歴。

# 修正版6334ローカルMSI完成・正式版は実入力の確認待ち（2026-10-10）

ソース e6e35a101fc2849a50489f41e757a471427b39b9。BUILD6334、全buildローカル、package終了0（1035.5秒）。
MSI SHA 7136ea32e39092ecdc7069cd0834e09d7fc0c2053fb89f3586410d5c8a7a599a、1569173504bytes、48ファイル。
正本/cache/新規clone118入力/3パッチ一致、WiX内容検査とcompiled binaries/モデル/runtime/英語辞書/元画像一致PASS。
最終4テストと実モデル18例、Standard183/200=91.5%を維持。誤入力Space2を補正した読みの候補へ改善。
詳細verification/local-6334-installer-20261010、corrected-alternatives-6334-validation、tasks66/ADR0053。
成果物C:/Users/aruik/kotori-first-check/corrected-alternatives-6334-release/Kotori64.msi。

Computer Useは接続切れSMB割当てのMXC認証1326を診断し、利用者の一時解除許可で復旧。
保存済みの2接続プロファイルは保持、現在の割当ては一時切断中。通常sandbox読取りコマンドも復旧。
導入済み6333の実設定画面で未適用/適用成功/適用無効を確認し、候補数3→4→3を復元保存。
メモ帳はaが英数として確定し、利用者へKotoriの「あ」への切替えをasyncで依頼中。
未返答を許可/切替え完了と扱わない。Nativeの初回候補/Tabはまだ未確認。

公開最新版はv1.1.0-rc.5。6334を正式版や新RCとして公開した事実はない。現在の導入先は6333。
ローカル候補6334の自動導入/ユーザーの再起動はしていない。新MSIの実機導入/更新/削除も未確認。
次: 入力方式の切替え回答後にComputer Useの新しいWindowを選び、実入力の候補/Tab/Spaceを観測。
必要なら利用者が6334を導入し、公開候補の版/稼働経路/候補を再照合してから正式版を判定する。
過去の取消し/接続timeoutやCLI結果を実TSFのPASSへ書き換えない。私的な履歴・NAS割当先は公開しない。

以下は履歴。

# 補正後Space2の手直し・画面操作復旧（2026-10-10）

ユーザーの「完成なら正式版、必要なら手直し」に対し、残る誤入力Space2を改善した。
BUILD6334、正本mzcorrection/cache mzicons/新規clone118入力一致、全3パッチ適用PASS。
最終4テスト対象、実モデル18例、Standard183/200=91.5%を確認。
ちょとまってくださいはSpace2でちょっとまってください、補正前へ戻る問題を改善。
入力composer/元候補/ユーザー辞書・履歴を保持、同期AI追加なし、誤読への学習を停止。
詳細tasks66、ADR0053、verification/corrected-alternatives-6334-validation。

Computer UseのJS自体がMXCのSMBドライブ認証1326で止まっていた。
ユーザー許可後、現在の2割当てを一時解除し、保存済み接続プロファイルを保持して復旧。
実Program Filesの6333設定画面で未適用/成功/適用無効を観測、候補数を3→4→3へ復元保存。
ダッシュボードの数値/CPU推移/支援ボタンも観測。私的なNAS割当先/他のメモ帳内容は公開しない。
メモ帳は英数入力となり、Kotoriの「あ」への切替えを利用者へ依頼中。TSF候補はまだ未確認。
詳細verification/rc5-native-settings-and-computer-use-20261010。

次は6334のソースを固定し、ローカルでMSIを作成・版/内容/ソース/モデル/辞書を照合する。
Windows日本語欄の初回候補/Tabと新MSIの実機受入れを確認して正式版を判定する。
現在の公開最新版はrc.5。単に名前を正式版へ変えて完成扱いにしない。
ローカルbuild/専用Session、導入済みIPC、実画面を区別し、過去の失敗記録を保持する。

以下は履歴。

# rc.5導入成功・更新後サーバーと7例を確認（2026-10-10）

利用者の再導入後、11:36にWindows Installer成功（状態0）、11:37に再起動。
登録版/本体/設定アプリ/TIP/rendererは3.34.6333.100。稼働server PID23700も再起動後の導入先。
モデル/AI実行環境/大規模英語辞書を含む29ハッシュが公開rc.5 MSIと一致。
専用IPC7例、補正後/Tab/Space1の足を痛めた・医療文・料理・誤入力補正、英語3語の候補を確認。
AI/予測/補正ON、利用者のUnreal設定を保持。入力確定・履歴リセットなし。
誤入力のSpace2「ちょ止まってください」は残るため未解決。
導入した設定アプリも6333の保存UX修正版と一致。ただし実画面の保存表示はまだ観測していない。
再起動後のComputer Use初期化も20秒/回復30秒timeout。TSF/実画面は未確認を保持。
詳細docs/verification/installed-6333-20261010、原票kotori-first-check/installed-6333-debug-20261010。
次は実画面が確認できる経路でTSFの初回表示/Tab/保存表示を受け入れ、誤入力の別候補を改善する。
古い取消し終了原票は保持。公開済みと導入成功、専用IPCとWindows入力欄を混同しない。

以下は履歴。11:24の取消し試行後、利用者の再導入は成功している。

# rc.5導入試行は取消し終了・実機は6332を維持（2026-10-10）

利用者の「インストールしましたができてますか？」を受けて現在の実物を再確認。
11:24に正しいrc.5ローカルMSIの処理が開始されたが、11:27のMsiInstaller 11708/1033は失敗・状態1602。
Windowsの取消し終了（ERROR_INSTALL_USEREXIT）であり、取消しのきっかけは詳細ログ不在のため不明。
登録版とserver/tool/TIP64/rendererは3.34.6332.100。4ハッシュはrc.4と一致し、rc.5とは不一致。
稼働server PID23680も02:54起動の6332。確認時にmsiexecプロセスなし。
したがって6333設定保存UXの実機導入/表示は未確認のまま。再起動待ちの成功とは扱わない。
専用IPCで「あしをいためた」は補正後/Tab/Space1で「足を痛めた」、Space2で「脚を痛めた」、終了0。
AI/予測/補正有効。品質Unrealの設定を維持し、入力確定・履歴リセット・自動再インストールは行っていない。
原票はkotori-first-check/rc5-install-attempt-20261010-1124/installation-check.json。
公開用要約docs/verification/rc5-install-attempt-20261010.json。
次は利用者が既存の完全なrc.5 MSIで導入を完了後、6333の版/稼働経路と実UIを再確認する。
既存のMSIは公開SHAと照合済み。低速なGitHubから再ダウンロードする必要はない。

以下は履歴。rc.5公開とこのPCへの導入成功を取り違えない。

# 設定保存UX修正版rc.5公開・実機デバッグの続き（2026-10-10）

rc.5公開済み。https://github.com/aruiki/KotoriIME-japanese-/releases/tag/v1.1.0-rc.5
ソース/tag4b50257746f8de22a9cc27f9ec1072e1a07e3461、MSI3.34.6333.100。
SHA563045647e34f21edec2fdb6643783ed041bfbea4d66844f4dc8847739d245be。
全buildローカル、package610.344秒終了0、48ファイルの内容/版/ソース/モデル等一致PASS。
未適用/保存中/成功/失敗を全タブ共通表示し、保存失敗時の変更と再試行を保持。
初回計測中のダッシュボードも空欄にしない。Windows Qt4テストと100/150/200%確認PASS。
117入力/3パッチ一致。モデル/候補/辞書処理は6332から不変。精度は前の記録91.5%として保持。

公開資産5件のGitHub digest/size一致、小さな4件は再取得一致。
MSI完全再取得は低速のため利用者の軽いチェック指定に従い停止。完全再取得PASSとは書かない。
詳細docs/verification/rc5-settings-release-20261010とADR0052。
main READMEの最新試用版リンクをrc.5へ更新済み。共有workflowは停止していない。

導入済み実機は利用者が導入した6332。29ファイル一致、稼働サーバー実設定を保持してIPC7例確認済み。
足/医療/料理と英語候補は確認。誤入力のSpace2には未補正候補の残存例。
今回の設定UXは6333に含むが、まだ利用者の導入確認なし。自動で導入先を上書きしていない。
Computer Use初期化は規定の回復後もtimeout。Windowsアプリの実際のTSF表示/候補フォーカスは未確認。
次は利用者の6333導入後、版/経路を再照合し、実入力と保存表示を確認する。
入力履歴・ユーザー辞書の原本はリセットしない。全要望完了/製品完成とは扱わない。

以下は履歴。公開前の状態と今回の公開完了を取り違えない。

# rc.4公開・6332導入済み本体の調査と設定UX修正（2026-10-10）

rc.4は公開済み（プレリリース、Latestではない）。
https://github.com/aruiki/KotoriIME-japanese-/releases/tag/v1.1.0-rc.4
ソース3a962f4554bf8edfb72afc8872835ecd3df43df0、MSI3.34.6332.100。
SHA2e88913147adbe49da5f32624590980b11a938ed58872fc0235d0206b4467e17。
公開後4資産を再取得しハッシュ一致。publication.jsonを追加。main READMEもrc.4と大規模辞書へ更新。
利用者が導入・再起動を完了。29ファイルが公開MSIと一致、稼働サーバーPID23680と導入パス照合。
専用IPCで足/医療/料理/誤入力/英語3語の7例確認。AI/予測/補正有効、品質Unrealは実設定を保持。
足を痛めたが予測/Tab/Space1で一致。脚を痛めたがSpace2。生の履歴や入力イベントは公開しない。
ただし誤入力Space2には「ちょ止まってください」の別候補が残る。
Computer Use新version初期化は規定の回復後もtimeout。WindowsアプリのTSF表示は未確認。

設定適用の無表示、保存失敗でもOKが閉じる問題、保存ボタンの再度未保存化を修正。
未適用/適用中/成功/失敗の常設表示と再試行を実装。初回計測中のダッシュボードにも案内を表示。
Windows Qtの非表示コンポーネント4テスト、100/150/200% PASS、製品tool build PASS。
設定サーバーmockと一時プロファイルを使用。導入6332にこのUX修正はまだ含まれない。
詳細ADR0052、docs/verification/installed-6332-and-settings-feedback-20261010。

正本mzcorrection/cache mzicons/新規clone117入力一致、全3パッチ適用PASS。
モデル/候補/辞書のコードは6332から不変。版と設定GUIのみ変更し、BUILD6333。
次はソース固定→全てローカルで新MSI作成→内容/版/出所照合→別候補rc.5を公開・再取得確認。
Windows実入力・削除、冷間全補正、全要望完了は未確認として保持する。

以下は履歴。公開前の作業順や6331の失敗を現在の状態と取り違えない。

# BUILD6332のローカルMSI完成・利用者指定で公開を先行（2026-10-10）

利用者の最新指示:「終わったらリリースしてください。その後実機にいれるので実機デバッグを開始してください。」
以前の実入力確認前に下書きを公開しない作業順序を、今回はこの明示的な指示で更新する。
Windows実入力の未検証をPASSへ書き換えず、公開済み候補としてその境界をリリースノートに記載する。

ソース3a962f4554bf8edfb72afc8872835ecd3df43df0。117入力/全3パッチ/新規clone一致PASS。
全ビルドはローカル。MSI622.4秒、終了0。ProductVersion3.34.6332.100>6331。
同梱48ファイル、辞書TSV/source/NOTICEの数・出典・ハッシュ、モデル3件/実行環境18件の同一性PASS。
サイズ1,569,157,120bytes、未署名。
MSI SHA-256: 2e88913147adbe49da5f32624590980b11a938ed58872fc0235d0206b4467e17。
完成ファイルC:/Users/aruik/kotori-first-check/rc4-release-20261010-3a962f4/Kotori64.msi。
原票は同ディレクトリのsource-manifest.json、local-build.json、release-evidence.json。

英語辞書31,068読み/36,402組、英語17テスト、実モデル+既存履歴コピー18例PASS。
Standard183/200=91.5%。詳細ADR0050/0051とrc4-installed-history-english-local-20261010。
6330/6331の実入力FAILは保存。AI評価を履歴が上書きする問題とTab取消し後の補正消失を修正。
普段の履歴・ユーザー辞書は変更していない。生の履歴コピー/入力ログは公開しない。

次はGitHubのrc.4下書きへ新MSI/ハッシュ/原票を差し替え、APIのdigestとsourceを照合して公開する。
公開後に再取得したMSIのハッシュを確認する。実機への導入は利用者が行うと指定している。
その後、このPCの導入版・稼働PID/TIP経路・実際の候補を調べる。全要望完了/Windows実入力PASSとは扱わない。

以下は履歴。公開順序については冒頭の最新の明示指示を優先する。

# 実環境の履歴による誤変換と英語辞書の拡張（2026-10-10）

BUILD6331は利用者の実入力で「あしをいためた」→「脚を炒めた」のFAIL。
本PCの稼働server/TIP/モデル/実行環境は6331MSIと一致。AIは痛めたを選んでいたが、
後続の文節履歴が炒めたを先頭へ戻していた。履歴の原本を変更せずコピーで再現・修正。
AI評価済みの先頭を履歴から保護し、AI未使用時の学習と別候補を保持した。
Tab→Esc→即Spaceで補正が消える別件も、同じ予測入力だけ世代を保つ専用取消しで修正。
詳細ADR0050。回帰は修正前FAIL→修正後PASS、関連7ターゲットPASS。

英語辞書を24手書き表記から31,068読み・36,402組へ拡張。固定公開辞書をbuild時に取得・加工し、
入力時は静的配列を検索する。CC BY-SA 3.0の出典・変更内容とTSV/metadataをMSIへ同梱する。
英語全17テストPASS、実モデル+既存履歴コピー18例（英語12、補正/医療/料理/ニュアンス6）PASS。
正式英字accessibilityが略語a11yの奥に埋もれる失敗も修正済み。詳細ADR0051。

正本C:/Users/aruik/mzcorrection、cache C:/Users/aruik/mzicons、BUILD6332。
Standard200文は183/200=91.5%で90%条件PASS。117入力の正本/cache/新規clone全一致、3パッチ適用PASS。
入力マニフェストSHA-256: 6b7c8aa90af32786e7bffa2dcbf6ce1c4041000b78dad182666abf07afb822c8。
新しいコードのソース固定・ローカルMSI作成へ進む。
原票C:/Users/aruik/kotori-first-check/history-english-cancel-final-v3.jsonほか。
Bazelを終了してパッチを再生成済み。次はソースcommit→ローカルMSI→内容/出所検査→実入力受け入れ。
6330/6331の失敗候補は保存し、rc.4は下書きを維持。新MSIの導入/TSF実入力/削除は未検証。
Computer UseのInitializeは規定の回復を含む複数回timeoutのため同じ試行を反復していない。
公開権限は既にある。検証済みCLI/MSIとWindows実入力の証拠を混同しない。
私的な学習履歴、ユーザー辞書、生ログはGitへ入れない。

以下は過去の記録。BUILD6331の「実入力確認待ち」は今回のFAILで更新された。

# rc.4修正版MSIの完成・実入力確認待ち（2026-10-10）

ソースb217ad4197db2d7ad1594ae98aae4d21398e65b9を固定し、全buildをローカルで実施。
MSIは608.637秒で終了0。内容の読み取り専用展開・112入力/3パッチ・同梱モデル/実行環境・本体の同一性PASS。
ProductVersion3.34.6331.100で6330より大きい。45ファイル、1,568,374,784bytes、未署名。
SHA-256: ed1a3ed93ebd3c81dd8ebdfe39509ad7cc40c0f3cba01bd2d064d73c804cb575。
完成ファイル: C:/Users/aruik/kotori-first-check/rc4-release-20261010-b217ad4/Kotori64.msi。
原票: 同じフォルダのlocal-build.json、source-manifest.json、release-evidence.json。
詳細: docs/verification/rc4-medical-fix-local-20261010.md。

初回/Spaceの文脈統一、無関係な同音語の優先昇格の撤回、Windows更新先の検査を修正。
Session190/Nuance9/Engine86、実モデル5例、準備後の即Space2例、特徴/特長の長文PASS。
修正後Standard183/200=91.5%。これはCLI/MSI内容の証拠であり、Windows実入力PASSではない。
モデル読込み中の即Spaceでは通常変換に戻り「炒めて」が残りうる。
Windows Computer Useの規定の回復を含むInitialize3回のtimeout以後、同じ試行は繰り返していない。
利用者へ具体的なMSIと今回の文の初回/Space1/Space2の確認を提示。結果は未回答。
rc.4は下書きのまま。今回の新MSIの実入力確認後に公開する。公開権限は既に利用者から与えられている。

以下はビルド前と失敗候補の履歴。

# rc.4修正版のローカルMSI作成へ（2026-10-10）

6330の実入力FAILを受け、無関係な同音語の昇格、初回/Spaceの文脈不一致、Windows更新先の検査を修正。
Session190/Nuance9/Engine86 PASS。実モデルの医療4例/料理1例、準備後の即Space2例、元の特徴/特長もPASS。
Standard AJIMEE183/200=91.5%。モデル未準備の即Spaceを全補正できたとは扱わない。
112入力の正本/cache/新規clone全一致、全3パッチ適用PASS。BUILD6331。
詳細: docs/verification/rc4-medical-fix-local-20261010.md。
ソースをcommitで固定しローカルMSIを作成する。rc.4はdraftのまま、同MSIの実入力確認後に公開。

以下は6330の失敗とその前の履歴。

# rc.4実入力の不合格と修正（2026-10-09夜、カード64）

利用者がBUILD6330を導入し、初回「脚を炒めてしまったため今日は医者に行こう」、
別結果「脚を炒めてしまったので今日は慰謝に行こう」を報告。実入力は利用者観測のFAIL。
入れたserver/tool/TIP64のSHA-256は25cd371のMSIと一致。新規serverでStandard/LLMが有効。
GitHubのrc.4はdraftのまま。4資産をアップロード済みだが、この候補を公開しない。
「ため/ので」が別入力かSpace回数かは確認待ち。勝手な接続詞変更が起きたと断定しない。

## 原因と進めている修正

- 再現: 「ため」「ので」の両方で、準備済みのAIは痛めて/医者を選ぶが、Space2で慰謝へ。
  一般の複数漢字の同音候補を文全体の優先候補へ昇格していた。説明できる語組だけを優先し、
  他のMozc候補は各文節の通常一覧に残す。医者/慰謝/異車/慰藉の負例と既知語組の複数文節を回帰確認。
- 初回/Spaceの文脈不一致: TSFタイマーは文脈を再送しない。取り直しだけ空の文脈を採点していた。
  Spaceが使う入力開始時のclient_contextを、Suggest/Previewの両方へ渡す。
  新規回帰テストが修正前FAIL（前後の文が空）、修正後の全Sessionを検証中。
- Windowsのタイマーのフォーカス検査をGetBaseからGetTopへ。編集セッション実行時も再検査。
  複数コンテキストの入力欄で正しい更新を取り消さず、待っている間にフォーカスが変われば更新しない。
- cold即SpaceはAIのロード中に通常変換へ戻り「炒めて」が残ることを再現。
  準備済み/モデル待ちを区別する。実入力の初回失敗をすべて説明・解消したとは扱わない。

証拠: C:/Users/aruik/kotori-first-check/user-medical-*.json、medical-context-regression-before.log。
利用者の設定/辞書/履歴/稼働IMEは変更していない。次のMSIは更新できるようBUILD6331以上にする。
修正後の実モデル、局所の回帰、Standard90%維持、パッチ再生成、ローカルMSIを順に行う。
新しいMSIの同じ文で実入力を確認してからrc.4を公開する。現在の6330は不合格候補として保存。

以下はその前に完了したローカル検証と配布準備の履歴。

# 全改善案の統合とローカル配布準備（2026-10-09、カード64）

利用者は全要望の改善後にリリース、全build/評価をローカルと指定。
実リポジトリは.local/kotori-readme、枝codex/complete-user-improvements。
正本C:/Users/aruik/mzcorrection、cache C:/Users/aruik/mzicons。112入力が全一致。
make_patches.shをBazel shutdown後に実行、新規cloneの0001〜0003適用と112ファイルの同一性PASS。
詳細はdocs/verification/all-user-improvements-local-20261009.mdと同名JSON。

## 今回の結果

- Space前のAI評価・局所生成補正を既定有効。読みは即時、候補は非同期・時間上限あり。
- 一字の読み修正を元の文へ投影し、漢字/名前/数値/文法/世代を守る。LM50ケースPASS。
  「ちょと待ってください」→「ちょっと待ってください」が初回約1017ms、直接Space約1.2msで同じ文。
  誤入力10文の期待一致3→4、全件の補正ではない。
- 予測29文、続き8、最終表示/Tab差0、初回本文の後に続きへ更新する差は2件。初回/最終を区別。
- 正常100文の共有補正関数で変更0。100文OFF/ONの生記録もSpace100/100・生成採用0、
  ただし集計が不正UTF-8ログで失敗した記録を保存。予測UTF-8拒否を修正し単体/実モデルを確認。
- ユーザーの長文がAIで7文節になる場合にSpace2が文頭へ戻る失敗を修正。
  FIXED_BOUNDARYの読み/境界は保持し意味候補を提示。Space2で特長はへ。他部分不変。Nuance9/Engine86 PASS。
- index/effect/internetが初回一覧/Tab/Spaceへ。F6〜F10を保ち文字種の末尾表示だけ除去。
- Kotoriダッシュボード、Windowsの操作部品、100/150/200%でGUI2ケースPASS、描画確認。
- 最終Standard AJIMEE183/200=91.5%、heldout289/300=96.3%。heldoutを調整に使わない。
  最後の境界候補修正前のEXEによる品質原票。後続は先頭選択/モデル/辞書を変えていない。
- 開発機をCPU強制してcold10文字/cold長文/warm即Spaceの3セッションが正常終了。
  Space約1.2/5.7/249ms。実際の低性能PCの受け入れ証拠ではない。

## 次の作業と公開境界

ソース25cd371を固定し、MSIを949秒でローカルbuild、終了0。BUILD6330、前のrc.3は6322。
19:23の旧MSIはその後の生成/予測修正前なので使わない。
新MSIの版3.34.6330.100/同梱/ハッシュをWiXの読み取り専用展開でPASS。
check_release.py --localの管理展開は5分以上止まったため検査clientのみ停止、失敗は保存。
最終SHAは86dc529a3d1ce6e10f0a51dfd7e033b3620eea9b3c8373daf9004e0b8c4094ec。
完成ファイルはC:/Users/aruik/kotori-first-check/rc4-release-20261009-25cd371/Kotori64.msi。
最後の境界修正後もStandard183/200=91.5%。112入力hashを再確認。
Windows Computer UseのInitializeは規定の回復を含む3回ともタイムアウトし停止。
最終MSIの実アプリ入力・上書き更新・削除は未実施。原票/Qt/MSI展開をTSF PASSへ置き換えない。
成果物ができてから具体的な残りの受け入れを提示する。未確認のまま公開済み/全完了と言わない。
ソース25cd371をGitHubの同名開発枝へpush済み。新しいPR/公開リリースはない。
公開準備のrc.4 draftへ最終MSIを保存する。draftを公開済みと扱わない。
共有workflowは停止せず、PRでcloud buildも起動しない。
普段使いのIME/登録/設定は変更していない。

以下は以前の対象ソースと実測の履歴。

# 最終ソース・成果物の固定（2026-10-09、ローカル）

カード63の最後の字下げ修正後にLM44ケースと本体3ターゲットbuild2.189秒を再確認。
最終EXE517ea9c3426370004b359aaec655d49d9253c3df678c16865ebc2413edfad151で固定10文の
OFF/ONとも初回・Tab・Space10/10、足の生成補正の初回/Space一致を再確認した。
最終源とbuildの全107ファイル一致、全3パッチの新規clone適用、0003不変を再確認。
整形前の実コードセクションは同一だが、CPU/挨拶等の旧成果物の原票も区別して保持する。
検証JSONの最上位が最終成果物。その他の残件・ローカル限定の指定は下の記録を継続する。

# ローカル開発へ切替・初回補正とTab本文保持（2026-10-09、カード62・63）

利用者はbuild等をすべてローカルで行うよう指定。改善案を再確認し、最優先の初回補正から開発を継続した。
ローカル枝codex/first-display-semantic-safety（e91784a）に文法・代名詞の変更拒否を実装し保存。
通常設定の同一EXEでOFF/ON各100文、初回とSpaceは両方100/100一致。正常文の補正誤採用0。
数字・固有名詞・代名詞・未変更語を含む実辞書対照6例、LM38テスト。初期の名詞接尾語の過剰拒否と失敗は履歴として残す。

その上のローカル枝codex/tab-first-display-consistencyにTabの本文保持を追加。
世代・モデル・設定・読み・文脈を再検査して初回評価済みの文をTabへ使い、本文を保つ強い続きだけ先頭を置換できる。
未評価の数字を許可する変更や追加の同期推論はない。弱い予測・別解は一覧に保持。
関連LM44・Session189・Engine84・モデル識別6はローカルPASS（変更なしのEngine/識別はBazelキャッシュ）。
最終session/converter/shared-correctionのbuild 5.800秒。固定10文はOFF/ONとも初回/Tab/Space一致10/10。
CPU強制2文の入力維持、最終EXEの制御例で足の生成補正の初回/Space一致を確認。
専用源mzcorrectionとbuild源mziconsの全107ファイル一致、全3パッチの新規clone適用、0003不変を確認した。
詳細と実測ハッシュは `docs/verification/2026-10-09-correction-grammar.md/.json` と `2026-10-09-tab-consistency.md/.json`。
通常100文は文法保護時点のソース。最終Tab変更後は10文で、100文を再実行した記録ではない。

残件: 強い続き予測の初回表示/Tabへの実モデル受け入れ、予測全体の適合率/表示率、全般の意味の保護、
低性能PC、実アプリTSF、MSIの導入/削除・配布の受け入れ。自由生成は既定無効を維持し、製品完成とは扱わない。
「おはようございます」は生成されたが、初回には採用されなかった。その条件を調整せず原票に残した。
次はこの残件に近い実装障害を調べる。heldoutは閾値調整に使用しない。実入力とCLIの証拠を区別する。

GitHubの当方の実行中MSIジョブ37892743123・37887768081はcancelledと確認した。
4 workflowの恒久停止は自動承認審査に拒否され、設定は変更していない。新たなクラウドbuildやpushはしていない。
前の「次はCI/MSIを待つ」は以下の履歴。再開時も利用者のローカルbuild指定を優先する。
公開版rc.3へ今回の差分は入っていない。ダッシュボードPR147・原源mzappnameは別に保持し、今回の源へ混ぜていない。

以下は以前の記録。

## 文中のニュアンス候補（2026-10-09）

カード60/ADR0045。関連単体・実辞書CLIで2回目Spaceの「特徴→特長」と位置26への焦点を確認。
最初の候補とほかの文節は保持。7組の有限な意味の選択肢で、LLM判断/意味まとまりの再分割は未完。
PR145依存の別ブランチ、CI/実アプリTSFは未実施。公開rc.3は変わらない。
次はKotoriの名称統一と、利用者依頼のAI/PC状態ダッシュボード・Stripe支援導線。

以下は過去の対象ソースと実測の履歴。

## 実モデル識別の追加（2026-10-09）

SHA-256と読み取り共有ハンドルをモデル所有者へ対応付け、保存・再取得・適用で不変の識別を照合した。
Windowsファイル6ケース、LM31ケース、Sessionテストと本体CLI buildが成功。
同一最終EXEの制御OFF/ONと取消/保護欄等の拒否を確認。詳細は
`docs/verification/2026-10-09-llm-model-identity.md` / `.json`。
既定無効。意味の保護、正常文100件の実セッション、CPU/Tab/実アプリは未完。
更新後CIはPR145で確認する。リリース名は全18件の公開名変更済み、README/自動命名はPR146。
新しい要望: 文中のニュアンス候補を優先する選択（特徴/特長）、設定画面とショートカットの名称をKotoriに統一。

以下は以前の対象ソースと実測の履歴。

# 入力世代の実装と英語候補の統合（2026-10-09午前、カード53・54）

ユーザーの「開発を続行」に従い実装を再開。公開済みrc.3は保持。
英語候補PR140は全8チェックとMSI導入/実入力/削除を確認し、merge commit40b7bccでmainへ統合済み。
カード53は完了。対応は24表記、指定3語の初回/Tab/Spaceを確認。rc.3には含まれない。
LLM補正は `codex/llm-correction-guards` / Draft PR145。baseをmainへ変更した。

## この変更で確認したこと

- EngineConverter所有の入力世代をConversionRequest/非同期ジョブへ渡す。編集・確定・取消・設定/Request変更・破棄で失効。コピーは別所有者、候補更新とSpaceでは同じ世代。
- 自由生成の保存時/適用直前に所有者・世代・読み/文脈/設定・モデルロード世代を確認。保護欄切替時も即失効。空の初期Requestからの変更漏れを修正。
- 関連13テスト追加、Request/Engine/LM/Sessionの4ターゲットPASS、LM合計29ケース。converter/session build、Python7ケースPASS。
- 同一最終EXEの制御試験で生成OFFの初回「炒め」がONの「痛め」へ変わりSpaceと一致。ON初回848.6ms、Space1.9ms。試験用の候補制限条件であり既定設定・実アプリTSFの受け入れではない。
- 実セッションでコピー後の補正保持、取消・AI無効化・保護欄・シークレット切替後の更新拒否を確認。
- 自由生成OFFのAJIMEE183/200（91.5%）、直前と200件の出力一致。indexの初回/Tab/Spaceも維持。
- パッチ再生成と新規cloneへの0001〜0003適用・実ソース空白検査成功。入力世代関連12ブロックだけ変更し、C案/英語候補/同梱パッチを保持。CIのテスト対象にRequest/Engine/Englishも追加。

対象ハッシュと正確な範囲は `docs/verification/2026-10-09-llm-input-generation.md` / `.json`。
実ソースは `C:\Users\aruik\mzicons`、原票・退避は `C:\Users\aruik\kotori-first-check`。
Bazelは検証後shutdown済み。普段使いIMEへインストールしていない。

## 次の具体的作業

1. 更新後PR145のCI/MSIを確認する。以前のhead011abf5のCIは新しい入力世代コードの検証ではない。
2. モデルロード世代に加えて実モデルSHA-256を補正へ対応付ける。意味の肯否/時制/主体の保護と正常文100件の実セッション試験を行う。
3. CPU/低性能環境・実アプリTSF・Tab高確信度予測・設定画面は未完。生成補正を既定有効や完成としない。

以下は同日の前段階とそれ以前の履歴。

# 開発再開の実装記録（2026-10-09午前、カード53・54）

自由生成した補正文がSpace前の最初の候補へ反映するところまで試作した。既定無効、rc.3には含まれない。
枝は `codex/llm-correction-guards`。英語候補PR140のhead236ddbfを依存として保持し、補正は別PRにする。

## 完了した実装と確認

- ビーム1のコピーだけでなく、ビーム2で同梱ベースモデルが足の例の修正文を生成。モデル置換・閾値引き下げはしない。
- 非同期ワーカーと実辞書/実モデルCLIで採用処理を共有。数値/単位、UTF-8、辞書の読み、保護語と有限スコア差5を検査。
- 「ます」「行」の人名としての別解が無関係な補正を止めていた。保護語の変更範囲との交差を両文で調べ、未変更語を保持する。保護・分割24テスト成功。
- 実セッションでは逆引きで辞書全体を繰り返し走査し時間切れ。Mozc既存の文単位キャッシュを使って解消した。
- 同一バイナリの制御試験（文候補1件/zenz生成0/続きと旧誤入力補正を無効）で、生成OFFの初回「炒め」がONの初回「痛め」に変わり、Space後も一致。既定設定や実アプリTSFの受け入れではない。
- 共有採用処理の正常文100件は誤書き換え0、時間上限超過0。統合後の4ターゲットbuildと関連5ターゲットtest、新規cloneへのパッチ適用は成功。
- 同梱モデル・補正OFFのAJIMEEは183/200（91.5%）。前のベースモデル結果も183/200。自由生成ONの精度評価とは区別する。
- 英語候補indexの初回/Tab/Spaceは統合後も成功。C案資源と同梱パッチを保持。

詳細とハッシュは `docs/verification/2026-10-09-llm-generated-correction.md` と同名JSON。
ローカル原票は `C:\Users\aruik\kotori-first-check\llm-*20261009*`。
元の試作ソースはmzfi、配布資源と統合したソースはmzicons。前文と過去の失敗実測は下に履歴として保持する。

## 未解決・次の具体的作業

1. 自由生成の結果にセッション/入力世代と設定・実モデル識別子を対応付け、保存時と表示時の両方で一致を検査する。読みが同じというだけで古い補正を使い回さない。
2. 確定・追加入力・保護欄・AI無効化・モデル設定変更、正常文100件を実セッション経路で評価。CPUと実アプリTSF、Tab高確信度予測も未完。
3. 更新後のCI/MSI成功まで試作PRを統合しない。英語PR140は更新後のMSIビルド37861976522を待ち、全8チェック成功後にmerge commitで統合する。
4. 自由生成は `KOTORI_LM_CORRECTION=1` と非同期処理の場合だけ。既定有効・完成・公開済みとは説明しない。設定画面構成はカード57。

以下は再開時点とそれ以前の履歴。

## main統合時の記録

# リリース名と導線の整理（2026-10-09、カード59）

全18件の公開済みリリースを版・段階・JST公開日の順に改名した。
タグ・本文・公開区分・添付ファイルの同一性を前後のAPI取得で照合済み。
rc.3はリリース候補のまま、Latestは正式版v1.0.0。検証は `docs/verification/2026-10-09-release-names.md`。
READMEにrc.3のMSI直接リンクと正式版へのリンクを併記し、両workflowの命名を共通化した。
この枝は候補表示・モデル・MSIを変更しない。統合状態とCIはカード59のPRで確認する。

英語候補PR140はmainに統合済み（40b7bcc）。第二段階のLLM生成補正はDraft PR145、
head 276efb8733cca08e87ceedc13c0d0907fb5ce42a。MSIを含む8チェック成功を確認した。
補正は既定無効。モデル同一性、意味の保護、通常文のセッション試験、CPU・実アプリ確認が残る。
リリース案内の反映後、PR145の開発を続ける。

以下は過去の記録。

# 開発再開（2026-10-09朝、カード53・54）

ユーザーが「おはようございます。開発を続行」と指示。残りの開発を再開する。
公開済みrc.3は保持。英語候補PR140へ最新mainを統合し、C案の資源と公開記録を維持する。
英語の3ソースとBUILD依存だけが検証済みPR140と一致し、それ以外のパッチがmainと同じことを照合。
初回表示・Tab・Spaceへの英語補助候補を再確認し、更新headの全CI成功後に統合する。
LLM自由生成補正は別の既定無効の試作。正常文への誤修正と足の例を修正しない原因を調べる。
最優先は初回表示の補正と採用条件を満たす続き予測。設定画面はカード57。

以下は前回終了時とそれ以前の記録。

# 本日の公開と終了（2026-10-09、v1.1.0-rc.3）

ユーザーの「最新版をリリースして本日は終わる」に従い、rc.3を公開した。追加の機能開発は行わない。
公開URL: https://github.com/aruiki/KotoriIME-japanese-/releases/tag/v1.1.0-rc.3
未署名のプレリリース。Latestはv1.0.0のまま。

PR142はMSIを含む全8チェック成功後にmerge commitで統合。公開準備PR143も全7チェック成功後に統合。
公開タグcommitは18e656368e633fa696e39919c49ceca14ab4ffae。
CI37809776639の導入・TSF実入力・削除で確認したMSIを、ADR0044の条件を満たしてそのまま公開した。
公開後check_release.pyで取得し直して終了0。SHA-256は4b95dc0cfbe9cbb642cf21fd33c419e636262a2942c3b0a2921fc1fed405b056。
版は3.34.6322.100（rc.2の3.34.6313.100より大きい）。同梱物・容量・server版・RPC部品なしを確認。
公開MSI内の8サイズのアプリアイコンはC案素材と一致。モデル3個はrc.2とハッシュ一致。
検証と出所は `docs/verification/2026-10-09-rc3-release.md` と同名JSON。
普段使いのIMEは変更していない。実アプリ/GPU/CPU/倍率を網羅した受け入れは未完了。

未完の英語候補PR140、mzfiの既定無効LLM補正試作、カード57の設定画面構成変更は含めていない。
未追跡の `docs/adr/0042-llm-correction.md` と `docs/tasks/54-llm-correction.md` は保持し、公開記録PRへ混ぜない。
次回は最新main・PR140・試作の状態を読み直してから残りの開発を再開する。本日はここで終了。

以下は過去の記録。

# 今回のリリース準備（2026-10-09、カード58）

ユーザーが「最新版をリリースして本日は終わる」と指示。公開後は追加の機能開発をしない。
対象はrc.2の改善＋C案のアイコン・WiX背景。英語候補PR140、LLM補正試作、設定画面の構成変更は含めない。
`codex/origami-release`はPR142（a544514）の上に文書だけを準備する枝。
PR142の全CI成功・統合後にbaseをmainへ変え、文書CIを確認して統合。
ADR0044の条件を確認し、導入・実入力・削除まで検証したCIのMSIを同じファイルのままrc.3へ公開する。

ローカルMSIは1391.631秒（約23分12秒）でbuild成功。
check_release.py --localで同梱物・容量・ggml-rpcなし・server/MSI版一致を確認し、終了コード0。
SHA-256は462a9dd1000f05da269d72adc91c053aa12d233087618eabe8e6f0167756a790。
ただし版は3.34.6239.100で公開rc.2より古いため、これは開発確認用で公開しない。
モデル3ファイルはmzからコピーし、前後のハッシュ一致を確認。普段使いのIMEへは導入していない。
記録は `docs/design/2026-10-kotori/integration/local-msi.json`。
公開用CI37809776639はQt完了後、本体のbuild中。通常7チェックは成功。

次はPR142のCI結果、統合、CI成果物の出所・ハッシュ・版の確認、rc.3公開、公開後check_release.py、記録して終了。

以下は過去の記録。

# 現在の作業（2026-10-09、C案の本体への組み込み）

ユーザーが「続けて」と指示。素材PR141はCI7チェック成功後にmainへ統合済み。
`codex/origami-integration`、カード56でアプリ・言語バー・設定・辞書・WiX背景をC案へ更新した。
別ツリー `C:\Users\aruik\mzicons` はmain 0d4461fのパッチだけを適用したもの。LLM試作は混ぜていない。
make_patches.shで0002・0003を再生成。新しいcloneで全3パッチの適用成功、変更ブロックは画像10ファイルだけ。
実際のアプリ/TIPのWindows資源2ターゲットは35.347秒でbuild成功。
旧Resource Compiler 10.0.10011.16384でも、全8サイズ32bit BMPのICOを受け付けた。
小鳥4資源は素材ICOとバイト一致。入力モード、使用許諾、NOTICE、変換コードは変更なし。
見本とハッシュは `docs/design/2026-10-kotori/integration/`。
設定ツール本体も269.019秒でbuild成功。実行ファイルの全8アイコンがC案素材と一致。
ツールは資源として読んだだけで起動していない。次はMSIを含むCI確認・統合。
公開済みrc.2と普段のIMEは変更していない。設定画面の構造変更はカード57。

英語候補PR140はMSI・導入・実入力・削除を含む全8チェックが成功（37797232942）。
現mainとの文書の競合で未統合。C案の統合後に最新mainとの競合を解消し、資源変更を失わずに統合する。
LLM補正はmzfiに保持し既定無効。21単体ケースPASS、生成CLI/converter/sessionのbuild成功。
正常文100件の生生成は変更なし68・許容表記1・その他変更27・空4。実際の採用ガード前の結果。
自由生成の初回候補採用と安全性は未検証。ローカル未追跡のカード54・ADR0042に最新結果を記録した。

以下は過去の記録。
# 現在の作業（2026-10-09）

最新の依頼は画像生成を使ったデザイン改善。3案のうちユーザーがC（折り紙の小鳥）を選択した。
`codex/visual-redesign` でC案の生成画像、通常/縮小用SVG、ICO8サイズ、PNG10サイズを準備。
`docs/design/2026-10-kotori/README.md`・ADR0043・カード55を参照。
アルファとPNG/ICOの画素一致、全フレームBMP、SDK10.0.26100.0の資源コンパイルを確認した。
まだMozc資源、公開MSI、インストール済みIMEへ適用していない。UI生成画像は動作画面ではない。

ユーザーは「もうしてあるなら良い。引き続き開発」とrc.2公開済みを了承した。新規リリースを先行させる依頼は解消済み。
rc.2のソースはf8284f51、MSI 3.34.6313.100、ハッシュは下の履歴に記載。新しい版は公開していない。
英字候補PR140（head 2dbc363）は通常7チェックPASS、MSI CI37797232942は確認時点で実行中。
CIが出そろって成功してからmerge commitで統合する。デザイン素材とLLM試作は混ぜない。

LLM生成補正は `C:\Users\aruik\mzfi` の試作で既定無効。同期打鍵経路には追加しない。
読み付き例示モードと生成用CLI、数値・固有表記・ユーザー辞書・未知漢字のガードを追加中。
同じ実行ファイルでのモデルA/Bは9セット535件でベース478正解、Instruct471正解。単純置換は採用しない。
詳細な実験結果は `C:\Users\aruik\kotori-first-check\model-ab\comparison.json`。
正常文100件を固定済みだが、生成補正での安全性・初回表示・CPU/GPU性能は未検証。
この試作を含むmzfiからデザイン用・配布用パッチを再生成しない。

以下は過去の記録。
# 最新インストーラーを先に公開 (2026-10-08、カード52、作業中)

ユーザーから、現在の最新版をGitHubへリリースしてから残りの開発を完成させるよう依頼あり。
公開・push・統合は依頼済み。PR138 (5ba2de7) の通常CIは成功、MSI CIはQtのビルド中。
今回の版はv1.1.0-rc.2を予定。英語綴り候補と自由なLLM文章補正は含まない。
`codex/installer-release-check` で公開ノートと使い捨てCIでの導入・実入力・削除検証を準備。
開発者の普段のIMEにはインストールしていない。CIで実導入検証が成功するまで公開しない。
追加の精度比較・CPU初回候補確認は `docs/verification/2026-10-08-release-check.md`。
次はPR138の全CI確認・merge commit、公開準備PRのCI・統合、mainからrc公開、check_release.py。
公開後はカード51の残り（英語候補、LLM文章補正、予測の評価）を続ける。

---

以下は過去の作業記録。

# 初回候補と候補欄の開発 (2026-10-08、カード51、未完了)

現在の依頼は本体の改善。初回表示の補正が最優先。続き予測、英語の綴り、LLMによる文章補正、候補UIも依頼されている。
`codex/first-candidate` で作業。Mozcは既存の `C:\Users\aruik\mz` を変更せず、`C:\Users\aruik\mzfi` で開発した。
ADR0040・`docs/verification/2026-10-08-first-candidate.md` に実装・実測・未検証を記録。

初回候補をAI評価まで待ち、評価自体をワーカーへ移した。評価できなかった結果の誤キャッシュを除去。
実モデルで最初の候補「足を痛めたので」を確認。候補描画も変更し100%・200%の画像を確認。
6テストターゲットPASS。インストール・配布更新は未実施。
英語候補・自由な文章補正生成は未実装。予測の採用条件の評価、精度と負荷の比較、TSF実機確認、CIが残る。

パッチ作成ツールは既存の `~/kotori-patch-work` を削除せず、毎回別の一時cloneを作るよう変更。
この既存cloneには未コミットの変更があるため破棄しない。

---

以下は過去の紹介文書の履歴。

# 紹介の方針 (2026-10-07、カード50)

メンテナは、製品の宣伝ではなく、オープンソースの優しいソフトとしての紹介を希望。
決め台詞・競争を煽る見出しを使わず、機能・比較結果・使い方を具体的に説明する。
README・比較図を更新。測定値・条件は保持。CIは当PRで確認してから統合。

# 紹介文書の更新 (2026-10-07、カード49)

README冒頭にMicrosoft IME・Google日本語入力との実入力比較を移動。
2026-10-01のbeta.8 / Unrealの既存測定値を使用し、版・条件・全5セットを明記。
91.5〜93.0%の前の文あり評価とは分離。図はdocs/images/gen_ime_comparison.pyで生成。
製品コード・精度・既存検証結果は変更していない。画像生成・目視・出典値の確認・diff check済み。
AJIMEE原データの3問と難しさの説明、出典・ライセンスを追加。各IMEの個別出力とは区別。集計値のCSVも追加。全15値と出典、差分のDecimal計算を確認。CI結果はPR136を参照し、通過後に統合する。

---

以下は2026-10-02時点の開発引き継ぎ。

# 引き継ぎ(2026-10-02 時点)

開発を別のセッション・エージェント・人に引き継ぐための現状のまとめ。作業の規約は `AGENTS.md`、環境とコマンドと
リリースは `docs/DEVELOPMENT.md`、次の作業は `docs/tasks/`。**作業を始める前に、この 3 つを読む。**

## 0. 方針(最優先で読む)

- 製品の目標は「高品質版 Google 日本語入力」(docs/adr/0014)。**Mozc をベースにし、AI の変換を組み込む**
  (docs/adr/0012)。`crates/`・`frontends/` の Rust 版は旧版で、機能は足さない。
- **精度は十分**(Standard で AJIMEE 91.5%、最終評価用のセット 300 問で 96.3%)。今は精度(Standard 90% 以上)を保ったまま、
  負荷・待ち時間・使い勝手を良くする。ハイエンド(GPU)向けの Standard / High / Unreal の中身は保ち、
  GPU のない PC は Low(CPU、zenz-medium、先回りの変換)で動かす(docs/adr/0024)。
- メンテナは判断を任せている。確認を取らずに進め、数値と一緒に報告する(`AGENTS.md`)。

## 1. 進み具合(Mozc 版)

| 項目 | 状態 |
| --- | --- |
| 名前・識別子(カード 21) | 完了 |
| AI の変換(docs/adr/0013〜0023) | 完了。zenz の生成と TinySwallow-1.5B の採点、入力中の予測と Tab、設定画面、LLM の重み |
| 負荷と待ち時間(docs/adr/0024〜0036、カード 23〜31・34・36〜39) | 完了。GPU の稼働率 68.6% → 42.6%、VRAM 1.66 → 1.49 GB(Standard)、10 分入力がなければ VRAM を空ける。ノート PC で固まらない(内蔵 GPU を使わない、時間の上限、先回りの変換、待ちの解消)。新しい Low。予測キャッシュの設定の区切り、AI の状態の表示、生成した文の文節分け、ユーザー辞書の保護、ハイコントラスト、予測の外れ候補の削減、Unreal の見直し(High + zenz-medium) |
| 評価の基盤 | 最終評価用のセット `kotori-heldout`(300 問、調整に使わない)、評価器の失敗の検出と実行条件の記録、予測の節約文字数と外れ候補、性能表(`docs/PERFORMANCE.md`)、単体テストを CI で |
| 配布 | GitHub Releases の `Kotori64.msi`。beta.5 は版の上げ方の誤りで上書きが効かなかったので、beta.6 で直した(カード 35)。公開後に `python mozc/tools/check_release.py` で版を確かめる。**Latest は製品版 `v1.0.0`(署名なし)**。メンテナの判断で、出荷条件がそろう前に rc.1 と同じ中身で出した(docs/adr/0037 の「変更」)。段階の出し分けは `docs/DEVELOPMENT.md` 1.5 |
| CI | Mozc (Windows) は約 15〜20 分(キャッシュが当たるとき)。Bazel のキャッシュは main(リリースの実行)でだけ保存する。PR ごとに保存すると上限 10 GB を超えて Qt のキャッシュが消え、95 分かかる |
| 実機での確認 | メンテナに頼んでいる(`docs/ACCEPTANCE.md`)。ノート PC、ダークモード・ハイコントラストの設定画面、上書きインストール |

精度(前の文あり、Acc@1):

| | AJIMEE | 日常 | 最終評価用(heldout、300 問) |
| --- | ---: | ---: | ---: |
| Mozc 単体 | 51.0% | 80.2% | 82.3% |
| Low(GPU で測定) | 88.0% | 97.5% | 98.3% |
| Low(GPU のない PC、CPU で測定) | 83.0% | 97.5% | 97.7% |
| Standard(既定) | 91.5% | 97.5% | 96.3% |
| High | 92.5% | 97.5% | 96.3% |
| Unreal | 93.0% | 97.5% | — |

## 2. コードの地図

| 場所 | 中身 |
| --- | --- |
| `mozc/patches/` | google/mozc(コミット固定)に当てるパッチ。ファイルごとの目的と ADR は `mozc/patches/README.md` |
| `mozc/tools/`、`mozc/eval_*.py` | 評価・計測・負荷試験・画像・パッチの作り直し(一覧と環境変数は `mozc/README.md`) |
| `eval/` | 評価セット(`eval/sets/`)、AJIMEE の取得(`eval/fetch.sh`)、記録(`eval/README.md`) |
| `.github/workflows/mozc-windows.yml` | MSI のビルドと単体テスト。手動で実行するとリリース |
| `training/zenz`、`training/llm` | 同梱するモデル(zenz-v2.5 small・medium、TinySwallow-1.5B)の取得と変換 |
| `docs/adr/` | 設計判断。0012 以降が Mozc 版、0001〜0011 は主に旧 Rust 版 |
| `docs/` | `SPEC.md`(仕様。Mozc 版に合わせた改訂は PR #113 で承認待ち)、`ACCEPTANCE.md`(実機の確認表)、`PERFORMANCE.md`、`IMPROVEMENT_PROPOSALS.md`(改善案 15 件、多くは対応済み)、`licenses.md` |
| `crates/`、`frontends/`、`justfile` | 旧 Rust 版(評価の基準として残す) |

## 3. メンテナの判断待ち・未決

| 事項 | 内容 |
| --- | --- |
| SPEC の改訂 | 先頭に「0. Mozc 版での読み替え」と今の性能の目標値を足す案を PR #113、製品版の前提(0.2)を #113 の上に積んだ #129 で出した。どちらも承認待ち(マージしない) |
| 署名の手段 | SignPath Foundation への申し込み(リポジトリの持ち主の名前でしかできない。カード 44) |
| 実機の結果 | ノート PC(内蔵 GPU)で Low が十分速いか。内蔵 GPU で zenz だけを動かす案(カード 32)はその結果で決める |
| Google 日本語入力・Microsoft IME との比較 | 同じ条件で比べる手順を作ってから(`docs/IMPROVEMENT_PROPOSALS.md` の案 12) |

## 3.5 試して効かなかったこと(2026-10-01、同じことを繰り返さない)

| 試したこと | 結果 |
| --- | --- |
| 打鍵ごとの予測を 100 ms 待ってから始める | 稼働率 47 → 32% だが、打ってすぐの当たりが 14 → 5。入力中の候補は次の打鍵でしか出し直されない |
| 軽い予測の続きを減らす・並べ替えを省く | 稼働率 41〜43% だが当たりが 16 → 13〜14 |
| ggml-vulkan の MMVQ を切る・強制する(`GGML_VK_DISABLE_MMVQ`・`FORCE_MMVQ`) | LLM の 1 回の計算の時間は変わらない。3 トークン以上で 2 倍近くかかる(2 トークン 8 ms、4〜6 トークン 11〜14 ms)のは変わらない |
| `GGML_VK_DISABLE_HOST_VISIBLE_VIDMEM` | メモリ(ワーキングセット 約 460 MB、Standard)は変わらない |
| Tab の時間の上限を要求が来た時から数える | Tab の最大(約 860 ms)は変わらない |
| CPU の先回りの変換で、やめたときも前置きの計算を残す | AJIMEE を打ってすぐ Space(60 文)の正解 38 → 38〜39、時間は揺れの範囲 |
| CPU の Low の生成の数を 3 にする | AJIMEE 75.5% → 67.0%(時間の上限に収まらない文が増える) |

GPU のない PC の Low は、打ってすぐ Space だと長い文で AI が間に合わない(AJIMEE 60 文、文脈なし: 打ってすぐ 38、
0.5 秒後 44、GPU なら 47)。日常の短い文は 15/15。

## 3.6 ATOK に近づける(2026-10-01 から)

ATOK の実機はないので、ATOK が強いとされる所のセット(`eval/sets/kotori-typo`・`homophone`・`names`)を作り、
Google 日本語入力・Microsoft IME と実際の IME で比べた(`eval/imebench/`)。同音語と人名・新語は Kotori が上回っている。
打ち間違いの補正はどの IME も弱く、Kotori で始めた(docs/adr/0036、2 → 14 / 40)。次は、書き換えの候補の絞り込みを良くする、
2 か所以上の打ち間違い、校正(ら抜き・二重敬語)の指摘。

## 3.7 製品版(v1.0)への道(2026-10-01 から)

機能と品質は v0.3.0-beta.8 で足りている。足りないのは「配る・直す・信じてもらう」ための部分(docs/adr/0037)。
v1.0 = 署名された MSI を、知らない人がそのまま入れて普段の IME として使い続けられる版。段階は
beta → `1.0.0-rc.N`(プレリリース)→ `1.0.0`。出荷条件は G1〜G8(精度、24 時間試験、実機、署名、表示、窓口、SPEC、
RC で報告 0)。カード 40〜47。

調べて分かった穴: 未署名、版が beta 固定、`ggml-rpc.dll`(通信の部品、未使用)を同梱、`libomp`・Qt の表示がない、
診断情報の書き出しがない、Issue のテンプレート・SECURITY.md がない、SPEC が「私用ベータ・公開配布は非目標」のまま、
MSI が 1.46 GiB(GitHub の上限 2 GiB)。

済み(2026-10-02): 40 版の段階(`-f channel=beta|rc|stable`)、41 `ggml-rpc.dll` を外し第三者の表示を同梱、42 診断情報の書き出しと異常終了の記録(ミニダンプは入力した文字が残るので使わない、docs/adr/0038)、43 `docs/USER_GUIDE.md`・Issue のテンプレート・SECURITY.md、46 SPEC の改訂案(#129)。`v1.0.0-rc.1` を出し、メンテナの判断で同じ中身の `v1.0.0`(署名なし)を出した。

残り(`1.0.x` で満たす): 45 の受け入れ(G1 精度、G2 24 時間試験、G3 実機)、44 署名(署名した版は `1.0.1`)、47 winget。

メンテナに頼むこと: 署名の手段(SignPath Foundation への申し込み)、SPEC の改訂の承認(#113、#129)、実機の確認
(`docs/ACCEPTANCE.md` を rc.1 で)、GitHub の非公開の脆弱性報告を有効にする(SECURITY.md が案内している。設定 → Security)、
winget への提出の承認。

## 3.8 手が止まったら AI の変換を出す(2026-10-02、1.1)

メンテナの依頼: 「Space を押す前に正しい変換を出せないか。Tab の予測も同様」。入力中の候補の窓は次の打鍵でしか作り直されない
ので、サーバーが Mozc の `Output.callback` で「少し後に取り直して」と頼み、TIP がタイマーで送る形にした(docs/adr/0039、
カード 48)。取り直しでは Space と同じ変換を候補の先頭に置き、その結果を覚えて Space でも使う(日常 81 問で取り直しの先頭 =
Space の結果 81 / 81、取り直し GPU 46〜95 ms・CPU 29〜73 ms、その後の Space 1〜2 ms)。精度は変わらない。
**実機(TSF)での確認が要る**(メモ帳・ブラウザ・Word で打って手を止める)。

Low(GPU で測る)の AJIMEE は、今の main で 87.5%(175 / 200)。表の 88.0% は前の版のもの(どの変更で 1 問変わったかは未調査)。

## 4. すぐにやること

1. リリースした MSI を展開して確かめる(版、同梱物、設定画面)。実機の報告が来たら最優先で直す。
2. **`1.0.0` を出した**ので、`1.0.x` は不具合の修正と、残りの出荷条件(署名・実機・24 時間試験)だけにする(改善は 1.1 へ)。
   次の修正版は `mozc/VERSION` を `1.0.1` に上げて `-f channel=stable`(同じ版は二度出せない。`release_tag.py`)。
3. 1.1(手が止まったら AI の変換を出す)は `1.1.0-rc.N` で実機の確認をしてから `1.1.0`。
4. 32(内蔵 GPU)は実機の結果待ち、33 は保留。

## 5. はまりどころ(このプロジェクトで実際に起きたこと)

Mozc 版と AI の作業でのはまりどころは `AGENTS.md` にある。以下は主に旧 Rust 版のもの。

- **Windows の CI でだけ落ちるテスト**: 読み込み中(mmap 中)のファイルを上書きできない。テストで
  同じファイル名のモデルや辞書を作り直さない(`crates/kotori-lm/src/zenz_tests.rs`)。
- **CRLF**: Windows の checkout ではテキストの改行が CRLF になる。テストで読むテキストは
  `.replace("\r\n", "\n")` してから比べる。
- **UNIX ソケットのパス長**: 108 バイトを超えると `bind` できない。深いディレクトリで
  サーバーを動かすときは `XDG_RUNTIME_DIR` を短くする。
- **`pkill -f` の自滅**: シェルの1行にパターンを含めると、そのシェル自身が殺される。
  PID を `ps -eo pid,args | grep "[k]otori-server"` で取ってから `kill` する。
- **テストのデッドロック**: 偽のサーバースレッドを `join` する前に、要求を送る順序になっているか
  確かめる(`crates/kotori-client/src/ffi_tests.rs`)。
- **TSF のヘッダ**: mingw の `msctf.h` には `ITfTextInputProcessorEx` などがない。Windows の
  コードは MSVC(または xwin の SDK と clang、`docs/DEVELOPMENT.md`)で確かめる。
- **TIP と Rust の CRT**: TIP は VC ランタイムを静的にリンクする(`/MT`)。`kotori-client` も
  `RUSTFLAGS="-C target-feature=+crt-static"` で作る。
- **依存の更新**: `prost`・`prost-build` は 0.13、`protox` は 0.7 に固定。`Cargo.lock` の更新は
  `CARGO_RESOLVER_INCOMPATIBLE_RUST_VERSIONS=fallback` を付ける(MSRV 1.80 を守る)。
- **zenz の特殊トークン**: config.json の eos_token_id は誤り(2 = `<s>`)。正しい終端は `</s>`(3)。
  変換スクリプトが直している(ADR 0007)。
- **llama.cpp と CRT**: llama.cpp の CMake は CMP0091 が NEW なので、`/MT` にするには
  `CMAKE_MSVC_RUNTIME_LIBRARY` が要る(`crates/kotori-lm/build.rs` が crt-static を見て決める)。
  サーバーが kotori-lm に依存するので、サーバーを作る CI のジョブはサブモジュールを取得する。
- **cherry-pick で積んだ変更**: 1本の作業ブランチで PR を順に出すときは、main に合わせて
  1コミットずつ cherry-pick し、`docs/tasks/README.md` の表の衝突は main 側に行の変更を当て直す。
- **LM の速度**: 91M のモデルは 2 スレッドで 1 トークン約 2ms。候補を増やすと遅延がほぼ比例して
  増える。数値は `just bench-lm` で測る。


# 英語候補の枝にあった記録（履歴）

# 先行リリースの準備 (2026-10-08)

ユーザーから「第一改善案が済んだらリリースを先に」と指示あり。
初回候補とUIはrc.2に公開済み。英字候補PR140と実導入検証PR139を統合して先行版を準備する。
自由なLLM補正とInstructモデル置換は別の試作として保持し、今回の配布には含めない。

rc.2の実導入・TSF実入力・削除はActions 37758533070で成功。日本語/東京を確定し、削除は3010（再起動要求）。
MSIのSHA-256はa079db01e45ac0e779feb7b0c2beff1bd2aa6d9ab9575f8f0537df94bfe3ed0b。
成果物のソースはf8284f51。result.jsonのsource_commitは検証器のコミットであり、成果物ソースとは異なる。
CIではMSIRESTARTMANAGERCONTROL=Disableを指定した。標準操作と同条件の削除確認ではない。
過去のランナー停止は履歴として残し、今回の検証条件と区別する。

初回候補の監査で予測の誤字3例を確認。実セッションの一括入力では初回に誤字を表示せず、通常の文変換を表示した。
続けて1文字ずつ入力する経路を検証する。精度・UIの既存確認を理由なく繰り返さない。

以下は過去の記録。
# 現在の状態 (2026-10-08、カード53を検証中)

v1.1.0-rc.2を公開済み。ソースはf8284f51、MSIは3.34.6313.100。
公開MSIのSHA-256はa079db01e45ac0e779feb7b0c2beff1bd2aa6d9ab9575f8f0537df94bfe3ed0b。
check_release.pyでハッシュ、版、同梱物を確認済み。リリース本文もrc.2の内容に更新した。
PR138の初回候補・UI変更は統合済み。自由なLLM文章補正はまだ実装していない。

PR140の英語候補は検証中。予測候補のNO_VARIANTS_EXPANSIONで英字候補が落ちる問題を修正。
実モデルのsession_handler_mainでindex/effect/internetが初回表示・Tab・Spaceすべてに存在することを確認。
現時点の対応は24表記に限定。汎用的なカタカナ英語変換が完成したとは扱わない。

PR139のインストーラー実入力試験は未完了。2回ともアンインストール中にGitHubランナーが停止した。
単なる一時障害とは断定できず、導入・実入力・削除の各段階の記録を残して原因を調べる必要がある。
公開MSIの内容検査と、実際の導入から削除までの成功確認は区別する。

以下は過去の記録。

# 初回候補と候補欄の開発 (2026-10-08、カード51、未完了)

現在の依頼は本体の改善。初回表示の補正が最優先。続き予測、英語の綴り、LLMによる文章補正、候補UIも依頼されている。
`codex/first-candidate` で作業。Mozcは既存の `C:\Users\aruik\mz` を変更せず、`C:\Users\aruik\mzfi` で開発した。
ADR0040・`docs/verification/2026-10-08-first-candidate.md` に実装・実測・未検証を記録。

初回候補をAI評価まで待ち、評価自体をワーカーへ移した。評価できなかった結果の誤キャッシュを除去。
実モデルで最初の候補「足を痛めたので」を確認。候補描画も変更し100%・200%の画像を確認。
6テストターゲットPASS。インストール・配布更新は未実施。
英語候補・自由な文章補正生成は未実装。予測の採用条件の評価、精度と負荷の比較、TSF実機確認、CIが残る。

パッチ作成ツールは既存の `~/kotori-patch-work` を削除せず、毎回別の一時cloneを作るよう変更。
この既存cloneには未コミットの変更があるため破棄しない。

---

以下は過去の紹介文書の履歴。

# 紹介の方針 (2026-10-07、カード50)

メンテナは、製品の宣伝ではなく、オープンソースの優しいソフトとしての紹介を希望。
決め台詞・競争を煽る見出しを使わず、機能・比較結果・使い方を具体的に説明する。
README・比較図を更新。測定値・条件は保持。CIは当PRで確認してから統合。

# 紹介文書の更新 (2026-10-07、カード49)

README冒頭にMicrosoft IME・Google日本語入力との実入力比較を移動。
2026-10-01のbeta.8 / Unrealの既存測定値を使用し、版・条件・全5セットを明記。
91.5〜93.0%の前の文あり評価とは分離。図はdocs/images/gen_ime_comparison.pyで生成。
製品コード・精度・既存検証結果は変更していない。画像生成・目視・出典値の確認・diff check済み。
AJIMEE原データの3問と難しさの説明、出典・ライセンスを追加。各IMEの個別出力とは区別。集計値のCSVも追加。全15値と出典、差分のDecimal計算を確認。CI結果はPR136を参照し、通過後に統合する。

---

以下は2026-10-02時点の開発引き継ぎ。

# 引き継ぎ(2026-10-02 時点)

開発を別のセッション・エージェント・人に引き継ぐための現状のまとめ。作業の規約は `AGENTS.md`、環境とコマンドと
リリースは `docs/DEVELOPMENT.md`、次の作業は `docs/tasks/`。**作業を始める前に、この 3 つを読む。**

## 0. 方針(最優先で読む)

- 製品の目標は「高品質版 Google 日本語入力」(docs/adr/0014)。**Mozc をベースにし、AI の変換を組み込む**
  (docs/adr/0012)。`crates/`・`frontends/` の Rust 版は旧版で、機能は足さない。
- **精度は十分**(Standard で AJIMEE 91.5%、最終評価用のセット 300 問で 96.3%)。今は精度(Standard 90% 以上)を保ったまま、
  負荷・待ち時間・使い勝手を良くする。ハイエンド(GPU)向けの Standard / High / Unreal の中身は保ち、
  GPU のない PC は Low(CPU、zenz-medium、先回りの変換)で動かす(docs/adr/0024)。
- メンテナは判断を任せている。確認を取らずに進め、数値と一緒に報告する(`AGENTS.md`)。

## 1. 進み具合(Mozc 版)

| 項目 | 状態 |
| --- | --- |
| 名前・識別子(カード 21) | 完了 |
| AI の変換(docs/adr/0013〜0023) | 完了。zenz の生成と TinySwallow-1.5B の採点、入力中の予測と Tab、設定画面、LLM の重み |
| 負荷と待ち時間(docs/adr/0024〜0036、カード 23〜31・34・36〜39) | 完了。GPU の稼働率 68.6% → 42.6%、VRAM 1.66 → 1.49 GB(Standard)、10 分入力がなければ VRAM を空ける。ノート PC で固まらない(内蔵 GPU を使わない、時間の上限、先回りの変換、待ちの解消)。新しい Low。予測キャッシュの設定の区切り、AI の状態の表示、生成した文の文節分け、ユーザー辞書の保護、ハイコントラスト、予測の外れ候補の削減、Unreal の見直し(High + zenz-medium) |
| 評価の基盤 | 最終評価用のセット `kotori-heldout`(300 問、調整に使わない)、評価器の失敗の検出と実行条件の記録、予測の節約文字数と外れ候補、性能表(`docs/PERFORMANCE.md`)、単体テストを CI で |
| 配布 | GitHub Releases の `Kotori64.msi`。beta.5 は版の上げ方の誤りで上書きが効かなかったので、beta.6 で直した(カード 35)。公開後に `python mozc/tools/check_release.py` で版を確かめる。**Latest は製品版 `v1.0.0`(署名なし)**。メンテナの判断で、出荷条件がそろう前に rc.1 と同じ中身で出した(docs/adr/0037 の「変更」)。段階の出し分けは `docs/DEVELOPMENT.md` 1.5 |
| CI | Mozc (Windows) は約 15〜20 分(キャッシュが当たるとき)。Bazel のキャッシュは main(リリースの実行)でだけ保存する。PR ごとに保存すると上限 10 GB を超えて Qt のキャッシュが消え、95 分かかる |
| 実機での確認 | メンテナに頼んでいる(`docs/ACCEPTANCE.md`)。ノート PC、ダークモード・ハイコントラストの設定画面、上書きインストール |

精度(前の文あり、Acc@1):

| | AJIMEE | 日常 | 最終評価用(heldout、300 問) |
| --- | ---: | ---: | ---: |
| Mozc 単体 | 51.0% | 80.2% | 82.3% |
| Low(GPU で測定) | 88.0% | 97.5% | 98.3% |
| Low(GPU のない PC、CPU で測定) | 83.0% | 97.5% | 97.7% |
| Standard(既定) | 91.5% | 97.5% | 96.3% |
| High | 92.5% | 97.5% | 96.3% |
| Unreal | 93.0% | 97.5% | — |

## 2. コードの地図

| 場所 | 中身 |
| --- | --- |
| `mozc/patches/` | google/mozc(コミット固定)に当てるパッチ。ファイルごとの目的と ADR は `mozc/patches/README.md` |
| `mozc/tools/`、`mozc/eval_*.py` | 評価・計測・負荷試験・画像・パッチの作り直し(一覧と環境変数は `mozc/README.md`) |
| `eval/` | 評価セット(`eval/sets/`)、AJIMEE の取得(`eval/fetch.sh`)、記録(`eval/README.md`) |
| `.github/workflows/mozc-windows.yml` | MSI のビルドと単体テスト。手動で実行するとリリース |
| `training/zenz`、`training/llm` | 同梱するモデル(zenz-v2.5 small・medium、TinySwallow-1.5B)の取得と変換 |
| `docs/adr/` | 設計判断。0012 以降が Mozc 版、0001〜0011 は主に旧 Rust 版 |
| `docs/` | `SPEC.md`(仕様。Mozc 版に合わせた改訂は PR #113 で承認待ち)、`ACCEPTANCE.md`(実機の確認表)、`PERFORMANCE.md`、`IMPROVEMENT_PROPOSALS.md`(改善案 15 件、多くは対応済み)、`licenses.md` |
| `crates/`、`frontends/`、`justfile` | 旧 Rust 版(評価の基準として残す) |

## 3. メンテナの判断待ち・未決

| 事項 | 内容 |
| --- | --- |
| SPEC の改訂 | 先頭に「0. Mozc 版での読み替え」と今の性能の目標値を足す案を PR #113、製品版の前提(0.2)を #113 の上に積んだ #129 で出した。どちらも承認待ち(マージしない) |
| 署名の手段 | SignPath Foundation への申し込み(リポジトリの持ち主の名前でしかできない。カード 44) |
| 実機の結果 | ノート PC(内蔵 GPU)で Low が十分速いか。内蔵 GPU で zenz だけを動かす案(カード 32)はその結果で決める |
| Google 日本語入力・Microsoft IME との比較 | 同じ条件で比べる手順を作ってから(`docs/IMPROVEMENT_PROPOSALS.md` の案 12) |

## 3.5 試して効かなかったこと(2026-10-01、同じことを繰り返さない)

| 試したこと | 結果 |
| --- | --- |
| 打鍵ごとの予測を 100 ms 待ってから始める | 稼働率 47 → 32% だが、打ってすぐの当たりが 14 → 5。入力中の候補は次の打鍵でしか出し直されない |
| 軽い予測の続きを減らす・並べ替えを省く | 稼働率 41〜43% だが当たりが 16 → 13〜14 |
| ggml-vulkan の MMVQ を切る・強制する(`GGML_VK_DISABLE_MMVQ`・`FORCE_MMVQ`) | LLM の 1 回の計算の時間は変わらない。3 トークン以上で 2 倍近くかかる(2 トークン 8 ms、4〜6 トークン 11〜14 ms)のは変わらない |
| `GGML_VK_DISABLE_HOST_VISIBLE_VIDMEM` | メモリ(ワーキングセット 約 460 MB、Standard)は変わらない |
| Tab の時間の上限を要求が来た時から数える | Tab の最大(約 860 ms)は変わらない |
| CPU の先回りの変換で、やめたときも前置きの計算を残す | AJIMEE を打ってすぐ Space(60 文)の正解 38 → 38〜39、時間は揺れの範囲 |
| CPU の Low の生成の数を 3 にする | AJIMEE 75.5% → 67.0%(時間の上限に収まらない文が増える) |

GPU のない PC の Low は、打ってすぐ Space だと長い文で AI が間に合わない(AJIMEE 60 文、文脈なし: 打ってすぐ 38、
0.5 秒後 44、GPU なら 47)。日常の短い文は 15/15。

## 3.6 ATOK に近づける(2026-10-01 から)

ATOK の実機はないので、ATOK が強いとされる所のセット(`eval/sets/kotori-typo`・`homophone`・`names`)を作り、
Google 日本語入力・Microsoft IME と実際の IME で比べた(`eval/imebench/`)。同音語と人名・新語は Kotori が上回っている。
打ち間違いの補正はどの IME も弱く、Kotori で始めた(docs/adr/0036、2 → 14 / 40)。次は、書き換えの候補の絞り込みを良くする、
2 か所以上の打ち間違い、校正(ら抜き・二重敬語)の指摘。

## 3.7 製品版(v1.0)への道(2026-10-01 から)

機能と品質は v0.3.0-beta.8 で足りている。足りないのは「配る・直す・信じてもらう」ための部分(docs/adr/0037)。
v1.0 = 署名された MSI を、知らない人がそのまま入れて普段の IME として使い続けられる版。段階は
beta → `1.0.0-rc.N`(プレリリース)→ `1.0.0`。出荷条件は G1〜G8(精度、24 時間試験、実機、署名、表示、窓口、SPEC、
RC で報告 0)。カード 40〜47。

調べて分かった穴: 未署名、版が beta 固定、`ggml-rpc.dll`(通信の部品、未使用)を同梱、`libomp`・Qt の表示がない、
診断情報の書き出しがない、Issue のテンプレート・SECURITY.md がない、SPEC が「私用ベータ・公開配布は非目標」のまま、
MSI が 1.46 GiB(GitHub の上限 2 GiB)。

済み(2026-10-02): 40 版の段階(`-f channel=beta|rc|stable`)、41 `ggml-rpc.dll` を外し第三者の表示を同梱、42 診断情報の書き出しと異常終了の記録(ミニダンプは入力した文字が残るので使わない、docs/adr/0038)、43 `docs/USER_GUIDE.md`・Issue のテンプレート・SECURITY.md、46 SPEC の改訂案(#129)。`v1.0.0-rc.1` を出し、メンテナの判断で同じ中身の `v1.0.0`(署名なし)を出した。

残り(`1.0.x` で満たす): 45 の受け入れ(G1 精度、G2 24 時間試験、G3 実機)、44 署名(署名した版は `1.0.1`)、47 winget。

メンテナに頼むこと: 署名の手段(SignPath Foundation への申し込み)、SPEC の改訂の承認(#113、#129)、実機の確認
(`docs/ACCEPTANCE.md` を rc.1 で)、GitHub の非公開の脆弱性報告を有効にする(SECURITY.md が案内している。設定 → Security)、
winget への提出の承認。

## 3.8 手が止まったら AI の変換を出す(2026-10-02、1.1)

メンテナの依頼: 「Space を押す前に正しい変換を出せないか。Tab の予測も同様」。入力中の候補の窓は次の打鍵でしか作り直されない
ので、サーバーが Mozc の `Output.callback` で「少し後に取り直して」と頼み、TIP がタイマーで送る形にした(docs/adr/0039、
カード 48)。取り直しでは Space と同じ変換を候補の先頭に置き、その結果を覚えて Space でも使う(日常 81 問で取り直しの先頭 =
Space の結果 81 / 81、取り直し GPU 46〜95 ms・CPU 29〜73 ms、その後の Space 1〜2 ms)。精度は変わらない。
**実機(TSF)での確認が要る**(メモ帳・ブラウザ・Word で打って手を止める)。

Low(GPU で測る)の AJIMEE は、今の main で 87.5%(175 / 200)。表の 88.0% は前の版のもの(どの変更で 1 問変わったかは未調査)。

## 4. すぐにやること

1. リリースした MSI を展開して確かめる(版、同梱物、設定画面)。実機の報告が来たら最優先で直す。
2. **`1.0.0` を出した**ので、`1.0.x` は不具合の修正と、残りの出荷条件(署名・実機・24 時間試験)だけにする(改善は 1.1 へ)。
   次の修正版は `mozc/VERSION` を `1.0.1` に上げて `-f channel=stable`(同じ版は二度出せない。`release_tag.py`)。
3. 1.1(手が止まったら AI の変換を出す)は `1.1.0-rc.N` で実機の確認をしてから `1.1.0`。
4. 32(内蔵 GPU)は実機の結果待ち、33 は保留。

## 5. はまりどころ(このプロジェクトで実際に起きたこと)

Mozc 版と AI の作業でのはまりどころは `AGENTS.md` にある。以下は主に旧 Rust 版のもの。

- **Windows の CI でだけ落ちるテスト**: 読み込み中(mmap 中)のファイルを上書きできない。テストで
  同じファイル名のモデルや辞書を作り直さない(`crates/kotori-lm/src/zenz_tests.rs`)。
- **CRLF**: Windows の checkout ではテキストの改行が CRLF になる。テストで読むテキストは
  `.replace("\r\n", "\n")` してから比べる。
- **UNIX ソケットのパス長**: 108 バイトを超えると `bind` できない。深いディレクトリで
  サーバーを動かすときは `XDG_RUNTIME_DIR` を短くする。
- **`pkill -f` の自滅**: シェルの1行にパターンを含めると、そのシェル自身が殺される。
  PID を `ps -eo pid,args | grep "[k]otori-server"` で取ってから `kill` する。
- **テストのデッドロック**: 偽のサーバースレッドを `join` する前に、要求を送る順序になっているか
  確かめる(`crates/kotori-client/src/ffi_tests.rs`)。
- **TSF のヘッダ**: mingw の `msctf.h` には `ITfTextInputProcessorEx` などがない。Windows の
  コードは MSVC(または xwin の SDK と clang、`docs/DEVELOPMENT.md`)で確かめる。
- **TIP と Rust の CRT**: TIP は VC ランタイムを静的にリンクする(`/MT`)。`kotori-client` も
  `RUSTFLAGS="-C target-feature=+crt-static"` で作る。
- **依存の更新**: `prost`・`prost-build` は 0.13、`protox` は 0.7 に固定。`Cargo.lock` の更新は
  `CARGO_RESOLVER_INCOMPATIBLE_RUST_VERSIONS=fallback` を付ける(MSRV 1.80 を守る)。
- **zenz の特殊トークン**: config.json の eos_token_id は誤り(2 = `<s>`)。正しい終端は `</s>`(3)。
  変換スクリプトが直している(ADR 0007)。
- **llama.cpp と CRT**: llama.cpp の CMake は CMP0091 が NEW なので、`/MT` にするには
  `CMAKE_MSVC_RUNTIME_LIBRARY` が要る(`crates/kotori-lm/build.rs` が crt-static を見て決める)。
  サーバーが kotori-lm に依存するので、サーバーを作る CI のジョブはサブモジュールを取得する。
- **cherry-pick で積んだ変更**: 1本の作業ブランチで PR を順に出すときは、main に合わせて
  1コミットずつ cherry-pick し、`docs/tasks/README.md` の表の衝突は main 側に行の変更を当て直す。
- **LM の速度**: 91M のモデルは 2 スレッドで 1 トークン約 2ms。候補を増やすと遅延がほぼ比例して
  増える。数値は `just bench-lm` で測る。
