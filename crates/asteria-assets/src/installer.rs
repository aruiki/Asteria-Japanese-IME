//! インストーラ (WiX UI) 用 BMP 画像 (`dialog.bmp`, `banner.bmp`) と `license.rtf`・NOTICE 群 (`gen_assets.py`)。

use std::fs;
use std::path::Path;

use anyhow::Result;
use image::{imageops, DynamicImage, GrayImage, Rgb, RgbImage, RgbaImage};

use crate::canvas::{draw_text_line, LoadedFont};
use crate::icons::render_product_mark;

/// `gen_assets.py` 互換の RTF ASCII エスケープ。
pub fn rtf_escape(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '\\' | '{' | '}' => {
                o.push('\\');
                o.push(ch);
            }
            '\n' => o.push_str("\\par\n"),
            c if (c as u32) < 128 => o.push(c),
            c => {
                let n = c as u32;
                let signed = if n < 32768 {
                    n as i32
                } else {
                    n as i32 - 65536
                };
                o.push_str(&format!("\\u{signed}?"));
            }
        }
    }
    o
}

fn gradient_rgb(w: u32, h: u32, top: [u8; 3], bottom: [u8; 3]) -> RgbImage {
    let mut img = RgbImage::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let t = (y as f32 / h as f32) * 0.75 + (x as f32 / w as f32) * 0.25;
            let px = [
                (top[0] as f32 + (bottom[0] as f32 - top[0] as f32) * t) as u8,
                (top[1] as f32 + (bottom[1] as f32 - top[1] as f32) * t) as u8,
                (top[2] as f32 + (bottom[2] as f32 - top[2] as f32) * t) as u8,
            ];
            img.put_pixel(x, y, Rgb(px));
        }
    }
    img
}

fn paste_rgba_onto_rgb(dst: &mut RgbImage, src: &RgbaImage, ox: u32, oy: u32) {
    for sy in 0..src.height() {
        let dy = oy + sy;
        if dy >= dst.height() {
            continue;
        }
        for sx in 0..src.width() {
            let dx = ox + sx;
            if dx >= dst.width() {
                continue;
            }
            let p = src.get_pixel(sx, sy).0;
            let a = p[3] as f32 / 255.0;
            let bg = dst.get_pixel(dx, dy).0;
            dst.put_pixel(
                dx,
                dy,
                Rgb([
                    (p[0] as f32 * a + bg[0] as f32 * (1.0 - a)).round() as u8,
                    (p[1] as f32 * a + bg[1] as f32 * (1.0 - a)).round() as u8,
                    (p[2] as f32 * a + bg[2] as f32 * (1.0 - a)).round() as u8,
                ]),
            );
        }
    }
}

