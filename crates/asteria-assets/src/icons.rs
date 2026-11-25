//! Asteria / Asteria の `.ico` / `.png` アイコン生成 (`asteria_mark.py`, `package_origami_icons.py`, `gen_icons.py`)。

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::Result;
use image::{Rgba, RgbaImage};

use crate::canvas::{
    alpha_composite, blend_pixel, fill_circle, fill_path, fill_round_rect, resize_lanczos,
    sdf_round_rect, stroke_path, stroke_round_rect, write_bmp_ico, FillStyle, LoadedFont, VecPath,
};

pub const U: f32 = 1024.0;
pub const SMALL_THRESHOLD: u32 = 32;
pub const ICON_SIZES: &[u32] = &[16, 20, 24, 32, 48, 64, 128, 256];
pub const PNG_SIZES: &[u32] = &[16, 20, 24, 32, 48, 64, 128, 256, 512, 1024];

pub const SHU: [u8; 4] = [0xF2, 0x5C, 0x2E, 255];
pub const WHITE: [u8; 4] = [255, 255, 255, 255];
pub const OUTLINE: [u8; 4] = [0x1A, 0x1A, 0x1A, 255];
pub const GRAY: [u8; 4] = [0x9A, 0x9A, 0x9A, 255];
pub const BRAND_INK: [u8; 4] = [0x28, 0x3C, 0x78, 255];

fn polygon(pts: &[(f32, f32)], closed: bool) -> VecPath {
    let mut p = VecPath::new();
    if let Some(&(x0, y0)) = pts.first() {
        p.move_to(x0, y0);
        for &(x, y) in &pts[1..] {
            p.line_to(x, y);
        }
        if closed {
            p.close();
        }
    }
    p
}

