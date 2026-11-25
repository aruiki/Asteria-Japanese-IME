//! 追加 AI モデルのバックグラウンドダウンローダ (`.part` レジューム + WinHTTP + SHA-256 検証)。
#![cfg_attr(not(windows), deny(unsafe_code))]
#![cfg_attr(windows, allow(unsafe_code))]

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;

use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadOutcome {
    Done,
    Cancelled,
    Failed,
}

pub trait DownloadSink: Send + Sync + 'static {
    fn on_progress(&self, id: &str, received: u64, total: u64);
    fn on_done(&self, id: &str, outcome: DownloadOutcome, message: &str);
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedUrl {
    pub https: bool,
    pub host: String,
    pub port: u16,
    pub path: String,
}

pub fn parse_url(url: &str) -> Option<ParsedUrl> {
    let (https, rest) = if let Some(r) = url.strip_prefix("https://") {
        (true, r)
    } else {
        (false, url.strip_prefix("http://")?)
    };
    let (host_port, path) = match rest.find('/') {
        Some(idx) => (&rest[..idx], &rest[idx..]),
        None => (rest, "/"),
    };
    let (host, port) = match host_port.rsplit_once(':') {
        Some((h, p)) => (h, p.parse().ok()?),
        None => (host_port, if https { 443 } else { 80 }),
    };
    if host.is_empty() {
        return None;
    }
    Some(ParsedUrl {
        https,
        host: host.to_owned(),
        port,
        path: path.to_owned(),
    })
}

