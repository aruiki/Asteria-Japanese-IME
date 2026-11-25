//! フロントエンド(C++ などの TSF TIP)から呼ぶ C ABI(docs/SPEC.md 10章、4.1、4.2)。
//!
//! 宣言は `include/asteria_client.h`。文字列はすべて NUL 終端の UTF-8。関数は状態コード
//! (`ASTERIA_OK` など)を返し、結果は出力引数に書く。`AsteriaOutput` は呼び出し側が
//! `asteria_output_free` で解放する。パニックは C 側へ漏らさない(REQ-13-2)。
//!
//! 接続は [`Managed`] が受け持つ。サーバーがなければ起動し、切れたら間隔を空けて
//! つなぎ直す(REQ-4-1、REQ-4-2)。つながっていない間はキーをアプリへ渡させる。

#![allow(unsafe_code)]

use std::ffi::{c_char, CStr, CString};
use std::panic::{catch_unwind, AssertUnwindSafe};

use asteria_proto::ipc::{self, request};

use crate::managed::{Connector, Managed, Reply, SystemConnector};

pub const ASTERIA_OK: i32 = 0;
/// キーを処理できなかった(未接続、または 200ms 以内に応答がない)。フロントエンドはキーを
/// アプリへ渡し、表示は保つ(4.2、REQ-4-2)。
pub const ASTERIA_PASS_THROUGH: i32 = 1;
pub const ASTERIA_ERR_ARGUMENT: i32 = -1;
/// 内部の不具合(パニック)。
pub const ASTERIA_ERR_INTERNAL: i32 = -3;

pub const ASTERIA_MOD_SHIFT: u32 = 1;
pub const ASTERIA_MOD_CTRL: u32 = 2;
pub const ASTERIA_MOD_ALT: u32 = 4;
pub const ASTERIA_MOD_META: u32 = 8;

/// C から見える接続。
pub struct AsteriaClient {
    managed: Managed<Box<dyn Connector>>,
}

impl AsteriaClient {
    /// 接続のしかたを指定して作る(テストや、別のトランスポートを使うとき)。
    pub fn with_connector(connector: Box<dyn Connector>) -> Self {
        Self {
            managed: Managed::new(connector),
        }
    }
}

/// C から見える表示の内容。文字列は NUL 終端にして持つ。
#[derive(Debug, Default)]
pub struct AsteriaOutput {
    consumed: bool,
    preedit: Vec<(CString, u32)>,
    cursor: u32,
    committed: CString,
    candidates: Vec<CString>,
    annotations: Vec<CString>,
    focused: u32,
    window_visible: bool,
    input_mode: u32,
}

/// NUL を含む文字列は NUL を除いて C の文字列にする。
fn c_string(s: String) -> CString {
    CString::new(s).unwrap_or_else(|e| {
        let bytes: Vec<u8> = e.into_vec().into_iter().filter(|&b| b != 0).collect();
        CString::new(bytes).unwrap_or_default()
    })
}

impl From<ipc::Output> for AsteriaOutput {
    fn from(o: ipc::Output) -> Self {
        let window = o.candidate_window.unwrap_or_default();
        Self {
            consumed: o.consumed,
            cursor: o.cursor,
            input_mode: o.input_mode as u32,
            preedit: o
                .preedit
                .into_iter()
                .map(|p| (c_string(p.text), p.attribute as u32))
                .collect(),
            committed: c_string(o.committed_text),
            focused: window.focused_index,
            window_visible: window.visible,
            annotations: window
                .candidates
                .iter()
                .map(|c| c_string(c.annotation.clone()))
                .collect(),
            candidates: window
                .candidates
                .into_iter()
                .map(|c| c_string(c.text))
                .collect(),
        }
    }
}

/// パニックを状態コードに変える。
pub(crate) fn guard(f: impl FnOnce() -> i32) -> i32 {
    catch_unwind(AssertUnwindSafe(f)).unwrap_or(ASTERIA_ERR_INTERNAL)
}

/// # Safety
/// `p` は NULL か、NUL 終端の文字列を指すこと。
pub(crate) unsafe fn str_arg<'a>(p: *const c_char) -> Option<&'a str> {
    if p.is_null() {
        return None;
    }
    // SAFETY: 呼び出し側が NUL 終端の文字列であることを保証する。
    unsafe { CStr::from_ptr(p) }.to_str().ok()
}

