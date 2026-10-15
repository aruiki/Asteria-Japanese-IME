# Asteriaの実入力デモ（2026-10-15）

READMEにはGIF、紹介サイトには操作できるMP4を掲載する。動画内の表示はAsteriaとし、候補選択を含む実際の入力操作を見せる。

## 収録内容

収録専用のWindows Forms TextBoxで、導入済みのAsteriaを当該プロセスに選択。ローマ字キーをSendInputで送り、Spaceで候補を選び、Enterで確定した。候補選択中の文字はIMM経由で読み取って照合し、正解文を入力欄に代入していない。ウィンドウの実画面だけを取り込み、別アプリへフォーカスが移ったら入力を止める。

実際の確定文:

- 貴社の記者が汽車で帰社した。
- 音声認識の精度が上がった。
- 明日の会議は１０時からです。

最初の文は文節ごとに候補を選ぶ操作を含む。2・3例目は候補を開き、先頭候補を選んで確定している。全例が無操作で一発変換できるという主張ではない。

当該プロセスへのmozc_tip64.dllのロードを確認し、実入力の確定結果と候補選択の原票を保存した。表示速度を早めたり、変換結果を描き替えたりしていない。330枚の実フレーム、1152×576、36.12秒。GIFは同一フレームをまとめた116フレーム、再生時間36.12秒。MP4は同じフレームと実測時間間隔から符号化。

## 成果物

- `docs/images/typing-demo.gif`: 224,890 bytes、SHA-256 `fcd94d57daddab1675263899f28bc986c518bbfa6186de8a9ac69a05b0122a69`
- 紹介サイト: https://aruiki.github.io/kanai/#typing-demo
- MP4: https://aruiki.github.io/kanai/typing-demo.mp4

READMEの相対画像参照、GIFの再生時間、実際の確定文とポスターを確認。サイトの再生操作・デコード・PC/スマートフォン幅はサイト側の検証で確認する。製品コード・モデル・MSIの変更ではなく、変換精度の新規評価でもない。

原票: 紹介サイトの作業領域 `.local/typing-demo/take-06/`（frames.tsv、candidate-trace.tsv、results.tsv、tip-path.txt、frame PNG）、`.local/typing-demo/encoded/receipt.json`。初期試行の失敗はtake-01〜05に保持。
