//! Windows MSI (WiX) インストーラ用 Rust CustomAction (`custom_action.cc` 置換)。
//!
//! Install/Upgrade/Uninstall 時にサーバー等の常駐プロセスを停止し、
//! 版付き x64 (`asteria_tip64-<ver>.dll`) と x86 (`x86\asteria_tip32-<ver>.dll`) を
//! 退避・整理・ロールバック復旧し、COM/TSF 登録・解除を行う（docs/adr/0072）。

#![deny(unsafe_code)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

pub const ERROR_SUCCESS: u32 = 0;
pub const ERROR_INSTALL_FAILURE: u32 = 1603;

/// 退避した使用中 TIP DLL の拡張子（`<元の名前>.<版>-<番号>.asteria-old`）。
pub const MOVED_TIP_SUFFIX: &str = ".asteria-old";

static MOVE_SEQ: AtomicU64 = AtomicU64::new(0);

/// MSI CustomAction ハンドル (MSIHANDLE = u32)。
pub type MsiHandle = u32;

/// 停止対象プロセス名 (taskkill /IM 互換)。
pub const STOP_PROCESS_IMAGES: &[&str] = &[
    "asteria-server.exe",
    "asteria-renderer.exe",
    "asteria-config.exe",
];

/// 標準導入先 (`%ProgramFiles%\Asteria`) を解決する。
pub fn default_install_dir() -> PathBuf {
    std::env::var_os("ProgramFiles")
        .or_else(|| std::env::var_os("ProgramW6432"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Program Files"))
        .join("Asteria")
}

/// `[INSTALLDIR]` または `[INSTALLDIR]|<ver>` 形式の CustomActionData を分解する。
pub fn parse_action_data(raw: &str) -> (PathBuf, Option<String>) {
    let trimmed = raw.trim();
    if let Some((dir, ver)) = trimmed.split_once('|') {
        let dir = dir.trim();
        let ver = ver.trim();
        let path = if dir.is_empty() {
            default_install_dir()
        } else {
            PathBuf::from(dir)
        };
        let version = (!ver.is_empty()).then(|| ver.to_owned());
        (path, version)
    } else if trimmed.is_empty() {
        (default_install_dir(), None)
    } else {
        (PathBuf::from(trimmed), None)
    }
}

/// ファイル名が IME TIP DLL（無印・版付き Asteria / 互換 Mozc）か判定する。
pub fn is_tip_dll_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    if !lower.ends_with(".dll") {
        return false;
    }
    lower == "asteria_tip.dll"
        || lower.starts_with("asteria_tip64-")
        || lower.starts_with("asteria_tip32-")
        || lower.starts_with("mozc_tip")
}

fn find_tip_dll(dir: &Path, bits: &str) -> Option<PathBuf> {
    let (search_dir, versioned_prefix, unversioned) = if bits == "32" {
        (dir.join("x86"), "asteria_tip32-", "asteria_tip.dll")
    } else {
        (dir.to_path_buf(), "asteria_tip64-", "asteria_tip.dll")
    };
    let mut versioned = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&search_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            let lower = name.to_ascii_lowercase();
            if lower.starts_with(versioned_prefix) && lower.ends_with(".dll") && path.is_file() {
                versioned.push(path);
            }
        }
    }
    versioned.sort();
    if let Some(latest) = versioned.pop() {
        return Some(latest);
    }
    let fallback = search_dir.join(unversioned);
    fallback.is_file().then_some(fallback)
}

/// 指定インストールディレクトリ内の x64 / x86 TSF TIP DLL パスを組み立てる。
pub fn tip_dll_targets(install_dir: &Path) -> Vec<(PathBuf, PathBuf)> {
    let sys_root = std::env::var_os("SystemRoot")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
    let mut pairs = Vec::new();
    if let Some(x64_dll) = find_tip_dll(install_dir, "64") {
        pairs.push((sys_root.join("System32").join("regsvr32.exe"), x64_dll));
    }
    if let Some(x86_dll) = find_tip_dll(install_dir, "32") {
        pairs.push((sys_root.join("SysWOW64").join("regsvr32.exe"), x86_dll));
    }
    pairs
}

/// `asteria-server.exe` 等を停止する。プロセスが存在しない場合はエラーにしない。
pub fn stop_processes() {
    for image in STOP_PROCESS_IMAGES {
        let _ = Command::new("taskkill").args(["/F", "/IM", image]).status();
    }
}

