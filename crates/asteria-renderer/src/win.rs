//! Mozc の IPC サーバー側(ipc/win32_ipc.cc と同じ約束)を Windows の名前付きパイプで受ける。
//!
//! Win32 API の FFI はこのモジュールに集める。

#![allow(unsafe_code)]

use std::ffi::c_void;
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle, RawHandle};
use std::ptr;

use windows_sys::Win32::Foundation::{
    GetLastError, LocalFree, ERROR_MORE_DATA, ERROR_PIPE_CONNECTED, INVALID_HANDLE_VALUE,
};
use windows_sys::Win32::Security::Authorization::{
    ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
};
use windows_sys::Win32::Security::{PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES};
use windows_sys::Win32::Storage::FileSystem::{
    FlushFileBuffers, ReadFile, WriteFile, FILE_FLAG_FIRST_PIPE_INSTANCE, PIPE_ACCESS_DUPLEX,
};
use windows_sys::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, PIPE_READMODE_MESSAGE,
    PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_MESSAGE, PIPE_UNLIMITED_INSTANCES, PIPE_WAIT,
};

const BUFFER_SIZE: u32 = 64 * 1024;

fn wide(s: &str) -> Vec<u16> {
    std::ffi::OsStr::new(s)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

/// `LocalAlloc` で確保された領域を `LocalFree` で解放する。
struct LocalBox(*mut c_void);

impl Drop for LocalBox {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: self.0 は Win32 API が LocalAlloc で確保して返したポインタで、他で解放しない。
            unsafe { LocalFree(self.0) };
        }
    }
}

/// Mozc の IPC サーバー。1 つのパイプのインスタンスで、1 往復ずつ受ける(Mozc の renderer と同じ)。
pub struct MessageServer {
    pipe: OwnedHandle,
    _descriptor: LocalBox,
}

impl MessageServer {
    /// `name` でパイプを開く。同名のパイプが既にあれば失敗する(乗っ取りの防止)。
    pub fn bind(name: &str, sddl: &str) -> io::Result<Self> {
        let sddl = wide(sddl);
        let mut sd: PSECURITY_DESCRIPTOR = ptr::null_mut();
        // SAFETY: sddl は NUL 終端、sd は有効な出力先。
        let ok = unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                sddl.as_ptr(),
                SDDL_REVISION_1,
                &mut sd,
                ptr::null_mut(),
            )
        };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        let descriptor = LocalBox(sd);
        let attrs = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: descriptor.0,
            bInheritHandle: 0,
        };
        let name = wide(name);
        // SAFETY: name は NUL 終端、attrs と記述子は呼び出しの間生きている。
        let handle = unsafe {
            CreateNamedPipeW(
                name.as_ptr(),
                PIPE_ACCESS_DUPLEX | FILE_FLAG_FIRST_PIPE_INSTANCE,
                PIPE_TYPE_MESSAGE | PIPE_READMODE_MESSAGE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
                PIPE_UNLIMITED_INSTANCES,
                BUFFER_SIZE,
                BUFFER_SIZE,
                0,
                &attrs,
            )
        };
        if handle == INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: CreateNamedPipeW が成功したので handle は所有権を持つ有効なハンドル。
        let pipe = unsafe { OwnedHandle::from_raw_handle(handle as RawHandle) };
        Ok(Self {
            pipe,
            _descriptor: descriptor,
        })
    }

    /// 接続を待って 1 通を読み、`respond` の返事を書いて接続を切る。
    pub fn serve_one(&self, respond: impl FnOnce(&[u8]) -> Vec<u8>) -> io::Result<()> {
        let h = self.pipe.as_raw_handle();
        // SAFETY: h は同期モードで作ったパイプで、OVERLAPPED は使わない。
        let ok = unsafe { ConnectNamedPipe(h, ptr::null_mut()) };
        // SAFETY: 直前の Win32 呼び出しのエラー値を読むだけ。
        if ok == 0 && unsafe { GetLastError() } != ERROR_PIPE_CONNECTED {
            return Err(io::Error::last_os_error());
        }
        let result = read_message(h).and_then(|req| write_all(h, &respond(&req)));
        // SAFETY: h は接続済みのパイプ。返事を読み終えるのを待ってから切る。
        unsafe {
            FlushFileBuffers(h);
            DisconnectNamedPipe(h);
        }
        result
    }
}

/// メッセージを 1 つ読む(ERROR_MORE_DATA のあいだ続きを読む)。
fn read_message(h: RawHandle) -> io::Result<Vec<u8>> {
    let mut out = Vec::new();
    let mut buf = vec![0u8; BUFFER_SIZE as usize];
    loop {
        let mut n = 0u32;
        // SAFETY: buf は BUFFER_SIZE バイトあり、呼び出しの間生きている。
        let ok = unsafe { ReadFile(h, buf.as_mut_ptr(), BUFFER_SIZE, &mut n, ptr::null_mut()) };
        out.extend_from_slice(&buf[..n as usize]);
        if ok != 0 {
            return Ok(out);
        }
        // SAFETY: 直前の Win32 呼び出しのエラー値を読むだけ。
        if unsafe { GetLastError() } != ERROR_MORE_DATA {
            return Err(io::Error::last_os_error());
        }
    }
}

fn write_all(h: RawHandle, data: &[u8]) -> io::Result<()> {
    let mut n = 0u32;
    // SAFETY: data は呼び出しの間生きている。空でも 1 つのメッセージとして送る(Mozc は空の返事を待つ)。
    let ok = unsafe { WriteFile(h, data.as_ptr(), data.len() as u32, &mut n, ptr::null_mut()) };
    if ok == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::io::{Read, Write};

    #[test]
    fn serves_one_round_trip() {
        let name = format!(r"\\.\pipe\asteria-renderer-test-{}", std::process::id());
        let server = MessageServer::bind(&name, "D:(A;;GA;;;WD)").expect("bind");
        let client_name = name.clone();
        let client = std::thread::spawn(move || {
            let mut f = loop {
                match std::fs::OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open(&client_name)
                {
                    Ok(f) => break f,
                    Err(_) => std::thread::sleep(std::time::Duration::from_millis(5)),
                }
            };
            f.write_all(b"hello").expect("write");
            let mut resp = Vec::new();
            let _ = f.read_to_end(&mut resp);
            resp
        });
        let mut got = Vec::new();
        server
            .serve_one(|req| {
                got = req.to_vec();
                b"ok".to_vec()
            })
            .expect("serve");
        assert_eq!(got, b"hello");
        assert_eq!(client.join().expect("join"), b"ok");
    }
}