/// 応答を出力引数に書く。
///
/// # Safety
/// `out` は NULL でなく、書き込める位置を指すこと。
unsafe fn write_reply(reply: Reply, out: *mut *mut AsteriaOutput) -> i32 {
    match reply {
        Reply::Output(o) => {
            // SAFETY: 関数の前提どおり。
            unsafe { *out = Box::into_raw(Box::new(AsteriaOutput::from(o))) };
            ASTERIA_OK
        }
        Reply::PassThrough => ASTERIA_PASS_THROUGH,
    }
}

/// 接続を作る。実際の接続は最初の要求のときに行う。`address` は UNIX ドメインソケットの
/// パスか名前付きパイプ名で、NULL なら既定の場所(4.2)。`server` はサーバーの実行ファイルで、
/// つながらないときに起動する(REQ-4-1)。NULL なら起動しない。文字列が不正なら NULL を返す。
///
/// # Safety
/// `address` と `server` は NULL か NUL 終端の UTF-8 文字列であること。
#[no_mangle]
pub unsafe extern "C" fn asteria_client_open(
    address: *const c_char,
    server: *const c_char,
) -> *mut AsteriaClient {
    catch_unwind(|| {
        // SAFETY: 関数の前提どおり。
        let (addr, srv) = unsafe { (str_arg(address), str_arg(server)) };
        if (!address.is_null() && addr.is_none()) || (!server.is_null() && srv.is_none()) {
            return std::ptr::null_mut();
        }
        let connector = SystemConnector {
            address: addr.map(str::to_owned),
            server: srv.map(std::path::PathBuf::from),
        };
        Box::into_raw(Box::new(AsteriaClient::with_connector(Box::new(connector))))
    })
    .unwrap_or(std::ptr::null_mut())
}

/// 接続を閉じて解放する。NULL なら何もしない。
///
/// # Safety
/// `client` は NULL か、`asteria_client_open` が返してまだ解放していないものであること。
#[no_mangle]
pub unsafe extern "C" fn asteria_client_free(client: *mut AsteriaClient) {
    if !client.is_null() {
        // SAFETY: 関数の前提どおり、Box::into_raw で作ったポインタで、ここで1回だけ解放する。
        let _ = catch_unwind(AssertUnwindSafe(|| drop(unsafe { Box::from_raw(client) })));
    }
}

/// サーバーにつながっているか(1 ならつながっている)。
///
/// # Safety
/// `client` は NULL か有効な接続であること。
#[no_mangle]
pub unsafe extern "C" fn asteria_client_connected(client: *const AsteriaClient) -> i32 {
    // SAFETY: 関数の前提どおり。
    i32::from(unsafe { client.as_ref() }.is_some_and(|c| c.managed.is_connected()))
}

/// セッションを作り、手元のセッション ID を `*session_id` に書く。サーバー側のセッションは
/// 最初の要求のときに作り、サーバーが再起動したら作り直す。`input_scope` は IPC の InputScope の値。
///
/// # Safety
/// `client` は有効な接続、`app_id` は NUL 終端の UTF-8、`session_id` は書き込める位置を指すこと。
#[no_mangle]
pub unsafe extern "C" fn asteria_create_session(
    client: *mut AsteriaClient,
    app_id: *const c_char,
    input_scope: u32,
    session_id: *mut u64,
) -> i32 {
    guard(|| {
        // SAFETY: 関数の前提どおり。NULL は下で弾く。
        let (Some(c), Some(app_id)) = (unsafe { client.as_mut() }, unsafe { str_arg(app_id) })
        else {
            return ASTERIA_ERR_ARGUMENT;
        };
        if session_id.is_null() {
            return ASTERIA_ERR_ARGUMENT;
        }
        let scope = ipc::InputScope::try_from(input_scope as i32).unwrap_or_default();
        let id = c.managed.create_session(app_id, scope);
        // SAFETY: NULL でないことを確かめた、書き込める位置。
        unsafe { *session_id = id };
        ASTERIA_OK
    })
}

