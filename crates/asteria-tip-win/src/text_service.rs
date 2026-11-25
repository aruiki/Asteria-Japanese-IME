//! TSF TextService COM 実装 (`text_service.cpp` / `notify_window.cpp` / `dllmain.cpp` 移植)。
#![allow(unsafe_code)]

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicI32, Ordering};

use asteria_client::ffi::{ASTERIA_MOD_ALT, ASTERIA_MOD_CTRL, ASTERIA_MOD_SHIFT};
use asteria_client::renderer_ffi::AsteriaRect;
use asteria_proto::ipc::CommandKind;
use windows::Win32::Foundation::{
    BOOL, CLASS_E_NOAGGREGATION, E_FAIL, E_INVALIDARG, FALSE, HINSTANCE, HMODULE, HWND, LPARAM,
    LRESULT, RECT, TRUE, WPARAM,
};
use windows::Win32::System::Com::{IClassFactory, IClassFactory_Impl};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, GetKeyboardState, ToUnicode, VK_CONTROL, VK_MENU, VK_SHIFT,
};
use windows::Win32::UI::TextServices::{
    IEnumTfDisplayAttributeInfo, ITfCompartmentEventSink, ITfCompartmentEventSink_Impl,
    ITfCompartmentMgr, ITfComposition, ITfCompositionSink, ITfCompositionSink_Impl, ITfContext,
    ITfContextComposition, ITfDisplayAttributeInfo, ITfDisplayAttributeProvider,
    ITfDisplayAttributeProvider_Impl, ITfDocumentMgr, ITfEditSession, ITfEditSession_Impl,
    ITfInputScope, ITfKeyEventSink, ITfKeyEventSink_Impl, ITfLangBarItem, ITfLangBarItemMgr,
    ITfRange, ITfReadOnlyProperty, ITfSource, ITfTextInputProcessor, ITfTextInputProcessorEx,
    ITfTextInputProcessorEx_Impl, ITfTextInputProcessor_Impl, ITfThreadMgr, ITfThreadMgrEventSink,
    ITfThreadMgrEventSink_Impl, GUID_COMPARTMENT_KEYBOARD_OPENCLOSE, GUID_PROP_ATTRIBUTE,
    GUID_PROP_INPUTSCOPE, TF_AE_NONE, TF_ANCHOR_END, TF_ANCHOR_START, TF_DEFAULT_SELECTION,
    TF_ES_ASYNCDONTCARE, TF_ES_READ, TF_ES_READWRITE, TF_ES_SYNC, TF_INVALID_COOKIE, TF_SELECTION,
    TF_SELECTIONSTYLE,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GetAncestor, GetWindowLongPtrW, KillTimer,
    RegisterClassExW, SetTimer, SetWindowLongPtrW, GA_ROOT, GWLP_USERDATA, GWLP_WNDPROC,
    HWND_MESSAGE, WM_APP, WM_TIMER, WNDCLASSEXW,
};
use windows_core::{
    implement, IUnknown, IUnknownImpl, Interface, Result, GUID, HSTRING, PCWSTR, VARIANT,
};

use crate::display_attribute::make_enum;
use crate::engine_bridge::{EngineBridge, EngineOutput};
use crate::mode_item::{is_keyboard_open, set_keyboard_open, ModeItem};
use crate::text::{
    attribute_guid_bytes, build_composition, focused_range, is_converting, is_open_close_key,
    is_private_input_scope, last_chars_utf16, MAX_LEFT_CONTEXT,
};

pub static G_DLL_REFS: AtomicI32 = AtomicI32::new(0);
pub static mut G_MODULE: HMODULE = HMODULE(std::ptr::null_mut());

pub fn dll_module() -> HMODULE {
    // SAFETY: DllMain (DLL_PROCESS_ATTACH) で設定された変更不能 DLL ハンドルを返す。
    unsafe { G_MODULE }
}

const WM_CANDIDATE_CLICKED: u32 = WM_APP;
const POLL_TIMER_ID: usize = 1;
const POLL_INTERVAL_MS: u32 = 30;
const NOTIFY_CLASS: &str = "AsteriaTipNotifyWindow";

fn key_down(vk: i32) -> bool {
    // SAFETY: GetKeyState はスレッドローカル入力状態のみを参照する。
    (unsafe { GetKeyState(vk) } as u16 & 0x8000) != 0
}

fn modifiers_mask() -> u32 {
    let mut m = 0u32;
    if key_down(VK_SHIFT.0 as i32) {
        m |= ASTERIA_MOD_SHIFT;
    }
    if key_down(VK_CONTROL.0 as i32) {
        m |= ASTERIA_MOD_CTRL;
    }
    if key_down(VK_MENU.0 as i32) {
        m |= ASTERIA_MOD_ALT;
    }
    m
}

