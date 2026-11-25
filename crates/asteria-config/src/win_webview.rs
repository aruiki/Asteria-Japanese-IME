//! WebView2 ウィンドウホスト (Windows 専用)。
#![allow(unsafe_code)]

use std::cell::RefCell;
use std::path::PathBuf;
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use asteria_client::managed::{Managed, SystemConnector};
use asteria_common::paths::{install_dir, profile_dir};
use asteria_proto::ipc::{request, response};
use serde_json::Value;
use webview2_com::Microsoft::Web::WebView2::Win32::{
    CreateCoreWebView2EnvironmentWithOptions, ICoreWebView2, ICoreWebView2Controller,
    ICoreWebView2Environment, ICoreWebView2WebMessageReceivedEventArgs,
};
use webview2_com::{
    CreateCoreWebView2ControllerCompletedHandler, CreateCoreWebView2EnvironmentCompletedHandler,
    WebMessageReceivedEventHandler,
};
use windows::Win32::Foundation::{HMODULE, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{GetSysColorBrush, COLOR_WINDOW};
use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::WinRT::EventRegistrationToken;
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetClientRect, GetMessageW, MessageBoxW,
    PostMessageW, PostQuitMessage, RegisterClassExW, ShowWindow, TranslateMessage, CW_USEDEFAULT,
    MB_ICONINFORMATION, MB_OK, MSG, SW_SHOW, WM_APP, WM_DESTROY, WM_SIZE, WNDCLASSEXW,
    WS_OVERLAPPEDWINDOW,
};
use windows_core::{HSTRING, PCWSTR, PWSTR};

use crate::bridge::{bundled_html, BridgeHost, SettingsBridge};

const CLASS_NAME: &str = "AsteriaConfigWindow";
const WM_PUSH_TO_WEB: u32 = WM_APP + 1;
const SUPPORT_URL: &str = "https://github.com/sponsors/aruiki";

struct WinHost {
    hwnd_num: usize,
    tx_web: mpsc::Sender<String>,
    data_dir: PathBuf,
    runtime_dir: PathBuf,
    client: Mutex<Managed<SystemConnector>>,
}

impl BridgeHost for WinHost {
    fn ipc_call(&self, body: request::Body) -> Result<response::Body, String> {
        let Ok(mut client) = self.client.lock() else {
            return Err("IPC クライアントのロックに失敗しました".to_owned());
        };
        for wait_ms in [0u64, 120, 220, 420, 500] {
            if wait_ms > 0 {
                std::thread::sleep(Duration::from_millis(wait_ms));
            }
            if let Some(resp) = client.raw_request(body.clone()) {
                return Ok(resp);
            }
        }
        Err("変換サーバーへ接続できません".to_owned())
    }

    fn save_text_dialog(&self, default_name: &str, text: &str) -> bool {
        let path = self.data_dir.join(default_name);
        std::fs::create_dir_all(&self.data_dir).is_ok() && std::fs::write(path, text).is_ok()
    }

    fn open_support_page(&self) {
        let url_w = HSTRING::from(SUPPORT_URL);
        let open_w = HSTRING::from("open");
        // SAFETY: ShellExecuteW で既定ブラウザーへ定数 HTTPS URL を渡す。
        unsafe {
            let _ = ShellExecuteW(
                None,
                PCWSTR(open_w.as_ptr()),
                PCWSTR(url_w.as_ptr()),
                PCWSTR::null(),
                PCWSTR::null(),
                SW_SHOW,
            );
        }
    }

    fn push_event(&self, event: Value) {
        let msg = serde_json::json!({ "event": event }).to_string();
        if self.tx_web.send(msg).is_ok() {
            // SAFETY: UI スレッドへ WebView 送信を通知する。
            unsafe {
                let _ = PostMessageW(
                    HWND(self.hwnd_num as *mut _),
                    WM_PUSH_TO_WEB,
                    WPARAM(0),
                    LPARAM(0),
                );
            }
        }
    }