/// セッションを消す。
///
/// # Safety
/// `client` は有効な接続であること。
#[no_mangle]
pub unsafe extern "C" fn asteria_delete_session(
    client: *mut AsteriaClient,
    session_id: u64,
) -> i32 {
    guard(|| {
        // SAFETY: 関数の前提どおり。
        let Some(c) = (unsafe { client.as_mut() }) else {
            return ASTERIA_ERR_ARGUMENT;
        };
        c.managed.delete_session(session_id);
        ASTERIA_OK
    })
}

/// 左文脈(確定済みのテキスト、最大 256 文字)を伝える(REQ-10-2)。
///
/// # Safety
/// `client` は有効な接続、`left_context` は NUL 終端の UTF-8 であること。
#[no_mangle]
pub unsafe extern "C" fn asteria_set_context(
    client: *mut AsteriaClient,
    session_id: u64,
    left_context: *const c_char,
) -> i32 {
    guard(|| {
        // SAFETY: 関数の前提どおり。
        let (Some(c), Some(ctx)) = (unsafe { client.as_mut() }, unsafe { str_arg(left_context) })
        else {
            return ASTERIA_ERR_ARGUMENT;
        };
        c.managed.set_context(session_id, ctx);
        ASTERIA_OK
    })
}

/// 左文脈とカーソル後方文脈を伝える(REQ-10-2)。
///
/// # Safety
/// `client` は有効な接続、`left_context` と `following_text` は NUL 終端の UTF-8 であること。
#[no_mangle]
pub unsafe extern "C" fn asteria_set_context_full(
    client: *mut AsteriaClient,
    session_id: u64,
    left_context: *const c_char,
    following_text: *const c_char,
) -> i32 {
    guard(|| {
        // SAFETY: 関数の前提どおり。
        let (Some(c), Some(left), Some(follow)) = (
            unsafe { client.as_mut() },
            unsafe { str_arg(left_context) },
            unsafe { str_arg(following_text) },
        ) else {
            return ASTERIA_ERR_ARGUMENT;
        };
        c.managed.set_full_context(session_id, left, follow);
        ASTERIA_OK
    })
}

/// サーバーの設定とユーザー辞書の再読み込みを要求する。
///
/// # Safety
/// `client` は有効な接続であること。
#[no_mangle]
pub unsafe extern "C" fn asteria_reload_server(client: *mut AsteriaClient) -> i32 {
    guard(|| {
        // SAFETY: 関数の前提どおり。
        let Some(c) = (unsafe { client.as_mut() }) else {
            return ASTERIA_ERR_ARGUMENT;
        };
        let _ = c.managed.raw_request(request::Body::Reload(ipc::Reload {}));
        ASTERIA_OK
    })
}

/// キーを送る。`ASTERIA_OK` なら `*out` に表示の内容を書く。`ASTERIA_PASS_THROUGH` なら書かない
/// (キーはアプリへ渡し、表示は保つ)。`modifiers` は `ASTERIA_MOD_*` の組み合わせ。
///
/// # Safety
/// `client` は有効な接続、`text` は NUL 終端の UTF-8(文字がなければ空文字列)、
/// `out` は書き込める位置を指すこと。
#[no_mangle]
pub unsafe extern "C" fn asteria_send_key(
    client: *mut AsteriaClient,
    session_id: u64,
    virtual_key: u32,
    text: *const c_char,
    modifiers: u32,
    key_up: i32,
    out: *mut *mut AsteriaOutput,
) -> i32 {
    guard(|| {
        // SAFETY: 関数の前提どおり。
        let (Some(c), Some(text)) = (unsafe { client.as_mut() }, unsafe { str_arg(text) }) else {
            return ASTERIA_ERR_ARGUMENT;
        };
        if out.is_null() {
            return ASTERIA_ERR_ARGUMENT;
        }
        let key = ipc::SendKey {
            session_id,
            virtual_key,
            text: text.into(),
            modifiers: Some(ipc::Modifiers {
                shift: modifiers & ASTERIA_MOD_SHIFT != 0,
                ctrl: modifiers & ASTERIA_MOD_CTRL != 0,
                alt: modifiers & ASTERIA_MOD_ALT != 0,
                meta: modifiers & ASTERIA_MOD_META != 0,
            }),
            key_up: key_up != 0,
        };
        let reply = c.managed.send_key(session_id, key);
        // SAFETY: out は NULL でないことを確かめた。
        unsafe { write_reply(reply, out) }
    })
}

