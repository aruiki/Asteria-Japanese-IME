//! 導入済み Asteria の実機プローブとライブ変換チェック（`installed_probe.py`, `live_conversion_check.py` 移植）。

use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{bail, Result};
use serde_json::{json, Value};

use asteria_common::hash::set_writable;
use asteria_common::paths::profile_dir;

use crate::installer_smoke::run_checked;

const LIVE_CASES: &[(&str, &str, &str)] = &[
    ("0", "nihonngo", "日本語"),
    ("1", "toukyou", "東京"),
    ("2", "kyouhaiitennkidesune", "今日はいい天気ですね"),
    ("3", "kaiginosiryouwookurimasu", "会議の資料を送ります"),
];
const LIVE_ON_PROTO_BYTES: &[u8] = &[0x80, 0x0D, 0x01];

fn restart_server() {
    let _ = Command::new("taskkill")
        .args(["/F", "/IM", "mozc_server.exe"])
        .output();
    thread::sleep(Duration::from_secs(2));
}

fn backup_profile_files(profile: &Path, saved: &Path) -> Result<()> {
    if saved.exists() {
        fs::remove_dir_all(saved)?;
    }
    fs::create_dir_all(saved)?;
    for entry in fs::read_dir(profile)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if matches!(ext, "db" | "txt" | "tsv" | "ini") {
                fs::copy(&path, saved.join(entry.file_name()))?;
            }
        }
    }
    Ok(())
}

fn restore_profile_files(saved: &Path, profile: &Path) -> Result<()> {
    for entry in fs::read_dir(saved)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let target = profile.join(entry.file_name());
        if target.exists() && fs::read(&target)? == fs::read(&path)? {
            continue;
        }
        if target.exists() {
            let _ = set_writable(&target);
        }
        fs::copy(&path, &target)?;
    }
    Ok(())
}

fn compile_imebench(out: &Path, repo_root: &Path) -> Result<std::path::PathBuf> {
    let windir = Path::new(&env::var("WINDIR")?).to_path_buf();
    let compiler = windir.join("Microsoft.NET/Framework64/v4.0.30319/csc.exe");
    let source = repo_root.join("eval/imebench/ImeBench.cs");
    let exe = out.join("ImeBench.exe");
    run_checked(
        &compiler.to_string_lossy(),
        &[
            "/nologo",
            "/target:winexe",
            "/platform:x64",
            "/reference:System.Windows.Forms.dll",
            "/reference:System.Drawing.dll",
            &format!("/out:{}", exe.display()),
            &source.to_string_lossy(),
        ],
    )?;
    Ok(exe)
}

/// 導入済み IME に対して `ImeBench.exe` で実入力し結果を出力する（`installed_probe.py` 移植）。
pub fn run_installed_probe(
    out: &Path,
    wait_ms: u64,
    live: bool,
    keys: &[String],
    repo_root: &Path,
) -> Result<String> {
    fs::create_dir_all(out)?;
    let out = fs::canonicalize(out)?;
    let exe = compile_imebench(&out, repo_root)?;
    let cases = out.join("input.tsv");
    let mut content = String::from("warm\taiueo\n");
    for (i, k) in keys.iter().enumerate() {
        content.push_str(&format!("{i}\t{k}\n"));
    }
    fs::write(&cases, content)?;
    let output = out.join("output.tsv");
    let profile = profile_dir();
    let saved = out.join("profile-backup");
    backup_profile_files(&profile, &saved)?;

    let wait_str = wait_ms.to_string();
    let mut cmd = Command::new(&exe);
    cmd.args([
        "{2A1ADAE4-8061-4AC0-A7D4-713BF8DCB591}",
        "{455A87E7-275F-4582-92CB-6600917A436D}",
    ])
    .arg(&cases)
    .arg(&output)
    .arg(&wait_str);
    if live {
        cmd.arg("live");
    }
    let res = cmd.output();
    restart_server();
    restore_profile_files(&saved, &profile)?;
    let proc_out = res?;
    if !proc_out.status.success() {
        bail!("ImeBench 失敗: {:?}", proc_out.status.code());
    }
    let text = fs::read_to_string(&output)?;
    Ok(text.strip_prefix('\u{feff}').unwrap_or(&text).to_owned())
}

