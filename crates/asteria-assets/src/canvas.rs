//! Pure-Rust 2D ベクタ・可変フォント描画キャンバスと ICO/BMP 出力ヘルパ。

use std::fs::File;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use ico::{IconDir, IconDirEntry, IconImage, ResourceType};
use image::{imageops, Rgba, RgbaImage};
use ttf_parser::{Face, OutlineBuilder, Tag};

#[derive(Debug, Clone, Copy)]
pub enum FillStyle {
    Solid([u8; 4]),
    LinearGradient {
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        c0: [u8; 4],
        c1: [u8; 4],
    },
}

impl FillStyle {
    pub fn sample(self, x: f32, y: f32) -> [u8; 4] {
        match self {
            FillStyle::Solid(c) => c,
            FillStyle::LinearGradient {
                x0,
                y0,
                x1,
                y1,
                c0,
                c1,
            } => {
                let dx = x1 - x0;
                let dy = y1 - y0;
                let len2 = dx * dx + dy * dy;
                let t = if len2 <= 1e-6 {
                    0.0
                } else {
                    (((x - x0) * dx + (y - y0) * dy) / len2).clamp(0.0, 1.0)
                };
                [
                    (c0[0] as f32 + (c1[0] as f32 - c0[0] as f32) * t).round() as u8,
                    (c0[1] as f32 + (c1[1] as f32 - c0[1] as f32) * t).round() as u8,
                    (c0[2] as f32 + (c1[2] as f32 - c0[2] as f32) * t).round() as u8,
                    (c0[3] as f32 + (c1[3] as f32 - c0[3] as f32) * t).round() as u8,
                ]
            }
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct VecPath {
    pub contours: Vec<Vec<(f32, f32)>>,
    cur: Vec<(f32, f32)>,
    last: (f32, f32),
}

impl VecPath {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn move_to(&mut self, x: f32, y: f32) {
        if self.cur.len() >= 2 {
            self.contours.push(std::mem::take(&mut self.cur));
        } else {
            self.cur.clear();
        }
        self.last = (x, y);
        self.cur.push((x, y));
    }

    pub fn line_to(&mut self, x: f32, y: f32) {
        if self.cur.is_empty() {
            self.cur.push(self.last);
        }
        self.last = (x, y);
        self.cur.push((x, y));
    }

    pub fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let (x0, y0) = self.last;
        let steps = 8;
        for i in 1..=steps {
            let t = i as f32 / steps as f32;
            let mt = 1.0 - t;
            let px = mt * mt * x0 + 2.0 * mt * t * x1 + t * t * x;
            let py = mt * mt * y0 + 2.0 * mt * t * y1 + t * t * y;
            self.line_to(px, py);
        }
    }

    pub fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let (x0, y0) = self.last;
        let steps = 10;
        for i in 1..=steps {
            let t = i as f32 / steps as f32;
            let mt = 1.0 - t;
            let px =
                mt * mt * mt * x0 + 3.0 * mt * mt * t * x1 + 3.0 * mt * t * t * x2 + t * t * t * x;
            let py =
                mt * mt * mt * y0 + 3.0 * mt * mt * t * y1 + 3.0 * mt * t * t * y2 + t * t * t * y;
            self.line_to(px, py);
        }
    }

    pub fn close(&mut self) {
        if let Some(&first) = self.cur.first() {
            if self.last != first {
                self.cur.push(first);
            }
            self.contours.push(std::mem::take(&mut self.cur));
            self.last = first;
        }
    }

    pub fn finish(mut self) -> Vec<Vec<(f32, f32)>> {
        if !self.cur.is_empty() {
            self.contours.push(self.cur);
        }
        self.contours
    }

    pub fn bounds(&self) -> Option<(f32, f32, f32, f32)> {
        let mut pts = self
            .contours
            .iter()
            .chain(std::iter::once(&self.cur))
            .flatten();
        let &(x0, y0) = pts.next()?;
        let (mut min_x, mut min_y, mut max_x, mut max_y) = (x0, y0, x0, y0);
        for &(x, y) in pts {
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
        }
        Some((min_x, min_y, max_x, max_y))
    }

    pub fn offset(&mut self, dx: f32, dy: f32) {
        for c in &mut self.contours {
            for p in c {
                p.0 += dx;
                p.1 += dy;
            }
        }
        for p in &mut self.cur {
            p.0 += dx;
            p.1 += dy;
        }
        self.last.0 += dx;
        self.last.1 += dy;
    }

    pub fn scale(&mut self, sx: f32, sy: f32) {
        for c in &mut self.contours {
            for p in c {
                p.0 *= sx;
                p.1 *= sy;
            }
        }
        for p in &mut self.cur {
            p.0 *= sx;
            p.1 *= sy;
        }
        self.last.0 *= sx;
        self.last.1 *= sy;
    }
}

struct GlyphCollector<'a> {
    path: &'a mut VecPath,
    scale: f32,
    ox: f32,
    oy: f32,
}

