//! 窓を作り、パイプで受けた命令を Direct2D で描く。Win32・COM の呼び出しはこのファイルに集める。

#![allow(unsafe_code)]

use std::cell::RefCell;
use std::sync::mpsc::{channel, Receiver};

use asteria_renderer::layout::{self, Content, Rect};
use asteria_renderer::palette::{self, Palette, Rgb};
use asteria_renderer::win::MessageServer;
use asteria_renderer::{decode_command, key_from_random, path_info, pipe_name, pipe_sddl};
use windows::core::{w, Interface, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D1_COLOR_F, D2D_POINT_2F, D2D_RECT_F, D2D_SIZE_U,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1CreateFactory, ID2D1Factory, ID2D1HwndRenderTarget, D2D1_DRAW_TEXT_OPTIONS_NONE,
    D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1_HWND_RENDER_TARGET_PROPERTIES,
    D2D1_PRESENT_OPTIONS_NONE, D2D1_RENDER_TARGET_PROPERTIES,
};
use windows::Win32::Graphics::DirectWrite::{
    DWriteCreateFactory, IDWriteFactory, IDWriteTextFormat, IDWriteTextLayout,
    DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL,
    DWRITE_FONT_WEIGHT_NORMAL, DWRITE_TEXT_METRICS,
};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MonitorFromPoint, ValidateRect, MONITORINFO, MONITOR_DEFAULTTONEAREST,
};
use windows::Win32::Security::Cryptography::{BCryptGenRandom, BCRYPT_USE_SYSTEM_PREFERRED_RNG};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD};
use windows::Win32::UI::HiDpi::{
    GetDpiForWindow, SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, PostMessageW, RegisterClassW,
    SetWindowPos, ShowWindow, HWND_TOPMOST, MSG, SWP_NOACTIVATE, SW_HIDE, SW_SHOWNOACTIVATE,
    WM_APP, WM_PAINT, WNDCLASSW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};

const WM_COMMAND_READY: u32 = WM_APP + 1;
/// 文字の大きさ(96 DPI のときの px)。今の窓の候補の文字と同じ。
const FONT_PX: f32 = 16.0;
const PAD_X: f32 = 8.0;
const PAD_Y: f32 = 3.0;

struct Ui {
    hwnd: HWND,
    d2d: ID2D1Factory,
    dwrite: IDWriteFactory,
    target: Option<ID2D1HwndRenderTarget>,
    content: Option<Content>,
    rx: Receiver<asteria_renderer::mozc::commands::RendererCommand>,
}

thread_local! {
    static UI: RefCell<Option<Ui>> = const { RefCell::new(None) };
}

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1).cloned())
}

/// 暗い配色か(Windows の「アプリ モード」)。
fn dark_mode() -> bool {
    let mut value = 1u32;
    let mut size = std::mem::size_of::<u32>() as u32;
    // SAFETY: value と size は呼び出しの間生きている有効な出力先。
    let r = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize"),
            w!("AppsUseLightTheme"),
            RRF_RT_REG_DWORD,
            None,
            Some((&mut value as *mut u32).cast()),
            Some(&mut size),
        )
    };
    r.is_ok() && value == 0
}

fn color(c: Rgb) -> D2D1_COLOR_F {
    let [r, g, b] = c.to_f32();
    D2D1_COLOR_F { r, g, b, a: 1.0 }
}