/// 導入済み IME でライブ変換を一時有効化して検証する（`live_conversion_check.py` 移植）。
pub fn run_live_conversion_check(out: &Path, wait_ms: u64, repo_root: &Path) -> Result<Value> {
    fs::create_dir_all(out)?;
    let out = fs::canonicalize(out)?;
    let profile = profile_dir();
    let config = profile.join("config1.db");
    let saved = out.join("profile-backup");
    restart_server();
    backup_profile_files(&profile, &saved)?;
    let backup = saved.join("config1.db");
    let exe = compile_imebench(&out, repo_root)?;

    let cases = out.join("input.tsv");
    let mut cases_tsv = String::from("first\tkakikukeko\n");
    for (i, r, _) in LIVE_CASES {
        cases_tsv.push_str(&format!("{i}\t{r}\n"));
    }
    fs::write(&cases, cases_tsv)?;
    let warm = out.join("warm.tsv");
    fs::write(&warm, "warm\taiueo\n")?;
    let output = out.join("output.tsv");

    let body = (|| -> Result<()> {
        let mut cfg_bytes = fs::read(&backup)?;
        cfg_bytes.extend_from_slice(LIVE_ON_PROTO_BYTES);
        fs::write(&config, cfg_bytes)?;
        restart_server();
        let status_file = profile.join("asteria_ai_status.txt");
        let _ = fs::remove_file(&status_file);
        let deadline = Instant::now() + Duration::from_secs(120);
        let warm_out = out.join("warm-out.tsv");
        loop {
            run_checked(
                &exe.to_string_lossy(),
                &[
                    "{2A1ADAE4-8061-4AC0-A7D4-713BF8DCB591}",
                    "{455A87E7-275F-4582-92CB-6600917A436D}",
                    &warm.to_string_lossy(),
                    &warm_out.to_string_lossy(),
                    "500",
                    "live",
                ],
            )?;
            let text = fs::read_to_string(&status_file).unwrap_or_default();
            if text.contains("state=llm") || Instant::now() > deadline {
                break;
            }
            thread::sleep(Duration::from_secs(3));
        }
        run_checked(
            &exe.to_string_lossy(),
            &[
                "{2A1ADAE4-8061-4AC0-A7D4-713BF8DCB591}",
                "{455A87E7-275F-4582-92CB-6600917A436D}",
                &cases.to_string_lossy(),
                &output.to_string_lossy(),
                &wait_ms.to_string(),
                "live",
            ],
        )?;
        Ok(())
    })();

    restart_server();
    restore_profile_files(&saved, &profile)?;
    body?;

    let err_file = out.join("output.tsv.error.txt");
    if err_file.exists() {
        bail!("{}", fs::read_to_string(&err_file)?);
    }
    let out_text = fs::read_to_string(&output)?;
    let out_text = out_text.strip_prefix('\u{feff}').unwrap_or(&out_text);
    let mut rows: HashMap<String, (String, String)> = HashMap::new();
    for line in out_text.lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() >= 3 {
            rows.insert(
                parts[0].to_owned(),
                (parts[1].to_owned(), parts[2].to_owned()),
            );
        }
    }
    let mut results = Vec::new();
    let mut passed = 0usize;
    for &(id, romaji, expected) in LIVE_CASES {
        let (committed, shown) = rows.get(id).cloned().unwrap_or_default();
        let ok = committed == expected;
        if ok {
            passed += 1;
        }
        results.push(json!({
            "romaji": romaji,
            "expected": expected,
            "committed": committed,
            "shown": shown,
            "ok": ok,
        }));
    }
    let mut profile_restored = true;
    for entry in fs::read_dir(&saved)? {
        let entry = entry?;
        let p = entry.path();
        if p.is_file() {
            let cur = profile.join(entry.file_name());
            if !cur.is_file() || fs::read(&cur)? != fs::read(&p)? {
                profile_restored = false;
                break;
            }
        }
    }
    let report = json!({
        "cases": results,
        "passed": passed,
        "total": LIVE_CASES.len(),
        "profile_restored": profile_restored,
    });
    fs::write(
        out.join("live.json"),
        serde_json::to_string_pretty(&report)?,
    )?;
    Ok(report)
}
