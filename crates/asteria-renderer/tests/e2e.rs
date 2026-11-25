//! 実際の asteria-renderer.exe を起こし、Mozc の TIP と同じ手順で命令を送って、窓が出る・消えることを確かめる。

#![cfg(windows)]
#![allow(unsafe_code, clippy::expect_used, clippy::unwrap_used, clippy::zombie_processes)]

use std::io::{Read, Write};
use std::time::{Duration, Instant};

use asteria_renderer::mozc::commands::candidate_window::Candidate;
use asteria_renderer::mozc::commands::renderer_command::{
    ApplicationInfo, CharacterPosition, CommandType, Point,
};
use asteria_renderer::mozc::commands::{CandidateWindow, Output, RendererCommand};
use asteria_renderer::mozc::ipc::IpcPathInfo;
use asteria_renderer::pipe_name;
use prost::Message;
use windows::core::w;
use windows::Win32::Foundation::RECT;
use windows::Win32::UI::WindowsAndMessaging::{FindWindowW, GetWindowRect, IsWindowVisible};

fn send(key: &str, cmd: &RendererCommand) -> Duration {
    let t0 = Instant::now();
    let mut f = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(pipe_name(key))
        .expect("open pipe");
    f.write_all(&cmd.encode_to_vec()).expect("write");
    let mut resp = Vec::new();
    let _ = f.read_to_end(&mut resp);
    t0.elapsed()
}

fn wait_until(mut f: impl FnMut() -> bool) -> bool {
    let end = Instant::now() + Duration::from_secs(5);
    while Instant::now() < end {
        if f() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    false
}

#[test]
fn shows_and_hides_candidates() {
    let profile = std::env::temp_dir().join(format!("asteria-renderer-e2e-{}", std::process::id()));
    std::fs::create_dir_all(&profile).unwrap();
    let ipc = profile.join("renderer.ipc");
    let _ = std::fs::remove_file(&ipc);
    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_asteria-renderer"))
        .args([
            "--profile",
            profile.to_str().unwrap(),
            "--version",
            "3.34.0.0",
        ])
        .spawn()
        .expect("spawn");
    assert!(wait_until(|| ipc.exists()), "renderer.ipc ができない");
    let info = IpcPathInfo::decode(std::fs::read(&ipc).unwrap().as_slice()).unwrap();
    assert_eq!(info.process_id(), child.id());

    let cmd = RendererCommand {
        r#type: Some(CommandType::Update as i32),
        visible: Some(true),
        output: Some(Output {
            candidate_window: Some(CandidateWindow {
                focused_index: Some(0),
                size: 2,
                candidate: ["変換", "返還"]
                    .iter()
                    .enumerate()
                    .map(|(i, v)| Candidate {
                        index: i as u32,
                        value: v.to_string(),
                        ..Default::default()
                    })
                    .collect(),
                ..Default::default()
            }),
            ..Default::default()
        }),
        application_info: Some(ApplicationInfo {
            composition_target: Some(CharacterPosition {
                top_left: Some(Point {
                    x: Some(200),
                    y: Some(200),
                }),
                line_height: Some(20),
                ..Default::default()
            }),
            ..Default::default()
        }),
        ..Default::default()
    };
    let rtt = send(info.key(), &cmd);
    // SAFETY: 文字列は静的。
    let hwnd = unsafe { FindWindowW(w!("AsteriaCandidateWindow"), None) }.expect("窓がない");
    // SAFETY: hwnd は見つけた窓。
    assert!(
        wait_until(|| unsafe { IsWindowVisible(hwnd) }.as_bool()),
        "窓が出ない"
    );
    let mut r = RECT::default();
    // SAFETY: r は有効な出力先。
    unsafe { GetWindowRect(hwnd, &mut r) }.unwrap();
    assert_eq!((r.left, r.top), (200, 220), "入力中の文字の下に出る");
    assert!(r.right - r.left > 20 && r.bottom - r.top > 20);

    let hide = RendererCommand {
        r#type: Some(CommandType::Update as i32),
        visible: Some(false),
        ..Default::default()
    };
    send(info.key(), &hide);
    // SAFETY: hwnd は見つけた窓。
    assert!(
        wait_until(|| !unsafe { IsWindowVisible(hwnd) }.as_bool()),
        "窓が消えない"
    );

    eprintln!("1 往復 {rtt:?}");
    let _ = child.kill();
    let _ = child.wait();
    let _ = std::fs::remove_dir_all(&profile);
}