fn key_text(wparam: WPARAM, lparam: LPARAM) -> String {
    let mut state = [0u8; 256];
    let mut buf = [0u16; 8];
    // SAFETY: スレッドのキーボード状態を読み、フラグ 0x4 (状態破壊しない) で Unicode 変換する。
    unsafe {
        if GetKeyboardState(&mut state).is_err() {
            return String::new();
        }
        let scan = ((lparam.0 >> 16) & 0xFF) as u32;
        let n = ToUnicode(wparam.0 as u32, scan, Some(&state), &mut buf, 0x4);
        if n > 0 {
            String::from_utf16_lossy(&buf[..n as usize])
        } else {
            String::new()
        }
    }
}

fn register_atom_by_bytes(mgr: &ITfThreadMgr, bytes: [u8; 16]) -> Option<u32> {
    let _ = mgr;
    let guid = GUID::from_u128(u128::from_be_bytes(bytes));
    // Display attribute atoms are resolved via ITfCategoryMgr::RegisterGUID in real host;
    // return hash token fallback if unregistered.
    // SAFETY: ITfCategoryMgr で GUID をアトム番号へ登録する。
    unsafe {
        let cat: ITfCategoryMgr_Helper = windows::Win32::System::Com::CoCreateInstance(
            &windows::Win32::UI::TextServices::CLSID_TF_CategoryMgr,
            None,
            windows::Win32::System::Com::CLSCTX_INPROC_SERVER,
        )
        .ok()?;
        cat.RegisterGUID(&guid).ok()
    }
}
use windows::Win32::UI::TextServices::ITfCategoryMgr as ITfCategoryMgr_Helper;

// ---- EditSession Wrapper ----
#[implement(ITfEditSession)]
struct ClosureEditSession<F>
where
    F: FnMut(u32) -> Result<()> + 'static,
{
    f: RefCell<Option<F>>,
}

impl<F> ClosureEditSession<F>
where
    F: FnMut(u32) -> Result<()> + 'static,
{
    fn into_session(f: F) -> ITfEditSession {
        Self {
            f: RefCell::new(Some(f)),
        }
        .into()
    }
}

impl<F> ITfEditSession_Impl for ClosureEditSession_Impl<F>
where
    F: FnMut(u32) -> Result<()> + 'static,
{
    fn DoEditSession(&self, ec: u32) -> Result<()> {
        if let Some(mut f) = self.f.borrow_mut().take() {
            f(ec)
        } else {
            Err(E_FAIL.into())
        }
    }
}

// ---- Notify Window ----
struct NotifyState {
    service_weak: std::ptr::NonNull<TextService_Impl>,
}

struct NotifyWindow {
    hwnd: HWND,
    state_ptr: *mut NotifyState,
}

impl NotifyWindow {
    unsafe extern "system" fn default_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        DefWindowProcW(hwnd, msg, wparam, lparam)
    }

    fn create(service: &TextService_Impl) -> Option<Self> {
        let class_w = HSTRING::from(NOTIFY_CLASS);
        // SAFETY: HWND_MESSAGE 配下のメッセージ専用隠しウィンドウを作成する。
        unsafe {
            let hinst: HINSTANCE = G_MODULE.into();
            let wc = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                lpfnWndProc: Some(Self::default_proc),
                hInstance: hinst,
                lpszClassName: PCWSTR(class_w.as_ptr()),
                ..Default::default()
            };
            let atom = RegisterClassExW(&wc);
            if atom == 0
                && windows::Win32::Foundation::GetLastError()
                    != windows::Win32::Foundation::ERROR_CLASS_ALREADY_EXISTS
            {
                return None;
            }
            let hwnd = CreateWindowExW(
                Default::default(),
                PCWSTR(class_w.as_ptr()),
                PCWSTR::null(),
                Default::default(),
                0,
                0,
                0,
                0,
                HWND_MESSAGE,
                None,
                hinst,
                None,
            )
            .ok()?;
            let ptr = Box::into_raw(Box::new(NotifyState {
                service_weak: std::ptr::NonNull::from(service),
            }));
            #[cfg(target_pointer_width = "64")]
            {
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, ptr as isize);
                SetWindowLongPtrW(
                    hwnd,
                    GWLP_WNDPROC,
                    (Self::wnd_proc as *const ()) as usize as isize,
                );
            }
            #[cfg(target_pointer_width = "32")]
            {
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, ptr as i32);
                SetWindowLongPtrW(
                    hwnd,
                    GWLP_WNDPROC,
                    (Self::wnd_proc as *const ()) as usize as i32,
                );
            }
            Some(Self {
                hwnd,
                state_ptr: ptr,
            })
        }
    }

    fn start_timer(&self) {
        // SAFETY: 有効な notify HWND に対する WM_TIMER 開始。
        unsafe {
            SetTimer(self.hwnd, POLL_TIMER_ID, POLL_INTERVAL_MS, None);
        }
    }

    fn stop_timer(&self) {
        // SAFETY: 有効な notify HWND に対する WM_TIMER 停止。
        unsafe {
            let _ = KillTimer(self.hwnd, POLL_TIMER_ID);
        }
    }

    unsafe extern "system" fn wnd_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut NotifyState;
        if !ptr.is_null() {
            let svc = (*ptr).service_weak.as_ref();
            if msg == WM_TIMER && wparam.0 == POLL_TIMER_ID {
                svc.on_poll_timer();
                return LRESULT(0);
            }
            if msg == WM_CANDIDATE_CLICKED {
                svc.on_candidate_clicked(wparam.0 as u32);
                return LRESULT(0);
            }
        }
        DefWindowProcW(hwnd, msg, wparam, lparam)
    }
}

