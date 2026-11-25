//! Win32 ウィンドウ撮影 (`capture_window.py`) と PE アイコン資源展開 (`dump_icons.py`)。
#![cfg_attr(windows, allow(unsafe_code))]

use std::path::Path;

use anyhow::Result;

#[cfg(windows)]
mod platform {
    use std::path::Path;
    use std::process::Command;
    use std::thread;
    use std::time::Duration;

    use anyhow::{bail, Context, Result};
    use image::{imageops, DynamicImage, Rgba, RgbaImage};
    use windows_sys::Win32::Foundation::{HWND, LPARAM, RECT};
    use windows_sys::Win32::Graphics::Gdi::{
        CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDIBits, GetWindowDC,
        ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS,
    };
    use windows_sys::Win32::Storage::Xps::PrintWindow;
    use windows_sys::Win32::System::LibraryLoader::{
        EnumResourceNamesW, FindResourceW, LoadLibraryExW, LoadResource, LockResource,
        SizeofResource, LOAD_LIBRARY_AS_DATAFILE, LOAD_LIBRARY_AS_IMAGE_RESOURCE,
    };
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        keybd_event, KEYEVENTF_KEYUP, VK_CONTROL, VK_TAB,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowRect, GetWindowThreadProcessId, IsWindowVisible, PostMessageW,
        SetForegroundWindow, SetProcessDPIAware, WM_CHAR,
    };

    const PW_RENDERFULLCONTENT: u32 = 2;
    const RT_ICON: *const u16 = 3 as *const u16;

    pub fn capture_window(
        exe: &Path,
        arg: Option<&str>,
        out: &Path,
        wait_secs: f64,
        tabs: usize,
        query: Option<&str>,
    ) -> Result<()> {
        // SAFETY: SetProcessDPIAware はプロセス単位の DPI 認識設定であり副作用がない。
        unsafe { SetProcessDPIAware() };

        let mut cmd = Command::new(exe);
        if let Some(a) = arg.filter(|s| !s.is_empty()) {
            cmd.arg(a);
        }
        let mut child = cmd.spawn()?;
        let target_pid = child.id();
        thread::sleep(Duration::from_secs_f64(wait_secs));

        struct SearchState {
            pid: u32,
            found: Option<HWND>,
        }
        unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> i32 {
            // SAFETY: lparam は呼び出し元スタック上の SearchState への有効ポインタ。
            let st = unsafe { &mut *(lparam as *mut SearchState) };
            let mut pid = 0u32;
            // SAFETY: pid への有効なポインタを渡して PID を取得する。
            unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
            // SAFETY: HWND の可視性と矩形を取得する Win32 API。
            if pid == st.pid && unsafe { IsWindowVisible(hwnd) } != 0 {
                let mut r = RECT::default();
                // SAFETY: r への有効ポインタ。
                unsafe { GetWindowRect(hwnd, &mut r) };
                if r.right - r.left > 100 {
                    st.found = Some(hwnd);
                    return 0;
                }
            }
            1
        }
        let mut st = SearchState {
            pid: target_pid,
            found: None,
        };
        // SAFETY: enum_proc は st を読み書きするのみでスレッド終了まで生存する。
        unsafe { EnumWindows(Some(enum_proc), (&mut st as *mut SearchState) as LPARAM) };
        let Some(hwnd) = st.found else {
            let _ = child.kill();
            bail!("ウィンドウが見つかりません");
        };

        let capture_to = |dest: &Path| -> Result<()> {
            let mut r = RECT::default();
            // SAFETY: hwnd の現在矩形を取得。
            unsafe { GetWindowRect(hwnd, &mut r) };
            let w = (r.right - r.left).max(1);
            let h = (r.bottom - r.top).max(1);
            // SAFETY: GDI デバイスコンテキストと互換ビットマップの確保・取得・解放を対で実行。
            let buf = unsafe {
                let hdc = GetWindowDC(hwnd);
                let mdc = CreateCompatibleDC(hdc);
                let bmp = CreateCompatibleBitmap(hdc, w, h);
                SelectObject(mdc, bmp);
                PrintWindow(hwnd, mdc, PW_RENDERFULLCONTENT);
                let mut bmi = BITMAPINFO {
                    bmiHeader: BITMAPINFOHEADER {
                        biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                        biWidth: w,
                        biHeight: -h,
                        biPlanes: 1,
                        biBitCount: 32,
                        biCompression: BI_RGB,
                        ..Default::default()
                    },
                    ..Default::default()
                };
                let mut raw = vec![0u8; (w * h * 4) as usize];
                GetDIBits(
                    mdc,
                    bmp,
                    0,
                    h as u32,
                    raw.as_mut_ptr().cast(),
                    &mut bmi,
                    DIB_RGB_COLORS,
                );
                DeleteObject(bmp);
                DeleteDC(mdc);
                ReleaseDC(hwnd, hdc);
                raw
            };
            let mut rgba = vec![0u8; buf.len()];
            for (dst, bgra) in rgba.chunks_exact_mut(4).zip(buf.chunks_exact(4)) {
                dst[0] = bgra[2];
                dst[1] = bgra[1];
                dst[2] = bgra[0];
                dst[3] = 255;
            }
            let img = RgbaImage::from_raw(w as u32, h as u32, rgba).context("image buffer")?;
            DynamicImage::ImageRgba8(img).to_rgb8().save(dest)?;
            println!("saved {} {w} {h}", dest.display());
            Ok(())
        };

        capture_to(out)?;
        let stem_base = out.with_extension("").to_string_lossy().into_owned();
        if let Some(q) = query.filter(|s| !s.is_empty()) {
            // SAFETY: Win32 キー入力および WM_CHAR 送出。
            unsafe {
                SetForegroundWindow(hwnd);
                thread::sleep(Duration::from_millis(500));
                keybd_event(VK_CONTROL as u8, 0, 0, 0);
                keybd_event(0x46, 0, 0, 0);
                keybd_event(0x46, 0, KEYEVENTF_KEYUP, 0);
                keybd_event(VK_CONTROL as u8, 0, KEYEVENTF_KEYUP, 0);
                thread::sleep(Duration::from_millis(500));
                for u in q.encode_utf16() {
                    PostMessageW(hwnd, WM_CHAR, u as usize, 0);
                    thread::sleep(Duration::from_millis(100));
                }
            }
            thread::sleep(Duration::from_millis(1500));
            capture_to(Path::new(&format!("{stem_base}-search.png")))?;
        }
        for i in 1..=tabs {
            // SAFETY: Ctrl+Tab 送出。
            unsafe {
                SetForegroundWindow(hwnd);
                thread::sleep(Duration::from_millis(500));
                keybd_event(VK_CONTROL as u8, 0, 0, 0);
                keybd_event(VK_TAB as u8, 0, 0, 0);
                keybd_event(VK_TAB as u8, 0, KEYEVENTF_KEYUP, 0);
                keybd_event(VK_CONTROL as u8, 0, KEYEVENTF_KEYUP, 0);
            }
            thread::sleep(Duration::from_millis(1500));
            capture_to(Path::new(&format!("{stem_base}-tab{i}.png")))?;
        }
        let _ = child.kill();
        Ok(())
    }

    pub fn dump_icons(binary: &Path, out: &Path) -> Result<usize> {
        let wide: Vec<u16> = binary
            .to_string_lossy()
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        // SAFETY: DATAFILE | IMAGE_RESOURCE で安全に PE 資源を読み込む。
        let hmod = unsafe {
            LoadLibraryExW(
                wide.as_ptr(),
                std::ptr::null_mut(),
                LOAD_LIBRARY_AS_DATAFILE | LOAD_LIBRARY_AS_IMAGE_RESOURCE,
            )
        };
        if hmod.is_null() {
            bail!("モジュールを開けません: {}", binary.display());
        }
        struct Names {
            ids: Vec<usize>,
        }
        unsafe extern "system" fn name_cb(
            _: *mut core::ffi::c_void,
            _: *const u16,
            name: *const u16,
            lparam: isize,
        ) -> i32 {
            // SAFETY: lparam は Names への有効ポインタ。
            let st = unsafe { &mut *(lparam as *mut Names) };
            st.ids.push(name as usize);
            1
        }
        let mut names = Names { ids: Vec::new() };
        // SAFETY: 同一スレッド上で即座に列挙完了する。
        unsafe {
            EnumResourceNamesW(
                hmod,
                RT_ICON,
                Some(name_cb),
                (&mut names as *mut Names) as isize,
            )
        };

        let mut imgs = Vec::new();
        for id in names.ids {
            // SAFETY: EnumResourceNamesW が返した識別子から RT_ICON バイト列を読み取る。
            let slice: Vec<u8> = unsafe {
                let res = FindResourceW(hmod, id as *const u16, RT_ICON);
                if res.is_null() {
                    continue;
                }
                let hres = LoadResource(hmod, res);
                let ptr = LockResource(hres) as *const u8;
                let sz = SizeofResource(hmod, res) as usize;
                if ptr.is_null() || sz == 0 {
                    continue;
                }
                std::slice::from_raw_parts(ptr, sz).to_vec()
            };
            if slice.starts_with(b"\x89PNG") {
                if let Ok(im) = image::load_from_memory(&slice) {
                    imgs.push(im.to_rgba8());
                }
            } else if slice.len() >= 12 {
                let w = i32::from_le_bytes([slice[4], slice[5], slice[6], slice[7]]).unsigned_abs();
                let mut ico_bytes = Vec::with_capacity(22 + slice.len());
                ico_bytes.extend_from_slice(&[0, 0, 1, 0, 1, 0]);
                ico_bytes.push((w & 255) as u8);
                ico_bytes.push((w & 255) as u8);
                ico_bytes.extend_from_slice(&[0, 0, 1, 0, 32, 0]);
                ico_bytes.extend_from_slice(&(slice.len() as u32).to_le_bytes());
                ico_bytes.extend_from_slice(&22u32.to_le_bytes());
                ico_bytes.extend_from_slice(&slice);
                if let Ok(im) = image::load_from_memory(&ico_bytes) {
                    imgs.push(im.to_rgba8());
                }
            }
        }
        let total_w: u32 = imgs
            .iter()
            .take(24)
            .map(|im| im.width().min(64) + 4)
            .sum::<u32>()
            + 4;
        let mut sheet = RgbaImage::from_pixel(total_w, 72, Rgba([200, 200, 200, 255]));
        let mut x = 4u32;
        for im in imgs.iter().take(24) {
            let s = im.width().min(64);
            let thumb = imageops::resize(im, s, s, imageops::FilterType::Lanczos3);
            crate::canvas::alpha_composite(&mut sheet, &thumb, x as i32, 4);
            x += s + 4;
        }
        sheet.save(out)?;
        Ok(imgs.len())
    }
}

#[cfg(not(windows))]
mod platform {
    use anyhow::{bail, Result};
    use std::path::Path;

    pub fn capture_window(
        _: &Path,
        _: Option<&str>,
        _: &Path,
        _: f64,
        _: usize,
        _: Option<&str>,
    ) -> Result<()> {
        bail!("capture-window は Windows 専用です")
    }

    pub fn dump_icons(_: &Path, _: &Path) -> Result<usize> {
        bail!("dump-icons は Windows 専用です")
    }
}

/// `capture_window.py`: Win32 `PrintWindow` でウィンドウを PNG 保存する。
pub fn run_capture_window(
    exe: &Path,
    arg: Option<&str>,
    out: &Path,
    wait_secs: f64,
    tabs: usize,
    query: Option<&str>,
) -> Result<()> {
    platform::capture_window(exe, arg, out, wait_secs, tabs, query)
}

/// `dump_icons.py`: PE ファイルのアイコン資源をタイル状 PNG へ書き出す。
pub fn run_dump_icons(binary: &Path, out: &Path) -> Result<usize> {
    platform::dump_icons(binary, out)
}