pub fn run(args: Vec<String>) -> anyhow::Result<()> {
    let profile = arg(&args, "--profile").ok_or_else(|| anyhow::anyhow!("--profile がない"))?;
    let version = arg(&args, "--version").unwrap_or_else(|| "0.0.0.0".to_string());
    let sid = asteria_renderer::win::current_user_sid()?;

    let mut random = [0u8; 16];
    // SAFETY: random は有効な出力先。
    unsafe { BCryptGenRandom(None, &mut random, BCRYPT_USE_SYSTEM_PREFERRED_RNG) }.ok()?;
    let key = key_from_random(&random);
    let server = MessageServer::bind(&pipe_name(&key), &pipe_sddl(&sid))?;
    // 鍵のファイルは、パイプを開いてから書く(TIP は書かれた鍵ですぐつなぐ)。
    std::fs::write(
        std::path::Path::new(&profile).join("renderer.ipc"),
        path_info(&key, std::process::id(), &version),
    )?;

    // SAFETY: 引数は値だけ。
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }
    let hwnd = create_window()?;
    let (tx, rx) = channel();
    let hwnd_raw = hwnd.0 as isize;
    std::thread::spawn(move || loop {
        let mut cmd = None;
        if server
            .serve_one(|req| {
                cmd = decode_command(req);
                Vec::new()
            })
            .is_err()
        {
            continue;
        }
        if let Some(c) = cmd {
            if tx.send(c).is_err() {
                return;
            }
            // SAFETY: hwnd は窓のスレッドが持つ窓。PostMessage はどのスレッドからでも呼べる。
            unsafe {
                let _ = PostMessageW(
                    HWND(hwnd_raw as *mut _),
                    WM_COMMAND_READY,
                    WPARAM(0),
                    LPARAM(0),
                );
            }
        }
    });

    // SAFETY: 引数は値だけ。
    let d2d: ID2D1Factory = unsafe { D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)? };
    // SAFETY: 引数は値だけ。
    let dwrite: IDWriteFactory = unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)? };
    UI.with(|ui| {
        *ui.borrow_mut() = Some(Ui {
            hwnd,
            d2d,
            dwrite,
            target: None,
            content: None,
            rx,
        });
    });

    let mut msg = MSG::default();
    // SAFETY: msg は有効な出力先。
    while unsafe { GetMessageW(&mut msg, None, 0, 0) }.as_bool() {
        // SAFETY: GetMessageW が書いた msg。
        unsafe { DispatchMessageW(&msg) };
    }
    Ok(())
}

fn create_window() -> anyhow::Result<HWND> {
    // SAFETY: 自分のモジュールのハンドルを得るだけ。
    let instance = unsafe { GetModuleHandleW(None)? };
    let class = w!("AsteriaCandidateWindow");
    let wc = WNDCLASSW {
        lpfnWndProc: Some(wnd_proc),
        hInstance: instance.into(),
        lpszClassName: class,
        ..Default::default()
    };
    // SAFETY: wc は呼び出しの間生きている。
    unsafe { RegisterClassW(&wc) };
    // SAFETY: class は登録済み。
    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW,
            class,
            PCWSTR::null(),
            WS_POPUP,
            0,
            0,
            1,
            1,
            None,
            None,
            instance,
            None,
        )?
    };
    Ok(hwnd)
}

extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_COMMAND_READY => {
            UI.with(|ui| {
                if let Some(ui) = ui.borrow_mut().as_mut() {
                    // 溜まった命令は最後の 1 つだけ描く(連打で古い表示を描かない)。
                    let mut last = None;
                    while let Ok(c) = ui.rx.try_recv() {
                        last = Some(c);
                    }
                    if let Some(c) = last {
                        apply(ui, &c);
                    }
                }
            });
            LRESULT(0)
        }
        WM_PAINT => {
            UI.with(|ui| {
                if let Some(ui) = ui.borrow_mut().as_mut() {
                    let _ = paint(ui);
                }
            });
            // SAFETY: hwnd は自分の窓。
            unsafe {
                let _ = ValidateRect(hwnd, None);
            }
            LRESULT(0)
        }
        // SAFETY: 既定の処理に渡すだけ。
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

fn scale(ui: &Ui) -> f32 {
    // SAFETY: ui.hwnd は自分の窓。
    let dpi = unsafe { GetDpiForWindow(ui.hwnd) };
    if dpi == 0 {
        1.0
    } else {
        dpi as f32 / 96.0
    }
}

