//! Engine IPC (`asteria-server.exe`) および Renderer IPC (`asteria-renderer.exe`) ブリッジ。
#![allow(unsafe_code)]

use std::ffi::{CStr, CString};
use std::path::PathBuf;
use std::ptr;

use asteria_client::ffi::{
    asteria_client_free, asteria_client_open, asteria_create_session, asteria_output_candidate,
    asteria_output_candidate_annotation, asteria_output_candidate_count,
    asteria_output_candidate_focused, asteria_output_candidate_visible, asteria_output_committed,
    asteria_output_consumed, asteria_output_cursor, asteria_output_free,
    asteria_output_preedit_attribute, asteria_output_preedit_count, asteria_output_preedit_text,
    asteria_poll_update, asteria_send_command, asteria_send_key, asteria_set_context_full,
    AsteriaClient, AsteriaOutput, ASTERIA_OK,
};
use asteria_client::renderer_ffi::{
    asteria_renderer_free, asteria_renderer_hide, asteria_renderer_open, asteria_renderer_show,
    AsteriaRect, AsteriaRenderer,
};
use windows::Win32::Foundation::HMODULE;
use windows::Win32::System::LibraryLoader::GetModuleFileNameW;

use crate::text::PreeditSegment;

#[derive(Debug, Clone, Default)]
pub struct CandidateList {
    pub texts: Vec<String>,
    pub annotations: Vec<String>,
    pub focused: u32,
    pub visible: bool,
}

#[derive(Debug, Clone, Default)]
pub struct EngineOutput {
    pub consumed: bool,
    pub preedit: Vec<PreeditSegment>,
    pub cursor: u32,
    pub committed: String,
    pub candidates: CandidateList,
}

pub struct EngineBridge {
    client: *mut AsteriaClient,
    renderer: *mut AsteriaRenderer,
    session_id: u64,
    app_id: CString,
}

fn cstr_to_string(p: *const i8) -> String {
    if p.is_null() {
        String::new()
    } else {
        // SAFETY: asteria-client が返した有効な NUL 終端 UTF-8 文字列。
        unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned()
    }
}

fn take_output(status: i32, out: *mut AsteriaOutput) -> Option<EngineOutput> {
    if status != ASTERIA_OK || out.is_null() {
        return None;
    }
    // SAFETY: out は asteria-client から所有権を受け取った有効なポインタであり、最後に free する。
    unsafe {
        let mut res = EngineOutput {
            consumed: asteria_output_consumed(out) != 0,
            cursor: asteria_output_cursor(out),
            committed: cstr_to_string(asteria_output_committed(out)),
            ..EngineOutput::default()
        };
        let preedit_count = asteria_output_preedit_count(out);
        for i in 0..preedit_count {
            res.preedit.push(PreeditSegment {
                text: cstr_to_string(asteria_output_preedit_text(out, i)),
                attribute: asteria_output_preedit_attribute(out, i),
            });
        }
        res.candidates.visible = asteria_output_candidate_visible(out) != 0;
        res.candidates.focused = asteria_output_candidate_focused(out);
        let cand_count = asteria_output_candidate_count(out);
        for i in 0..cand_count {
            res.candidates
                .texts
                .push(cstr_to_string(asteria_output_candidate(out, i)));
            res.candidates
                .annotations
                .push(cstr_to_string(asteria_output_candidate_annotation(out, i)));
        }
        asteria_output_free(out);
        Some(res)
    }
}

fn module_dir(module: HMODULE) -> Option<PathBuf> {
    let mut buf = vec![0u16; 512];
    loop {
        // SAFETY: 呼び出し元が保持する DLL モジュールハンドルからパス長を取得する。
        let n = unsafe { GetModuleFileNameW(module, &mut buf) } as usize;
        if n == 0 {
            return None;
        }
        if n < buf.len() {
            let s = String::from_utf16_lossy(&buf[..n]);
            return PathBuf::from(s).parent().map(PathBuf::from);
        }
        buf.resize(buf.len() * 2, 0);
    }
}

fn process_exe_name() -> String {
    let mut buf = vec![0u16; 512];
    // SAFETY: GetModuleFileNameW(None) は現在のプロセスの実行ファイル名を返す。
    let n = unsafe { GetModuleFileNameW(HMODULE::default(), &mut buf) } as usize;
    let s = String::from_utf16_lossy(&buf[..n.min(buf.len())]);
    PathBuf::from(s)
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or("unknown.exe")
        .to_owned()
}

fn find_sibling_exe(module: HMODULE, exe_name: &str) -> Option<CString> {
    let dir = module_dir(module)?;
    let here = dir.join(exe_name);
    let candidate = if here.is_file() {
        here
    } else if let Some(parent) = dir.parent() {
        let up = parent.join(exe_name);
        if up.is_file() {
            up
        } else {
            here
        }
    } else {
        here
    };
    CString::new(candidate.to_string_lossy().as_bytes()).ok()
}

