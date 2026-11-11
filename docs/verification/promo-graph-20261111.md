# 紹介動画と README の GIF のグラフを測り直した値に差し替える(2026-11-11)

利用者の依頼「README と動画用に根拠を作る」の続き。比較グラフの数字を
[2026-11-11 の実入力の測定](accuracy-realinput-20261111.md)に差し替えた。**利用者の承認の前なので、
公式サイト(aruiki/kanai)の動画はまだ替えていない**([要件](../marketing/promo-video-v3-requirements.md)の「完成の条件」)。

## 変えたもの

| | 前(2026-10) | 後(2026-11-11) |
| --- | ---: | ---: |
| 前の文で正解が決まる同音語・Asteria | 91%(274/300) | 91%(273/300) |
| 同・Google日本語入力 | 44%(133/300) | 45%(135/300) |
| 日常の短文・Asteria | 92% | 95%(474/500) |
| 日常の短文・Google日本語入力 | 87%(434/500) | 87%(434/500) |
| 注記の版と日付 | Asteria v3.0.0(3.34.6457、2026年10月25日)/ Google 3.34.6260(10月23日) | Asteria 開発版(3.34.6464、2026年11月11日)/ Google 3.34.6260(2026年11月11日) |

棒の長さは正解率そのもの(91.0%・45.0%・94.8%・86.8%)。それ以外の場面(Word・ホワイトボード・設定画面・
表紙・終わり)と音は元のまま。

## 作り方

元の動画を組み立てた道具(紙の地・琴の音・場面の合成)はこの PC にもリポジトリにも残っていなかった。
そこで、元の映像のグラフの場面(フレーム 830〜1000)だけを
[promo_graph_patch.py](../marketing/promo_graph_patch.py) で描き直した。

- 紙の地は、表紙・終わり・最初の場面・グラフの場面の紙を重ね、画素ごとに明るい方(文字やマークの無い方)を使う。
- 字体は元の映像に合わせて選んだ。数字は Noto Sans JP 44px(誤差 3/255)、注記は Source Han Sans 22px。
- 動きは元の映像から読む。各フレームの棒の長さと、出ている数字(描いた数字との照合)を読み、同じ割合で新しい値に置き換える。
  段ごとの現れ方(同音語の段 27.5〜28.0 秒、短文の段 28.1〜28.6 秒、注記 29.3〜30.0 秒)と、最後の
  消え方(32.9〜33.2 秒)も元の映像から測った不透明度を使う。
- 確かめ方: `--identity`(元の値と元の注記)で描き直すと元の映像と一致する(画面全体の平均差 0.4〜1.2/255、
  目で見て区別できない)ことを確かめてから、新しい値で描いた。
- 動画は libx264 CRF 17 で書き出し直し、音はそのまま写した(38.90 秒、1920×1080、30fps)。差し替えた
  フレームが元のフレーム番号と一致することを、動いている途中のフレームで確かめた。
- README の GIF は、GIF の 98〜178 枚目(グラフの場面)を、同じ時刻の描き直したフレームの縮小に替えた。
  枚数(179)と各枚の表示時間(70/60 ms、計 11.94 秒)は元のまま。

```bash
# 作業フォルダーに元の動画のフレームを出す(ffmpeg は imageio-ffmpeg 同梱のもの)
ffmpeg -i docs/marketing/assets/asteria-v3-promo.mp4 -vf fps=10 $PROMO_WORK/all/a%03d.png
ffmpeg -i docs/marketing/assets/asteria-v3-promo.mp4 -vf "select='between(n\,807\,1005)'" -fps_mode passthrough -start_number 807 $PROMO_WORK/f30/n%04d.png
python docs/marketing/promo_graph_patch.py --identity --out ident     # 元と一致するか
python docs/marketing/promo_graph_patch.py --values 91.0,45.0,94.8,86.8 --foot docs/marketing/promo_graph_foot.json --out new
ffmpeg -i 元の動画 -framerate 30 -start_number 830 -i $PROMO_WORK/new/n%04d.png \
  -filter_complex "[1:v]setpts=PTS+830/30/TB[p];[0:v][p]overlay=eof_action=pass,format=yuv420p[v]" \
  -map "[v]" -map 0:a -c:v libx264 -preset slow -crf 17 -movflags +faststart -c:a copy asteria-v3-promo.mp4
```

## 承認の後にすること

- aruiki/kanai の `pages/asteria-v3-promo.mp4` と `site-assets/asteria-v3-promo.mp4` を差し替え、README の
  動画のリンクの `?v=` を新しい値にする(古い動画がキャッシュに残らないように)。