impl Drop for NotifyWindow {
    fn drop(&mut self) {
        // SAFETY: GWLP_USERDATA をクリアして HWND とヒープを確保解放する。
        unsafe {
            if !self.hwnd.is_invalid() {
                SetWindowLongPtrW(self.hwnd, GWLP_USERDATA, 0);
                let _ = DestroyWindow(self.hwnd);
            }
            if !self.state_ptr.is_null() {
                drop(Box::from_raw(self.state_ptr));
            }
        }
    }
}

// ---- Inner Mutable State ----
struct Inner {
    thread_mgr: Option<ITfThreadMgr>,
    client_id: u32,
    engine: Option<EngineBridge>,
    composition: Option<ITfComposition>,
    candidate_context: Option<ITfContext>,
    poll_context: Option<ITfContext>,
    private_field: Option<bool>,
    thread_mgr_cookie: u32,
    open_close_cookie: u32,
    attr_atoms: [u32; 3],
    candidates_shown: bool,
    mode_item: Option<windows_core::ComObject<ModeItem>>,
    notify: Option<NotifyWindow>,
}

impl Default for Inner {
    fn default() -> Self {
        Self {
            thread_mgr: None,
            client_id: 0,
            engine: None,
            composition: None,
            candidate_context: None,
            poll_context: None,
            private_field: None,
            thread_mgr_cookie: TF_INVALID_COOKIE,
            open_close_cookie: TF_INVALID_COOKIE,
            attr_atoms: [0; 3],
            candidates_shown: false,
            mode_item: None,
            notify: None,
        }
    }
}

// ---- TextService COM Object ----
#[implement(
    ITfTextInputProcessor,
    ITfTextInputProcessorEx,
    ITfKeyEventSink,
    ITfCompositionSink,
    ITfDisplayAttributeProvider,
    ITfThreadMgrEventSink,
    ITfCompartmentEventSink
)]
pub struct TextService {
    inner: Rc<RefCell<Inner>>,
}

impl TextService {
    pub fn new() -> Self {
        G_DLL_REFS.fetch_add(1, Ordering::Relaxed);
        Self {
            inner: Rc::new(RefCell::new(Inner::default())),
        }
    }
}

impl Drop for TextService {
    fn drop(&mut self) {
        G_DLL_REFS.fetch_sub(1, Ordering::Relaxed);
    }
}

impl TextService_Impl {
    fn is_private_field(&self, ctx: &ITfContext) -> bool {
        if let Some(cached) = self.inner.borrow().private_field {
            return cached;
        }
        let client_id = self.inner.borrow().client_id;
        let is_priv = Rc::new(RefCell::new(false));
        let flag_ref = Rc::clone(&is_priv);
        let ctx_clone = ctx.clone();
        let session = ClosureEditSession::into_session(move |cookie| {
            // SAFETY: TSF sync read edit session 内で GUID_PROP_INPUTSCOPE を読み取る。
            unsafe {
                let mut sel = [TF_SELECTION::default()];
                let mut fetched = 0u32;
                let range: ITfRange = if ctx_clone
                    .GetSelection(cookie, TF_DEFAULT_SELECTION, &mut sel, &mut fetched)
                    .is_ok()
                    && fetched == 1
                {
                    match std::mem::ManuallyDrop::into_inner(sel[0].range.clone()) {
                        Some(r) => r,
                        None => return Ok(()),
                    }
                } else {
                    ctx_clone.GetStart(cookie)?
                };
                if let Ok(prop) = ctx_clone.GetProperty(&GUID_PROP_INPUTSCOPE) {
                    let ro: ITfReadOnlyProperty = prop.cast()?;
                    if let Ok(val) = ro.GetValue(cookie, &range) {
                        if let Ok(u) = IUnknown::try_from(&val) {
                            if let Ok(iscope) = u.cast::<ITfInputScope>() {
                                let mut ptr_scopes = std::ptr::null_mut();
                                let mut count = 0u32;
                                if iscope.GetInputScopes(&mut ptr_scopes, &mut count).is_ok()
                                    && !ptr_scopes.is_null()
                                {
                                    let slice = std::slice::from_raw_parts(
                                        ptr_scopes as *const i32,
                                        count as usize,
                                    );
                                    *flag_ref.borrow_mut() = is_private_input_scope(slice);
                                    windows::Win32::System::Com::CoTaskMemFree(Some(
                                        ptr_scopes.cast(),
                                    ));
                                }
                            }
                        }
                    }
                }
            }
            Ok(())
        });
        // SAFETY: 同期読み取り EditSession を実行する。
        let res = unsafe { ctx.RequestEditSession(client_id, &session, TF_ES_SYNC | TF_ES_READ) };
        let result = *is_priv.borrow();
        if res.is_ok() {
            self.inner.borrow_mut().private_field = Some(result);
        }
        result
    }