impl EngineBridge {
    pub fn open(module: HMODULE) -> Self {
        let server_c = find_sibling_exe(module, "asteria-server.exe");
        let renderer_c = find_sibling_exe(module, "asteria-renderer.exe");
        let server_ptr = server_c.as_ref().map_or(ptr::null(), |c| c.as_ptr());
        let renderer_ptr = renderer_c.as_ref().map_or(ptr::null(), |c| c.as_ptr());

        // SAFETY: CString が有効な NUL 終端 UTF-8 ポインタを提供する。
        let renderer = unsafe { asteria_renderer_open(ptr::null(), renderer_ptr) };
        // SAFETY: CString が有効な NUL 終端 UTF-8 ポインタを提供する。
        let client = unsafe { asteria_client_open(ptr::null(), server_ptr) };
        let app_id = CString::new(process_exe_name()).unwrap_or_default();

        let mut bridge = Self {
            client,
            renderer,
            session_id: 0,
            app_id,
        };
        bridge.ensure_session();
        bridge
    }

    fn ensure_session(&mut self) -> bool {
        if self.session_id != 0 {
            return true;
        }
        if self.client.is_null() {
            return false;
        }
        let mut sid = 0u64;
        // SAFETY: client と sid は有効。0 = INPUT_SCOPE_DEFAULT。
        let status =
            unsafe { asteria_create_session(self.client, self.app_id.as_ptr(), 0, &mut sid) };
        if status == ASTERIA_OK && sid != 0 {
            self.session_id = sid;
            true
        } else {
            false
        }
    }

    pub fn set_context(&mut self, left_context: &str, following_text: &str) {
        if !self.ensure_session() {
            return;
        }
        let Ok(c_left) = CString::new(left_context) else {
            return;
        };
        let Ok(c_follow) = CString::new(following_text) else {
            return;
        };
        // SAFETY: client と c_left, c_follow は有効。
        unsafe {
            asteria_set_context_full(
                self.client,
                self.session_id,
                c_left.as_ptr(),
                c_follow.as_ptr(),
            );
        }
    }

    pub fn send_key(
        &mut self,
        vk: u32,
        text: &str,
        modifiers: u32,
        key_up: bool,
    ) -> Option<EngineOutput> {
        if !self.ensure_session() {
            return None;
        }
        let c_text = CString::new(text).unwrap_or_default();
        let mut out: *mut AsteriaOutput = ptr::null_mut();
        // SAFETY: 有効なポインタ群を渡し、戻り値を take_output で確実に解放する。
        let status = unsafe {
            asteria_send_key(
                self.client,
                self.session_id,
                vk,
                c_text.as_ptr(),
                modifiers,
                i32::from(key_up),
                &mut out,
            )
        };
        take_output(status, out)
    }

    pub fn send_command(&mut self, kind: u32, argument: u32) -> Option<EngineOutput> {
        if !self.ensure_session() {
            return None;
        }
        let mut out: *mut AsteriaOutput = ptr::null_mut();
        // SAFETY: 有効なポインタ群を渡し、戻り値を take_output で解放する。
        let status =
            unsafe { asteria_send_command(self.client, self.session_id, kind, argument, &mut out) };
        take_output(status, out)
    }

    pub fn poll_update(&mut self) -> Option<EngineOutput> {
        if !self.ensure_session() {
            return None;
        }
        let mut out: *mut AsteriaOutput = ptr::null_mut();
        // SAFETY: 有効な client と出力先ポインタを渡す。
        let status = unsafe { asteria_poll_update(self.client, self.session_id, &mut out) };
        take_output(status, out)
    }

    pub fn show_candidates(
        &mut self,
        list: &CandidateList,
        caret: AsteriaRect,
        owner_hwnd: u64,
        notify_hwnd: u64,
    ) {
        if self.renderer.is_null() || !list.visible || list.texts.is_empty() {
            return;
        }
        let c_texts: Vec<CString> = list
            .texts
            .iter()
            .map(|s| CString::new(s.as_str()).unwrap_or_default())
            .collect();
        let c_notes: Vec<CString> = list
            .annotations
            .iter()
            .map(|s| CString::new(s.as_str()).unwrap_or_default())
            .collect();
        let p_texts: Vec<*const i8> = c_texts.iter().map(|c| c.as_ptr()).collect();
        let p_notes: Vec<*const i8> = c_notes.iter().map(|c| c.as_ptr()).collect();
        // SAFETY: c_texts / c_notes がこの呼び出し全体を通して生存し、長さ count で一致する。
        unsafe {
            asteria_renderer_show(
                self.renderer,
                p_texts.as_ptr(),
                p_notes.as_ptr(),
                p_texts.len(),
                list.focused,
                &caret,
                owner_hwnd,
                notify_hwnd,
            );
        }
    }

    pub fn hide_candidates(&mut self) {
        if !self.renderer.is_null() {
            // SAFETY: self.renderer は有効な AsteriaRenderer ハンドル。
            unsafe { asteria_renderer_hide(self.renderer) };
        }
    }
}

impl Drop for EngineBridge {
    fn drop(&mut self) {
        self.hide_candidates();
        if !self.renderer.is_null() {
            // SAFETY: open で取得した renderer を解放する。
            unsafe { asteria_renderer_free(self.renderer) };
            self.renderer = ptr::null_mut();
        }
        if !self.client.is_null() {
            // SAFETY: open で取得した client を解放する。
            unsafe { asteria_client_free(self.client) };
            self.client = ptr::null_mut();
        }
    }
}
