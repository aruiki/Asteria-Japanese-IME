//! renderer への送信の C ABI のテスト(docs/adr/0010)。

#![allow(unsafe_code, clippy::unwrap_used)]

use std::ffi::{c_char, CString};
use std::io::{self, Write};
use std::net::{TcpListener, TcpStream};
use std::ptr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::{Duration, Instant};

use asteria_proto::renderer::{renderer_message, RendererMessage};
use asteria_proto::{read_message, PROTOCOL_MAJOR};

use super::ffi::{ASTERIA_ERR_ARGUMENT, ASTERIA_OK, ASTERIA_PASS_THROUGH};
use super::renderer::{RendererConnector, RENDERER_WRITE_TIMEOUT};
use super::renderer_ffi::*;

/// ループバック TCP の決まったアドレスへつなぐ。
struct Tcp(std::net::SocketAddr);

impl RendererConnector for Tcp {
    fn connect(&mut self) -> io::Result<Box<dyn Write + Send>> {
        let stream = TcpStream::connect(self.0)?;
        stream.set_write_timeout(Some(RENDERER_WRITE_TIMEOUT))?;
        Ok(Box::new(stream))
    }

    fn launch(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// 受けたメッセージを渡す偽の renderer を立て、C ABI の接続を返す。
fn fake_renderer() -> (*mut AsteriaRenderer, mpsc::Receiver<RendererMessage>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        while let Ok(Some(msg)) = read_message::<_, RendererMessage>(&mut stream) {
            if tx.send(msg).is_err() {
                return;
            }
        }
    });
    let r = AsteriaRenderer::with_connector(Box::new(Tcp(addr)));
    (Box::into_raw(Box::new(r)), rx)
}

fn recv(rx: &mpsc::Receiver<RendererMessage>) -> renderer_message::Body {
    let msg = rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(msg.protocol_major, PROTOCOL_MAJOR);
    msg.body.unwrap()
}

#[test]
fn show_and_hide_reach_the_renderer() {
    let (r, rx) = fake_renderer();
    let texts = [CString::new("今日").unwrap(), CString::new("京").unwrap()];
    let notes = [CString::new("日付").unwrap()];
    let text_ptrs: Vec<*const c_char> = texts.iter().map(|t| t.as_ptr()).collect();
    let note_ptrs = [notes[0].as_ptr(), ptr::null()];
    let caret = AsteriaRect {
        left: -100,
        top: 20,
        right: -80,
        bottom: 40,
    };
    // SAFETY: r は fake_renderer が作った有効な接続で、配列と矩形は呼び出しの間生きている。
    unsafe {
        assert_eq!(asteria_renderer_connected(r), 0);
        let status = asteria_renderer_show(
            r,
            text_ptrs.as_ptr(),
            note_ptrs.as_ptr(),
            2,
            1,
            &caret,
            0x10,
            0x20,
        );
        assert_eq!(status, ASTERIA_OK);
        assert_eq!(asteria_renderer_connected(r), 1);
        match recv(&rx) {
            renderer_message::Body::Show(s) => {
                let texts: Vec<_> = s.candidates.iter().map(|c| c.text.as_str()).collect();
                assert_eq!(texts, ["今日", "京"]);
                assert_eq!(s.candidates[0].annotation, "日付");
                assert_eq!(s.candidates[1].annotation, "");
                assert_eq!(s.focused_index, 1);
                let c = s.caret.unwrap();
                assert_eq!((c.left, c.top, c.right, c.bottom), (-100, 20, -80, 40));
                assert_eq!((s.owner_window, s.notify_window), (0x10, 0x20));
            }
            other => panic!("{other:?}"),
        }
        // 注釈の配列は NULL でもよい。
        let status = asteria_renderer_show(r, text_ptrs.as_ptr(), ptr::null(), 1, 0, &caret, 0, 0);
        assert_eq!(status, ASTERIA_OK);
        assert!(matches!(recv(&rx), renderer_message::Body::Show(s) if s.candidates.len() == 1));
        assert_eq!(asteria_renderer_hide(r), ASTERIA_OK);
        assert!(matches!(recv(&rx), renderer_message::Body::Hide(_)));

        // 引数の誤り。
        assert_eq!(
            asteria_renderer_show(r, ptr::null(), ptr::null(), 1, 0, &caret, 0, 0),
            ASTERIA_ERR_ARGUMENT
        );
        assert_eq!(
            asteria_renderer_show(r, text_ptrs.as_ptr(), ptr::null(), 1, 0, ptr::null(), 0, 0),
            ASTERIA_ERR_ARGUMENT
        );
        assert_eq!(asteria_renderer_hide(ptr::null_mut()), ASTERIA_ERR_ARGUMENT);
        asteria_renderer_free(r);
        asteria_renderer_free(ptr::null_mut());
    }
}

/// つながらず、起動を頼まれた回数を数える。
struct Missing(Arc<AtomicUsize>);

impl RendererConnector for Missing {
    fn connect(&mut self) -> io::Result<Box<dyn Write + Send>> {
        Err(io::Error::from(io::ErrorKind::NotFound))
    }

    fn launch(&mut self) -> io::Result<()> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

#[test]
fn without_renderer_show_returns_at_once_and_launches_it() {
    let launches = Arc::new(AtomicUsize::new(0));
    let r = Box::into_raw(Box::new(AsteriaRenderer::with_connector(Box::new(
        Missing(Arc::clone(&launches)),
    ))));
    let caret = AsteriaRect::default();
    let start = Instant::now();
    // SAFETY: r は有効な接続で、候補は 0 件(texts は読まない)。
    unsafe {
        assert_eq!(
            asteria_renderer_show(r, ptr::null(), ptr::null(), 0, 0, &caret, 0, 0),
            ASTERIA_PASS_THROUGH
        );
        // 間隔を空けるまではつなぎ直さない(起動も頼まない)。
        assert_eq!(asteria_renderer_hide(r), ASTERIA_PASS_THROUGH);
        asteria_renderer_free(r);
    }
    assert!(start.elapsed() < Duration::from_millis(50));
    assert_eq!(launches.load(Ordering::SeqCst), 1);

    // 文字列が UTF-8 でなければ NULL。
    let bad = [0xFFu8, 0];
    // SAFETY: bad は NUL 終端のバイト列。
    assert!(unsafe { asteria_renderer_open(bad.as_ptr().cast(), ptr::null()) }.is_null());
}
