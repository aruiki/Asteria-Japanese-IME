# Asteria「月の余白」の採用と制作記録

- 日付: 2026-10-11
- 選択: 利用者が10案中の05を明示的に採用。月、濃紺、生成り、墨と紙の質感。
- 生成方法: Codex内蔵image_gen。参照画像は concepts/05-moon-ink.png。
- 製品アイコン: asteria-icon.png。文字なし、タイル外は透過。
- READMEヒーロー: ../../images/asteria-hero.png。
- 図版7枚: Pillowで描画。数値・棒の比率・測定条件をコードで管理。
- 図版再生成: python docs/images/gen_readme_images.py
- 共通スタイル: docs/images/asteria_style.py（紺の紙、生成り、明朝見出し、細い罫線）。
- 実入力比較: eval/imebench/comparison-2026-10-01.csvを直接読み込む。
- 実画面の写真(settings*.png / candidate-window*.png)は今回の図版生成対象外。
- 設定・辞書ソフトのアイコンは従来の図柄を保持。

## 2026-10-19の情報更新

READMEとプレビューの主な掲載値を、旧Kotoriの88.4%からAsteria v1.1.4開発候補の188/200（94.0%）へ更新。94.0%はStandard・AI準備後・前文ありのオフライン開発評価であり、旧198問の実入力比較とは別条件。旧比較は日付付きの過去資料へ移し、値を保持する。

`../../images/evaluation-2026-10-19.json`に固定原票から12件の正解数と改善例4件を転記し、source・converter・原票SHAを記録。accuracy/compare/examples/lightnessを最新の開発評価から生成する。未測定のHigh/Unreal/Lowや他IMEの最新精度は補間しない。旧比較の生成元は引き続き当時のCSV。

測定の説明は[ベンチマーク](../../BENCHMARKS.md)。図版の更新だけで実アプリの検証や製品リリースが完了した扱いにはしない。

## moonIcon

Use case: identity-preserve. Adapt the supplied SELECTED Asteria concept 05 into the final Windows product app icon. Preserve its exact identity: one bold crescent moon made by a single ivory sumi brush stroke, opening toward upper right, same graceful curved shape and warm dry brush paper texture, against very dark midnight navy. Create one centered moon, no letters or wordmark. Strong calm silhouette with a broad solid ivory main body, restrained imperfect dry-brush edges rather than thin disconnected scratch lines, visible at small sizes. Moon occupies about 70% of a matte dark navy rounded square tile; tile occupies 94% of canvas. Keep the night-paper texture subtle and artisan quality, elegant flat appearance. Genuine transparent background outside the tile only. No star, no neon or glow, no metallic gold, no glass, no UI mockup, no additional objects. This reference image is a visual identity reference; remove all its text for this app icon.

## moonHero

Use case: logo-brand. Create the final wide landscape README hero for Asteria Japanese input software, faithfully extending selected concept 05 (reference image). Wide panoramic 3:1 horizontal composition. Very dark midnight navy matte paper with restrained sumi ink and fibrous texture, quiet large negative space. Right third: preserve the reference ivory crescent moon painted in a single assured sumi brushstroke, opening to upper right, its distinct paper and dry brush texture. Left two thirds: precise elegant generously spaced ivory serif wordmark "Asteria", smaller clean Japanese name "アステリア" underneath, and small tagline "考えたまま、言葉になる。" below. These are the ONLY texts. Exact text, no punctuation changes, no extra claims, no numbers or badges. Keep all lettering and moon well inside margins, balanced and calm. Refined literary Japanese tool identity, coherent reference palette. No stars, no neon, no glow, no gradients resembling glass, no horizon, no scenery, no metallic objects, no mockup.

## 過去の試作

10案は [比較ギャラリー](concepts/index.html)、旧星の試作はstar-icon-draft.png / star-hero-draft.pngに保存。