/// `gen_assets.py`: WiX UI BMP 画像とライセンス文 (`license.rtf`, `NOTICE-*.txt`) を生成する。
pub fn run_gen_assets(out: &Path, images_only: bool, repo_root: &Path) -> Result<()> {
    fs::create_dir_all(out)?;
    let asteria_png = repo_root.join("docs/design/2026-10-asteria/asteria-icon.png");

    // dialog.bmp (493x312)
    let mut dlg = RgbImage::from_pixel(493, 312, Rgb([240, 240, 240]));
    let mut panel_rgb = gradient_rgb(164, 312, [17, 28, 45], [13, 22, 36]);
    let mut glow = GrayImage::new(164, 312);
    for y in 0..312i32 {
        for x in 0..164i32 {
            let nx = (x as f32 - 50.0) / 140.0;
            let ny = (y as f32 - 45.0) / 125.0;
            if nx * nx + ny * ny <= 1.0 {
                glow.put_pixel(x as u32, y as u32, image::Luma([8]));
            }
        }
    }
    let glow_blur = imageops::blur(&glow, 44.0);
    for y in 0..312 {
        for x in 0..164 {
            let m = glow_blur.get_pixel(x, y).0[0] as f32 / 255.0;
            let bg = panel_rgb.get_pixel(x, y).0;
            let fg = [55u8, 65, 77];
            panel_rgb.put_pixel(
                x,
                y,
                Rgb([
                    (fg[0] as f32 * m + bg[0] as f32 * (1.0 - m)).round() as u8,
                    (fg[1] as f32 * m + bg[1] as f32 * (1.0 - m)).round() as u8,
                    (fg[2] as f32 * m + bg[2] as f32 * (1.0 - m)).round() as u8,
                ]),
            );
        }
    }
    let mark = render_product_mark(&asteria_png, 132)?;
    paste_rgba_onto_rgb(&mut panel_rgb, &mark, 16, 44);
    let mut panel_rgba = DynamicImage::ImageRgb8(panel_rgb).to_rgba8();
    let serif = LoadedFont::find_system_or_fallback(&[("NotoSerifJP-VF.ttf", 0, None)]);
    let goths = LoadedFont::find_system_or_fallback(&[("YuGothM.ttc", 0, None)]);
    let title = "Asteria";
    let tw = serif
        .as_ref()
        .and_then(|f| f.measure_width(title, 25.0).ok())
        .unwrap_or(80.0);
    draw_text_line(
        &mut panel_rgba,
        serif.as_ref(),
        82.0 - tw * 0.5,
        194.0,
        title,
        25.0,
        [239, 224, 199, 255],
    );
    let sub = "日本語入力";
    let sw = goths
        .as_ref()
        .and_then(|f| f.measure_width(sub, 14.0).ok())
        .unwrap_or(70.0);
    draw_text_line(
        &mut panel_rgba,
        goths.as_ref(),
        82.0 - sw * 0.5,
        229.0,
        sub,
        14.0,
        [176, 185, 195, 255],
    );
    for py in 262..=264 {
        for px in 74..=90 {
            panel_rgba.put_pixel(px, py, image::Rgba([239, 224, 199, 255]));
        }
    }
    let panel_final = DynamicImage::ImageRgba8(panel_rgba).to_rgb8();
    imageops::replace(&mut dlg, &panel_final, 0, 0);
    DynamicImage::ImageRgb8(dlg).save(out.join("dialog.bmp"))?;

    // banner.bmp (493x58)
    let mut ban = RgbImage::from_pixel(493, 58, Rgb([255, 255, 255]));
    for px in 0..493 {
        ban.put_pixel(px, 57, Rgb([224, 224, 224]));
    }
    let icon = render_product_mark(&asteria_png, 48)?;
    paste_rgba_onto_rgb(&mut ban, &icon, 437, 5);
    DynamicImage::ImageRgb8(ban).save(out.join("banner.bmp"))?;

    if images_only {
        return Ok(());
    }

    let rtf_body = rtf_escape(LICENSE_TEXT);
    let rtf = format!(
        "{{\\rtf1\\ansi\\ansicpg932\\deff0{{\\fonttbl{{\\f0\\fnil\\fcharset128 Yu Gothic UI;}}}}\
\\viewkind4\\uc1\\pard\\f0\\fs18 {rtf_body}}}"
    );
    fs::write(out.join("license.rtf"), rtf.as_bytes())?;
    fs::write(
        out.join("NOTICE-tinyswallow.txt"),
        NOTICE_TINYSWALLOW.as_bytes(),
    )?;
    fs::write(
        out.join("NOTICE-third-party.txt"),
        NOTICE_THIRD_PARTY.as_bytes(),
    )?;
    fs::write(out.join("NOTICE-zenz.txt"), NOTICE_ZENZ.as_bytes())?;
    Ok(())
}

pub const LICENSE_TEXT: &str = "\
Asteria日本語入力 使用許諾と第三者ソフトウェアの表示

Asteria日本語入力(以下 Asteria)は、Apache License 2.0 または MIT License で提供する Rust 製の AI 日本語入力です。

本ソフトウェアは「現状のまま」提供され、明示または黙示を問わず、いかなる保証もありません。