pub struct ModelDownloader {
    cancel: Arc<AtomicBool>,
    running: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl ModelDownloader {
    pub fn start<S: DownloadSink>(
        id: String,
        url: String,
        dest: PathBuf,
        expected_size: u64,
        expected_sha256: String,
        sink: Arc<S>,
    ) -> Self {
        let cancel = Arc::new(AtomicBool::new(false));
        let running = Arc::new(AtomicBool::new(true));
        let cancel_bg = Arc::clone(&cancel);
        let running_bg = Arc::clone(&running);

        let handle = std::thread::spawn(move || {
            let (outcome, msg) = run_download(
                &id,
                &url,
                &dest,
                expected_size,
                &expected_sha256,
                &cancel_bg,
                &*sink,
            );
            running_bg.store(false, Ordering::SeqCst);
            sink.on_done(&id, outcome, &msg);
        });

        Self {
            cancel,
            running,
            handle: Some(handle),
        }
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }
}

impl Drop for ModelDownloader {
    fn drop(&mut self) {
        self.cancel();
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

pub fn part_path(dest: &Path) -> PathBuf {
    let mut os = dest.as_os_str().to_os_string();
    os.push(".part");
    PathBuf::from(os)
}

pub fn verify_sha256_file(path: &Path, expected_hex: &str) -> std::io::Result<bool> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    let hex = format!("{:x}", hasher.finalize());
    Ok(hex.eq_ignore_ascii_case(expected_hex))
}

#[cfg(not(windows))]
fn run_download<S: DownloadSink>(
    _id: &str,
    _url: &str,
    _dest: &PathBuf,
    _expected_size: u64,
    _expected_sha256: &str,
    _cancel: &AtomicBool,
    _sink: &S,
) -> (DownloadOutcome, String) {
    (
        DownloadOutcome::Failed,
        "モデルのダウンロードは Windows 専用です".to_owned(),
    )
}

#[cfg(windows)]
fn run_download<S: DownloadSink>(
    id: &str,
    url: &str,
    dest: &Path,
    expected_size: u64,
    expected_sha256: &str,
    cancel: &AtomicBool,
    sink: &S,
) -> (DownloadOutcome, String) {
    use std::fs::OpenOptions;
    use std::io::Write;
    use std::time::{Duration, Instant};
    use windows::Win32::Networking::WinHttp::{
        WinHttpCloseHandle, WinHttpConnect, WinHttpOpen, WinHttpOpenRequest, WinHttpQueryHeaders,
        WinHttpReadData, WinHttpReceiveResponse, WinHttpSendRequest,
        WINHTTP_ACCESS_TYPE_DEFAULT_PROXY, WINHTTP_FLAG_SECURE, WINHTTP_QUERY_STATUS_CODE,
    };
    use windows_core::{HSTRING, PCWSTR};

    struct WinHttpHandle(*mut core::ffi::c_void);
    impl Drop for WinHttpHandle {
        fn drop(&mut self) {
            if !self.0.is_null() {
                // SAFETY: WinHttpHandle は WinHttpOpen/Connect/OpenRequest で開いた有効ハンドルを所有する。
                unsafe {
                    let _ = WinHttpCloseHandle(self.0);
                }
            }
        }
    }

    let Some(parsed) = parse_url(url) else {
        return (DownloadOutcome::Failed, "URL 形式が不正です".to_owned());
    };
    if let Some(parent) = dest.parent() {
        if let Err(e) = fs::create_dir_all(parent) {
            return (DownloadOutcome::Failed, format!("フォルダ作成に失敗: {e}"));
        }
    }
    let part = part_path(dest);
    let existing = fs::metadata(&part).map(|m| m.len()).unwrap_or(0);
    let resume_from = if existing < expected_size {
        existing
    } else {
        0
    };

    let agent_w = HSTRING::from("Asteria-Config/1.0");
    let host_w = HSTRING::from(parsed.host.as_str());
    let verb_w = HSTRING::from("GET");
    let path_w = HSTRING::from(parsed.path.as_str());

    // SAFETY: WinHTTP セッション・接続・リクエストを順番に構築し、RAII WinHttpHandle で解放する。
    let (h_req, status_code) = unsafe {
        let sess = WinHttpOpen(
            PCWSTR(agent_w.as_ptr()),
            WINHTTP_ACCESS_TYPE_DEFAULT_PROXY,
            PCWSTR::null(),
            PCWSTR::null(),
            0,
        );
        if sess.is_null() {
            return (
                DownloadOutcome::Failed,
                "WinHttpOpen に失敗しました".to_owned(),
            );
        }
        let _sess = WinHttpHandle(sess);

        let conn = WinHttpConnect(sess, PCWSTR(host_w.as_ptr()), parsed.port, 0);
        if conn.is_null() {
            return (
                DownloadOutcome::Failed,
                "サーバーへ接続できません".to_owned(),
            );
        }
        let _conn = WinHttpHandle(conn);

        let flags = if parsed.https {
            WINHTTP_FLAG_SECURE
        } else {
            Default::default()
        };
        let req = WinHttpOpenRequest(
            conn,
            PCWSTR(verb_w.as_ptr()),
            PCWSTR(path_w.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            std::ptr::null(),
            flags,
        );
        if req.is_null() {
            return (
                DownloadOutcome::Failed,
                "HTTP 要求を作成できません".to_owned(),
            );
        }
        let h_req = WinHttpHandle(req);

        let extra_headers: Option<Vec<u16>> = if resume_from > 0 {
            Some(
                format!("Range: bytes={resume_from}-\r\n")
                    .encode_utf16()
                    .collect(),
            )
        } else {
            None
        };
        if WinHttpSendRequest(req, extra_headers.as_deref(), None, 0, 0, 0).is_err()
            || WinHttpReceiveResponse(req, std::ptr::null_mut()).is_err()
        {
            return (
                DownloadOutcome::Failed,
                "ダウンロード通信に失敗しました".to_owned(),
            );
        }

        let mut status_buf = [0u16; 16];
        let mut status_bytes = std::mem::size_of_val(&status_buf) as u32;
        let ok = WinHttpQueryHeaders(
            req,
            WINHTTP_QUERY_STATUS_CODE,
            PCWSTR::null(),
            Some(status_buf.as_mut_ptr().cast()),
            &mut status_bytes,
            std::ptr::null_mut(),
        )
        .is_ok();
        let code: u16 = if ok {
            let n_chars = (status_bytes as usize) / 2;
            String::from_utf16_lossy(&status_buf[..n_chars])
                .trim()
                .parse()
                .unwrap_or(0)
        } else {
            0
        };
        (std::mem::ManuallyDrop::new((_sess, _conn, h_req)), code)
    };
    let (_sess, _conn, h_req) = std::mem::ManuallyDrop::into_inner(h_req);

    if !(200..300).contains(&status_code) {
        return (
            DownloadOutcome::Failed,
            format!("HTTP エラー応答: {status_code}"),
        );
    }
    let (append, mut received) = if resume_from > 0 && status_code == 206 {
        (true, resume_from)
    } else {
        (false, 0u64)
    };

    let mut file = match OpenOptions::new()
        .create(true)
        .write(true)
        .append(append)
        .truncate(!append)
        .open(&part)
    {
        Ok(f) => f,
        Err(e) => {
            return (
                DownloadOutcome::Failed,
                format!("ファイルを開けません: {e}"),
            )
        }
    };

    let mut buf = vec![0u8; 64 * 1024];
    let mut last_notify = Instant::now();
    loop {
        if cancel.load(Ordering::Relaxed) {
            return (
                DownloadOutcome::Cancelled,
                "キャンセルされました".to_owned(),
            );
        }
        let mut read_bytes = 0u32;
        // SAFETY: h_req は有効な WinHTTP リクエストハンドルであり、buf へ最大 buf.len() バイト読み込む。
        let res = unsafe {
            WinHttpReadData(
                h_req.0,
                buf.as_mut_ptr().cast(),
                buf.len() as u32,
                &mut read_bytes,
            )
        };
        if let Err(e) = res {
            return (DownloadOutcome::Failed, format!("受信エラー: {e}"));
        }
        if read_bytes == 0 {
            break;
        }
        if let Err(e) = file.write_all(&buf[..read_bytes as usize]) {
            return (DownloadOutcome::Failed, format!("書き込みエラー: {e}"));
        }
        received += u64::from(read_bytes);
        if last_notify.elapsed() >= Duration::from_millis(200) {
            sink.on_progress(id, received, expected_size);
            last_notify = Instant::now();
        }
    }
    if let Err(e) = file.flush() {
        return (DownloadOutcome::Failed, format!("フラッシュ失敗: {e}"));
    }
    drop(file);
    sink.on_progress(id, received, expected_size);

    match verify_sha256_file(&part, expected_sha256) {
        Ok(true) => {
            if let Err(e) = fs::rename(&part, dest) {
                return (DownloadOutcome::Failed, format!("ファイル配置に失敗: {e}"));
            }
            (DownloadOutcome::Done, String::new())
        }
        Ok(false) => {
            let _ = fs::remove_file(&part);
            (
                DownloadOutcome::Failed,
                "SHA-256 チェックサムが一致しません".to_owned(),
            )
        }
        Err(e) => (DownloadOutcome::Failed, format!("検証エラー: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_https_url_correctly() {
        let p = parse_url("https://huggingface.co/foo/bar/resolve/main/model.gguf").unwrap();
        assert!(p.https);
        assert_eq!(p.host, "huggingface.co");
        assert_eq!(p.port, 443);
        assert_eq!(p.path, "/foo/bar/resolve/main/model.gguf");
    }
}