/// `regsvr32` で導入済み TIP DLL を登録または解除する。
pub fn run_regsvr32(install_dir: &Path, unregister: bool) -> bool {
    let mut ok = true;
    for (exe, dll) in tip_dll_targets(install_dir) {
        let mut cmd = Command::new(&exe);
        if unregister {
            cmd.arg("/u");
        }
        cmd.arg("/s").arg(&dll);
        if !cmd.status().is_ok_and(|s| s.success()) {
            ok = false;
        }
    }
    ok
}

fn tip_directories(install_dir: &Path) -> [PathBuf; 2] {
    [install_dir.to_path_buf(), install_dir.join("x86")]
}

fn matches_version(name: &str, version: &str) -> bool {
    let suffix = format!("-{version}.dll").to_ascii_lowercase();
    name.to_ascii_lowercase().ends_with(&suffix)
}

/// 退避済みの `*.asteria-old` のうちロックが外れたものを削除する。
pub fn cleanup_old_tips(install_dir: &Path) {
    for dir in tip_directories(install_dir) {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.ends_with(MOVED_TIP_SUFFIX) {
                    let _ = std::fs::remove_file(&path);
                }
            }
        }
    }
}

/// 古い版（または全版）の TIP DLL を `.asteria-old` へリネームして退避し、可能なら即削除、
/// 使用中なら再起動時削除を予約する。
pub fn move_aside_tips(install_dir: &Path, current_version: Option<&str>, keep_current: bool) {
    let tag = current_version.unwrap_or("0");
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    for dir in tip_directories(install_dir) {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut to_move = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if !is_tip_dll_name(name) || !path.is_file() {
                continue;
            }
            if keep_current {
                if let Some(ver) = current_version {
                    if matches_version(name, ver) {
                        continue;
                    }
                }
            }
            let name = name.to_owned();
            to_move.push((path, name));
        }
        for (src, name) in to_move {
            let seq = MOVE_SEQ.fetch_add(1, Ordering::Relaxed);
            let moved_name = format!("{name}.{tag}-{now_ms}-{seq}{MOVED_TIP_SUFFIX}");
            let moved = dir.join(moved_name);
            if std::fs::rename(&src, &moved).is_ok() && std::fs::remove_file(&moved).is_err() {
                schedule_delete_on_reboot(&moved);
            }
        }
    }
}

/// ロールバック時に `<元の名前>.<version>-...asteria-old` を元のファイル名へ戻す。
pub fn restore_moved_tips(install_dir: &Path, version: &str) {
    let marker = format!(".{version}-");
    for dir in tip_directories(install_dir) {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if !name.ends_with(MOVED_TIP_SUFFIX) {
                continue;
            }
            let Some(at) = name.find(&marker) else {
                continue;
            };
            let original = dir.join(&name[..at]);
            if !original.exists() {
                let _ = std::fs::rename(&path, &original);
            }
        }
    }
}

#[cfg(windows)]
fn schedule_delete_on_reboot(path: &Path) {
    ffi::schedule_delete_on_reboot(path);
}

#[cfg(not(windows))]
fn schedule_delete_on_reboot(_path: &Path) {}

/// MSI の ProductVersion (`Major.Minor.Patch`) 形式へ正規化する。
pub fn normalize_msi_version(cargo_version: &str) -> String {
    let core = cargo_version.split(['-', '+']).next().unwrap_or("0.1.0");
    let mut nums: Vec<u32> = core
        .split('.')
        .take(3)
        .filter_map(|p| p.parse::<u32>().ok())
        .collect();
    while nums.len() < 3 {
        nums.push(0);
    }
    format!(
        "{}.{}.{}",
        nums[0].min(255),
        nums[1].min(255),
        nums[2].min(65535)
    )
}

#[cfg(windows)]
mod ffi {
    #![allow(unsafe_code)]

    use std::os::windows::ffi::OsStrExt;
    use std::path::{Path, PathBuf};

    use super::{
        cleanup_old_tips, default_install_dir, move_aside_tips, parse_action_data,
        restore_moved_tips, run_regsvr32, stop_processes, MsiHandle, ERROR_INSTALL_FAILURE,
        ERROR_SUCCESS,
    };
    use windows::Win32::Foundation::{BOOL, HMODULE};
    use windows::Win32::Storage::FileSystem::{MoveFileExW, MOVEFILE_DELAY_UNTIL_REBOOT};
    use windows::Win32::System::ApplicationInstallationAndServicing::{MsiGetPropertyW, MSIHANDLE};
    use windows::Win32::System::SystemServices::DLL_PROCESS_ATTACH;
    use windows_core::{w, PCWSTR, PWSTR};