fn text_format(ui: &Ui, px: f32) -> windows::core::Result<IDWriteTextFormat> {
    // SAFETY: 引数の文字列は静的。
    unsafe {
        ui.dwrite.CreateTextFormat(
            w!("Yu Gothic UI"),
            None,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_FONT_STYLE_NORMAL,
            DWRITE_FONT_STRETCH_NORMAL,
            px,
            w!("ja-jp"),
        )
    }
}

fn text_layout(
    ui: &Ui,
    format: &IDWriteTextFormat,
    s: &str,
) -> windows::core::Result<(IDWriteTextLayout, f32, f32)> {
    let text: Vec<u16> = s.encode_utf16().collect();
    // SAFETY: text と format は呼び出しの間生きている。
    let layout = unsafe { ui.dwrite.CreateTextLayout(&text, format, 4096.0, 4096.0)? };
    let mut m = DWRITE_TEXT_METRICS::default();
    // SAFETY: m は有効な出力先。
    unsafe { layout.GetMetrics(&mut m)? };
    Ok((layout, m.widthIncludingTrailingWhitespace, m.height))
}

/// 1 行の高さと各列の幅(番号・候補・説明)を測る(DPI を掛けた px)。
fn measure(ui: &Ui, content: &Content, s: f32) -> windows::core::Result<(f32, [f32; 3], f32)> {
    let format = text_format(ui, FONT_PX * s)?;
    let mut cols = [0f32; 3];
    let mut line = 0f32;
    for row in &content.rows {
        for (i, t) in [&row.shortcut, &row.value, &row.description]
            .iter()
            .enumerate()
        {
            if t.is_empty() {
                continue;
            }
            let (_, w, h) = text_layout(ui, &format, t)?;
            cols[i] = cols[i].max(w);
            line = line.max(h);
        }
    }
    let footer_h = if content.footer.is_some() { line } else { 0.0 };
    Ok((line + 2.0 * PAD_Y * s, cols, footer_h))
}

fn apply(ui: &mut Ui, command: &asteria_renderer::mozc::commands::RendererCommand) {
    let content = layout::candidate_window(command).and_then(layout::content);
    let caret = layout::caret_rect(command);
    let (Some(content), Some(caret)) = (content, caret) else {
        ui.content = None;
        // SAFETY: ui.hwnd は自分の窓。
        unsafe {
            let _ = ShowWindow(ui.hwnd, SW_HIDE);
        }
        return;
    };
    let s = scale(ui);
    let Ok((row_h, cols, footer_h)) = measure(ui, &content, s) else {
        return;
    };
    let width = cols
        .iter()
        .filter(|w| **w > 0.0)
        .map(|w| w + 2.0 * PAD_X * s)
        .sum::<f32>()
        + 2.0;
    let height = row_h * content.rows.len() as f32 + footer_h + 2.0;
    let size = (width.ceil() as i32, height.ceil() as i32);
    // SAFETY: 値だけを渡す。
    let monitor = unsafe {
        MonitorFromPoint(
            POINT {
                x: caret.left,
                y: caret.bottom,
            },
            MONITOR_DEFAULTTONEAREST,
        )
    };
    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    // SAFETY: info は有効な出力先。
    let _ = unsafe { GetMonitorInfoW(monitor, &mut info) };
    let RECT {
        left,
        top,
        right,
        bottom,
    } = info.rcWork;
    let r = layout::place(
        caret,
        size,
        Rect {
            left,
            top,
            right,
            bottom,
        },
    );
    ui.content = Some(content);
    // SAFETY: ui.hwnd は自分の窓。
    unsafe {
        let _ = SetWindowPos(
            ui.hwnd,
            HWND_TOPMOST,
            r.left,
            r.top,
            r.width(),
            r.height(),
            SWP_NOACTIVATE,
        );
        let _ = ShowWindow(ui.hwnd, SW_SHOWNOACTIVATE);
    }
    if let Some(t) = &ui.target {
        // SAFETY: t は自分の窓の描画先。
        let _ = unsafe {
            t.Resize(&D2D_SIZE_U {
                width: r.width() as u32,
                height: r.height() as u32,
            })
        };
    }
    let _ = paint(ui);
}

