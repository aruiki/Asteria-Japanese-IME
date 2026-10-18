# 通常変換の開発・受け入れ条件

ユーザーの2026-10-18指示「普通の変換に必要なものを欠かさずに開発」を、次の項目で追跡する。Mozcに実装があること、ソースにファイルがあること、実際の配布バイナリで入力できることを分ける。AI準備後の平均精度だけで合格にしない。

| 必要なもの | 現在使う経路・データ | 確認方法と残る境界 |
| --- | --- | --- |
| 基本語彙・読み・品詞・単語頻度 | OSSのdictionary00〜09、id.defとcost、手動ドメイン辞書、読み補正 | 実OSSデータで通常例と固定4開発セット。語彙数だけで品質を判定しない。新語/専門語/人名の広い範囲は未保証 |
| 活用・接続・助詞 | cforms.def、connection_single_column、POS規則、ImmutableConverter | 活用/助詞の通常例、converterとOSS DataManagerのテスト |
| 連語 | 元40件を保持したfast_collocations、抑制データ、既存CollocationRewriter | 固定コーパスから生成、AI/ユーザー候補の保護、同条件の改善/改悪と同梱SHA検査 |
| 文節分け・伸縮・移動 | segmenter.def、boundary.def、EngineConverter、Session | 実セッションでShift矢印/矢印、読み保持と表示・確定一致。実TSFの下線/位置は別 |
| 数字・助数詞・金額・年月日 | NumberRewriter、DateRewriter、suffix、counter suffix、zero_query_number | 数字/助数詞/年月日の通常例、対応rewriterテスト。日付依存の「今日」は別途時刻を固定して確認 |
| 英数・外来語・文字種変換 | Romanji表、Composer、TransliterationRewriter、既存英字辞書 | F6〜F10の実セッション操作、数字混在/カタカナ/半角。JISかな/濁点の実辞書例も確認。キー配列全体は別途 |
| 記号・句読点・単漢字・絵文字 | symbol、single_kanji、emoji、emoticon、a11y description | 句読点の通常例、DataManagerでデータを検査。記号一覧・絵文字表示・アクセシビリティの実画面は別途 |
| ユーザー辞書 | UserDictionary、ユーザー品詞・辞書storage | 実プロファイル形式の隔離辞書で登録語の候補包含/選択/確定。GUI登録/取込/削除/更新保持は実MSIで別途 |
| 確定履歴・個人適応 | UserSegmentHistoryRewriter、UserHistoryPredictor | 確定した選択の再利用、未確定/取消の非学習とシークレットの抑止。明示保存後の別PID再起動とcleanup対照も確認。更新保持は別途 |
| 候補選択・取消・編集・確定 | Sessionの候補ID、キーマップ、Composer | Spaceの循環、指定候補選択、Esc/Backspace、表示とEnter結果の一致 |
| 再変換・確定取消 | SessionとWindows TSF | 実辞書の選択文字再変換・削除要求/再編集/再確定を確認。実アプリは別途 |
| 保護入力・フォーカス・学習停止 | InputFieldType、incognito、文脈の世代とfield通知 | 既存C++安全性試験と、実アプリのパスワード/欄移動。AIなしの通常例では証明できない |
| AI不在・準備中・障害・資源不足 | Fast Path、非同期初期化、Mozc fallback | モデルなし通常例、初回即Space、CPU/低メモリ障害試験。実低性能機・実TSFの確認を残す |
| 配布時の欠落防止 | OSSデータの埋込、Bazel、WiX、check_release | ソース・バイナリ・MSIの版/SHAを固定し、実管理展開と導入/入力/削除。ビルド成功を実入力の代わりにしない |

通常例は`eval/normal-conversion-basics.json`、操作試験は`mozc/tools/normal_conversion_acceptance.py`。これは開発用の受け入れ例であり、独立精度ベンチマークではない。読み・参照を結果に合わせて緩めない。試験の入力方法を誤った場合は元の失敗結果を保持し、原因と修正を記録する。

```powershell
python -X utf8 mozc/tools/normal_conversion_acceptance.py --exe <固定session_handler_main.exe> --source-commit <commit> --proto <生成済みprotocolの親> --romanji-table <Mozc>/src/data/preedit/romanji-hiragana.tsv --out <新規出力先>
```

実アプリと導入の確認は[ACCEPTANCE.md](ACCEPTANCE.md)を引き続き使う。ユーザーの「あいであきょく」は初回「アイデア局」/準備後「アイデア曲」という既存課題として保持する。Google日本語入力との直接比較、全語彙の網羅、全通常操作の実画面確認は未完了。

2026-10-18の[実行結果](verification/accuracy-fast-collocations-20261018.md): AIなし実辞書の通常例/操作28項目が成功。OSSデータ・数字・日付・ユーザー辞書・文節履歴のC++165件も成功した。表の「別途」「未保証」の項目を、この結果で済にしない。

2026-10-19の[追加結果](verification/accuracy-normal-conversion-20261019.md): AI待機中に通常候補が消える不具合と、文字種メニュー非表示時のページ移動を修正。C++379件、固定実辞書の通常28項目/保存・再変換等7項目が成功。初期入力/全語彙/実TSFをこの検査だけで合格にしない。