    const ERROR_MORE_DATA_CODE: u32 = 234;

    pub(super) fn schedule_delete_on_reboot(path: &Path) {
        let wide: Vec<u16> = path
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        // SAFETY: NUL 終端済み UTF-16 パスを `MoveFileExW(MOVEFILE_DELAY_UNTIL_REBOOT)` に渡す。
        let _ = unsafe {
            MoveFileExW(
                PCWSTR(wide.as_ptr()),
                PCWSTR::null(),
                MOVEFILE_DELAY_UNTIL_REBOOT,
            )
        };
    }

    fn read_action_data(install: MsiHandle) -> String {
        let mut len = 0u32;
        let mut empty = [0u16; 1];
        // SAFETY: CustomActionData の必要バッファ長を取得する。
        let first = unsafe {
            MsiGetPropertyW(
                MSIHANDLE(install),
                w!("CustomActionData"),
                PWSTR(empty.as_mut_ptr()),
                Some(&mut len),
            )
        };
        if (first == ERROR_MORE_DATA_CODE || first == ERROR_SUCCESS) && len > 0 {
            let mut buf = vec![0u16; (len + 1) as usize];
            let mut cap = len + 1;
            // SAFETY: len + 1 で確保した有効バッファへ UTF-16 プロパティを読み取る。
            let second = unsafe {
                MsiGetPropertyW(
                    MSIHANDLE(install),
                    w!("CustomActionData"),
                    PWSTR(buf.as_mut_ptr()),
                    Some(&mut cap),
                )
            };
            if second == ERROR_SUCCESS && cap > 0 {
                return String::from_utf16_lossy(&buf[..cap as usize])
                    .trim_end_matches('\0')
                    .to_owned();
            }
        }
        String::new()
    }

    fn resolve_install_dir(install: MsiHandle) -> PathBuf {
        let (dir, _) = parse_action_data(&read_action_data(install));
        if dir.as_os_str().is_empty() {
            default_install_dir()
        } else {
            dir
        }
    }

    #[no_mangle]
    pub extern "system" fn DllMain(
        _module: HMODULE,
        reason: u32,
        _reserved: *mut core::ffi::c_void,
    ) -> BOOL {
        let _ = reason == DLL_PROCESS_ATTACH;
        BOOL(1)
    }

    /// インストール・更新・削除前に常駐プロセスを停止する MSI CustomAction。
    ///
    /// # Safety
    /// Windows Installer (MSI) から有効な `MSIHANDLE` で呼び出される stdcall エントリポイント。
    #[no_mangle]
    pub unsafe extern "system" fn StopAsteriaProcesses(_install: MsiHandle) -> u32 {
        stop_processes();
        ERROR_SUCCESS
    }

    /// 導入済み TIP DLL を `regsvr32` で登録する MSI CustomAction。
    ///
    /// # Safety
    /// Windows Installer (MSI) から有効な `MSIHANDLE` で呼び出される stdcall エントリポイント。
    #[no_mangle]
    pub unsafe extern "system" fn RegisterTipDlls(install: MsiHandle) -> u32 {
        if run_regsvr32(&resolve_install_dir(install), false) {
            ERROR_SUCCESS
        } else {
            ERROR_INSTALL_FAILURE
        }
    }

    /// 導入済み TIP DLL の登録を解除する MSI CustomAction。
    ///
    /// # Safety
    /// Windows Installer (MSI) から有効な `MSIHANDLE` で呼び出される stdcall エントリポイント。
    #[no_mangle]
    pub unsafe extern "system" fn UnregisterTipDlls(install: MsiHandle) -> u32 {
        stop_processes();
        let _ = run_regsvr32(&resolve_install_dir(install), true);
        ERROR_SUCCESS
    }

    /// 更新前に旧版 TIP DLL を退避し、前回の退避ファイル（`*.asteria-old`）を削除する MSI CustomAction。
    ///
    /// # Safety
    /// Windows Installer (MSI) から有効な `MSIHANDLE` で呼び出される stdcall エントリポイント。
    #[no_mangle]
    pub unsafe extern "system" fn MoveAsideOldTip(install: MsiHandle) -> u32 {
        let (dir, version) = parse_action_data(&read_action_data(install));
        cleanup_old_tips(&dir);
        move_aside_tips(&dir, version.as_deref(), true);
        ERROR_SUCCESS
    }

