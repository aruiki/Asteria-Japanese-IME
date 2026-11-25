# Asteria のアイコンと画面デザイン（2026-10-09）

ユーザーが選んだ **C案（折り紙の小鳥）** を採用候補として作り込んだ。
白いタイル、藍の紙、白い胸、朱色のくちばしを使う。文章や性能を誇張するコピーは加えない。

## 使える素材

- [Windows用ICO](icons/asteria-origami-v3.ico)：16 / 20 / 24 / 32 / 48 / 64 / 128 / 256px。全フレームBMP。
- [1024px PNG](icons/asteria-origami-v3-1024.png)：外側は透過。各サイズのPNGも同梱。
- [通常サイズのSVG](source/asteria-origami-v3.svg) / [32px以下のSVG](source/asteria-origami-v3-small.svg)。
- [明るい背景・暗い背景の実サイズ見本](icons/icon-sizes.png)。
- [画像生成によるC案の検討画像](origami-refinement.png)。

生成画像を検討の元にし、実際のアイコンは編集可能なSVGで形を整えた。
大きい版は紙の重なりに控えめな明暗を付け、小さい版は面を単純化して鳥を大きくし、目とくちばしを残す。
生成画像の細かな外周ノイズはSVGの素材へ持ち込まない。

```powershell
python mozc/tools/package_origami_icons.py docs/design/2026-10-asteria/icons
```

`skia-python` と `Pillow` が必要。フォント、通信、モデルは不要。
[検証記録](verification.json)に素材のSHA-256、ICOのサイズと形式、Windows SDKでの資源コンパイル結果を保存した。
全8フレームのICOを読み直し、PNGとの画素一致とアルファの保持を確認。100%で明暗両方の縮小見本を目視した。
これはアイコン素材の検証であり、MSIの導入や実際のIME操作の受け入れ結果ではない。

## 画面の方向

[画面の検討画像](ui-study.png)は候補欄と設定画面のデザイン見本。動作中の画面ではない。
この画像はC案選択前のB案アイコンを使っている。今後の実装ではC案へ統一する。

- 候補欄は文を主体にし、淡い藍の選択背景と控えめな番号を使う。意味のない下端の表示は増やさない。
- 設定画面は余白を設け、一般・AI変換・辞書・プライバシーへ整理する。
- Windowsのタイトルバー、キーボード操作、倍率、ハイコントラストを保つ。
- AIの状態や、このPCで処理することを短く表示する。装飾目的のAIマークや決め台詞は加えない。

## 本体への適用

素材PR141はmainへ統合済み。カード56で実際のMozc資源をC案へ更新した。
アプリ・言語バー・ツールの小鳥、設定・辞書のアイコン、インストーラーの背景に適用。
[実資源の見本](integration/windows-icons.png)、[WiX背景画像](integration/dialog.png)、
[資源の検証記録](integration/verification.json)を保存した。

```powershell
python mozc/tools/gen_icons.py <Mozcのsrc/data/images/win> <見本.png> --brand-only
python mozc/tools/gen_assets.py <Mozcのsrc/data/asteria> --images-only
```

入力モードを再生成せず、使用許諾とNOTICEも更新しない。
Mozc作業ツリーから `make_patches.sh` でパッチを再生成する。パッチを手で編集しない。
本体のWindows資源2ターゲットと設定ツールはビルド成功。
実行ファイル内の8アイコンも素材と一致した。[取り出したアイコン](integration/embedded-app-icons.png)。
MSIはPRのCIで確認する。
公開済みrc.2とインストール済みIMEはまだ変更していない。
設定画面の構造変更はカード57で扱う。生成された画面見本と実装済み画面を区別する。
## 検討過程

[3案の比較](icon-exploration.png)、[B案](bird-study.png)、[B案の縮小見本](bird-study-sizes.png)は履歴として残す。
最終候補はC案。画像生成には組み込みの `image_gen` を使用した。[プロンプト記録](PROMPTS.md)。