同梱している主な部品とライセンス:
・Asteria 本体 (asteria-server.exe, asteria-renderer.exe, asteria-config.exe, asteria_tip.dll): Apache-2.0 OR MIT
・システム辞書原典 (Mozc 辞書データ / IPAdic 由来): BSD-3-Clause(Google Inc. 等)
・llama.cpp / ggml: MIT(Copyright The ggml authors)
・LLVM OpenMP ランタイム(libomp.dll): Apache-2.0 WITH LLVM-exception
・zenz-v2.5-small: CC BY-SA 4.0(© Keita Miwa。元は ku-nlp/gpt2-small-japanese-char)。8 bit に量子化して同梱
・genmai-310M: CC BY-SA 4.0(subdiox。zenz-v2.5-medium © Keita Miwa を変換用に追加学習したもの。元は ku-nlp/gpt2-medium-japanese-char)。8 bit に量子化して同梱
・TinySwallow-1.5B: Apache-2.0(Sakana AI)。5 bit に量子化して同梱
・カタカナ英語辞書: CC BY-SA 3.0(カタカナ語英字辞典コミュニティ、KEINOS、EDICT/EDRDG)。加工済みデータと出所は AsteriaEnglish.tsv、AsteriaEnglish-source.json、NOTICE-katakana-english.txt
・初期変換の追加連語: CC BY-SA 4.0(Wikipedia日本語版の執筆者、Keita Miwa)。短い連語へ加工。出典と変更内容は NOTICE-fast-collocations.txt
・短文の追加語彙: SudachiDict 20261015、Apache-2.0。system.dict に統合および加工TSVを同梱

各ライセンスの内容・参照先と帰属表示は、インストール先(C:\\Program Files\\Asteria)の NOTICE-*.txt と LICENSE-* にあります。

Asteria は既定でインターネットに接続しません。変換と AI の処理は、すべてこの PC の中で行います。
";

pub const NOTICE_TINYSWALLOW: &str = "\
Asteria日本語入力に同梱している言語モデル(LLM)

TinySwallow-1.5B (c) Sakana AI
  https://huggingface.co/SakanaAI/TinySwallow-1.5B
  ライセンス: Apache License 2.0 https://www.apache.org/licenses/LICENSE-2.0

変更点: 公開されている重みを llama.cpp で GGUF(f16)にし、Q5_K_M に量子化した。
学習した内容は変えていない。

llama.cpp / ggml (MIT): https://github.com/ggml-org/llama.cpp
  インストール先の LICENSE-llama.cpp.txt を参照。
";

pub const NOTICE_THIRD_PARTY: &str = "\
Asteria日本語入力に同梱している第三者のソフトウェア

この PC のインストール先(C:\\Program Files\\Asteria)にあるファイルと、そのライセンスの一覧です。
AI のモデルは NOTICE-zenz.txt と NOTICE-tinyswallow.txt にあります。
カタカナ英語辞書は NOTICE-katakana-english.txt、加工済みデータは AsteriaEnglish.tsv にあります。

■ Asteria Rust バイナリ (asteria-server.exe, asteria-renderer.exe, asteria-config.exe, asteria_tip.dll)
  Apache License 2.0 OR MIT License。利用クレートの帰属は Cargo.lock / about.toml を参照。

■ システム辞書原典データ (Mozc OSS 辞書・IPAdic・SudachiDict)
  BSD-3-Clause (Google Inc.) / Apache-2.0 (SudachiDict)

■ Microsoft Edge WebView2 (Windows 標準 WebView2 ランタイム / COM ICoreWebView2)
  設定画面 (asteria-config.exe) は Windows 導入済みの Microsoft Edge WebView2 環境を使用します。

■ llama.cpp / ggml b11259(llama.dll、ggml*.dll)
  MIT License、Copyright The ggml authors。全文は LICENSE-llama.cpp.txt。
  https://github.com/ggml-org/llama.cpp の公式の Windows ビルドを使用しています。
  ggml-base.dllだけは同じb11259のソースから再構築しています。
  変更: context bufferの確保失敗をNULL/例外で返し、IME全体の強制終了を避けます。
  変更箇所とビルド手順: AsteriaのMozcパッチ内src/bazel/llama_oom.patchとBUILD.llama_cpp.bazel。