    /// アンインストール前に現在の版を含む全 TIP DLL を退避・削除する MSI CustomAction。
    ///
    /// # Safety
    /// Windows Installer (MSI) から有効な `MSIHANDLE` で呼び出される stdcall エントリポイント。
    #[no_mangle]
    pub unsafe extern "system" fn MoveAsideAllTip(install: MsiHandle) -> u32 {
        let (dir, version) = parse_action_data(&read_action_data(install));
        cleanup_old_tips(&dir);
        move_aside_tips(&dir, version.as_deref(), false);
        ERROR_SUCCESS
    }

    /// インストール失敗時のロールバックで、退避済み TIP DLL を元の名前へ戻す MSI CustomAction。
    ///
    /// # Safety
    /// Windows Installer (MSI) から有効な `MSIHANDLE` で呼び出される stdcall エントリポイント。
    #[no_mangle]
    pub unsafe extern "system" fn RestoreMovedTip(install: MsiHandle) -> u32 {
        let (dir, version) = parse_action_data(&read_action_data(install));
        if let Some(ver) = version {
            restore_moved_tips(&dir, &ver);
        }
        ERROR_SUCCESS
    }

    /// 退避残骸（`*.asteria-old`）を掃除する MSI CustomAction。
    ///
    /// # Safety
    /// Windows Installer (MSI) から有効な `MSIHANDLE` で呼び出される stdcall エントリポイント。
    #[no_mangle]
    pub unsafe extern "system" fn CleanupOldTips(install: MsiHandle) -> u32 {
        let (dir, _) = parse_action_data(&read_action_data(install));
        cleanup_old_tips(&dir);
        ERROR_SUCCESS
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_tip_targets_and_normalizes_msi_version() {
        let dir = std::env::temp_dir().join(format!("asteria-msi-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("x86")).unwrap();
        std::fs::write(dir.join("asteria_tip64-5.5.0.dll"), b"").unwrap();
        std::fs::write(dir.join("x86").join("asteria_tip32-5.5.0.dll"), b"").unwrap();
        std::fs::write(dir.join("asteria_tip.dll.5.4.0-1.asteria-old"), b"").unwrap();

        let targets = tip_dll_targets(&dir);
        assert_eq!(targets.len(), 2);
        assert!(targets[0].0.to_string_lossy().contains("System32"));
        assert!(targets[0]
            .1
            .to_string_lossy()
            .ends_with("asteria_tip64-5.5.0.dll"));
        assert!(targets[1].0.to_string_lossy().contains("SysWOW64"));
        assert!(targets[1]
            .1
            .to_string_lossy()
            .ends_with("asteria_tip32-5.5.0.dll"));

        assert_eq!(normalize_msi_version("0.1.0-beta.1"), "0.1.0");
        assert_eq!(normalize_msi_version("1.2.34"), "1.2.34");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn move_aside_cleanup_and_restore_roundtrip() {
        let dir = std::env::temp_dir().join(format!("asteria-msi-move-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("x86")).unwrap();

        let old64 = dir.join("asteria_tip64-5.4.0.dll");
        let cur64 = dir.join("asteria_tip64-5.5.0.dll");
        let old32 = dir.join("x86").join("asteria_tip32-5.4.0.dll");
        let leftover = dir.join("asteria_tip.dll.5.3.0-99.asteria-old");
        std::fs::write(&old64, b"old64").unwrap();
        std::fs::write(&cur64, b"cur64").unwrap();
        std::fs::write(&old32, b"old32").unwrap();
        std::fs::write(&leftover, b"leftover").unwrap();

        cleanup_old_tips(&dir);
        assert!(!leftover.exists());

        // Unlocked files are renamed and immediately deleted cleanly.
        move_aside_tips(&dir, Some("5.5.0"), true);
        assert!(!old64.exists());
        assert!(!old32.exists());
        assert!(cur64.exists());

        // Simulated locked `.asteria-old` artifact restores cleanly on rollback.
        let moved_name = dir.join("asteria_tip64-5.4.0.dll.5.5.0-1234.asteria-old");
        std::fs::write(&moved_name, b"locked-old64").unwrap();
        restore_moved_tips(&dir, "5.5.0");
        assert!(old64.is_file());
        assert!(!moved_name.exists());

        let (p, v) = parse_action_data(r"C:\Program Files\Asteria|5.5.0");
        assert_eq!(p, PathBuf::from(r"C:\Program Files\Asteria"));
        assert_eq!(v.as_deref(), Some("5.5.0"));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
