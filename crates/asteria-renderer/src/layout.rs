//! 候補の窓の中身と大きさ・位置を決める(描画に依らない部分。カード 98 の 2)。
//!
//! 文字の幅は呼ぶ側(DirectWrite)が測って渡す。ここでは並べ方と、画面からはみ出さない位置だけを決める。

use crate::mozc::commands::{CandidateWindow, RendererCommand};

/// 1 行(1 つの候補)。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// ショートカット(1〜9 など)。
    pub shortcut: String,
    /// 候補の表記(前後の注釈を含む)。
    pub value: String,
    /// 種類の説明([全]カタカナ など)。
    pub description: String,
    pub focused: bool,
}

/// 窓に出す内容。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Content {
    pub rows: Vec<Row>,
    /// 「12/40」のような位置の表示(候補が 1 ページに収まらないとき)。
    pub footer: Option<String>,
}

/// 候補の一覧から窓の中身を作る。候補がなければ None(窓を隠す)。
pub fn content(window: &CandidateWindow) -> Option<Content> {
    if window.candidate.is_empty() {
        return None;
    }
    let rows: Vec<Row> = window
        .candidate
        .iter()
        .map(|c| {
            let a = c.annotation.as_ref();
            Row {
                shortcut: a.map(|a| a.shortcut().to_string()).unwrap_or_default(),
                value: format!(
                    "{}{}{}",
                    a.map(|a| a.prefix()).unwrap_or(""),
                    c.value,
                    a.map(|a| a.suffix()).unwrap_or("")
                ),
                description: a.map(|a| a.description().to_string()).unwrap_or_default(),
                focused: window.focused_index.is_some_and(|f| f == c.index),
            }
        })
        .collect();
    let footer = (window.size as usize > rows.len()).then(|| {
        let focused = window.focused_index.map_or(0, |f| f + 1);
        format!("{}/{}", focused, window.size)
    });
    Some(Content { rows, footer })
}

/// 受けた命令から、出すべき候補の一覧を取り出す。隠すときは None。
pub fn candidate_window(command: &RendererCommand) -> Option<&CandidateWindow> {
    if !command.visible() {
        return None;
    }
    command.output.as_ref()?.candidate_window.as_ref()
}

/// 画面上の矩形(ピクセル)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl Rect {
    pub fn width(&self) -> i32 {
        self.right - self.left
    }
    pub fn height(&self) -> i32 {
        self.bottom - self.top
    }
}

/// 入力中の文字の下(はみ出すなら上)に、作業領域に収まるように窓を置く。
///
/// `caret` は入力中の文字の矩形、`size` は窓の幅と高さ、`work` はモニタの作業領域。
pub fn place(caret: Rect, size: (i32, i32), work: Rect) -> Rect {
    let (w, h) = size;
    let mut left = caret.left;
    if left + w > work.right {
        left = work.right - w;
    }
    left = left.max(work.left);
    let mut top = caret.bottom;
    if top + h > work.bottom {
        // 下に入らなければ入力中の文字の上に出す。
        top = caret.top - h;
    }
    top = top.max(work.top);
    Rect {
        left,
        top,
        right: left + w,
        bottom: top + h,
    }
}

/// 命令から入力中の文字の矩形を取る(なければ None)。
pub fn caret_rect(command: &RendererCommand) -> Option<Rect> {
    if let Some(target) = command
        .application_info
        .as_ref()
        .and_then(|a| a.composition_target.as_ref())
    {
        let p = target.top_left.as_ref()?;
        return Some(Rect {
            left: p.x(),
            top: p.y(),
            right: p.x() + 1,
            bottom: p.y() + target.line_height() as i32,
        });
    }
    command.preedit_rectangle.as_ref().map(|r| Rect {
        left: r.left(),
        top: r.top(),
        right: r.right(),
        bottom: r.bottom(),
    })
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::mozc::commands::candidate_window::Candidate;
    use crate::mozc::commands::{Annotation, Output};

    fn cand(index: u32, value: &str, shortcut: &str) -> Candidate {
        Candidate {
            index,
            value: value.to_string(),
            annotation: Some(Annotation {
                shortcut: Some(shortcut.to_string()),
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    #[test]
    fn builds_rows_and_footer() {
        let w = CandidateWindow {
            focused_index: Some(1),
            size: 20,
            candidate: vec![cand(0, "変換", "1"), cand(1, "返還", "2")],
            ..Default::default()
        };
        let c = content(&w).unwrap();
        assert_eq!(c.rows.len(), 2);
        assert_eq!(c.rows[0].shortcut, "1");
        assert!(!c.rows[0].focused);
        assert!(c.rows[1].focused);
        assert_eq!(c.footer.as_deref(), Some("2/20"));
    }

    #[test]
    fn no_footer_when_all_shown() {
        let w = CandidateWindow {
            size: 1,
            candidate: vec![cand(0, "変換", "1")],
            ..Default::default()
        };
        assert_eq!(content(&w).unwrap().footer, None);
    }

    #[test]
    fn empty_window_hides() {
        assert!(content(&CandidateWindow::default()).is_none());
    }

    #[test]
    fn hidden_command_has_no_window() {
        let cmd = RendererCommand {
            visible: Some(false),
            output: Some(Output {
                candidate_window: Some(CandidateWindow::default()),
                ..Default::default()
            }),
            ..Default::default()
        };
        assert!(candidate_window(&cmd).is_none());
    }

    #[test]
    fn places_below_then_above() {
        let work = Rect {
            left: 0,
            top: 0,
            right: 1000,
            bottom: 800,
        };
        let caret = Rect {
            left: 100,
            top: 100,
            right: 101,
            bottom: 120,
        };
        assert_eq!(
            place(caret, (200, 300), work),
            Rect {
                left: 100,
                top: 120,
                right: 300,
                bottom: 420
            }
        );
        let low = Rect {
            left: 900,
            top: 700,
            right: 901,
            bottom: 720,
        };
        let r = place(low, (200, 300), work);
        assert_eq!((r.left, r.top), (800, 400));
    }
}