    fn gpu_vram_gb(&self) -> f64 {
        let gpu = asteria_lm::llama_dll::detect_runtime_gpu(&self.runtime_dir);
        gpu.total_vram_bytes as f64 / (1u64 << 30) as f64
    }

    fn data_dir(&self) -> PathBuf {
        self.data_dir.clone()
    }

    fn runtime_dir(&self) -> PathBuf {
        self.runtime_dir.clone()
    }
}

struct WindowCtx {
    controller: Option<ICoreWebView2Controller>,
    webview: Option<ICoreWebView2>,
    rx_web: mpsc::Receiver<String>,
}

thread_local! {
    static CTX: RefCell<Option<WindowCtx>> = const { RefCell::new(None) };
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_SIZE => {
            CTX.with(|c| {
                if let Some(ctx) = c.borrow().as_ref() {
                    if let Some(ctrl) = &ctx.controller {
                        let mut rc = RECT::default();
                        if GetClientRect(hwnd, &mut rc).is_ok() {
                            let _ = ctrl.SetBounds(rc);
                        }
                    }
                }
            });
            LRESULT(0)
        }
        WM_PUSH_TO_WEB => {
            CTX.with(|c| {
                if let Some(ctx) = c.borrow().as_ref() {
                    if let Some(wv) = &ctx.webview {
                        while let Ok(json_str) = ctx.rx_web.try_recv() {
                            let w = HSTRING::from(json_str);
                            let _ = wv.PostWebMessageAsJson(PCWSTR(w.as_ptr()));
                        }
                    }
                }
            });
            LRESULT(0)
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

pub fn run_settings_window() -> anyhow::Result<()> {
    // SAFETY: COM STA 初期化。
    let co = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    let res = run_window_inner();
    if co.is_ok() {
        // SAFETY: 対になる CoUninitialize。
        unsafe { CoUninitialize() };
    }
    res
}

fn show_webview_fallback(hwnd: HWND, data_dir: &std::path::Path) {
    let config_path = data_dir.join("asteria_settings.json");
    let msg = format!(
        "Microsoft Edge WebView2 ランタイムが利用できないため、設定画面を表示できません。\n\n設定ファイルは次の場所にあります:\n{}\n\nWebView2 Evergreen ランタイムをインストールすると GUI 設定画面が開けます。",
        config_path.display()
    );
    let msg_w = HSTRING::from(msg);
    let title_w = HSTRING::from("Asteria");
    // SAFETY: 親ウィンドウへモーダル情報ダイアログを表示し、終了メッセージを通知する。
    unsafe {
        MessageBoxW(
            hwnd,
            PCWSTR(msg_w.as_ptr()),
            PCWSTR(title_w.as_ptr()),
            MB_OK | MB_ICONINFORMATION,
        );
        PostQuitMessage(0);
    }
}

fn run_window_inner() -> anyhow::Result<()> {
    let class_w = HSTRING::from(CLASS_NAME);
    let title_w = HSTRING::from("Asteria");
    // SAFETY: Win32 トップレベルウィンドウを生成する。
    let hwnd = unsafe {
        let hinst: HMODULE = GetModuleHandleW(None)?;
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(wnd_proc),
            hInstance: hinst.into(),
            hbrBackground: GetSysColorBrush(COLOR_WINDOW),
            lpszClassName: PCWSTR(class_w.as_ptr()),
            ..Default::default()
        };
        let _ = RegisterClassExW(&wc);
        let h = CreateWindowExW(
            Default::default(),
            PCWSTR(class_w.as_ptr()),
            PCWSTR(title_w.as_ptr()),
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            960,
            680,
            None,
            None,
            hinst,
            None,
        )?;
        let _ = ShowWindow(h, SW_SHOW);
        h
    };

    let webview_dir = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("Asteria")
        .join("webview2");
    let data_dir = profile_dir();
    let runtime_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(PathBuf::from))
        .unwrap_or_else(install_dir);
    let server_exe = [
        runtime_dir.join("asteria-server.exe"),
        install_dir().join("asteria-server.exe"),
    ]
    .into_iter()
    .find(|p| p.is_file());
    let connector = SystemConnector {
        address: None,
        server: server_exe,
    };
    let (tx_web, rx_web) = mpsc::channel::<String>();
    let host: Arc<dyn BridgeHost> = Arc::new(WinHost {
        hwnd_num: hwnd.0 as usize,
        tx_web: tx_web.clone(),
        data_dir: data_dir.clone(),
        runtime_dir,
        client: Mutex::new(Managed::new(connector)),
    });
    let bridge = Arc::new(SettingsBridge::new(host));

    CTX.with(|c| {
        *c.borrow_mut() = Some(WindowCtx {
            controller: None,
            webview: None,
            rx_web,
        });
    });

    let user_data_w = HSTRING::from(webview_dir.to_string_lossy().as_ref());
    let data_dir_fallback = data_dir.clone();
    let html_content = bundled_html();

    // SAFETY: CreateCoreWebView2EnvironmentWithOptions で WebView2 を非同期初期化する。
    unsafe {
        let env_handler = CreateCoreWebView2EnvironmentCompletedHandler::create(Box::new(
            move |err, env: Option<ICoreWebView2Environment>| {
                if let Err(e) = err {
                    show_webview_fallback(hwnd, &data_dir_fallback);
                    return Err(e);
                }
                let Some(env) = env else {
                    show_webview_fallback(hwnd, &data_dir_fallback);
                    return Err(windows_core::Error::from_win32());
                };
                let bridge_ctrl = Arc::clone(&bridge);
                let html_clone = html_content.clone();
                let ctrl_handler =
                    CreateCoreWebView2ControllerCompletedHandler::create(Box::new(
                        move |err2, ctrl: Option<ICoreWebView2Controller>| {
                            err2?;
                            let controller = ctrl.ok_or_else(windows_core::Error::from_win32)?;
                            let webview = controller.CoreWebView2()?;
                            let mut rc = RECT::default();
                            let _ = GetClientRect(hwnd, &mut rc);
                            let _ = controller.SetBounds(rc);
                            let _ = controller.SetIsVisible(true);

                            let bridge_msg = Arc::clone(&bridge_ctrl);
                            let wv_reply = webview.clone();
                            let msg_handler =
                                WebMessageReceivedEventHandler::create(Box::new(
                                    move |_sender,
                                          args: Option<
                                        ICoreWebView2WebMessageReceivedEventArgs,
                                    >| {
                                        let Some(args) = args else {
                                            return Ok(());
                                        };
                                        let mut raw_pwstr = PWSTR::null();
                                        args.TryGetWebMessageAsString(&mut raw_pwstr)?;
                                        let req_str = raw_pwstr.to_string().unwrap_or_default();
                                        let resp_json = bridge_msg.handle_json(&req_str);
                                        let resp_w = HSTRING::from(resp_json);
                                        let _ =
                                            wv_reply.PostWebMessageAsJson(PCWSTR(resp_w.as_ptr()));
                                        Ok(())
                                    },
                                ));
                            let mut token = EventRegistrationToken::default();
                            let _ = webview.add_WebMessageReceived(&msg_handler, &mut token);

                            let html_w = HSTRING::from(html_clone);
                            let _ = webview.NavigateToString(PCWSTR(html_w.as_ptr()));

                            CTX.with(|c| {
                                if let Some(ctx) = c.borrow_mut().as_mut() {
                                    ctx.controller = Some(controller);
                                    ctx.webview = Some(webview);
                                }
                            });
                            Ok(())
                        },
                    ));
                env.CreateCoreWebView2Controller(hwnd, &ctrl_handler)
            },
        ));
        if let Err(e) = CreateCoreWebView2EnvironmentWithOptions(
            PCWSTR::null(),
            PCWSTR(user_data_w.as_ptr()),
            None,
            &env_handler,
        ) {
            show_webview_fallback(hwnd, &data_dir);
            return Err(e.into());
        }
    }

    // SAFETY: Win32 メッセージループ。
    unsafe {
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    Ok(())
}
