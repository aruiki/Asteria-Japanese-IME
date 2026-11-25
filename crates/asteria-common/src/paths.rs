//! Asteria (および旧 Asteria) の導入先・プロファイル・DLL パス解決。

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn program_files() -> PathBuf {
    env::var_os("ProgramFiles")
        .or_else(|| env::var_os("ProgramW6432"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Program Files"))
}

fn program_files_x86() -> PathBuf {
    env::var_os("ProgramFiles(x86)")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Program Files (x86)"))
}

fn local_low() -> PathBuf {
    let home = env::var_os("USERPROFILE")
        .or_else(|| env::var_os("HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("~"));
    home.join("AppData").join("LocalLow")
}

/// 導入先ディレクトリ（64bit `Program Files\Asteria` 優先、未導入かつ旧 `(x86)\Asteria` があれば旧先）。
pub fn install_dir() -> PathBuf {
    let modern = program_files().join("Asteria");
    let legacy = program_files_x86().join("Asteria");
    if modern.exists() || !legacy.exists() {
        modern
    } else {
        legacy
    }
}

/// ユーザープロファイル保存先 (`LocalLow\Asteria`)。
pub fn profile_dir() -> PathBuf {
    local_low().join("Asteria")
}

/// `directory` 内の IME DLL パス。純 Rust 版（版付き `asteria_tip{bits}-*.dll` / 無印 `asteria_tip.dll`）を優先し、
/// 無ければ従来の `mozc_tip{bits}` / 版付き `mozc_tip{bits}-*.dll` を返す。
pub fn tip_path(directory: &Path, bits: &str) -> PathBuf {
    let (rust_dir, rust_prefix, fallback) = if bits == "32" {
        let x86 = directory.join("x86");
        (x86.clone(), "asteria_tip32-", x86.join("asteria_tip.dll"))
    } else {
        (
            directory.to_path_buf(),
            "asteria_tip64-",
            directory.join("asteria_tip.dll"),
        )
    };

    let mut rust_versioned = Vec::new();
    if let Ok(entries) = fs::read_dir(&rust_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                let lower = name.to_ascii_lowercase();
                if lower.starts_with(rust_prefix) && lower.ends_with(".dll") {
                    rust_versioned.push(path);
                }
            }
        }
    }
    rust_versioned.sort();
    if let Some(latest) = rust_versioned.pop() {
        return latest;
    }
    if fallback.is_file() {
        return fallback;
    }

    let prefix = format!("mozc_tip{bits}");
    let exact = format!("{prefix}.dll");
    let versioned_prefix = format!("{prefix}-");

    let mut found = Vec::new();
    if let Ok(entries) = fs::read_dir(directory) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.ends_with(".dll") && (name == exact || name.starts_with(&versioned_prefix))
                {
                    found.push(path);
                }
            }
        }
    }
    found.sort();
    found.pop().unwrap_or(fallback)
}

/// 読み込まれたモジュールパスが `install` 配下の IME DLL か判定する。
pub fn is_tip_module(module_path: &Path, install: &Path, bits: &str) -> bool {
    let Some(name) = module_path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    let lower = name.to_ascii_lowercase();
    if !lower.ends_with(".dll") {
        return false;
    }
    let Some(parent) = module_path.parent() else {
        return false;
    };
    let resolved_parent = fs::canonicalize(parent).unwrap_or_else(|_| parent.to_path_buf());
    let resolved_install = fs::canonicalize(install).unwrap_or_else(|_| install.to_path_buf());
    let x86_dir = install.join("x86");
    let resolved_x86 = fs::canonicalize(&x86_dir).unwrap_or(x86_dir);

    if bits == "32" {
        if lower == "asteria_tip.dll" || lower.starts_with("asteria_tip32-") {
            return resolved_parent == resolved_x86;
        }
    } else if lower == "asteria_tip.dll" || lower.starts_with("asteria_tip64-") {
        return resolved_parent == resolved_install;
    }

    let prefix = format!("mozc_tip{bits}");
    let exact = format!("{prefix}.dll");
    let versioned_prefix = format!("{prefix}-");
    let named = lower == exact || lower.starts_with(&versioned_prefix);
    named && resolved_parent == resolved_install
}

/// カレントディレクトリから上位を辿ってリポジトリルートを探す。
pub fn find_repo_root(start: &Path) -> Option<PathBuf> {
    let mut cur = Some(start);
    while let Some(dir) = cur {
        if dir.join("Cargo.toml").is_file() && dir.join("mozc").is_dir() {
            return Some(dir.to_path_buf());
        }
        cur = dir.parent();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn tip_path_and_module_match_versioned_dlls() {
        let dir = TempDir::new().unwrap();
        let install = dir.path().join("install");
        fs::create_dir_all(&install).unwrap();
        let versioned = install.join("mozc_tip64-3.34.6448.100.dll");
        fs::write(&versioned, b"dll").unwrap();
        fs::write(
            install.join("mozc_tip64-3.34.6448.100.dll.asteria-old"),
            b"old",
        )
        .unwrap();

        assert_eq!(tip_path(&install, "64"), versioned);
        assert!(is_tip_module(&versioned, &install, "64"));
        assert!(!is_tip_module(
            &dir.path().join("mozc_tip64-3.34.6448.100.dll"),
            &install,
            "64"
        ));
        assert!(!is_tip_module(&install.join("other.dll"), &install, "64"));

        let rust_x64 = install.join("asteria_tip64-5.5.0.dll");
        let x86_dir = install.join("x86");
        fs::create_dir_all(&x86_dir).unwrap();
        let rust_x86 = x86_dir.join("asteria_tip32-5.5.0.dll");
        fs::write(&rust_x64, b"dll64").unwrap();
        fs::write(&rust_x86, b"dll32").unwrap();
        fs::write(
            install.join("asteria_tip64-5.4.0.dll.5.5.0-1.asteria-old"),
            b"old",
        )
        .unwrap();
        assert_eq!(tip_path(&install, "64"), rust_x64);
        assert_eq!(tip_path(&install, "32"), rust_x86);
        assert!(is_tip_module(&rust_x64, &install, "64"));
        assert!(is_tip_module(&rust_x86, &install, "32"));
    }
}
