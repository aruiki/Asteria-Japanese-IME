# AI保留確定の表示残留と二重入力（2026-10-21）

利用者は「ゴールに設定して」が二重に挿入されると報告し、アプリ/タイミングは「すべて」と回答した。その後、導入版が古く最新では解消している可能性も指摘した。導入済み6438と最新RC4/6440を分離して調べた。再ビルドと署名なしの正式v2.0.0公開は許可済み。

## 修正前の証拠

[PR180の初回Windows比較run39575173488](https://github.com/aruiki/Asteria-Japanese-IME/actions/runs/39575173488)は両版とも追加検査の最初のSpace直後Enterで失敗した。旧6438のSHAはc21972edab537e1ea1473f63312f188b5b533baa700aa77fa9c3111364818c32、RC4は2b37f6fbdcb069251e820e2009b096111cc2253e79ffac65d973fa11ce18e301。各々の導入・通常入力と既存候補操作8項目は成功している。

EDITホストで、最終WM_IME_COMPOSITIONのflagsは8121、GCS_RESULTSTRとGCS_COMPSTRが同時に立ち、resultもpreeditも「ゴールに設定して」だった。確定後も未確定表示が残り、終了イベントが来ないため検査は失敗した。初回結果を単に「二重のresultイベント」として説明していない。

原票はtarget/accuracy-guide/duplicate-native-39575173488/{6438,6440}/acceptance.json。導入版識別・全イベント・失敗のままの判定を保存した。

以前の隔離Session検査はresultの回数だけを見て8試行とも1回と数えていた。生出力を再監査すると、最新6440相当の冷間試行ではKOTORI_REFRESHがresultと同じpreeditを返していた。これは確認不足であり、その8件を二重入力解消の証拠にしない。再監査原票はduplicate-stale-output-baseline-audit-v1.json。

## 原因と変更

Session::KotoriRefreshSuggestionが、保留Space/Tabの変換完了でOutputへ変換表示を書いた後、同じOutputへCommitの確定結果を追記していた。Commitが不要になったpreeditや候補情報を上書きしないため、確定本文がTSFへ未確定表示として再適用される。

変換から保留確定へ進む箇所で一度OutputをClearし、Commitの現在の出力を作り直す。後続キーは従来の再生経路へ通し、そのキーが作った新しいpreeditを保持する。同じ文字列を意図して2回入力する動作を変更する内容照合や重複除去は加えていない。

Mozc source: 785bd83a7f236b49925f7a83764d5beadf3bb378。修正前039a7251fa16a77a91cb67f5158e20073a3f3762からの製品変更はsession.ccの出力初期化のみ。session_test.ccへSpace/Tabの残留検出を追加した。

## ローカル確認

- 修正前: SpaceとTabの2回帰が古いpreedit/all_candidate_wordsの残留で失敗。duplicate-stale-output-red-v1.log/xmlを保存。
- 修正後: Session198、EngineConverter92、SessionHandlerシナリオ81の計371件、全3target成功。後続キー・文脈・取消等の既存回帰を含む。原票duplicate-fixed-native-v1/cpp-xml/。
- 修正後の実辞書/モデルを使う隔離SessionはGPU/CPU各4条件（冷間Space/Enter、温間Space/Enter、Tab/Enter、意図した同文2回）が成功。各確定応答にpreedit/候補/次callbackが残らず、さらに8回の遅いrefreshにもresult/preeditがない。原票duplicate-fixed-session-v3/summary.json。実TSF試験とは別。
- 固定SessionHandler SHA: 81f85a90fb23d2161ec668d9fe92c79ff47360e7850604cfc6fc40617149ea3b。converter_mainはRC4の評価時と全バイト同一、SHA14284c10d8ef4193eb95dc37e052ce67de4b0457ac223ecdccab0adc3f2be4d2。ランキング・辞書・モデルの変更や新たな精度向上の主張はない。
- make_patches.shで再生成し、3patchの順次適用と140ソースの正規化バイト一致を確認。0001/0003は不変。原票duplicate-stale-output-patch-receipt-v1.json。
- Pythonの受入れ原票検証3件、Windows C#のコンパイル成功。追加試験は利用者の実デスクトップで実行していない。

## 新MSIでの確認待ち

新MSI自身を導入し、既存8条件に加えてEDIT/RichEdit計28条件とEdgeのtextarea/contenteditable計14条件を実SendInputで検査する。候補クリックは選択を確認してからEnterで確定する。DevToolsによる文字入力は使わない。新MSIの全CI、導入/再導入/入力/削除、SHA、署名状態、版上昇、公開後の実取得は未完了。旧版で失敗した追加条件の合格を旧MSIへ求めず、新MSIのWindows buildの必須検査として適用する。

この検証を全アプリ・GPU実機の導入TSF・低性能PC・保護欄内部・旧版更新・再起動・独立製品完成判定の代わりにしない。
