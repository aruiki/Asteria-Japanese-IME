//! タスクバー入力モード表示 (`あ` / `A`, `mode_item.cpp` 移植)。
#![allow(unsafe_code)]

use std::cell::RefCell;

use windows::Win32::Foundation::{BOOL, E_INVALIDARG, POINT, RECT};
use windows::Win32::Graphics::Gdi::{
    CreateBitmap, CreateCompatibleDC, CreateDIBSection, CreateFontW, DeleteObject, DrawTextW,
    FillRect, GdiFlush, GetDC, GetStockObject, ReleaseDC, SelectObject, SetBkMode,
    ANTIALIASED_QUALITY, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, BLACK_BRUSH, CLIP_DEFAULT_PRECIS,
    DEFAULT_CHARSET, DEFAULT_PITCH, DIB_RGB_COLORS, DT_CENTER, DT_NOPREFIX, DT_SINGLELINE,
    DT_VCENTER, FF_DONTCARE, FW_NORMAL, HBITMAP, HGDIOBJ, OUT_DEFAULT_PRECIS, TRANSPARENT,
};
use windows::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD};
use windows::Win32::UI::TextServices::{
    ITfCompartmentMgr, ITfLangBarItem, ITfLangBarItemButton, ITfLangBarItemButton_Impl,
    ITfLangBarItemSink, ITfLangBarItem_Impl, ITfMenu, ITfSource, ITfSource_Impl, ITfThreadMgr,
    TfLBIClick, GUID_COMPARTMENT_KEYBOARD_OPENCLOSE, GUID_LBI_INPUTMODE, TF_LANGBARITEMINFO,
    TF_LBI_ICON, TF_LBI_STATUS, TF_LBI_STATUS_HIDDEN, TF_LBI_STYLE_BTN_MENU,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateIconIndirect, GetSystemMetrics, HICON, ICONINFO, SM_CXSMICON,
};
use windows_core::{implement, IUnknown, Interface, Result, BSTR, GUID, HSTRING, PCWSTR, VARIANT};

use crate::guids::CLSID_TEXT_SERVICE_U128;

fn light_taskbar() -> bool {
    let mut val = 0u32;
    let mut size = std::mem::size_of::<u32>() as u32;
    let subkey = HSTRING::from("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize");
    let value_name = HSTRING::from("SystemUsesLightTheme");
    // SAFETY: NUL 終端 HSTRING を用いて REG_DWORD を照会する。
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            PCWSTR(subkey.as_ptr()),
            PCWSTR(value_name.as_ptr()),
            RRF_RT_REG_DWORD,
            None,
            Some((&mut val as *mut u32).cast()),
            Some(&mut size),
        )
    };
    status.is_ok() && val != 0
}

pub fn make_mode_icon(open: bool) -> HICON {
    // SAFETY: GDI DIB セクションに `あ` / `A` を描画して HICON を構築し、すべての一時オブジェクトを解放する。
    unsafe {
        let size = GetSystemMetrics(SM_CXSMICON).max(16);
        let screen = GetDC(None);
        let dc = CreateCompatibleDC(screen);
        ReleaseDC(None, screen);
        if dc.is_invalid() {
            return HICON::default();
        }
        let mut bi = BITMAPINFO::default();
        bi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        bi.bmiHeader.biWidth = size;
        bi.bmiHeader.biHeight = -size;
        bi.bmiHeader.biPlanes = 1;
        bi.bmiHeader.biBitCount = 32;
        bi.bmiHeader.biCompression = BI_RGB.0;

        let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
        let color =
            CreateDIBSection(dc, &bi, DIB_RGB_COLORS, &mut bits, None, 0).unwrap_or_default();
        let mask_bytes = vec![0u8; ((size + 15) / 16 * 2 * size) as usize];
        let mask: HBITMAP = CreateBitmap(size, size, 1, 1, Some(mask_bytes.as_ptr().cast()));
        let font_name = HSTRING::from("Yu Gothic UI");
        let font = CreateFontW(
            -size * 7 / 8,
            0,
            0,
            0,
            FW_NORMAL.0 as i32,
            0,
            0,
            0,
            DEFAULT_CHARSET.0 as u32,
            OUT_DEFAULT_PRECIS.0 as u32,
            CLIP_DEFAULT_PRECIS.0 as u32,
            ANTIALIASED_QUALITY.0 as u32,
            (DEFAULT_PITCH.0 | FF_DONTCARE.0) as u32,
            PCWSTR(font_name.as_ptr()),
        );

        let mut icon = HICON::default();
        if !color.is_invalid() && !mask.is_invalid() && !bits.is_null() {
            let old_bm = SelectObject(dc, color);
            let old_font = if !font.is_invalid() {
                SelectObject(dc, font)
            } else {
                HGDIOBJ::default()
            };
            let mut rc = RECT {
                left: 0,
                top: 0,
                right: size,
                bottom: size,
            };
            FillRect(
                dc,
                &rc,
                windows::Win32::Graphics::Gdi::HBRUSH(GetStockObject(BLACK_BRUSH).0),
            );
            SetBkMode(dc, TRANSPARENT);
            windows::Win32::Graphics::Gdi::SetTextColor(
                dc,
                windows::Win32::Foundation::COLORREF(0x00FFFFFF),
            );
            let mut ch_buf: Vec<u16> = if open { "あ" } else { "A" }.encode_utf16().collect();
            DrawTextW(
                dc,
                &mut ch_buf,
                &mut rc,
                DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
            );
            let _ = GdiFlush();

            let ink: u32 = if light_taskbar() {
                0x0000_0000
            } else {
                0x00FF_FFFF
            };
            let px = std::slice::from_raw_parts_mut(bits.cast::<u32>(), (size * size) as usize);
            for p in px {
                let v = *p;
                let alpha = (v & 0xFF).max((v >> 8) & 0xFF).max((v >> 16) & 0xFF);
                *p = (alpha << 24) | ink;
            }
            if !old_font.is_invalid() {
                SelectObject(dc, old_font);
            }
            SelectObject(dc, old_bm);
            let ii = ICONINFO {
                fIcon: BOOL(1),
                xHotspot: 0,
                yHotspot: 0,
                hbmMask: mask,
                hbmColor: color,
            };
            icon = CreateIconIndirect(&ii).unwrap_or_default();
        }
        if !font.is_invalid() {
            let _ = DeleteObject(font);
        }
        if !mask.is_invalid() {
            let _ = DeleteObject(mask);
        }
        if !color.is_invalid() {
            let _ = DeleteObject(color);
        }
        let _ = windows::Win32::Graphics::Gdi::DeleteDC(dc);
        icon
    }
}