/// `asteria-origami-v3.svg` / `asteria-origami-v3-small.svg` を 1024x1024 キャンバスへ描画する。
pub fn draw_origami(canvas: &mut RgbaImage, small: bool) {
    let s = canvas.width() as f32 / U;
    if small {
        fill_round_rect(
            canvas,
            24.0 * s,
            24.0 * s,
            1000.0 * s,
            1000.0 * s,
            220.0 * s,
            FillStyle::Solid([0xF7, 0xF8, 0xFC, 255]),
        );
        stroke_round_rect(
            canvas,
            24.0 * s,
            24.0 * s,
            1000.0 * s,
            1000.0 * s,
            220.0 * s,
            16.0 * s,
            FillStyle::Solid([0xC8, 0xCD, 0xDA, 255]),
        );
        let tx = |x: f32, y: f32| -> (f32, f32) {
            (
                (512.0 + (x - 512.0) * 1.16) * s,
                (512.0 + (y - 512.0) * 1.16) * s,
            )
        };
        let p = |pts: &[(f32, f32)], closed| {
            let v: Vec<_> = pts.iter().map(|&(x, y)| tx(x, y)).collect();
            polygon(&v, closed)
        };
        fill_path(
            canvas,
            &p(&[(382.0, 478.0), (652.0, 320.0), (586.0, 736.0)], true),
            FillStyle::Solid([0x28, 0x3C, 0x78, 255]),
        );
        fill_path(
            canvas,
            &p(&[(652.0, 320.0), (766.0, 378.0), (586.0, 736.0)], true),
            FillStyle::Solid(WHITE),
        );
        fill_path(
            canvas,
            &p(&[(766.0, 378.0), (842.0, 414.0), (766.0, 465.0)], true),
            FillStyle::Solid([0xF1, 0x76, 0x5D, 255]),
        );
        fill_path(
            canvas,
            &p(
                &[
                    (194.0, 380.0),
                    (434.0, 506.0),
                    (586.0, 736.0),
                    (310.0, 662.0),
                ],
                true,
            ),
            FillStyle::Solid([0x47, 0x61, 0xA8, 255]),
        );
        stroke_path(
            canvas,
            &p(&[(434.0, 506.0), (586.0, 736.0)], false),
            10.0 * s,
            FillStyle::Solid([0x6D, 0x85, 0xC2, 255]),
        );
        let (ex, ey) = tx(700.0, 405.0);
        fill_circle(
            canvas,
            ex,
            ey,
            26.0 * 1.16 * s,
            FillStyle::Solid([0x21, 0x32, 0x5F, 255]),
        );
    } else {
        fill_round_rect(
            canvas,
            66.0 * s,
            70.0 * s,
            958.0 * s,
            958.0 * s,
            204.0 * s,
            FillStyle::Solid([0, 0, 0, 12]),
        );
        fill_round_rect(
            canvas,
            64.0 * s,
            64.0 * s,
            960.0 * s,
            956.0 * s,
            204.0 * s,
            FillStyle::LinearGradient {
                x0: 64.0 * s,
                y0: 64.0 * s,
                x1: (64.0 + 0.45 * 896.0) * s,
                y1: 956.0 * s,
                c0: [0xFF, 0xFF, 0xFF, 255],
                c1: [0xF1, 0xF2, 0xF6, 255],
            },
        );
        stroke_round_rect(
            canvas,
            64.0 * s,
            64.0 * s,
            960.0 * s,
            956.0 * s,
            204.0 * s,
            2.0 * s,
            FillStyle::Solid([0xDF, 0xE2, 0xEA, 255]),
        );
        let sc = |pts: &[(f32, f32)], dy: f32, closed: bool| {
            let v: Vec<_> = pts.iter().map(|&(x, y)| (x * s, (y + dy) * s)).collect();
            polygon(&v, closed)
        };
        fill_path(
            canvas,
            &sc(
                &[
                    (199.0, 391.0),
                    (389.0, 486.0),
                    (652.0, 323.0),
                    (768.0, 380.0),
                    (834.0, 414.0),
                    (768.0, 464.0),
                    (589.0, 739.0),
                    (312.0, 665.0),
                ],
                8.0,
                true,
            ),
            FillStyle::Solid([0x14, 0x23, 0x50, 18]),
        );
        fill_path(
            canvas,
            &sc(&[(382.0, 478.0), (652.0, 320.0), (586.0, 736.0)], 0.0, true),
            FillStyle::LinearGradient {
                x0: 382.0 * s,
                y0: 320.0 * s,
                x1: (382.0 + 0.8 * 270.0) * s,
                y1: 736.0 * s,
                c0: [0x42, 0x5B, 0xA6, 255],
                c1: [0x20, 0x2F, 0x67, 255],
            },
        );
        let head = sc(&[(652.0, 320.0), (766.0, 378.0), (586.0, 736.0)], 0.0, true);
        fill_path(
            canvas,
            &head,
            FillStyle::LinearGradient {
                x0: 586.0 * s,
                y0: 320.0 * s,
                x1: (586.0 + 0.8 * 180.0) * s,
                y1: 736.0 * s,
                c0: WHITE,
                c1: [0xEC, 0xEE, 0xF4, 255],
            },
        );
        stroke_path(
            canvas,
            &head,
            1.5 * s,
            FillStyle::Solid([255, 255, 255, 140]),
        );
        fill_path(
            canvas,
            &sc(&[(766.0, 378.0), (832.0, 412.0), (766.0, 460.0)], 0.0, true),
            FillStyle::LinearGradient {
                x0: 766.0 * s,
                y0: 378.0 * s,
                x1: 832.0 * s,
                y1: 460.0 * s,
                c0: [0xFF, 0x92, 0x77, 255],
                c1: [0xEF, 0x62, 0x4A, 255],
            },
        );
        fill_path(
            canvas,
            &sc(
                &[
                    (194.0, 380.0),
                    (434.0, 506.0),
                    (586.0, 736.0),
                    (310.0, 662.0),
                ],
                0.0,
                true,
            ),
            FillStyle::LinearGradient {
                x0: 194.0 * s,
                y0: 380.0 * s,
                x1: (194.0 + 0.75 * 392.0) * s,
                y1: 736.0 * s,
                c0: [0x52, 0x6D, 0xB9, 255],
                c1: [0x2B, 0x3E, 0x7C, 255],
            },
        );
        stroke_path(
            canvas,
            &sc(
                &[(194.0, 380.0), (434.0, 506.0), (586.0, 736.0)],
                0.0,
                false,
            ),
            2.0 * s,
            FillStyle::Solid([0x82, 0x96, 0xD1, 128]),
        );
        stroke_path(
            canvas,
            &sc(&[(652.0, 320.0), (586.0, 736.0)], 0.0, false),
            2.0 * s,
            FillStyle::Solid([255, 255, 255, 153]),
        );
        fill_circle(
            canvas,
            700.0 * s,
            405.0 * s,
            20.0 * s,
            FillStyle::Solid([0x23, 0x34, 0x63, 255]),
        );
    }
}

pub fn render_origami_icon(size: u32) -> RgbaImage {
    let hi = size.max(64);
    let mut canvas = RgbaImage::new(hi, hi);
    draw_origami(&mut canvas, size <= SMALL_THRESHOLD);
    if hi == size {
        canvas
    } else {
        resize_lanczos(&canvas, size, size)
    }
}