    fn send_left_context(&self, ctx: &ITfContext) {
        const MAX_FOLLOWING_CHARS: usize = 64;
        let client_id = self.inner.borrow().client_id;
        let captured_left = Rc::new(RefCell::new(String::new()));
        let captured_follow = Rc::new(RefCell::new(String::new()));
        let left_ref = Rc::clone(&captured_left);
        let follow_ref = Rc::clone(&captured_follow);
        let ctx_clone = ctx.clone();
        let session = ClosureEditSession::into_session(move |cookie| {
            // SAFETY: カーソル左側の最大 256 UTF-16 文字と右側 64 UTF-16 文字を読み取る。
            unsafe {
                let mut sel = [TF_SELECTION::default()];
                let mut fetched = 0u32;
                ctx_clone.GetSelection(cookie, TF_DEFAULT_SELECTION, &mut sel, &mut fetched)?;
                if fetched != 1 {
                    return Ok(());
                }
                if let Some(range) = std::mem::ManuallyDrop::into_inner(sel[0].range.clone()) {
                    if let Ok(follow_range) = range.Clone() {
                        if follow_range.Collapse(cookie, TF_ANCHOR_END).is_ok() {
                            let mut sh_f = 0i32;
                            let _ = follow_range.ShiftEnd(
                                cookie,
                                MAX_FOLLOWING_CHARS as i32,
                                &mut sh_f,
                                std::ptr::null(),
                            );
                            let mut fbuf = [0u16; MAX_FOLLOWING_CHARS];
                            let mut fn_read = 0u32;
                            if follow_range
                                .GetText(cookie, 0, &mut fbuf, &mut fn_read)
                                .is_ok()
                            {
                                *follow_ref.borrow_mut() =
                                    String::from_utf16_lossy(&fbuf[..fn_read as usize]);
                            }
                        }
                    }
                    range.Collapse(cookie, TF_ANCHOR_START)?;
                    let mut shifted = 0i32;
                    let _ = range.ShiftStart(
                        cookie,
                        -(MAX_LEFT_CONTEXT as i32),
                        &mut shifted,
                        std::ptr::null(),
                    );
                    let mut buf = [0u16; MAX_LEFT_CONTEXT];
                    let mut read_n = 0u32;
                    if range.GetText(cookie, 0, &mut buf, &mut read_n).is_ok() {
                        let tail = last_chars_utf16(&buf[..read_n as usize], MAX_LEFT_CONTEXT);
                        *left_ref.borrow_mut() = String::from_utf16_lossy(&tail);
                    }
                }
            }
            Ok(())
        });
        // SAFETY: 同期読み取り EditSession。
        let _ = unsafe { ctx.RequestEditSession(client_id, &session, TF_ES_SYNC | TF_ES_READ) };
        let left = captured_left.borrow().clone();
        let follow = captured_follow.borrow().clone();
        if let Some(eng) = self.inner.borrow_mut().engine.as_mut() {
            eng.set_context(&left, &follow);
        }
    }

