//! README 用図版・比較グラフの描画 (`docs/images/{asteria_style,gen_ime_comparison,gen_readme_images}.py`)。

use std::fs;
use std::path::Path;

use anyhow::{bail, Context, Result};
use image::{Rgba, RgbaImage};
use serde_json::Value;

use crate::canvas::{draw_text_line, rgba_from_hex, LoadedFont};

pub const SCALE: u32 = 2;
pub const NAVY: &str = "#111c2d";
pub const PANEL: &str = "#192638";
pub const CREAM: &str = "#fff0d7";
pub const TEXT: &str = "#faf5eb";
pub const SUB: &str = "#cbd3dc";
pub const LINE: &str = "#526174";
pub const GRAY: &str = "#bbc8d6";
pub const GRAY2: &str = "#91a7bd";

pub struct Figure {
    pub width: u32,
    pub height: u32,
    pub image: RgbaImage,
    pub body_top: f32,
    sans: Option<LoadedFont>,
    bold: Option<LoadedFont>,
}

impl Figure {
    pub fn new(height: u32, title: &str, subtitle: &str, width: u32) -> Self {
        let mut image = RgbaImage::from_pixel(width * SCALE, height * SCALE, rgba_from_hex(NAVY));
        let mut seed: u64 = 5;
        let mut next = || {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            (seed >> 33) as u32
        };
        let n_specks = (width * height) / 50;
        for _ in 0..n_specks {
            let x = next() % (width * SCALE);
            let y = next() % (height * SCALE);
            let len = 1 + (next() % 6);
            for dx in 0..=len {
                let px = (x + dx).min(width * SCALE - 1);
                image.put_pixel(px, y, Rgba([22, 33, 48, 255]));
            }
        }
        let mut f = Self {
            width,
            height,
            image,
            body_top: 0.0,
            sans: LoadedFont::find_system_or_fallback(&[
                ("YuGothM.ttc", 0, None),
                ("NotoSansJP-VF.ttf", 0, None),
            ]),
            bold: LoadedFont::find_system_or_fallback(&[
                ("YuGothB.ttc", 0, None),
                ("NotoSansJP-VF.ttf", 0, Some(700.0)),
            ]),
        };
        let y = f.paragraph(32.0, 24.0, title, 30.0, TEXT, true, None, Some(43.0));
        let y = f.paragraph(32.0, y + 12.0, subtitle, 18.0, SUB, false, None, Some(28.0));
        f.rule(32.0, y + 16.0, width as f32 - 32.0, LINE);
        f.body_top = y + 38.0;
        f
    }

    pub fn text(&mut self, x: f32, y: f32, value: &str, size: f32, color: &str, bold: bool) -> f32 {
        let font = if bold {
            self.bold.as_ref()
        } else {
            self.sans.as_ref()
        };
        let col = rgba_from_hex(color).0;
        draw_text_line(
            &mut self.image,
            font,
            x * SCALE as f32,
            y * SCALE as f32,
            value,
            size * SCALE as f32,
            col,
        ) / SCALE as f32
    }

    #[allow(clippy::too_many_arguments)]
    pub fn paragraph(
        &mut self,
        x: f32,
        mut y: f32,
        value: &str,
        size: f32,
        color: &str,
        bold: bool,
        width: Option<f32>,
        line_height: Option<f32>,
    ) -> f32 {
        let avail = width.unwrap_or(self.width as f32 - 32.0 - x);
        let lh = line_height.unwrap_or((size * 1.5).round());
        let mut lines_out = Vec::new();
        {
            let font = if bold {
                self.bold.as_ref()
            } else {
                self.sans.as_ref()
            };
            for line in value.split('\n') {
                let mut cur = String::new();
                for ch in line.chars() {
                    let mut probe = cur.clone();
                    probe.push(ch);
                    let w = font
                        .and_then(|f| f.measure_width(&probe, size).ok())
                        .unwrap_or(probe.chars().count() as f32 * size);
                    if !cur.is_empty() && w > avail {
                        lines_out.push(std::mem::take(&mut cur));
                    }
                    cur.push(ch);
                }
                if !cur.is_empty() {
                    lines_out.push(cur);
                }
            }
        }
        for line in lines_out {
            self.text(x, y, &line, size, color, bold);
            y += lh;
        }
        y
    }