/// `package_origami_icons.py`: PNG 一式と `.ico` を出力する。
pub fn run_package_origami_icons(out: &Path) -> Result<()> {
    fs::create_dir_all(out)?;
    let mut rendered: BTreeMap<u32, RgbaImage> = BTreeMap::new();
    for &sz in PNG_SIZES {
        let img = render_origami_icon(sz);
        img.save(out.join(format!("asteria-origami-v3-{sz}.png")))?;
        rendered.insert(sz, img);
    }
    let ico_frames: Vec<RgbaImage> = ICON_SIZES.iter().map(|s| rendered[s].clone()).collect();
    write_bmp_ico(&ico_frames, &out.join("asteria-origami-v3.ico"))?;

    let mut preview = RgbaImage::from_pixel(760, 380, Rgba([0xF4, 0xF5, 0xF8, 255]));
    for (row, bg) in [Rgba([0xF4, 0xF5, 0xF8, 255]), Rgba([0x20, 0x23, 0x2B, 255])]
        .into_iter()
        .enumerate()
    {
        let y0 = (row * 190) as i32;
        for py in (row as u32 * 190)..((row as u32 + 1) * 190) {
            for px in 0..760 {
                preview.put_pixel(px, py, bg);
            }
        }
        let mut x = 24i32;
        for sz in [128u32, 64, 48, 32, 24, 20, 16] {
            alpha_composite(&mut preview, &rendered[&sz], x, y0 + 32);
            x += sz.max(36) as i32 + 24;
        }
    }
    preview.save(out.join("icon-sizes.png"))?;
    Ok(())
}

pub fn paper_tile(c: &mut RgbaImage, small: bool) {
    let inset = if small { 24.0 } else { 64.0 };
    let radius = if small { 220.0 } else { 204.0 };
    fill_round_rect(
        c,
        inset,
        inset,
        U - inset,
        U - inset,
        radius,
        FillStyle::Solid([0xF7, 0xF8, 0xFC, 255]),
    );
    stroke_round_rect(
        c,
        inset,
        inset,
        U - inset,
        U - inset,
        radius,
        if small { 16.0 } else { 2.0 },
        FillStyle::Solid([0xC8, 0xCD, 0xDA, 255]),
    );
}

fn with_dot(
    c: &mut RgbaImage,
    mut path: VecPath,
    dot_r: f32,
    dot_dx: f32,
    shift: f32,
    color: [u8; 4],
) {
    if let Some((bx0, by0, bx1, by1)) = path.bounds() {
        let left = U * 0.5 - ((bx1 - bx0) + dot_dx + dot_r) * 0.5;
        path.offset(left - bx0, U * 0.5 - (by0 + by1) * 0.5 + shift);
        fill_path(c, &path, FillStyle::Solid(color));
        if let Some((_, _, nb_x1, nb_y1)) = path.bounds() {
            fill_circle(
                c,
                nb_x1 + dot_dx,
                nb_y1 - dot_r,
                dot_r,
                FillStyle::Solid(SHU),
            );
        }
    }
}

fn draw_gear(c: &mut RgbaImage, cx: f32, cy: f32, r_out: f32, r_in: f32, r_hole: f32) {
    let tw = r_out * 0.46;
    let rad = tw * 0.22;
    for y in 0..c.height() {
        for x in 0..c.width() {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let mut d = (px - cx).hypot(py - cy) - r_in;
            for k in 0..8 {
                let ang = (std::f32::consts::TAU / 8.0) * k as f32;
                let cos = ang.cos();
                let sin = ang.sin();
                let rx = (px - cx) * cos + (py - cy) * sin + cx;
                let ry = -(px - cx) * sin + (py - cy) * cos + cy;
                let td = sdf_round_rect(rx, ry, cx - tw * 0.5, cy - r_out, cx + tw * 0.5, cy, rad);
                d = d.min(td);
            }
            let hole_d = (px - cx).hypot(py - cy) - r_hole;
            d = d.max(-hole_d);
            let cov = (0.5 - d).clamp(0.0, 1.0);
            if cov > 0.0 {
                blend_pixel(c, x, y, BRAND_INK, cov);
            }
        }
    }
}

pub fn render_product_mark(asteria_icon_png: &Path, size: u32) -> Result<RgbaImage> {
    let img = image::open(asteria_icon_png)?.to_rgba8();
    Ok(resize_lanczos(&img, size, size))
}

