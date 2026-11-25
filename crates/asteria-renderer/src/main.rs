//! Asteria の候補の窓(カード 98)。Mozc の mozc_renderer と同じ約束で命令を受け、Direct2D で描く。
//!
//! 使い方: `asteria-renderer --profile <ユーザーのプロファイル> --version <Mozc の版>`
//! (設定で切り替えて試すための段階。製品の既定は今の mozc_renderer のまま)

#[cfg(windows)]
mod app;

#[cfg(windows)]
fn main() {
    if let Err(e) = app::run(std::env::args().skip(1).collect()) {
        eprintln!("asteria-renderer: {e}");
        std::process::exit(1);
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("asteria-renderer は Windows 専用");
}