    fn apply_output(&self, ctx: &ITfContext, out: EngineOutput) {
        let (client_id, atoms, notify_hwnd) = {
            let st = self.inner.borrow();
            (
                st.client_id,
                st.attr_atoms,
                st.notify
                    .as_ref()
                    .map_or(0u64, |n| n.hwnd.0 as usize as u64),
            )
        };
        let comp = build_composition(&out.preedit, out.cursor as usize);
        let converting = is_converting(&out.preedit);
        if converting {
            let mut st = self.inner.borrow_mut();
            st.poll_context = Some(ctx.clone());
            if let Some(n) = &st.notify {
                n.start_timer();
            }
        } else {
            let mut st = self.inner.borrow_mut();
            if let Some(n) = &st.notify {
                n.stop_timer();
            }
            st.poll_context = None;
        }

        let inner_ref = Rc::clone(&self.inner);
        let ctx_clone = ctx.clone();
        let out_clone = out.clone();
        let session = ClosureEditSession::into_session(move |cookie| {
            // SAFETY: TSF コンポジションを更新・確定し、候補ウィンドウ表示座標を計算する。
            unsafe {
                let committed_w: Vec<u16> = out_clone.committed.encode_utf16().collect();
                let mut current_comp = inner_ref.borrow().composition.clone();

                if !committed_w.is_empty() {
                    let range: ITfRange = if let Some(c) = current_comp.take() {
                        let r = c.GetRange()?;
                        let _ = c.EndComposition(cookie);
                        inner_ref.borrow_mut().composition = None;
                        r
                    } else {
                        let mut sel = [TF_SELECTION::default()];
                        let mut fetched = 0u32;
                        ctx_clone.GetSelection(
                            cookie,
                            TF_DEFAULT_SELECTION,
                            &mut sel,
                            &mut fetched,
                        )?;
                        if fetched != 1 {
                            return Ok(());
                        }
                        std::mem::ManuallyDrop::into_inner(sel[0].range.clone()).ok_or(E_FAIL)?
                    };
                    range.SetText(cookie, 0, &committed_w)?;
                    range.Collapse(cookie, windows::Win32::UI::TextServices::TF_ANCHOR_END)?;
                    let sel = TF_SELECTION {
                        range: std::mem::ManuallyDrop::new(Some(range)),
                        style: TF_SELECTIONSTYLE {
                            ase: TF_AE_NONE,
                            fInterimChar: FALSE,
                        },
                    };
                    ctx_clone.SetSelection(cookie, &[sel])?;
                }

                if comp.text_utf16.is_empty() {
                    if let Some(c) = current_comp.take() {
                        let r = c.GetRange()?;
                        r.SetText(cookie, 0, &[])?;
                        let _ = c.EndComposition(cookie);
                        inner_ref.borrow_mut().composition = None;
                    }
                    if let Some(eng) = inner_ref.borrow_mut().engine.as_mut() {
                        eng.hide_candidates();
                    }
                    inner_ref.borrow_mut().candidates_shown = false;
                    return Ok(());
                }

                // 進行中の preedit を書き込む。
                let comp_obj = match current_comp {
                    Some(c) => c,
                    None => {
                        let cc: ITfContextComposition = ctx_clone.cast()?;
                        let mut sel = [TF_SELECTION::default()];
                        let mut fetched = 0u32;
                        ctx_clone.GetSelection(
                            cookie,
                            TF_DEFAULT_SELECTION,
                            &mut sel,
                            &mut fetched,
                        )?;
                        let insert_range = if fetched == 1 {
                            std::mem::ManuallyDrop::into_inner(sel[0].range.clone())
                                .ok_or(E_FAIL)?
                        } else {
                            ctx_clone.GetStart(cookie)?
                        };
                        let sink: ITfCompositionSink = return_noop_sink(&inner_ref, &ctx_clone); // replaced below
                        let new_c = cc.StartComposition(cookie, &insert_range, &sink)?;
                        inner_ref.borrow_mut().composition = Some(new_c.clone());
                        new_c
                    }
                };
                let full_range = comp_obj.GetRange()?;
                full_range.SetText(cookie, 0, &comp.text_utf16)?;

                // 表示属性を各文節に付与する。
                if let Ok(prop) = ctx_clone.GetProperty(&GUID_PROP_ATTRIBUTE) {
                    for r in &comp.ranges {
                        let part = full_range.Clone()?;
                        part.Collapse(cookie, TF_ANCHOR_START)?;
                        let mut shifted = 0i32;
                        let _ = part.ShiftEnd(cookie, r.end, &mut shifted, std::ptr::null());
                        let _ = part.ShiftStart(cookie, r.begin, &mut shifted, std::ptr::null());
                        let atom = atoms[(r.attribute as usize).min(2)];
                        let v = VARIANT::from(atom as i32);
                        let _ = prop.SetValue(cookie, &part, &v);
                    }
                }

                // カーソルを composition の cursor_utf16 に合わせる。
                let caret_range = full_range.Clone()?;
                caret_range.Collapse(cookie, TF_ANCHOR_START)?;
                let mut shifted = 0i32;
                let _ = caret_range.ShiftStart(
                    cookie,
                    comp.cursor_utf16,
                    &mut shifted,
                    std::ptr::null(),
                );
                let _ =
                    caret_range.ShiftEnd(cookie, comp.cursor_utf16, &mut shifted, std::ptr::null());
                let sel = TF_SELECTION {
                    range: std::mem::ManuallyDrop::new(Some(caret_range)),
                    style: TF_SELECTIONSTYLE {
                        ase: TF_AE_NONE,
                        fInterimChar: FALSE,
                    },
                };
                let _ = ctx_clone.SetSelection(cookie, &[sel]);

                // 候補ウィンドウ位置計算＆送信。
                let mut showed = false;
                if out_clone.candidates.visible && !out_clone.candidates.texts.is_empty() {
                    let (fb, fe) = focused_range(&comp);
                    if let (Ok(part), Ok(view)) = (full_range.Clone(), ctx_clone.GetActiveView()) {
                        part.Collapse(cookie, TF_ANCHOR_START)?;
                        let mut sh = 0i32;
                        let _ = part.ShiftEnd(cookie, fe, &mut sh, std::ptr::null());
                        let _ = part.ShiftStart(cookie, fb, &mut sh, std::ptr::null());
                        let mut rect = RECT::default();
                        let mut clipped = FALSE;
                        if view
                            .GetTextExt(cookie, &part, &mut rect, &mut clipped)
                            .is_ok()
                        {
                            if let Ok(hwnd) = view.GetWnd() {
                                if !hwnd.is_invalid() {
                                    let owner = GetAncestor(hwnd, GA_ROOT).0 as usize as u64;
                                    let caret_r = AsteriaRect {
                                        left: rect.left,
                                        top: rect.top,
                                        right: rect.right,
                                        bottom: rect.bottom,
                                    };
                                    if let Some(eng) = inner_ref.borrow_mut().engine.as_mut() {
                                        eng.show_candidates(
                                            &out_clone.candidates,
                                            caret_r,
                                            owner,
                                            notify_hwnd,
                                        );
                                        showed = true;
                                    }
                                }
                            }
                        }
                    }
                }
                if !showed {
                    if let Some(eng) = inner_ref.borrow_mut().engine.as_mut() {
                        eng.hide_candidates();
                    }
                }
                let mut st = inner_ref.borrow_mut();
                st.candidates_shown = showed;
                st.candidate_context = if showed {
                    Some(ctx_clone.clone())
                } else {
                    None
                };
            }
            Ok(())
        });

        // SAFETY: EditSession を同期/非同期許可で発行する。
        let _ = unsafe {
            ctx.RequestEditSession(client_id, &session, TF_ES_ASYNCDONTCARE | TF_ES_READWRITE)
        };
    }

