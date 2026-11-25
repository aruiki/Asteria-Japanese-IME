# 画像生成の記録

使用機能: 組み込み `image_gen`。外部API/CLIは使用していない。
生成画像は検討用。配布可能な縮小用素材はSVGを元に再生成する。

## 最終C案のプロンプト全文

参照: icon-exploration.png の右端C。transparent_background=true。
出力: origami-refinement.png。

> Use case: logo-brand. Create a refined production app icon for Asteria Japanese Input. Reference image 1 is a three-concept board. The user explicitly selected C, the RIGHTMOST icon: a geometric indigo origami little bird on a white rounded square. Use only C as the identity reference; ignore A and B. Deliver ONE standalone icon, no board, no text, no miniature versions, no labels. Square canvas with true transparent background outside the white tile. Tile occupies about 90 percent of canvas, rounded square with carefully continuous corners, pearl-white matte surface, very restrained soft depth and narrow edge shading. Preserve C's recognizable right-facing paper bird: broad indigo folded triangular tail at left, indigo central body rising into a fold at top, white triangular head/chest at right, one small round dark indigo eye, small coral-orange triangular beak. Refine the geometry so all fold edges meet intentionally and the silhouette reads as a small friendly bird rather than a paper boat or letter. Bird occupies about 68 percent of tile width, centered optically, generous even air around it. Two or three broad blue paper planes only, subtly different indigo tones, believable folds, crisp clean edges, minimal physical paper relief. Do not add a detached wing, extra folds, feather details, glossy plastic, grain, sparkles or dramatic cast shadows. Shape, eye and beak must remain clear at 16-32 pixels. Quiet, warm, carefully crafted visual quality appropriate for a gentle open-source desktop app. No Apple logo, no other brand symbol, no lettering or watermark. Preserve actual alpha transparency and perfectly clean transparent exterior with no colored speckles.

外周に生成由来のノイズが残ったため、このPNGをそのままICOに縮小していない。
形と色をSVGで実装し、通常用と縮小用を分けた。紙の質感をSVGで完全再現したという意味ではない。

## 先行検討の指定概要

### 3案の比較（icon-exploration.png）

Asteriaという穏やかなオープンソース日本語入力のアプリアイコンを3案で比較。
既存の藍と朱を継承。Aは「こ」と朱の点、Bは丸みのある白い小鳥、Cは白いタイルの上の折り紙の小鳥。
明るく静かなボードに大きい見本と小さい見本を置く。Appleのロゴや宣伝文を入れない。

### B案（bird-study.png、選択前の履歴）

Bを参照し、藍のタイル、白い小鳥、朱のくちばし、広い羽の面を整理。
単体の透過アプリアイコン。16pxで認識できる輪郭。文字は入れない。

### 画面見本（ui-study.png、C選択前）

B案を参照し、Windowsのメモ帳とコンパクトな候補欄、隣にAsteriaの設定画面。
例文「足を痛めたので、今日は休みます。」。候補欄は3行、淡い藍の選択行。
設定は「一般」「AI変換」「辞書」「プライバシー」、品質Low/Standard/High/Unreal、
「このPCで処理します」「入力中の予測」「Tabで候補を選べます」「AIは動作しています」。
Windowsのウィンドウ操作を保ち、白と中間色、余白、控えめな色、静かな書体で整理。
これは画像として生成したデザイン見本。例文の補正動作や設定の実装を検証したものではない。