    pub fn rule(&mut self, x: f32, y: f32, right: f32, color: &str) {
        let c = rgba_from_hex(color);
        let x0 = (x * SCALE as f32).round().max(0.0) as u32;
        let x1 = (right * SCALE as f32)
            .round()
            .min(self.image.width() as f32) as u32;
        let y0 = (y * SCALE as f32).round().max(0.0) as u32;
        for py in y0..(y0 + 2).min(self.image.height()) {
            for px in x0..x1 {
                self.image.put_pixel(px, py, c);
            }
        }
    }

    pub fn panel(&mut self, x: f32, y: f32, w: f32, h: f32) {
        let c = rgba_from_hex(PANEL);
        let x0 = (x * SCALE as f32).round().max(0.0) as u32;
        let y0 = (y * SCALE as f32).round().max(0.0) as u32;
        let x1 = ((x + w) * SCALE as f32)
            .round()
            .min(self.image.width() as f32) as u32;
        let y1 = ((y + h) * SCALE as f32)
            .round()
            .min(self.image.height() as f32) as u32;
        for py in y0..y1 {
            for px in x0..x1 {
                self.image.put_pixel(px, py, c);
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn bar(
        &mut self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        value: f64,
        maximum: f64,
        color: &str,
    ) {
        if value <= 0.0 || maximum <= 0.0 {
            return;
        }
        let c = rgba_from_hex(color);
        let x0 = (x * SCALE as f32).round().max(0.0) as u32;
        let y0 = (y * SCALE as f32).round().max(0.0) as u32;
        let x1 = ((x as f64 + width as f64 * value / maximum) * SCALE as f64).round() as u32;
        let y1 = ((y + height) * SCALE as f32).round() as u32;
        for py in y0..y1.min(self.image.height()) {
            for px in x0..x1.min(self.image.width()) {
                self.image.put_pixel(px, py, c);
            }
        }
    }

    pub fn footer(&mut self, lines: &[&str]) {
        let mut wrapped = Vec::new();
        for &line in lines {
            let mut cur = String::new();
            for ch in line.chars() {
                if !cur.is_empty()
                    && (cur.chars().count() + 1) as f32 * 16.0 > self.width as f32 - 64.0
                {
                    wrapped.push(std::mem::take(&mut cur));
                }
                cur.push(ch);
            }
            wrapped.push(cur);
        }
        let start = self.height as f32 - 24.0 - (wrapped.len() as f32) * 25.0;
        self.rule(32.0, start - 16.0, self.width as f32 - 32.0, LINE);
        for (i, line) in wrapped.iter().enumerate() {
            self.text(32.0, start + i as f32 * 25.0, line, 16.0, SUB, false);
        }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(p) = path.parent() {
            fs::create_dir_all(p)?;
        }
        self.image.save(path)?;
        Ok(())
    }
}

/// `gen_ime_comparison.py`: 固定 CSV (`eval/imebench/comparison-2026-10-01.csv`) から図版を生成する。
pub fn run_gen_ime_comparison(out_dir: Option<&Path>, repo_root: &Path) -> Result<()> {
    let csv_path = repo_root.join("eval/imebench/comparison-2026-10-01.csv");
    let mut rdr = csv::Reader::from_path(&csv_path)?;
    let mut rows = Vec::new();
    for r in rdr.records() {
        let rec = r?;
        let pct: f64 = rec.get(6).context("ajimee_198_percent")?.parse()?;
        rows.push((
            rec.get(0).unwrap_or("").to_owned(),
            rec.get(1).unwrap_or("").to_owned(),
            rec.get(5).unwrap_or("").to_owned(),
            pct,
        ));
    }
    if rows.len() < 3 {
        bail!("comparison CSV 行数不足");
    }
    let mut f = Figure::new(
        790,
        "過去の実入力比較（2026-10-01）",
        "AJIMEE-Bench 198問 ／ Space 1回の第1候補",
        640,
    );
    let names = [
        "Asteria beta.8・Unreal",
        "Microsoft IME",
        "Google 日本語入力",
    ];
    let colors = [CREAM, GRAY, GRAY2];
    for (i, ((_, _, _, val), (name, color))) in
        rows.iter().zip(names.iter().zip(colors.iter())).enumerate()
    {
        let y = f.body_top + i as f32 * 133.0;
        f.text(32.0, y, name, 25.0, TEXT, true);
        f.text(453.0, y - 9.0, &format!("{val:.1}%"), 38.0, color, true);
        f.bar(32.0, y + 49.0, 560.0, 34.0, *val, 100.0, color);
    }
    let (ours, msime, google) = (rows[0].3, rows[1].3, rows[2].3);
    f.text(
        32.0,
        570.0,
        &format!("Microsoft IMEとの差 +{:.1}ポイント", ours - msime),
        24.0,
        CREAM,
        true,
    );
    f.text(
        32.0,
        612.0,
        &format!("Google 日本語入力との差 +{:.1}ポイント", ours - google),
        24.0,
        CREAM,
        true,
    );
    f.footer(&[
        "2026-10-01 ／ RTX 3060 ／ 全IMEで前の文なし",
        "Asteria beta.8・Unreal ／ Microsoft IME（Windows 11）",
        "Google 日本語入力：3.34.6260。現在版の再測定ではありません。",
    ]);
    let dest = out_dir
        .unwrap_or_else(|| Path::new("docs/images"))
        .join("ime-comparison.png");
    let full = if dest.is_absolute() {
        dest
    } else {
        repo_root.join(dest)
    };
    f.save(&full)
}

/// `gen_readme_images.py`: `docs/images/evaluation-2026-10-19.json` から README 用 PNG 一式を描画する。
pub fn run_gen_readme_images(out_dir: Option<&Path>, repo_root: &Path) -> Result<()> {
    let dir = match out_dir {
        Some(d) if d.is_absolute() => d.to_path_buf(),
        Some(d) => repo_root.join(d),
        None => repo_root.join("docs/images"),
    };
    let data: Value = serde_json::from_str(&fs::read_to_string(
        repo_root.join("docs/images/evaluation-2026-10-19.json"),
    )?)?;

    // features.png
    let items = [
        (
            "01",
            "文脈を読む",
            ["同じ読みを持つ言葉も、", "文章のつながりから選びます。"],
        ),
        (
            "02",
            "続きを予測する",
            [
                "入力を止めると、裏で次の候補を",
                "考えます。Tabで選べます。",
            ],
        ),
        (
            "03",
            "PCの中で完結",
            ["辞書もAIモデルもPC内に。", "入力した文章を外へ送りません。"],
        ),
        (
            "04",
            "いつもの操作で",
            ["Mozcの操作、学習、", "ユーザー辞書を使えます。"],
        ),
        (
            "05",
            "環境に合わせる",
            ["専用GPUがないPCでは、", "CPU向けの設定に切り替えます。"],
        ),
        (
            "06",
            "使わない間は休む",
            ["10分間入力がなければ、", "AIモデルのVRAMを解放します。"],
        ),
    ];
    let mut ff = Figure::new(
        1200,
        "考えることに、集中できる入力を。",
        "いつもの日本語入力に、文脈を読むAIを。",
        640,
    );
    for (i, (num, head, lines)) in items.iter().enumerate() {
        let y = ff.body_top + i as f32 * 160.0;
        ff.panel(24.0, y - 6.0, 592.0, 144.0);
        ff.text(40.0, y + 10.0, num, 24.0, CREAM, true);
        ff.text(96.0, y + 7.0, head, 30.0, TEXT, true);
        for (j, line) in lines.iter().enumerate() {
            ff.text(40.0, y + 58.0 + j as f32 * 34.0, line, 24.0, TEXT, false);
        }
    }
    ff.save(&dir.join("features.png"))?;

    // examples.png
    let mut ef = Figure::new(
        1220,
        "日常の言葉を、基礎から。",
        "追加連語による改善例 ／ AIなし・前の文なし",
        640,
    );
    if let Some(rows) = data["ordinary_examples"].as_array() {
        for (i, row) in rows.iter().enumerate() {
            let y = ef.body_top + i as f32 * 244.0;
            ef.panel(24.0, y - 8.0, 592.0, 228.0);
            ef.text(
                40.0,
                y,
                row["input"].as_str().unwrap_or(""),
                20.0,
                SUB,
                false,
            );
            let yy = y + 40.0;
            ef.text(40.0, yy, "変更前", 20.0, SUB, true);
            ef.text(
                40.0,
                yy + 29.0,
                row["before"].as_str().unwrap_or(""),
                28.0,
                TEXT,
                true,
            );
            ef.text(40.0, yy + 73.0, "Asteria 開発版", 20.0, CREAM, true);
            ef.text(
                40.0,
                yy + 102.0,
                row["after"].as_str().unwrap_or(""),
                28.0,
                CREAM,
                true,
            );
        }
    }
    ef.footer(&[
        "2026-10-19更新 ／ v1.1.4開発候補・オフライン評価",
        "日常81問：65→69正解。改善4問・改悪0問。",
        "実アプリの初回変換を測った結果ではありません。",
    ]);
    ef.save(&dir.join("examples.png"))?;

    // accuracy.png
    let mut af = Figure::new(
        1050,
        "Asteria 開発版の変換精度",
        "AJIMEE-Bench 200問 ／ オフライン評価",
        640,
    );
    let top = af.body_top;
    let best = &data["modes"][2]["scores"]["ajimee"];
    let b_pct =
        100.0 * best["correct"].as_f64().unwrap_or(0.0) / best["total"].as_f64().unwrap_or(1.0);
    af.text(32.0, top, &format!("{b_pct:.1}%"), 76.0, CREAM, true);
    af.text(32.0, top + 112.0, "188 / 200 正解", 28.0, TEXT, true);
    af.text(
        32.0,
        top + 162.0,
        "Standard・AI準備後・前の文あり",
        24.0,
        SUB,
        false,
    );
    af.rule(32.0, top + 220.0, 608.0, LINE);
    for (i, color) in [GRAY2, GRAY, CREAM].into_iter().enumerate() {
        let mode = &data["modes"][i];
        let y = top + 253.0 + i as f32 * 130.0;
        let sc = &mode["scores"]["ajimee"];
        let c = sc["correct"].as_u64().unwrap_or(0);
        let t = sc["total"].as_u64().unwrap_or(1);
        let val = 100.0 * c as f64 / t as f64;
        af.text(
            32.0,
            y,
            mode["label"].as_str().unwrap_or(""),
            24.0,
            TEXT,
            true,
        );
        af.text(470.0, y + 35.0, &format!("{val:.1}%"), 34.0, color, true);
        af.text(32.0, y + 42.0, &format!("{c} / {t} 正解"), 22.0, SUB, false);
        af.bar(32.0, y + 86.0, 560.0, 23.0, val, 100.0, color);
    }
    af.footer(&[
        "2026-10-19更新 ／ 次期版 v1.1.4 開発候補",
        "AIなしはMozc＋追加連語。StandardはRTX 3060で測定。",
        "開発用データでの評価。初回や実アプリの精度とは別です。",
        "他IMEを含む最新の実入力比較は未実施です。",
    ]);
    af.save(&dir.join("accuracy.png"))?;

    // compare.png
    let mut cf = Figure::new(
        1760,
        "四つの開発評価で、条件を確認。",
        "Asteria v1.1.4 開発候補 ／ オフライン評価",
        640,
    );
    if let Some(sets) = data["sets"].as_array() {
        for (i, grp) in sets.iter().enumerate() {
            let key = grp["key"].as_str().unwrap_or("");
            let name = grp["label"].as_str().unwrap_or("");
            let total = data["modes"][0]["scores"][key]["total"]
                .as_u64()
                .unwrap_or(1);
            let y = cf.body_top + i as f32 * 350.0;
            cf.text(32.0, y, &format!("{name} · {total}問"), 28.0, TEXT, true);
            for (j, color) in [GRAY2, GRAY, CREAM].into_iter().enumerate() {
                let yy = y + 53.0 + j as f32 * 90.0;
                let mode = &data["modes"][j];
                let sc = &mode["scores"][key];
                let val = 100.0 * sc["correct"].as_f64().unwrap_or(0.0)
                    / sc["total"].as_f64().unwrap_or(1.0);
                cf.text(
                    32.0,
                    yy,
                    mode["label"].as_str().unwrap_or(""),
                    22.0,
                    TEXT,
                    j == 2,
                );
                cf.text(480.0, yy - 5.0, &format!("{val:.1}%"), 29.0, color, true);
                cf.bar(32.0, yy + 37.0, 560.0, 23.0, val, 100.0, color);
            }
            if i < 3 {
                cf.rule(32.0, y + 324.0, 608.0, LINE);
            }
        }
    }
    cf.footer(&[
        "2026-10-19更新 ／ StandardはAI準備後・RTX 3060",
        "AIなしはMozc＋追加連語。前文ありは指定した文脈を使用。",
        "調整・回帰用の固定データ。独立した製品評価ではありません。",
        "他IMEとの実入力比較とは測定条件が異なります。",
    ]);
    cf.save(&dir.join("compare.png"))?;

    // lightness.png
    let mut lf = Figure::new(
        1230,
        "入力を続けられるか、確かめる。",
        "ネイティブ入力処理の検査 ／ CPU・RTX 3060",
        640,
    );
    if let Some(timing) = data["native_input"].as_array() {
        let panels = [
            (
                "キー応答の最大",
                "通常キーの応答時間",
                "key_max_ms",
                "ms",
                4.0f64,
            ),
            (
                "Space応答の最大",
                "起動直後とAI準備後を含む",
                "space_max_ms",
                "ms",
                600.0,
            ),
            (
                "入力シナリオの成功",
                "文字欠落・確定差分などを確認",
                "passed",
                "件",
                30.0,
            ),
        ];
        for (i, (head, sub, field, unit, vmax)) in panels.into_iter().enumerate() {
            let y = lf.body_top + i as f32 * 270.0;
            lf.panel(24.0, y - 8.0, 592.0, 248.0);
            lf.text(40.0, y + 4.0, head, 30.0, TEXT, true);
            lf.text(40.0, y + 49.0, sub, 20.0, SUB, false);
            for (j, r) in timing.iter().enumerate() {
                let yy = y + 91.0 + j as f32 * 67.0;
                let color = if j == 0 { GRAY } else { CREAM };
                let val = r[field].as_f64().unwrap_or(0.0);
                lf.text(
                    40.0,
                    yy,
                    r["label"].as_str().unwrap_or(""),
                    22.0,
                    TEXT,
                    true,
                );
                let fmt = if unit == "ms" {
                    format!("{val:.2} {unit}")
                } else {
                    format!("{val:.0} / 30 {unit}")
                };
                lf.text(400.0, yy - 4.0, &fmt, 28.0, color, true);
                lf.bar(40.0, yy + 37.0, 552.0, 17.0, val, vmax, color);
            }
        }
    }
    lf.footer(&[
        "2026-10-19更新 ／ v1.1.4開発候補・固定バイナリ",
        "各30件：新規起動15件＋AI準備後15件。最大値を表示。",
        "Windows SessionHandlerの検査。実アプリの測定とは別です。",
        "低性能PCの実機検証や旧版との速度比較は未実施です。",
    ]);
    lf.save(&dir.join("lightness.png"))?;

    // how.png
    let mut hf = Figure::new(
        970,
        "言葉を選ぶ、その仕組み。",
        "候補づくりと文脈の評価を、PCの中で。",
        640,
    );
    let steps = [
        (
            "01",
            "候補を集める",
            ["Mozcの辞書と変換用AIが", "複数の候補を作ります。"],
        ),
        (
            "02",
            "文脈で評価する",
            ["前の文とのつながりから", "自然さを採点します。"],
        ),
        (
            "03",
            "読みを確かめる",
            ["生成された候補の読みを", "Mozcの辞書で確認します。"],
        ),
        (
            "04",
            "候補を表示する",
            ["評価に基づいて、", "変換結果を表示します。"],
        ),
    ];
    for (i, (num, head, lines)) in steps.iter().enumerate() {
        let y = hf.body_top + i as f32 * 175.0;
        hf.panel(24.0, y - 8.0, 592.0, 153.0);
        hf.text(40.0, y + 8.0, num, 26.0, CREAM, true);
        hf.text(96.0, y + 4.0, head, 30.0, TEXT, true);
        for (j, line) in lines.iter().enumerate() {
            hf.text(40.0, y + 60.0 + j as f32 * 35.0, line, 24.0, TEXT, false);
        }
    }
    hf.footer(&["AIが間に合わないときはMozcの結果で入力を続けます。"]);
    hf.save(&dir.join("how.png"))?;

    run_gen_ime_comparison(Some(&dir), repo_root)?;
    Ok(())
}