#[derive(Clone, Copy)]
enum IconKind {
    Product,
    LegacyProduct,
    Dictionary,
    Properties,
    Mode(&'static str, bool, bool),
}

struct FontSet {
    serif: Option<LoadedFont>,
    sans: Option<LoadedFont>,
    sans_bold: Option<LoadedFont>,
}

impl FontSet {
    fn system() -> Self {
        Self {
            serif: LoadedFont::find_system_or_fallback(&[
                ("NotoSerifJP-VF.ttf", 0, Some(800.0)),
                ("YuGothB.ttc", 0, None),
            ]),
            sans: LoadedFont::find_system_or_fallback(&[
                ("NotoSansJP-VF.ttf", 0, Some(900.0)),
                ("YuGothB.ttc", 0, None),
            ]),
            sans_bold: LoadedFont::find_system_or_fallback(&[
                ("NotoSansJP-VF.ttf", 0, Some(700.0)),
                ("YuGothB.ttc", 0, None),
            ]),
        }
    }
}

fn render_icon_kind(
    kind: IconKind,
    size: u32,
    fonts: &FontSet,
    asteria_png: &Path,
) -> Result<RgbaImage> {
    match kind {
        IconKind::Product => render_product_mark(asteria_png, size),
        IconKind::LegacyProduct => Ok(render_origami_icon(size)),
        IconKind::Dictionary => {
            let small = size <= SMALL_THRESHOLD;
            let mut c = RgbaImage::new(U as u32, U as u32);
            paper_tile(&mut c, small);
            let font = if small {
                fonts.sans.as_ref()
            } else {
                fonts.serif.as_ref()
            };
            if let Some(f) = font {
                let p = f.glyph_path("辞", if small { 600.0 } else { 520.0 })?;
                with_dot(
                    &mut c,
                    p,
                    if small { 70.0 } else { 42.0 },
                    if small { 96.0 } else { 48.0 },
                    0.0,
                    BRAND_INK,
                );
            }
            Ok(resize_lanczos(&c, size, size))
        }
        IconKind::Properties => {
            let small = size <= SMALL_THRESHOLD;
            let mut c = RgbaImage::new(U as u32, U as u32);
            paper_tile(&mut c, small);
            let sc = if small { 1.18 } else { 1.0 };
            draw_gear(&mut c, U * 0.5, U * 0.5, 300.0 * sc, 236.0 * sc, 110.0 * sc);
            fill_circle(&mut c, U * 0.5, U * 0.5, 58.0 * sc, FillStyle::Solid(SHU));
            Ok(resize_lanczos(&c, size, size))
        }
        IconKind::Mode(text, underline, disabled) => {
            let small = size <= SMALL_THRESHOLD;
            let mut c = RgbaImage::new(U as u32, U as u32);
            if let Some(f) = fonts.sans_bold.as_ref() {
                let mut p = f.glyph_path(text, if small { 820.0 } else { 760.0 })?;
                if let Some((bx0, by0, bx1, by1)) = p.bounds() {
                    let bw = (bx1 - bx0).max(1.0);
                    let bh = (by1 - by0).max(1.0);
                    let box_w = U * if small { 0.92 } else { 0.86 };
                    let box_h = U * if underline {
                        0.66
                    } else if small {
                        0.92
                    } else {
                        0.86
                    };
                    let scale = (box_w / bw).min(box_h / bh);
                    p.scale(scale, scale);
                    if let Some((nb0, nt0, nb1, nt1)) = p.bounds() {
                        let dy = if underline { -125.0 } else { 0.0 };
                        p.offset(
                            U * 0.5 - (nb0 + nb1) * 0.5,
                            U * 0.5 - (nt0 + nt1) * 0.5 + dy,
                        );
                    }
                    let edge = if small { 96.0 } else { 64.0 };
                    let fill = if disabled { GRAY } else { WHITE };
                    stroke_path(&mut c, &p, edge, FillStyle::Solid(OUTLINE));
                    if underline {
                        if let Some((b0, _, b1, bot)) = p.bounds() {
                            let l = b0;
                            let t = bot + 100.0;
                            let r = b1;
                            let b = bot + 190.0;
                            fill_round_rect(
                                &mut c,
                                l - edge * 0.5,
                                t - edge * 0.5,
                                r + edge * 0.5,
                                b + edge * 0.5,
                                40.0 + edge * 0.5,
                                FillStyle::Solid(OUTLINE),
                            );
                            fill_round_rect(&mut c, l, t, r, b, 40.0, FillStyle::Solid(fill));
                        }
                    }
                    fill_path(&mut c, &p, FillStyle::Solid(fill));
                }
            }
            Ok(resize_lanczos(&c, size, size))
        }
    }
}

/// `gen_icons.py`: Mozc Win32 インストーラ用 `.ico` 群を一括生成する。
pub fn run_gen_icons(
    out: &Path,
    preview: Option<&Path>,
    brand_only: bool,
    repo_root: &Path,
) -> Result<usize> {
    fs::create_dir_all(out)?;
    let asteria_png = repo_root.join("docs/design/2026-10-asteria/asteria-icon.png");
    let fonts = FontSet::system();

    let mut specs: Vec<(&'static str, IconKind)> = vec![
        ("product_icon.ico", IconKind::Product),
        ("tools_legacy_product.ico", IconKind::LegacyProduct),
        ("product_icon_langbar.ico", IconKind::Product),
        ("tools_icon.ico", IconKind::LegacyProduct),
        ("tools_icon_a.ico", IconKind::LegacyProduct),
        ("tools_properties.ico", IconKind::Properties),
        ("tools_properties_a.ico", IconKind::Properties),
        ("tools_dictionary.ico", IconKind::Dictionary),
        ("tools_dictionary_a.ico", IconKind::Dictionary),
    ];
    if !brand_only {
        specs.extend([
            ("ms_hiragana.ico", IconKind::Mode("あ", false, false)),
            ("ms_hiragana_a.ico", IconKind::Mode("あ", false, false)),
            ("ms_katakana.ico", IconKind::Mode("カ", false, false)),
            ("ms_katakana_a.ico", IconKind::Mode("カ", false, false)),
            ("ms_katakana_half.ico", IconKind::Mode("ｶ", true, false)),
            ("ms_katakana_half_a.ico", IconKind::Mode("ｶ", true, false)),
            ("ms_alpha.ico", IconKind::Mode("Ａ", false, false)),
            ("ms_alpha_a.ico", IconKind::Mode("Ａ", false, false)),
            ("ms_alpha_half.ico", IconKind::Mode("A", true, false)),
            ("ms_alpha_half_a.ico", IconKind::Mode("A", true, false)),
            ("ms_direct_input.ico", IconKind::Mode("A", false, false)),
            ("ms_direct_input_a.ico", IconKind::Mode("A", false, false)),
            ("ms_disabled.ico", IconKind::Mode("×", false, true)),
            ("ms_disabled_a.ico", IconKind::Mode("×", false, true)),
        ]);
    }

    let mut rendered: BTreeMap<&'static str, Vec<RgbaImage>> = BTreeMap::new();
    for (name, kind) in &specs {
        let mut imgs = Vec::with_capacity(ICON_SIZES.len());
        for &sz in ICON_SIZES {
            imgs.push(render_icon_kind(*kind, sz, &fonts, &asteria_png)?);
        }
        write_bmp_ico(&imgs, &out.join(name))?;
        rendered.insert(*name, imgs);
    }

    if let Some(pv) = preview {
        let unique: Vec<&'static str> = specs
            .iter()
            .filter_map(|(n, _)| {
                if n.ends_with("_a.ico") {
                    None
                } else {
                    Some(*n)
                }
            })
            .collect();
        let cell = 150u32;
        let mut sheet = RgbaImage::new(cell * unique.len() as u32, 2 * cell);
        for (j, bg) in [Rgba([243, 243, 243, 255]), Rgba([32, 32, 32, 255])]
            .into_iter()
            .enumerate()
        {
            for py in (j as u32 * cell)..((j as u32 + 1) * cell) {
                for px in 0..sheet.width() {
                    sheet.put_pixel(px, py, bg);
                }
            }
            for (i, &n) in unique.iter().enumerate() {
                if let Some(imgs) = rendered.get(n) {
                    let big = resize_lanczos(&imgs[imgs.len() - 1], 88, 88);
                    alpha_composite(
                        &mut sheet,
                        &big,
                        (i as u32 * cell + 8) as i32,
                        (j as u32 * cell + 8) as i32,
                    );
                    let mut x = i as u32 * cell + 8;
                    for k in [3usize, 2, 0] {
                        alpha_composite(
                            &mut sheet,
                            &imgs[k],
                            x as i32,
                            (j as u32 * cell + 108) as i32,
                        );
                        x += ICON_SIZES[k] + 8;
                    }
                }
            }
        }
        sheet.save(pv)?;
    }

    Ok(specs.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn origami_icon_renders_and_packages_bmp_ico() {
        let img16 = render_origami_icon(16);
        assert_eq!(img16.dimensions(), (16, 16));
        let img256 = render_origami_icon(256);
        assert_eq!(img256.dimensions(), (256, 256));

        let dir = TempDir::new().unwrap();
        run_package_origami_icons(dir.path()).unwrap();
        assert!(dir.path().join("asteria-origami-v3.ico").is_file());
        assert!(dir.path().join("icon-sizes.png").is_file());
    }
}