impl OutlineBuilder for GlyphCollector<'_> {
    fn move_to(&mut self, x: f32, y: f32) {
        self.path
            .move_to(self.ox + x * self.scale, self.oy - y * self.scale);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.path
            .line_to(self.ox + x * self.scale, self.oy - y * self.scale);
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.path.quad_to(
            self.ox + x1 * self.scale,
            self.oy - y1 * self.scale,
            self.ox + x * self.scale,
            self.oy - y * self.scale,
        );
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.path.curve_to(
            self.ox + x1 * self.scale,
            self.oy - y1 * self.scale,
            self.ox + x2 * self.scale,
            self.oy - y2 * self.scale,
            self.ox + x * self.scale,
            self.oy - y * self.scale,
        );
    }
    fn close(&mut self) {
        self.path.close();
    }
}

#[derive(Clone)]
pub struct LoadedFont {
    bytes: Vec<u8>,
    index: u32,
    wght: Option<f32>,
}

impl LoadedFont {
    pub fn load(path: &Path, index: u32, wght: Option<f32>) -> Result<Self> {
        let bytes =
            std::fs::read(path).with_context(|| format!("フォント読込失敗: {}", path.display()))?;
        let _ = Face::parse(&bytes, index).context("フォントパース失敗")?;
        Ok(Self { bytes, index, wght })
    }

    pub fn find_system_or_fallback(candidates: &[(&str, u32, Option<f32>)]) -> Option<Self> {
        let dirs = [
            PathBuf::from(r"C:\Windows\Fonts"),
            PathBuf::from("/usr/share/fonts"),
        ];
        for (name, idx, wght) in candidates {
            for dir in &dirs {
                let p = dir.join(name);
                if p.is_file() {
                    if let Ok(f) = Self::load(&p, *idx, *wght) {
                        return Some(f);
                    }
                }
            }
        }
        None
    }

    fn face(&self) -> Result<Face<'_>> {
        let mut face = Face::parse(&self.bytes, self.index).context("face parse")?;
        if let Some(w) = self.wght {
            let _ = face.set_variation(Tag::from_bytes(b"wght"), w);
        }
        Ok(face)
    }

    pub fn glyph_path(&self, text: &str, px_size: f32) -> Result<VecPath> {
        let face = self.face()?;
        let upem = face.units_per_em() as f32;
        let scale = px_size / upem;
        let mut path = VecPath::new();
        let mut pen_x = 0.0f32;
        for ch in text.chars() {
            if let Some(gid) = face.glyph_index(ch) {
                let mut collector = GlyphCollector {
                    path: &mut path,
                    scale,
                    ox: pen_x,
                    oy: 0.0,
                };
                let _ = face.outline_glyph(gid, &mut collector);
                if let Some(adv) = face.glyph_hor_advance(gid) {
                    pen_x += adv as f32 * scale;
                }
            } else {
                pen_x += px_size * 0.5;
            }
        }
        Ok(path)
    }

    pub fn measure_width(&self, text: &str, px_size: f32) -> Result<f32> {
        let face = self.face()?;
        let upem = face.units_per_em() as f32;
        let scale = px_size / upem;
        let mut w = 0.0f32;
        for ch in text.chars() {
            if let Some(gid) = face.glyph_index(ch) {
                if let Some(adv) = face.glyph_hor_advance(gid) {
                    w += adv as f32 * scale;
                }
            } else {
                w += px_size * 0.5;
            }
        }
        Ok(w)
    }
}

