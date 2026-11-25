//! Windows Direct2D / DirectWrite 候補ウィンドウ (`AsteriaCandidateWindow`)。
#![allow(unsafe_code)]

use std::cell::RefCell;
use std::sync::mpsc::{self, Receiver};

use asteria_proto::renderer::{renderer_message, Show};
use windows::Win32::Foundation::{COLORREF, HMODULE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Direct2D::Common::{D2D1_COLOR_F, D2D_RECT_F, D2D_SIZE_U};
use windows::Win32::Graphics::Direct2D::{
    D2D1CreateFactory, ID2D1Factory, ID2D1HwndRenderTarget, D2D1_FACTORY_TYPE_SINGLE_THREADED,
    D2D1_HWND_RENDER_TARGET_PROPERTIES, D2D1_PRESENT_OPTIONS_NONE, D2D1_RENDER_TARGET_PROPERTIES,
};
use windows::Win32::Graphics::DirectWrite::{
    DWriteCreateFactory, IDWriteFactory, IDWriteTextFormat, DWRITE_FACTORY_TYPE_SHARED,
    DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT_BOLD, DWRITE_FONT_WEIGHT_NORMAL,
    DWRITE_PARAGRAPH_ALIGNMENT_CENTER, DWRITE_READING_DIRECTION_LEFT_TO_RIGHT,
    DWRITE_TEXT_ALIGNMENT_LEADING, DWRITE_TEXT_ALIGNMENT_TRAILING, DWRITE_WORD_WRAPPING_NO_WRAP,
};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, InvalidateRect, MonitorFromPoint, MonitorFromRect, MONITORINFO,
    MONITOR_DEFAULTTONEAREST,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::{
    SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, PostMessageW, RegisterClassExW,
    SetWindowPos, ShowWindow, TranslateMessage, CS_DROPSHADOW, CS_HREDRAW, CS_VREDRAW,
    HWND_TOPMOST, MSG, SWP_NOACTIVATE, SWP_SHOWWINDOW, SW_HIDE, WM_APP, WM_ERASEBKGND,
    WM_LBUTTONDOWN, WM_PAINT, WNDCLASSEXW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
    WS_POPUP,
};
use windows_core::{HSTRING, PCWSTR};

use crate::layout::{
    build_visible_page, hit_test_row, RgbColor, ThemePalette, VisiblePage, FOOTER_HEIGHT,
    PADDING_Y, ROW_HEIGHT,
};
use crate::placement::{place, Rect};

const CLASS_NAME: &str = "AsteriaCandidateWindow";
const WM_SYNC_CHANNEL: u32 = WM_APP + 10;

pub enum UiEvent {
    Show(Show),
    Hide,
}

fn color_f(c: RgbColor) -> D2D1_COLOR_F {
    let (r, g, b) = c.as_f32();
    D2D1_COLOR_F { r, g, b, a: 1.0 }
}

struct RendererState {
    hwnd: HWND,
    rx: Receiver<UiEvent>,
    d2d: ID2D1Factory,
    fmt_main: IDWriteTextFormat,
    fmt_bold: IDWriteTextFormat,
    fmt_note: IDWriteTextFormat,
    fmt_footer: IDWriteTextFormat,
    target: Option<ID2D1HwndRenderTarget>,
    current_show: Option<Show>,
    current_page: Option<VisiblePage>,
}

impl RendererState {
    fn work_area_for_caret(caret: Rect) -> Rect {
        // SAFETY: caret 矩形を基準にモニタ作業領域を取得する。
        unsafe {
            let r = RECT {
                left: caret.left,
                top: caret.top,
                right: caret.right,
                bottom: caret.bottom,
            };
            let mon = if caret.right > caret.left && caret.bottom > caret.top {
                MonitorFromRect(&r, MONITOR_DEFAULTTONEAREST)
            } else {
                MonitorFromPoint(
                    POINT {
                        x: caret.left,
                        y: caret.top,
                    },
                    MONITOR_DEFAULTTONEAREST,
                )
            };
            let mut mi = MONITORINFO {
                cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                ..Default::default()
            };
            if GetMonitorInfoW(mon, &mut mi).as_bool() {
                Rect {
                    left: mi.rcWork.left,
                    top: mi.rcWork.top,
                    right: mi.rcWork.right,
                    bottom: mi.rcWork.bottom,
                }
            } else {
                Rect {
                    left: 0,
                    top: 0,
                    right: 1920,
                    bottom: 1080,
                }
            }
        }
    }

    fn apply_show(&mut self, show: Show) {
        if show.candidates.is_empty() {
            self.apply_hide();
            return;
        }
        let page = build_visible_page(&show);
        let caret_proto = show.caret.unwrap_or_default();
        let caret = Rect {
            left: caret_proto.left,
            top: caret_proto.top,
            right: caret_proto.right,
            bottom: caret_proto.bottom,
        };
        let work = Self::work_area_for_caret(caret);
        let pos = place(work, caret, page.size);

        // SAFETY: WS_EX_NOACTIVATE ウィンドウをフォーカスを奪わず位置合わせして表示する。
        unsafe {
            let _ = SetWindowPos(
                self.hwnd,
                HWND_TOPMOST,
                pos.x,
                pos.y,
                page.size.width,
                page.size.height,
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            );
            if let Some(rt) = &self.target {
                let _ = rt.Resize(&D2D_SIZE_U {
                    width: page.size.width.max(1) as u32,
                    height: page.size.height.max(1) as u32,
                });
            }
            let _ = InvalidateRect(self.hwnd, None, false);
        }
        self.current_page = Some(page);
        self.current_show = Some(show);
    }

    fn apply_hide(&mut self) {
        self.current_page = None;
        self.current_show = None;
        // SAFETY: ウィンドウを非表示にする。
        unsafe {
            let _ = ShowWindow(self.hwnd, SW_HIDE);
        }
    }

    fn paint(&mut self) {
        let (Some(show), Some(page)) = (&self.current_show, &self.current_page) else {
            return;
        };
        // SAFETY: Direct2D HwndRenderTarget を遅延初期化して候補リストを描画する。
        unsafe {
            if self.target.is_none() {
                let props = D2D1_RENDER_TARGET_PROPERTIES::default();
                let hwnd_props = D2D1_HWND_RENDER_TARGET_PROPERTIES {
                    hwnd: self.hwnd,
                    pixelSize: D2D_SIZE_U {
                        width: page.size.width.max(1) as u32,
                        height: page.size.height.max(1) as u32,
                    },
                    presentOptions: D2D1_PRESENT_OPTIONS_NONE,
                };
                if let Ok(rt) = self.d2d.CreateHwndRenderTarget(&props, &hwnd_props) {
                    self.target = Some(rt);
                }
            }
            let Some(rt) = self.target.clone() else {
                return;
            };
            let palette = ThemePalette::resolve(show.dark_mode, show.high_contrast);
            let w = page.size.width as f32;
            let h = page.size.height as f32;

            rt.BeginDraw();
            rt.Clear(Some(&color_f(palette.bg)));

            if let (
                Ok(brush_text),
                Ok(brush_sel_bg),
                Ok(brush_sel_fg),
                Ok(brush_note),
                Ok(brush_accent),
                Ok(brush_footer_bg),
                Ok(brush_footer_fg),
                Ok(brush_border),
            ) = (
                rt.CreateSolidColorBrush(&color_f(palette.text), None),
                rt.CreateSolidColorBrush(&color_f(palette.selected_bg), None),
                rt.CreateSolidColorBrush(&color_f(palette.selected_text), None),
                rt.CreateSolidColorBrush(&color_f(palette.annotation), None),
                rt.CreateSolidColorBrush(&color_f(palette.accent), None),
                rt.CreateSolidColorBrush(&color_f(palette.footer_bg), None),
                rt.CreateSolidColorBrush(&color_f(palette.footer_text), None),
                rt.CreateSolidColorBrush(&color_f(palette.border), None),
            ) {
                for (i, row) in page.rows.iter().enumerate() {
                    let top = (PADDING_Y + (i as i32) * ROW_HEIGHT) as f32;
                    let bottom = top + ROW_HEIGHT as f32;
                    if row.selected {
                        let sel_rc = D2D_RECT_F {
                            left: 4.0,
                            top: top + 1.0,
                            right: w - 4.0,
                            bottom: bottom - 1.0,
                        };
                        rt.FillRectangle(&sel_rc, &brush_sel_bg);
                        let accent_rc = D2D_RECT_F {
                            left: 4.0,
                            top: top + 4.0,
                            right: 7.0,
                            bottom: bottom - 4.0,
                        };
                        rt.FillRectangle(&accent_rc, &brush_accent);
                    }

                    let num_w: Vec<u16> = format!("{}", row.shortcut).encode_utf16().collect();
                    let num_rc = D2D_RECT_F {
                        left: 12.0,
                        top,
                        right: 28.0,
                        bottom,
                    };
                    rt.DrawText(
                        &num_w,
                        &self.fmt_note,
                        &num_rc,
                        &brush_note,
                        windows::Win32::Graphics::Direct2D::D2D1_DRAW_TEXT_OPTIONS_NONE,
                        Default::default(),
                    );

                    let cand_w: Vec<u16> = row.text.encode_utf16().collect();
                    let cand_rc = D2D_RECT_F {
                        left: 32.0,
                        top,
                        right: w - 16.0,
                        bottom,
                    };
                    let fmt = if row.selected {
                        &self.fmt_bold
                    } else {
                        &self.fmt_main
                    };
                    let fg = if row.selected {
                        &brush_sel_fg
                    } else {
                        &brush_text
                    };
                    rt.DrawText(
                        &cand_w,
                        fmt,
                        &cand_rc,
                        fg,
                        windows::Win32::Graphics::Direct2D::D2D1_DRAW_TEXT_OPTIONS_CLIP,
                        Default::default(),
                    );

                    if !row.annotation.is_empty() {
                        let note_w: Vec<u16> = row.annotation.encode_utf16().collect();
                        let note_rc = D2D_RECT_F {
                            left: w * 0.48,
                            top,
                            right: w - 12.0,
                            bottom,
                        };
                        rt.DrawText(
                            &note_w,
                            &self.fmt_note,
                            &note_rc,
                            &brush_note,
                            windows::Win32::Graphics::Direct2D::D2D1_DRAW_TEXT_OPTIONS_CLIP,
                            Default::default(),
                        );
                    }
                }

                // フッターバー描画
                let footer_top = h - FOOTER_HEIGHT as f32;
                let footer_rc = D2D_RECT_F {
                    left: 1.0,
                    top: footer_top,
                    right: w - 1.0,
                    bottom: h - 1.0,
                };
                rt.FillRectangle(&footer_rc, &brush_footer_bg);
                let ft_w: Vec<u16> = page.footer_text.encode_utf16().collect();
                let ft_rc = D2D_RECT_F {
                    left: 12.0,
                    top: footer_top,
                    right: w - 12.0,
                    bottom: h - 1.0,
                };
                rt.DrawText(
                    &ft_w,
                    &self.fmt_footer,
                    &ft_rc,
                    &brush_footer_fg,
                    windows::Win32::Graphics::Direct2D::D2D1_DRAW_TEXT_OPTIONS_CLIP,
                    Default::default(),
                );

                // 外枠線
                let frame = D2D_RECT_F {
                    left: 0.5,
                    top: 0.5,
                    right: w - 0.5,
                    bottom: h - 0.5,
                };
                rt.DrawRectangle(&frame, &brush_border, 1.0, None);
            }

            if rt.EndDraw(None, None).is_err() {
                self.target = None;
            }
        }
    }
}

thread_local! {
    static UI_STATE: RefCell<Option<RendererState>> = const { RefCell::new(None) };
}

fn make_text_format(
    dw: &IDWriteFactory,
    size: f32,
    bold: bool,
    align_right: bool,
) -> anyhow::Result<IDWriteTextFormat> {
    let family = HSTRING::from("Yu Gothic UI");
    let locale = HSTRING::from("ja-JP");
    // SAFETY: DirectWrite テキストフォーマット生成。
    unsafe {
        let fmt = dw.CreateTextFormat(
            PCWSTR(family.as_ptr()),
            None,
            if bold {
                DWRITE_FONT_WEIGHT_BOLD
            } else {
                DWRITE_FONT_WEIGHT_NORMAL
            },
            DWRITE_FONT_STYLE_NORMAL,
            windows::Win32::Graphics::DirectWrite::DWRITE_FONT_STRETCH_NORMAL,
            size,
            PCWSTR(locale.as_ptr()),
        )?;
        fmt.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
        fmt.SetReadingDirection(DWRITE_READING_DIRECTION_LEFT_TO_RIGHT)?;
        fmt.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;
        fmt.SetTextAlignment(if align_right {
            DWRITE_TEXT_ALIGNMENT_TRAILING
        } else {
            DWRITE_TEXT_ALIGNMENT_LEADING
        })?;
        Ok(fmt)
    }
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_SYNC_CHANNEL => {
            UI_STATE.with(|cell| {
                if let Some(st) = cell.borrow_mut().as_mut() {
                    while let Ok(ev) = st.rx.try_recv() {
                        match ev {
                            UiEvent::Show(s) => st.apply_show(s),
                            UiEvent::Hide => st.apply_hide(),
                        }
                    }
                }
            });
            LRESULT(0)
        }
        WM_ERASEBKGND => LRESULT(1),
        WM_PAINT => {
            let mut ps = windows::Win32::Graphics::Gdi::PAINTSTRUCT::default();
            let _ = windows::Win32::Graphics::Gdi::BeginPaint(hwnd, &mut ps);
            UI_STATE.with(|cell| {
                if let Some(st) = cell.borrow_mut().as_mut() {
                    st.paint();
                }
            });
            let _ = windows::Win32::Graphics::Gdi::EndPaint(hwnd, &ps);
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
            UI_STATE.with(|cell| {
                if let Some(st) = cell.borrow().as_ref() {
                    if let (Some(show), Some(page)) = (&st.current_show, &st.current_page) {
                        if let Some(idx) = hit_test_row(page, y) {
                            let target_hwnd = HWND(show.notify_window as usize as *mut _);
                            if !target_hwnd.is_invalid() {
                                let _ = PostMessageW(target_hwnd, WM_APP, WPARAM(idx), LPARAM(0));
                            }
                        }
                    }
                }
            });
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

/// Windows 候補ウィンドウを起動し、IPC 名前付きパイプスレッドの指示で更新・描画する。
pub fn run_windows_renderer() -> anyhow::Result<()> {
    use anyhow::Context;
    use asteria_proto::renderer::RendererMessage;
    use asteria_proto::{read_message, PROTOCOL_MAJOR};

    // SAFETY: プロセス単位で PerMonitorV2 DPI 対応とウィンドウクラスを登録する。
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }

    let name = asteria_client::default_renderer_pipe_name().context("パイプ名を決められない")?;
    let (ready_tx, ready_rx) = mpsc::sync_channel::<anyhow::Result<()>>(1);
    let (tx, rx) = mpsc::channel::<UiEvent>();

    let class_w = HSTRING::from(CLASS_NAME);
    // SAFETY: Win32 ウィンドウと Direct2D/DirectWrite ファクトリを作成する。
    let hwnd = unsafe {
        let hinst: HMODULE = GetModuleHandleW(None)?;
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW | CS_DROPSHADOW,
            lpfnWndProc: Some(wnd_proc),
            hInstance: hinst.into(),
            hbrBackground: windows::Win32::Graphics::Gdi::HBRUSH(
                windows::Win32::Graphics::Gdi::CreateSolidBrush(COLORREF(0x00FF_FFFF)).0,
            ),
            lpszClassName: PCWSTR(class_w.as_ptr()),
            ..Default::default()
        };
        let _ = RegisterClassExW(&wc);
        CreateWindowExW(
            WS_EX_NOACTIVATE | WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
            PCWSTR(class_w.as_ptr()),
            PCWSTR::null(),
            WS_POPUP,
            0,
            0,
            260,
            200,
            None,
            None,
            hinst,
            None,
        )?
    };

    // SAFETY: Direct2D / DirectWrite ファクトリの生成。
    let d2d: ID2D1Factory = unsafe { D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)? };
    // SAFETY: DirectWrite 共有ファクトリ生成。
    let dwrite: IDWriteFactory = unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)? };
    let fmt_main = make_text_format(&dwrite, 14.0, false, false)?;
    let fmt_bold = make_text_format(&dwrite, 14.0, true, false)?;
    let fmt_note = make_text_format(&dwrite, 12.0, false, true)?;
    let fmt_footer = make_text_format(&dwrite, 11.5, false, false)?;

    UI_STATE.with(|cell| {
        *cell.borrow_mut() = Some(RendererState {
            hwnd,
            rx,
            d2d,
            fmt_main,
            fmt_bold,
            fmt_note,
            fmt_footer,
            target: None,
            current_show: None,
            current_page: None,
        });
    });

    let hwnd_num = hwnd.0 as usize;
    std::thread::spawn(move || {
        let mut listener = match asteria_client::PipeListener::bind(&name) {
            Ok(l) => {
                eprintln!("asteria-renderer: {name} で待ち受け");
                let _ = ready_tx.send(Ok(()));
                l
            }
            Err(e) => {
                let _ = ready_tx.send(Err(anyhow::anyhow!(
                    "{name} で待ち受けできない(ほかの renderer が動いている): {e}"
                )));
                return;
            }
        };
        loop {
            let Ok(mut stream) = listener.accept() else {
                break;
            };
            let tx = tx.clone();
            std::thread::spawn(move || {
                while let Ok(Some(msg)) = read_message::<_, RendererMessage>(&mut stream) {
                    if msg.protocol_major != PROTOCOL_MAJOR {
                        continue;
                    }
                    let ev = match msg.body {
                        Some(renderer_message::Body::Show(s)) => UiEvent::Show(s),
                        Some(renderer_message::Body::Hide(_)) => UiEvent::Hide,
                        None => continue,
                    };
                    if tx.send(ev).is_ok() {
                        // SAFETY: UI スレッドのメッセージループを起こしてチャンネルを反映する。
                        unsafe {
                            let _ = PostMessageW(
                                HWND(hwnd_num as *mut _),
                                WM_SYNC_CHANNEL,
                                WPARAM(0),
                                LPARAM(0),
                            );
                        }
                    }
                }
            });
        }
    });
    ready_rx
        .recv()
        .context("レンダラ待ち受けスレッドの起動に失敗")??;

    // SAFETY: 標準 Win32 メッセージループ。
    unsafe {
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    Ok(())
}
