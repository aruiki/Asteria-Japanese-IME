//! 候補ウィンドウのページ分割・配色・行高さ・ヒットテスト計算 (OS 非依存)。

// 定数や色の変換の多くは Windows の win_ui だけが使う。ほかの OS の CI(clippy -D warnings)で dead_code にしない。
#![cfg_attr(not(windows), allow(dead_code))]

use asteria_proto::renderer::Show;

use crate::placement::Size;

pub const PAGE_SIZE: usize = 9;
pub const ROW_HEIGHT: i32 = 30;
pub const FOOTER_HEIGHT: i32 = 24;
pub const PADDING_Y: i32 = 6;
pub const MIN_WIDTH: i32 = 260;
pub const MAX_WIDTH: i32 = 520;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RgbColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl RgbColor {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    pub fn as_f32(self) -> (f32, f32, f32) {
        (
            f32::from(self.r) / 255.0,
            f32::from(self.g) / 255.0,
            f32::from(self.b) / 255.0,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemePalette {
    pub bg: RgbColor,
    pub border: RgbColor,
    pub selected_bg: RgbColor,
    pub text: RgbColor,
    pub selected_text: RgbColor,
    pub annotation: RgbColor,
    pub accent: RgbColor,
    pub footer_bg: RgbColor,
    pub footer_text: RgbColor,
}

impl ThemePalette {
    pub fn resolve(dark_mode: bool, high_contrast: bool) -> Self {
        if high_contrast {
            Self {
                bg: RgbColor::new(0, 0, 0),
                border: RgbColor::new(255, 255, 255),
                selected_bg: RgbColor::new(31, 240, 255),
                text: RgbColor::new(255, 255, 255),
                selected_text: RgbColor::new(0, 0, 0),
                annotation: RgbColor::new(255, 255, 0),
                accent: RgbColor::new(31, 240, 255),
                footer_bg: RgbColor::new(0, 0, 0),
                footer_text: RgbColor::new(255, 255, 255),
            }
        } else if dark_mode {
            Self {
                bg: RgbColor::new(40, 40, 44),
                border: RgbColor::new(72, 72, 78),
                selected_bg: RgbColor::new(62, 68, 82),
                text: RgbColor::new(242, 242, 245),
                selected_text: RgbColor::new(255, 255, 255),
                annotation: RgbColor::new(168, 174, 186),
                accent: RgbColor::new(126, 176, 255),
                footer_bg: RgbColor::new(32, 32, 36),
                footer_text: RgbColor::new(165, 168, 178),
            }
        } else {
            Self {
                bg: RgbColor::new(250, 250, 252),
                border: RgbColor::new(212, 214, 220),
                selected_bg: RgbColor::new(232, 240, 254),
                text: RgbColor::new(28, 30, 34),
                selected_text: RgbColor::new(16, 48, 112),
                annotation: RgbColor::new(108, 114, 126),
                accent: RgbColor::new(26, 115, 232),
                footer_bg: RgbColor::new(242, 243, 247),
                footer_text: RgbColor::new(96, 102, 112),
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisibleRow {
    pub global_index: usize,
    pub shortcut: usize,
    pub text: String,
    pub annotation: String,
    pub selected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisiblePage {
    pub page_index: usize,
    pub total_pages: usize,
    pub page_start: usize,
    pub rows: Vec<VisibleRow>,
    pub size: Size,
    pub footer_text: String,
}

pub fn build_visible_page(show: &Show) -> VisiblePage {
    let total = show.candidates.len();
    let focused = (show.focused_index as usize).min(total.saturating_sub(1));
    let page_start = (focused / PAGE_SIZE) * PAGE_SIZE;
    let page_index = focused / PAGE_SIZE;
    let total_pages = total.div_ceil(PAGE_SIZE).max(1);
    let end = (page_start + PAGE_SIZE).min(total);

    let mut max_chars = 8usize;
    let mut rows = Vec::with_capacity(end.saturating_sub(page_start));
    for (offset, idx) in (page_start..end).enumerate() {
        let cand = &show.candidates[idx];
        let chars_len = cand.text.chars().count()
            + if cand.annotation.is_empty() {
                0
            } else {
                cand.annotation.chars().count() + 3
            };
        max_chars = max_chars.max(chars_len);
        rows.push(VisibleRow {
            global_index: idx,
            shortcut: offset + 1,
            text: cand.text.clone(),
            annotation: cand.annotation.clone(),
            selected: idx == focused,
        });
    }

    let width = ((max_chars as i32) * 16 + 72).clamp(MIN_WIDTH, MAX_WIDTH);
    let height = PADDING_Y * 2 + (rows.len() as i32) * ROW_HEIGHT + FOOTER_HEIGHT;
    let footer_text = if !show.footer_hint.is_empty() {
        show.footer_hint.clone()
    } else {
        format!(
            "{}/{} · 1–{} で選択",
            page_index + 1,
            total_pages,
            rows.len().max(1)
        )
    };

    VisiblePage {
        page_index,
        total_pages,
        page_start,
        rows,
        size: Size { width, height },
        footer_text,
    }
}

/// ウィンドウ内のローカル `(x, y)` 座標から候補の全体インデックスを返す。
pub fn hit_test_row(page: &VisiblePage, y: i32) -> Option<usize> {
    if y < PADDING_Y {
        return None;
    }
    let rel = (y - PADDING_Y) / ROW_HEIGHT;
    if rel < 0 {
        return None;
    }
    page.rows.get(rel as usize).map(|r| r.global_index)
}

#[cfg(test)]
mod tests {
    use super::*;
    use asteria_proto::renderer::Candidate;

    #[test]
    fn builds_page_and_hit_tests_correctly() {
        let mut show = Show::default();
        for i in 0..12 {
            show.candidates.push(Candidate {
                text: format!("候補{i}"),
                annotation: if i == 10 {
                    "説明".into()
                } else {
                    String::new()
                },
            });
        }
        show.focused_index = 10;
        let page = build_visible_page(&show);
        assert_eq!(page.page_index, 1);
        assert_eq!(page.total_pages, 2);
        assert_eq!(page.page_start, 9);
        assert_eq!(page.rows.len(), 3);
        assert!(page.rows[1].selected);
        assert_eq!(hit_test_row(&page, PADDING_Y + ROW_HEIGHT + 4), Some(10));
        assert_eq!(hit_test_row(&page, PADDING_Y + ROW_HEIGHT * 5), None);

        let dark = ThemePalette::resolve(true, false);
        let hc = ThemePalette::resolve(false, true);
        assert_ne!(dark.bg, hc.bg);
    }
}