/// 遅れて終わった処理(LM のリランク)で表示が変わったかを尋ねる(docs/adr/0008)。
/// 変わっていれば `ASTERIA_OK` で `*out` に新しい表示を書く。変わっていない・つながっていない
/// ときは `ASTERIA_PASS_THROUGH`(接続もサーバーの起動もしない)。
///
/// # Safety
/// `client` は有効な接続、`out` は書き込める位置を指すこと。
#[no_mangle]
pub unsafe extern "C" fn asteria_poll_update(
    client: *mut AsteriaClient,
    session_id: u64,
    out: *mut *mut AsteriaOutput,
) -> i32 {
    guard(|| {
        // SAFETY: 関数の前提どおり。
        let Some(c) = (unsafe { client.as_mut() }) else {
            return ASTERIA_ERR_ARGUMENT;
        };
        if out.is_null() {
            return ASTERIA_ERR_ARGUMENT;
        }
        let reply = c.managed.poll_update(session_id);
        // SAFETY: out は NULL でないことを確かめた。
        unsafe { write_reply(reply, out) }
    })
}

/// コマンド(確定・取消など、IPC の CommandKind の値)を送る。戻り値と `*out` は
/// `asteria_send_key` と同じ。
///
/// # Safety
/// `client` は有効な接続、`out` は書き込める位置を指すこと。
#[no_mangle]
pub unsafe extern "C" fn asteria_send_command(
    client: *mut AsteriaClient,
    session_id: u64,
    kind: u32,
    argument: u32,
    out: *mut *mut AsteriaOutput,
) -> i32 {
    guard(|| {
        // SAFETY: 関数の前提どおり。
        let Some(c) = (unsafe { client.as_mut() }) else {
            return ASTERIA_ERR_ARGUMENT;
        };
        if out.is_null() {
            return ASTERIA_ERR_ARGUMENT;
        }
        let kind = ipc::CommandKind::try_from(kind as i32).unwrap_or_default();
        let reply = c.managed.send_command(session_id, kind, argument);
        // SAFETY: out は NULL でないことを確かめた。
        unsafe { write_reply(reply, out) }
    })
}

/// 表示の内容を解放する。NULL なら何もしない。
///
/// # Safety
/// `out` は NULL か、この API が返してまだ解放していないものであること。
#[no_mangle]
pub unsafe extern "C" fn asteria_output_free(out: *mut AsteriaOutput) {
    if !out.is_null() {
        // SAFETY: 関数の前提どおり、Box::into_raw で作ったポインタで、ここで1回だけ解放する。
        drop(unsafe { Box::from_raw(out) });
    }
}

/// # Safety
/// `out` は NULL か有効な `AsteriaOutput` を指すこと。
unsafe fn output_ref<'a>(out: *const AsteriaOutput) -> Option<&'a AsteriaOutput> {
    // SAFETY: 関数の前提どおり。
    unsafe { out.as_ref() }
}

/// キーを消費したか(0 ならアプリへ渡す)。
///
/// # Safety
/// `out` は NULL か有効な `AsteriaOutput` を指すこと。以下のアクセサも同じ。
#[no_mangle]
pub unsafe extern "C" fn asteria_output_consumed(out: *const AsteriaOutput) -> i32 {
    // SAFETY: 関数の前提どおり。
    i32::from(unsafe { output_ref(out) }.is_some_and(|o| o.consumed))
}

/// プリエディットの区間の数。
///
/// # Safety
/// `out` は NULL か有効な `AsteriaOutput` を指すこと。
#[no_mangle]
pub unsafe extern "C" fn asteria_output_preedit_count(out: *const AsteriaOutput) -> usize {
    // SAFETY: 関数の前提どおり。
    unsafe { output_ref(out) }.map_or(0, |o| o.preedit.len())
}

/// `i` 番目の区間の文字列。範囲外なら NULL。文字列は `out` を解放するまで有効。
///
/// # Safety
/// `out` は NULL か有効な `AsteriaOutput` を指すこと。
#[no_mangle]
pub unsafe extern "C" fn asteria_output_preedit_text(
    out: *const AsteriaOutput,
    i: usize,
) -> *const c_char {
    // SAFETY: 関数の前提どおり。
    unsafe { output_ref(out) }
        .and_then(|o| o.preedit.get(i))
        .map_or(std::ptr::null(), |(s, _)| s.as_ptr())
}