pub fn is_keyboard_open(thread_mgr: &ITfThreadMgr, client_id: u32) -> bool {
    // SAFETY: TSF スレッドマネージャの open/close コンパートメントから状態を読む。
    unsafe {
        let Ok(cm) = thread_mgr.cast::<ITfCompartmentMgr>() else {
            return false;
        };
        let Ok(comp) = cm.GetCompartment(&GUID_COMPARTMENT_KEYBOARD_OPENCLOSE) else {
            return false;
        };
        let _ = client_id;
        let Ok(v) = comp.GetValue() else {
            return false;
        };
        i32::try_from(&v).unwrap_or(0) != 0
    }
}

pub fn set_keyboard_open(thread_mgr: &ITfThreadMgr, client_id: u32, open: bool) {
    // SAFETY: open/close コンパートメントに VT_I4 を書き込む。
    unsafe {
        if let Ok(cm) = thread_mgr.cast::<ITfCompartmentMgr>() {
            if let Ok(comp) = cm.GetCompartment(&GUID_COMPARTMENT_KEYBOARD_OPENCLOSE) {
                let v = VARIANT::from(i32::from(open));
                let _ = comp.SetValue(client_id, &v);
            }
        }
    }
}

#[implement(ITfLangBarItem, ITfLangBarItemButton, ITfSource)]
pub struct ModeItem {
    thread_mgr: ITfThreadMgr,
    client_id: u32,
    sink: RefCell<Option<ITfLangBarItemSink>>,
}

impl ModeItem {
    pub fn new(thread_mgr: ITfThreadMgr, client_id: u32) -> Self {
        Self {
            thread_mgr,
            client_id,
            sink: RefCell::new(None),
        }
    }

    pub fn notify_update(&self) {
        if let Some(sink) = self.sink.borrow().as_ref() {
            // SAFETY: 有効な sink へアイコン・ツールチップ・状態の再取得を通知する。
            unsafe {
                let _ = sink.OnUpdate(TF_LBI_ICON | TF_LBI_STATUS);
            }
        }
    }
}

impl ITfLangBarItem_Impl for ModeItem_Impl {
    fn GetInfo(&self, pinfo: *mut TF_LANGBARITEMINFO) -> Result<()> {
        if pinfo.is_null() {
            return Err(E_INVALIDARG.into());
        }
        // SAFETY: TSF が渡した TF_LANGBARITEMINFO 構造体を初期化する。
        unsafe {
            let mut info = TF_LANGBARITEMINFO {
                clsidService: GUID::from_u128(CLSID_TEXT_SERVICE_U128),
                guidItem: GUID_LBI_INPUTMODE,
                dwStyle: TF_LBI_STYLE_BTN_MENU,
                ulSort: 0,
                ..Default::default()
            };
            let desc: Vec<u16> = "Asteria 入力モード".encode_utf16().collect();
            let n = desc.len().min(info.szDescription.len() - 1);
            info.szDescription[..n].copy_from_slice(&desc[..n]);
            *pinfo = info;
        }
        Ok(())
    }