pub fn blend_pixel(img: &mut RgbaImage, x: u32, y: u32, src: [u8; 4], cov: f32) {
    if cov <= 0.0 || src[3] == 0 {
        return;
    }
    let sa = (src[3] as f32 / 255.0) * cov.clamp(0.0, 1.0);
    if sa <= 0.0 {
        return;
    }
    let dst = img.get_pixel_mut(x, y);
    let da = dst[3] as f32 / 255.0;
    let out_a = sa + da * (1.0 - sa);
    if out_a <= 1e-6 {
        return;
    }
    for i in 0..3 {
        let c = (src[i] as f32 * sa + dst[i] as f32 * da * (1.0 - sa)) / out_a;
        dst[i] = c.round().clamp(0.0, 255.0) as u8;
    }
    dst[3] = (out_a * 255.0).round().clamp(0.0, 255.0) as u8;
}

pub fn sdf_round_rect(x: f32, y: f32, l: f32, t: f32, r: f32, b: f32, rad: f32) -> f32 {
    let cx = (l + r) * 0.5;
    let cy = (t + b) * 0.5;
    let hw = (r - l) * 0.5 - rad;
    let hh = (b - t) * 0.5 - rad;
    let qx = (x - cx).abs() - hw;
    let qy = (y - cy).abs() - hh;
    qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0) - rad
}

