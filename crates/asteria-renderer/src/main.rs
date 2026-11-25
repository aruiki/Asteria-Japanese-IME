//! Windows の候補ウィンドウ(docs/SPEC.md 4.1、REQ-10-7、11.3、docs/adr/0010)。

mod layout;
#[allow(dead_code)]
mod placement;
#[cfg(windows)]
mod win_ui;

#[cfg(windows)]
fn main() -> anyhow::Result<()> {
    win_ui::run_windows_renderer()
}

#[cfg(not(windows))]
fn main() {
    eprintln!("asteria-renderer は Windows 専用(REQ-4-3)");
}