fn paint(ui: &mut Ui) -> windows::core::Result<()> {
    let Some(content) = ui.content.clone() else {
        return Ok(());
    };
    if ui.target.is_none() {
        let mut rc = RECT::default();
        // SAFETY: rc は有効な出力先。
        unsafe { windows::Win32::UI::WindowsAndMessaging::GetClientRect(ui.hwnd, &mut rc)? };
        let props = D2D1_HWND_RENDER_TARGET_PROPERTIES {
            hwnd: ui.hwnd,
            pixelSize: D2D_SIZE_U {
                width: rc.right as u32,
                height: rc.bottom as u32,
            },
            presentOptions: D2D1_PRESENT_OPTIONS_NONE,
        };
        // SAFETY: props は呼び出しの間生きている。
        ui.target = Some(unsafe {
            ui.d2d
                .CreateHwndRenderTarget(&D2D1_RENDER_TARGET_PROPERTIES::default(), &props)?
        });
    }
    let s = scale(ui);
    let (row_h, cols, _) = measure(ui, &content, s)?;
    let pal: &Palette = palette::for_theme(dark_mode());
    let format = text_format(ui, FONT_PX * s)?;
    let Some(t) = ui.target.clone() else {
        return Ok(());
    };
    // SAFETY: t は自分の窓の描画先。BeginDraw と EndDraw を対にする。
    unsafe {
        t.SetDpi(96.0, 96.0);
        t.BeginDraw();
        t.Clear(Some(&color(pal.background)));
        let size = t.GetSize();
        let border = t.CreateSolidColorBrush(&color(pal.border), None)?;
        t.DrawRectangle(
            &D2D_RECT_F {
                left: 0.5,
                top: 0.5,
                right: size.width - 0.5,
                bottom: size.height - 0.5,
            },
            &border,
            1.0,
            None,
        );
        let fg = t.CreateSolidColorBrush(&color(pal.foreground), None)?;
        let sub = t.CreateSolidColorBrush(&color(pal.secondary), None)?;
        let focus = t.CreateSolidColorBrush(&color(pal.focused_background), None)?;
        let focus_border = t.CreateSolidColorBrush(&color(pal.focused_border), None)?;
        let mut y = 1.0;
        for row in &content.rows {
            if row.focused {
                let r = D2D_RECT_F {
                    left: 1.0,
                    top: y,
                    right: size.width - 1.0,
                    bottom: y + row_h,
                };
                t.FillRectangle(&r, &focus);
                t.DrawRectangle(&r, &focus_border, 1.0, None);
            }
            let mut x = 1.0;
            for (i, text) in [&row.shortcut, &row.value, &row.description]
                .iter()
                .enumerate()
            {
                if cols[i] <= 0.0 {
                    continue;
                }
                if !text.is_empty() {
                    let (layout, _, _) = text_layout(ui, &format, text)?;
                    let brush = if i == 1 { &fg } else { &sub };
                    t.DrawTextLayout(
                        D2D_POINT_2F {
                            x: x + PAD_X * s,
                            y: y + PAD_Y * s,
                        },
                        &layout,
                        brush,
                        D2D1_DRAW_TEXT_OPTIONS_NONE,
                    );
                }
                x += cols[i] + 2.0 * PAD_X * s;
            }
            y += row_h;
        }
        if let Some(footer) = &content.footer {
            let (layout, w, _) = text_layout(ui, &format, footer)?;
            t.DrawTextLayout(
                D2D_POINT_2F {
                    x: size.width - w - PAD_X * s,
                    y,
                },
                &layout,
                &sub,
                D2D1_DRAW_TEXT_OPTIONS_NONE,
            );
        }
        let r = t.EndDraw(None, None);
        if r.is_err() {
            // 描画先を失った(GPU の再起動など)。次の描画で作り直す。
            ui.target = None;
        }
        let _ = t.cast::<ID2D1HwndRenderTarget>();
    }
    Ok(())
}