    fn on_poll_timer(&self) {
        let ctx_opt = self.inner.borrow().poll_context.clone();
        let Some(ctx) = ctx_opt else {
            if let Some(n) = &self.inner.borrow().notify {
                n.stop_timer();
            }
            return;
        };
        let out_opt = self
            .inner
            .borrow_mut()
            .engine
            .as_mut()
            .and_then(EngineBridge::poll_update);
        if let Some(out) = out_opt {
            if let Some(n) = &self.inner.borrow().notify {
                n.stop_timer();
            }
            self.inner.borrow_mut().poll_context = None;
            self.apply_output(&ctx, out);
        }
    }

    fn on_candidate_clicked(&self, index: u32) {
        let ctx_opt = self.inner.borrow().candidate_context.clone();
        let Some(ctx) = ctx_opt else {
            return;
        };
        let out_opt = self
            .inner
            .borrow_mut()
            .engine
            .as_mut()
            .and_then(|e| e.send_command(CommandKind::SelectCandidate as u32, index));
        if let Some(out) = out_opt {
            self.apply_output(&ctx, out);
        }
    }

    fn cleanup_deactivate(&self) {
        let mut st = self.inner.borrow_mut();
        if let Some(n) = st.notify.take() {
            n.stop_timer();
        }
        st.poll_context = None;
        st.candidate_context = None;
        st.composition = None;
        if let Some(mut eng) = st.engine.take() {
            eng.hide_candidates();
        }
        if let Some(tm) = st.thread_mgr.clone() {
            // SAFETY: AdviseSink で登録したシンクをすべて解除する。
            unsafe {
                if st.thread_mgr_cookie != TF_INVALID_COOKIE {
                    if let Ok(src) = tm.cast::<ITfSource>() {
                        let _ = src.UnadviseSink(st.thread_mgr_cookie);
                    }
                    st.thread_mgr_cookie = TF_INVALID_COOKIE;
                }
                if st.open_close_cookie != TF_INVALID_COOKIE {
                    if let Ok(cm) = tm.cast::<ITfCompartmentMgr>() {
                        if let Ok(comp) = cm.GetCompartment(&GUID_COMPARTMENT_KEYBOARD_OPENCLOSE) {
                            if let Ok(src) = comp.cast::<ITfSource>() {
                                let _ = src.UnadviseSink(st.open_close_cookie);
                            }
                        }
                    }
                    st.open_close_cookie = TF_INVALID_COOKIE;
                }
                if let Some(item) = st.mode_item.take() {
                    if let Ok(lbm) = tm.cast::<ITfLangBarItemMgr>() {
                        let lb: ITfLangBarItem = item.to_interface();
                        let _ = lbm.RemoveItem(&lb);
                    }
                }
            }
        }
        st.thread_mgr = None;
    }
}