    fn GetStatus(&self) -> Result<u32> {
        let open = is_keyboard_open(&self.thread_mgr, self.client_id);
        Ok(if open { 0 } else { TF_LBI_STATUS_HIDDEN })
    }

    fn Show(&self, _fshow: BOOL) -> Result<()> {
        Err(windows::Win32::Foundation::E_NOTIMPL.into())
    }

    fn GetTooltipString(&self) -> Result<BSTR> {
        let open = is_keyboard_open(&self.thread_mgr, self.client_id);
        Ok(BSTR::from(if open {
            "Asteria: ひらがな (クリックで半角英数)"
        } else {
            "Asteria: 直接入力 (クリックでひらがな)"
        }))
    }
}

const MENU_ID_TOGGLE_MODE: u32 = 1;
const MENU_ID_OPEN_CONFIG: u32 = 2;
const MENU_ID_RELOAD_SERVER: u32 = 3;

fn spawn_config_gui() {
    use windows::Win32::System::LibraryLoader::GetModuleFileNameW;
    let hmod = crate::text_service::dll_module();
    let mut buf = vec![0u16; 512];
    // SAFETY: 自身の DLL パスから兄弟 `asteria-config.exe` の場所を解決する。
    let n = unsafe { GetModuleFileNameW(hmod, &mut buf) } as usize;
    if n == 0 || n >= buf.len() {
        return;
    }
    let dll_path = std::path::PathBuf::from(String::from_utf16_lossy(&buf[..n]));
    if let Some(dir) = dll_path.parent() {
        let candidate = dir.join("asteria-config.exe");
        let exe = if candidate.is_file() {
            Some(candidate)
        } else {
            dir.parent()
                .map(|p| p.join("asteria-config.exe"))
                .filter(|p| p.is_file())
        };
        if let Some(path) = exe {
            let _ = std::process::Command::new(path).spawn();
        }
    }
}

impl ITfLangBarItemButton_Impl for ModeItem_Impl {
    fn OnClick(&self, _click: TfLBIClick, _pt: &POINT, _prcarea: *const RECT) -> Result<()> {
        let open = is_keyboard_open(&self.thread_mgr, self.client_id);
        set_keyboard_open(&self.thread_mgr, self.client_id, !open);
        Ok(())
    }

    fn InitMenu(&self, pmenu: Option<&ITfMenu>) -> Result<()> {
        let Some(menu) = pmenu else {
            return Ok(());
        };
        let items = [
            (MENU_ID_TOGGLE_MODE, "ひらがな / 直接入力を切り替え"),
            (MENU_ID_OPEN_CONFIG, "Asteria の設定..."),
            (MENU_ID_RELOAD_SERVER, "辞書・設定を再読み込み"),
        ];
        for (id, label) in items {
            let wide: Vec<u16> = label.encode_utf16().collect();
            // SAFETY: 有効な ITfMenu へ UTF-16 ラベル項目を登録する。
            unsafe {
                let _ = menu.AddMenuItem(id, 0, None, None, &wide, std::ptr::null_mut());
            }
        }
        Ok(())
    }

    fn OnMenuSelect(&self, wid: u32) -> Result<()> {
        match wid {
            MENU_ID_TOGGLE_MODE => {
                let open = is_keyboard_open(&self.thread_mgr, self.client_id);
                set_keyboard_open(&self.thread_mgr, self.client_id, !open);
            }
            MENU_ID_OPEN_CONFIG => spawn_config_gui(),
            MENU_ID_RELOAD_SERVER => {
                let mut cli = asteria_client::managed::Managed::new(
                    asteria_client::managed::SystemConnector::default(),
                );
                let _ = cli.raw_request(asteria_proto::ipc::request::Body::Reload(
                    asteria_proto::ipc::Reload {},
                ));
            }
            _ => {}
        }
        Ok(())
    }

    fn GetIcon(&self) -> Result<HICON> {
        let open = is_keyboard_open(&self.thread_mgr, self.client_id);
        Ok(make_mode_icon(open))
    }

    fn GetText(&self) -> Result<BSTR> {
        let open = is_keyboard_open(&self.thread_mgr, self.client_id);
        Ok(BSTR::from(if open { "あ" } else { "A" }))
    }
}

impl ITfSource_Impl for ModeItem_Impl {
    fn AdviseSink(&self, _riid: *const GUID, punk: Option<&IUnknown>) -> Result<u32> {
        let unk = punk.ok_or(E_INVALIDARG)?;
        let sink: ITfLangBarItemSink = unk.cast()?;
        *self.sink.borrow_mut() = Some(sink);
        Ok(1)
    }

    fn UnadviseSink(&self, dwcookie: u32) -> Result<()> {
        if dwcookie == 1 {
            *self.sink.borrow_mut() = None;
        }
        Ok(())
    }
}
