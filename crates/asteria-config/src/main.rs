//! Asteria 設定 GUI (`asteria-config`, WebView2 ホスト + SettingsBridge)。
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

pub mod bridge;
pub mod downloader;
#[cfg(windows)]
mod win_webview;

#[cfg(windows)]
fn main() -> anyhow::Result<()> {
    win_webview::run_settings_window()
}

#[cfg(not(windows))]
fn main() {
    eprintln!("asteria-config は Windows 専用");
}