pub fn fill_round_rect(
    img: &mut RgbaImage,
    l: f32,
    t: f32,
    r: f32,
    b: f32,
    rad: f32,
    fill: FillStyle,
) {
    let x0 = (l.floor() as i32 - 1).max(0) as u32;
    let y0 = (t.floor() as i32 - 1).max(0) as u32;
    let x1 = (r.ceil() as i32 + 1).min(img.width() as i32) as u32;
    let y1 = (b.ceil() as i32 + 1).min(img.height() as i32) as u32;
    for y in y0..y1 {
        for x in x0..x1 {
            let d = sdf_round_rect(x as f32 + 0.5, y as f32 + 0.5, l, t, r, b, rad);
            let cov = (0.5 - d).clamp(0.0, 1.0);
            if cov > 0.0 {
                blend_pixel(img, x, y, fill.sample(x as f32 + 0.5, y as f32 + 0.5), cov);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn stroke_round_rect(
    img: &mut RgbaImage,
    l: f32,
    t: f32,
    r: f32,
    b: f32,
    rad: f32,
    width: f32,
    stroke: FillStyle,
) {
    let half = width * 0.5;
    let x0 = ((l - half).floor() as i32 - 1).max(0) as u32;
    let y0 = ((t - half).floor() as i32 - 1).max(0) as u32;
    let x1 = ((r + half).ceil() as i32 + 1).min(img.width() as i32) as u32;
    let y1 = ((b + half).ceil() as i32 + 1).min(img.height() as i32) as u32;
    for y in y0..y1 {
        for x in x0..x1 {
            let d = sdf_round_rect(x as f32 + 0.5, y as f32 + 0.5, l, t, r, b, rad).abs() - half;
            let cov = (0.5 - d).clamp(0.0, 1.0);
            if cov > 0.0 {
                blend_pixel(
                    img,
                    x,
                    y,
                    stroke.sample(x as f32 + 0.5, y as f32 + 0.5),
                    cov,
                );
            }
        }
    }
}

pub fn fill_circle(img: &mut RgbaImage, cx: f32, cy: f32, rad: f32, fill: FillStyle) {
    let x0 = ((cx - rad).floor() as i32 - 1).max(0) as u32;
    let y0 = ((cy - rad).floor() as i32 - 1).max(0) as u32;
    let x1 = ((cx + rad).ceil() as i32 + 1).min(img.width() as i32) as u32;
    let y1 = ((cy + rad).ceil() as i32 + 1).min(img.height() as i32) as u32;
    for y in y0..y1 {
        for x in x0..x1 {
            let d = (x as f32 + 0.5 - cx).hypot(y as f32 + 0.5 - cy) - rad;
            let cov = (0.5 - d).clamp(0.0, 1.0);
            if cov > 0.0 {
                blend_pixel(img, x, y, fill.sample(x as f32 + 0.5, y as f32 + 0.5), cov);
            }
        }
    }
}

/// Non-zero winding + 4x4 SSAA で `VecPath` を塗りつぶす。
pub fn fill_path(img: &mut RgbaImage, path: &VecPath, fill: FillStyle) {
    let Some((bx0, by0, bx1, by1)) = path.bounds() else {
        return;
    };
    let contours: Vec<Vec<(f32, f32)>> = path.clone().finish();
    let x0 = (bx0.floor() as i32 - 1).max(0) as u32;
    let y0 = (by0.floor() as i32 - 1).max(0) as u32;
    let x1 = (bx1.ceil() as i32 + 1).min(img.width() as i32) as u32;
    let y1 = (by1.ceil() as i32 + 1).min(img.height() as i32) as u32;
    for y in y0..y1 {
        for sy in 0..4 {
            let py = y as f32 + (sy as f32 + 0.5) * 0.25;
            let mut edges: Vec<(f32, i32)> = Vec::new();
            for c in &contours {
                for w in c.windows(2) {
                    let (ax, ay) = w[0];
                    let (bx, by) = w[1];
                    if (ay <= py && by > py) || (by <= py && ay > py) {
                        let t = (py - ay) / (by - ay);
                        let ix = ax + t * (bx - ax);
                        edges.push((ix, if by > ay { 1 } else { -1 }));
                    }
                }
            }
            if edges.is_empty() {
                continue;
            }
            edges.sort_by(|a, b| a.0.total_cmp(&b.0));
            for x in x0..x1 {
                for sx in 0..4 {
                    let px = x as f32 + (sx as f32 + 0.5) * 0.25;
                    let mut winding = 0i32;
                    for &(ix, dir) in &edges {
                        if ix >= px {
                            break;
                        }
                        winding += dir;
                    }
                    if winding != 0 {
                        blend_pixel(img, x, y, fill.sample(px, py), 1.0 / 16.0);
                    }
                }
            }
        }
    }
}

fn dist_to_segment(px: f32, py: f32, ax: f32, ay: f32, bx: f32, by: f32) -> f32 {
    let dx = bx - ax;
    let dy = by - ay;
    let len2 = dx * dx + dy * dy;
    let t = if len2 <= 1e-8 {
        0.0
    } else {
        (((px - ax) * dx + (py - ay) * dy) / len2).clamp(0.0, 1.0)
    };
    (px - (ax + t * dx)).hypot(py - (ay + t * dy))
}

pub fn stroke_path(img: &mut RgbaImage, path: &VecPath, width: f32, stroke: FillStyle) {
    let Some((bx0, by0, bx1, by1)) = path.bounds() else {
        return;
    };
    let half = width * 0.5;
    let contours = path.clone().finish();
    let x0 = ((bx0 - half).floor() as i32 - 1).max(0) as u32;
    let y0 = ((by0 - half).floor() as i32 - 1).max(0) as u32;
    let x1 = ((bx1 + half).ceil() as i32 + 1).min(img.width() as i32) as u32;
    let y1 = ((by1 + half).ceil() as i32 + 1).min(img.height() as i32) as u32;
    for y in y0..y1 {
        let py = y as f32 + 0.5;
        for x in x0..x1 {
            let px = x as f32 + 0.5;
            let mut min_d = f32::MAX;
            for c in &contours {
                for w in c.windows(2) {
                    let (ax, ay) = w[0];
                    let (bx, by) = w[1];
                    if px < ax.min(bx) - half - 2.0
                        || px > ax.max(bx) + half + 2.0
                        || py < ay.min(by) - half - 2.0
                        || py > ay.max(by) + half + 2.0
                    {
                        continue;
                    }
                    min_d = min_d.min(dist_to_segment(px, py, ax, ay, bx, by));
                }
            }
            let cov = (0.5 - (min_d - half)).clamp(0.0, 1.0);
            if cov > 0.0 {
                blend_pixel(img, x, y, stroke.sample(px, py), cov);
            }
        }
    }
}

pub fn draw_text_line(
    img: &mut RgbaImage,
    font: Option<&LoadedFont>,
    x: f32,
    y_top: f32,
    text: &str,
    px_size: f32,
    color: [u8; 4],
) -> f32 {
    let Some(f) = font else {
        // Headless fallback when Windows/system Japanese TTF isn't present.
        let approx_w = text.chars().count() as f32 * px_size * 0.65;
        let rx = x.round().max(0.0) as u32;
        let ry = y_top.round().max(0.0) as u32;
        let rw = approx_w.round().max(1.0) as u32;
        let rh = px_size.round().max(1.0) as u32;
        for py in ry..(ry + rh).min(img.height()) {
            for px in rx..(rx + rw).min(img.width()) {
                blend_pixel(img, px, py, color, 0.2);
            }
        }
        return approx_w;
    };
    let Ok(mut path) = f.glyph_path(text, px_size) else {
        return 0.0;
    };
    let baseline = y_top + px_size * 0.82;
    path.offset(x, baseline);
    fill_path(img, &path, FillStyle::Solid(color));
    f.measure_width(text, px_size).unwrap_or(0.0)
}

pub fn alpha_composite(dst: &mut RgbaImage, src: &RgbaImage, ox: i32, oy: i32) {
    for sy in 0..src.height() {
        let dy = oy + sy as i32;
        if dy < 0 || dy >= dst.height() as i32 {
            continue;
        }
        for sx in 0..src.width() {
            let dx = ox + sx as i32;
            if dx < 0 || dx >= dst.width() as i32 {
                continue;
            }
            let p = *src.get_pixel(sx, sy);
            blend_pixel(dst, dx as u32, dy as u32, p.0, 1.0);
        }
    }
}

pub fn resize_lanczos(src: &RgbaImage, w: u32, h: u32) -> RgbaImage {
    imageops::resize(src, w, h, imageops::FilterType::Lanczos3)
}

/// RC2176 回避のため、全フレームを BMP(DIB) 形式で `.ico` に書き出す。
pub fn write_bmp_ico(images: &[RgbaImage], out: &Path) -> Result<()> {
    let mut dir = IconDir::new(ResourceType::Icon);
    for img in images {
        let icon_img = IconImage::from_rgba_data(img.width(), img.height(), img.as_raw().clone());
        let entry = IconDirEntry::encode_as_bmp(&icon_img)
            .with_context(|| format!("ICO BMP エンコード失敗: {}x{}", img.width(), img.height()))?;
        dir.add_entry(entry);
    }
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut f = File::create(out)?;
    dir.write(&mut f)?;
    Ok(())
}

pub fn parse_hex_color(hex: &str) -> Result<[u8; 4]> {
    let s = hex.trim_start_matches('#');
    if s.len() == 6 {
        let r = u8::from_str_radix(&s[0..2], 16)?;
        let g = u8::from_str_radix(&s[2..4], 16)?;
        let b = u8::from_str_radix(&s[4..6], 16)?;
        Ok([r, g, b, 255])
    } else if s.len() == 8 {
        let r = u8::from_str_radix(&s[0..2], 16)?;
        let g = u8::from_str_radix(&s[2..4], 16)?;
        let b = u8::from_str_radix(&s[4..6], 16)?;
        let a = u8::from_str_radix(&s[6..8], 16)?;
        Ok([r, g, b, a])
    } else {
        bail!("不正な16進カラー: {hex}")
    }
}

pub fn rgba_from_hex(hex: &str) -> Rgba<u8> {
    Rgba(parse_hex_color(hex).unwrap_or([0, 0, 0, 255]))
}