/// `i` 番目の区間の属性(IPC の SegmentAttribute の値)。範囲外なら 0。
///
/// # Safety
/// `out` は NULL か有効な `AsteriaOutput` を指すこと。
#[no_mangle]
pub unsafe extern "C" fn asteria_output_preedit_attribute(
    out: *const AsteriaOutput,
    i: usize,
) -> u32 {
    // SAFETY: 関数の前提どおり。
    unsafe { output_ref(out) }
        .and_then(|o| o.preedit.get(i))
        .map_or(0, |(_, a)| *a)
}

/// プリエディット内のカーソル位置(文字数)。
///
/// # Safety
/// `out` は NULL か有効な `AsteriaOutput` を指すこと。
#[no_mangle]
pub unsafe extern "C" fn asteria_output_cursor(out: *const AsteriaOutput) -> u32 {
    // SAFETY: 関数の前提どおり。
    unsafe { output_ref(out) }.map_or(0, |o| o.cursor)
}

/// 確定した文字列(なければ空文字列)。`out` が NULL なら NULL。
///
/// # Safety
/// `out` は NULL か有効な `AsteriaOutput` を指すこと。
#[no_mangle]
pub unsafe extern "C" fn asteria_output_committed(out: *const AsteriaOutput) -> *const c_char {
    // SAFETY: 関数の前提どおり。
    unsafe { output_ref(out) }.map_or(std::ptr::null(), |o| o.committed.as_ptr())
}

/// 候補ウィンドウを表示するか。
///
/// # Safety
/// `out` は NULL か有効な `AsteriaOutput` を指すこと。
#[no_mangle]
pub unsafe extern "C" fn asteria_output_candidate_visible(out: *const AsteriaOutput) -> i32 {
    // SAFETY: 関数の前提どおり。
    i32::from(unsafe { output_ref(out) }.is_some_and(|o| o.window_visible))
}

/// 候補の数。
///
/// # Safety
/// `out` は NULL か有効な `AsteriaOutput` を指すこと。
#[no_mangle]
pub unsafe extern "C" fn asteria_output_candidate_count(out: *const AsteriaOutput) -> usize {
    // SAFETY: 関数の前提どおり。
    unsafe { output_ref(out) }.map_or(0, |o| o.candidates.len())
}

/// `i` 番目の候補。範囲外なら NULL。文字列は `out` を解放するまで有効。
///
/// # Safety
/// `out` は NULL か有効な `AsteriaOutput` を指すこと。
#[no_mangle]
pub unsafe extern "C" fn asteria_output_candidate(
    out: *const AsteriaOutput,
    i: usize,
) -> *const c_char {
    // SAFETY: 関数の前提どおり。
    unsafe { output_ref(out) }
        .and_then(|o| o.candidates.get(i))
        .map_or(std::ptr::null(), |s| s.as_ptr())
}

/// `i` 番目の候補の注釈(5.6)。注釈がなければ空文字列、範囲外なら NULL。文字列は `out` を
/// 解放するまで有効。
///
/// # Safety
/// `out` は NULL か有効な `AsteriaOutput` を指すこと。
#[no_mangle]
pub unsafe extern "C" fn asteria_output_candidate_annotation(
    out: *const AsteriaOutput,
    i: usize,
) -> *const c_char {
    // SAFETY: 関数の前提どおり。
    unsafe { output_ref(out) }
        .and_then(|o| o.annotations.get(i))
        .map_or(std::ptr::null(), |s| s.as_ptr())
}

/// 選択中の候補の位置。
///
/// # Safety
/// `out` は NULL か有効な `AsteriaOutput` を指すこと。
#[no_mangle]
pub unsafe extern "C" fn asteria_output_candidate_focused(out: *const AsteriaOutput) -> u32 {
    // SAFETY: 関数の前提どおり。
    unsafe { output_ref(out) }.map_or(0, |o| o.focused)
}

/// 入力モード(IPC の InputMode の値)。
///
/// # Safety
/// `out` は NULL か有効な `AsteriaOutput` を指すこと。
#[no_mangle]
pub unsafe extern "C" fn asteria_output_input_mode(out: *const AsteriaOutput) -> u32 {
    // SAFETY: 関数の前提どおり。
    unsafe { output_ref(out) }.map_or(0, |o| o.input_mode)
}