#[implement(ITfCompositionSink)]
struct NoopCompSink;
impl ITfCompositionSink_Impl for NoopCompSink_Impl {
    fn OnCompositionTerminated(
        &self,
        _ecwrite: u32,
        _pcomposition: Option<&ITfComposition>,
    ) -> Result<()> {
        Ok(())
    }
}

fn return_noop_sink(_inner: &Rc<RefCell<Inner>>, _ctx: &ITfContext) -> ITfCompositionSink {
    NoopCompSink.into()
}

impl ITfTextInputProcessor_Impl for TextService_Impl {
    fn Activate(&self, ptim: Option<&ITfThreadMgr>, tid: u32) -> Result<()> {
        self.ActivateEx(ptim, tid, 0)
    }

    fn Deactivate(&self) -> Result<()> {
        self.cleanup_deactivate();
        Ok(())
    }
}

impl ITfTextInputProcessorEx_Impl for TextService_Impl {
    fn ActivateEx(&self, ptim: Option<&ITfThreadMgr>, tid: u32, _dwflags: u32) -> Result<()> {
        let tm = ptim.ok_or(E_INVALIDARG)?.clone();
        self.cleanup_deactivate();

        let atoms = [
            register_atom_by_bytes(&tm, attribute_guid_bytes(0)).unwrap_or(1),
            register_atom_by_bytes(&tm, attribute_guid_bytes(1)).unwrap_or(2),
            register_atom_by_bytes(&tm, attribute_guid_bytes(2)).unwrap_or(3),
        ];

        // SAFETY: HMODULE を渡しサーバー・レンダラーへ接続。
        let engine = EngineBridge::open(unsafe { G_MODULE });
        let notify = NotifyWindow::create(self);
        let mode_obj = windows_core::ComObject::new(ModeItem::new(tm.clone(), tid));

        // SAFETY: TSF シンクと言語バー項目を登録する。
        unsafe {
            if let Ok(lbm) = tm.cast::<ITfLangBarItemMgr>() {
                let lb: ITfLangBarItem = mode_obj.to_interface();
                let _ = lbm.AddItem(&lb);
            }
            if let Ok(src) = tm.cast::<ITfSource>() {
                let sink = self.as_interface::<ITfThreadMgrEventSink>();
                if let Ok(cookie) = src.AdviseSink(&ITfThreadMgrEventSink::IID, &*sink) {
                    self.inner.borrow_mut().thread_mgr_cookie = cookie;
                }
            }
            if let Ok(cm) = tm.cast::<ITfCompartmentMgr>() {
                if let Ok(comp) = cm.GetCompartment(&GUID_COMPARTMENT_KEYBOARD_OPENCLOSE) {
                    if let Ok(src) = comp.cast::<ITfSource>() {
                        let sink = self.as_interface::<ITfCompartmentEventSink>();
                        if let Ok(cookie) = src.AdviseSink(&ITfCompartmentEventSink::IID, &*sink) {
                            self.inner.borrow_mut().open_close_cookie = cookie;
                        }
                    }
                }
            }
        }

        let mut st = self.inner.borrow_mut();
        st.thread_mgr = Some(tm);
        st.client_id = tid;
        st.attr_atoms = atoms;
        st.engine = Some(engine);
        st.notify = notify;
        st.mode_item = Some(mode_obj);
        Ok(())
    }
}

impl ITfKeyEventSink_Impl for TextService_Impl {
    fn OnSetFocus(&self, fforeground: BOOL) -> Result<()> {
        if fforeground.as_bool() {
            self.inner.borrow_mut().private_field = None;
        }
        Ok(())
    }

    fn OnTestKeyDown(
        &self,
        pic: Option<&ITfContext>,
        wparam: WPARAM,
        _lparam: LPARAM,
    ) -> Result<BOOL> {
        let vk = wparam.0 as u32;
        if is_open_close_key(vk) {
            return Ok(TRUE);
        }
        let Some(ctx) = pic else {
            return Ok(FALSE);
        };
        let open = self
            .inner
            .borrow()
            .thread_mgr
            .as_ref()
            .is_some_and(|tm| is_keyboard_open(tm, self.inner.borrow().client_id));
        if !open || self.is_private_field(ctx) {
            return Ok(FALSE);
        }
        Ok(TRUE)
    }