■ 短文の追加語彙(short_lexicon.tsv)
  SudachiDict 20261015、Works Applications Co., Ltd.、Apache-2.0。
  UniDic・NEologd等の全帰属・ライセンス全文はNOTICE-short-lexicon.txt。
  名詞をMozcの品詞・共通語彙コストへ対応し、既存の読み/表記を上書きしない。
  全量Wikipedia頻度による選択・コスト加工はCC BY-SA 4.0。
  Wikipedia日本語版の執筆者、Keita Miwa / zenz-v2.5-dataset。
  加工済みTSVと固定生成記録short_lexicon.jsonを同梱する。

■ 初期変換の追加連語(fast_collocations.txt)
  CC BY-SA 4.0、Wikipedia日本語版の執筆者、Keita Miwa。
  zenz-v2.5-datasetのWikipedia学習区間から短い連語へ加工。
  取得区間・変更・帰属はNOTICE-fast-collocations.txtとfast_collocations.json。
  元のMozc40件はBSD-3-Clause。

■ 用例由来の追加連語
  Japanese Usage Dictionary 2025-01-25、BSD-2-Clause。
  Copyright 2011 Hiroyuki Komatsu, Shoji Ogura, Tsuyoshi Horo。
  引用用例の短い連語を抽出し、末尾をMozcの活用規則で展開。
  出典とライセンス全文はNOTICE-fast-collocations.txt。

■ LLVM OpenMP ランタイム(libomp.dll)
  Apache License 2.0 with LLVM Exceptions。全文は LICENSE-llvm-openmp.txt。
  https://github.com/llvm/llvm-project(llama.cpp の公式の Windows ビルドに含まれるもの)。

■ Microsoft Visual C++ ランタイム(vcruntime140*.dll、msvcp140*.dll)
  Microsoft Visual Studio のライセンスで再配布が認められたもの。
";

pub const NOTICE_ZENZ: &str = "\
Asteria IME に同梱している言語モデル

zenz-v2.5-small (c) Keita Miwa
  https://huggingface.co/Miwa-Keita/zenz-v2.5-small
  ライセンス: CC BY-SA 4.0 https://creativecommons.org/licenses/by-sa/4.0/
  元にしたモデル: ku-nlp/gpt2-small-japanese-char (CC BY-SA 4.0)

genmai-310M (c) subdiox
  https://huggingface.co/subdiox/genmai-310M
  ライセンス: CC BY-SA 4.0 https://creativecommons.org/licenses/by-sa/4.0/
  元にしたモデル: zenz-v2.5-medium (c) Keita Miwa (CC BY-SA 4.0)、ku-nlp/gpt2-medium-japanese-char (CC BY-SA 4.0)

変更点: 公開されている重みを llama.cpp の convert_hf_to_gguf.py で GGUF にし、
8 bit(q8_0)に量子化した。語彙と学習した内容は変えていない。
これらのファイル(zenz-v2.5-small-q8_0.gguf、genmai-310M-q8_0.gguf)は同じ CC BY-SA 4.0 で
利用できる。

llama.cpp (MIT): https://github.com/ggml-org/llama.cpp


追加生成モデル: zenz-v2.5-native-generator-20261017-q8_0.gguf
  zenz-v2.5-small (c) Keita Miwa をAsteria開発者が追加学習した派生モデル。
  CC BY-SA 4.0 https://creativecommons.org/licenses/by-sa/4.0/
  元モデル: https://huggingface.co/Miwa-Keita/zenz-v2.5-small
  変更: 公開コーパスの参照表記と実変換候補からLoRA対比学習(step400)し、統合後Q8_0へ量子化。
  用途: 追加の候補生成のみ。元のzenzとTinySwallowで候補を採点する。
  SHA-256と学習原票識別: native_generator.json
  学習・取得手順: https://github.com/aruiki/Asteria-Japanese-IME/tree/main/training/contrastive
  この派生モデルもCC BY-SA 4.0で利用できる。原モデル提供者はAsteriaを推奨していない。
";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rtf_escape_handles_ascii_and_japanese() {
        assert_eq!(rtf_escape("A\\{B}\nあ"), "A\\\\\\{B\\}\\par\n\\u12354?");
        assert!(rtf_escape(LICENSE_TEXT).is_ascii());
    }
}