    fn OnKeyDown(&self, pic: Option<&ITfContext>, wparam: WPARAM, lparam: LPARAM) -> Result<BOOL> {
        let vk = wparam.0 as u32;
        if is_open_close_key(vk) {
            let (tm_opt, cid) = {
                let st = self.inner.borrow();
                (st.thread_mgr.clone(), st.client_id)
            };
            if let Some(tm) = tm_opt {
                let open = is_keyboard_open(&tm, cid);
                set_keyboard_open(&tm, cid, !open);
            }
            return Ok(TRUE);
        }
        let Some(ctx) = pic else {
            return Ok(FALSE);
        };
        if self.is_private_field(ctx) {
            return Ok(FALSE);
        }
        if self.inner.borrow().composition.is_none() {
            self.send_left_context(ctx);
        }
        let text = key_text(wparam, lparam);
        let mods = modifiers_mask();
        let out_opt = self
            .inner
            .borrow_mut()
            .engine
            .as_mut()
            .and_then(|e| e.send_key(vk, &text, mods, false));
        match out_opt {
            Some(out) => {
                let consumed = out.consumed;
                if consumed {
                    self.apply_output(ctx, out);
                }
                Ok(BOOL::from(consumed))
            }
            None => Ok(FALSE),
        }
    }

    fn OnTestKeyUp(
        &self,
        _pic: Option<&ITfContext>,
        _wparam: WPARAM,
        _lparam: LPARAM,
    ) -> Result<BOOL> {
        Ok(FALSE)
    }

    fn OnKeyUp(&self, _pic: Option<&ITfContext>, _wparam: WPARAM, _lparam: LPARAM) -> Result<BOOL> {
        Ok(FALSE)
    }

    fn OnPreservedKey(&self, _pic: Option<&ITfContext>, _rguid: *const GUID) -> Result<BOOL> {
        Ok(FALSE)
    }
}

impl ITfCompositionSink_Impl for TextService_Impl {
    fn OnCompositionTerminated(
        &self,
        _ecwrite: u32,
        _pcomposition: Option<&ITfComposition>,
    ) -> Result<()> {
        let mut st = self.inner.borrow_mut();
        st.composition = None;
        if let Some(eng) = st.engine.as_mut() {
            eng.hide_candidates();
        }
        Ok(())
    }
}

impl ITfDisplayAttributeProvider_Impl for TextService_Impl {
    fn EnumDisplayAttributeInfo(&self) -> Result<IEnumTfDisplayAttributeInfo> {
        Ok(make_enum())
    }

    fn GetDisplayAttributeInfo(&self, guid: *const GUID) -> Result<ITfDisplayAttributeInfo> {
        crate::display_attribute::find_attribute_info(guid)
    }
}

impl ITfThreadMgrEventSink_Impl for TextService_Impl {
    fn OnInitDocumentMgr(&self, _pdim: Option<&ITfDocumentMgr>) -> Result<()> {
        Ok(())
    }
    fn OnUninitDocumentMgr(&self, _pdim: Option<&ITfDocumentMgr>) -> Result<()> {
        Ok(())
    }
    fn OnSetFocus(
        &self,
        _pdimfocus: Option<&ITfDocumentMgr>,
        _pdimprevfocus: Option<&ITfDocumentMgr>,
    ) -> Result<()> {
        self.inner.borrow_mut().private_field = None;
        Ok(())
    }
    fn OnPushContext(&self, _pic: Option<&ITfContext>) -> Result<()> {
        Ok(())
    }
    fn OnPopContext(&self, _pic: Option<&ITfContext>) -> Result<()> {
        Ok(())
    }
}

impl ITfCompartmentEventSink_Impl for TextService_Impl {
    fn OnChange(&self, rguid: *const GUID) -> Result<()> {
        if !rguid.is_null() {
            // SAFETY: GUID 比較のため非 NULL ポインタを読み取る。
            let g = unsafe { *rguid };
            if g == GUID_COMPARTMENT_KEYBOARD_OPENCLOSE {
                if let Some(item) = &self.inner.borrow().mode_item {
                    item.notify_update();
                }
            }
        }
        Ok(())
    }
}

// ---- IClassFactory ----
#[implement(IClassFactory)]
pub struct ClassFactory;

impl IClassFactory_Impl for ClassFactory_Impl {
    fn CreateInstance(
        &self,
        punkouter: Option<&IUnknown>,
        riid: *const GUID,
        ppvobject: *mut *mut core::ffi::c_void,
    ) -> Result<()> {
        if ppvobject.is_null() || riid.is_null() {
            return Err(E_INVALIDARG.into());
        }
        // SAFETY: NULL チェック済み出力ポインタ初期化。
        unsafe {
            *ppvobject = std::ptr::null_mut();
        }
        if punkouter.is_some() {
            return Err(CLASS_E_NOAGGREGATION.into());
        }
        let svc: IUnknown = TextService::new().into();
        // SAFETY: valid riid + non-null ppvobject.
        unsafe { svc.query(riid, ppvobject) }.ok()
    }

    fn LockServer(&self, flock: BOOL) -> Result<()> {
        if flock.as_bool() {
            G_DLL_REFS.fetch_add(1, Ordering::Relaxed);
        } else {
            G_DLL_REFS.fetch_sub(1, Ordering::Relaxed);
        }
        Ok(())
    }
}
