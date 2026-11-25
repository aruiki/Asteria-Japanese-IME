//! 使い捨て Windows ランナーでの MSI 導入・実入力・再導入・削除スモーク試験（`mozc/tools/installer_smoke.py` 移植）。

use std::collections::{HashMap, HashSet};
use std::env;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};

use asteria_common::hash::sha256_file;
use asteria_common::paths::{install_dir, is_tip_module, tip_path};

use crate::check_release::msi_property;

pub const ACCEPTANCE_CHECKS: &[&str] = &[
    "plain_preedit_space_commit",
    "richedit_preedit_space_commit",
    "candidate_down_page_roundtrip_commit",
    "cancel_and_resume",
    "backspace_and_continue",
    "f6_to_f10",
    "focus_between_ordinary_fields",
    "password_host_visible_behavior",
    "password_scope_ime_on_not_learned",
];

pub fn run_checked(program: &str, args: &[&str]) -> Result<()> {
    let out = Command::new(program)
        .args(args)
        .output()
        .with_context(|| format!("コマンド実行失敗: {program}"))?;
    if !out.status.success() {
        bail!(
            "exit {:?}: {program}\n{}{}",
            out.status.code(),
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }
    Ok(())
}

/// 導入済みフォルダ内のプロセスのみ強制終了する。
pub fn stop_installed_processes(install: &Path) -> Result<()> {
    let script = "$root = [IO.Path]::GetFullPath($env:ASTERIA_SMOKE_INSTALL) + '\\'; \
                  Get-Process | Where-Object { $_.Path -and $_.Path.StartsWith($root, [StringComparison]::OrdinalIgnoreCase) } | Stop-Process -Force";
    let out = Command::new("powershell")
        .args(["-NoProfile", "-Command", script])
        .env("ASTERIA_SMOKE_INSTALL", install)
        .output()
        .context("Stop-Process 実行失敗")?;
    if !out.status.success() {
        bail!("プロセス停止失敗: {}", String::from_utf8_lossy(&out.stderr));
    }
    Ok(())
}

/// `msiexec` を実行し、UTF-16 ログを監視しながら進捗を出力する。
pub fn run_msi(arguments: &[&str], log: &Path, ui: bool) -> Result<i32> {
    let mut cmd = Command::new("msiexec");
    cmd.args(arguments);
    if !ui {
        cmd.args(["/qn", "/norestart", "MSIRESTARTMANAGERCONTROL=Disable"]);
    }
    cmd.arg("/L*vx!").arg(log);
    let mut child = cmd.spawn().context("msiexec 起動失敗")?;
    let started = Instant::now();
    let mut offset: usize = 0;

    let code = loop {
        if log.exists() {
            if let Ok(mut file) = File::open(log) {
                let mut bytes = Vec::new();
                if file.read_to_end(&mut bytes).is_ok() && bytes.len() > offset {
                    let slice = &bytes[offset..];
                    let even_len = slice.len() & !1;
                    if even_len > 0 {
                        let u16s: Vec<u16> = slice[..even_len]
                            .chunks_exact(2)
                            .map(|c| u16::from_le_bytes([c[0], c[1]]))
                            .collect();
                        let chunk_text = String::from_utf16_lossy(&u16s);
                        for line in chunk_text.lines() {
                            if [
                                "Action start",
                                "Action ended",
                                "RESTART MANAGER",
                                "Restart",
                                "Return value 3",
                                "MainEngineThread",
                            ]
                            .iter()
                            .any(|k| line.contains(k))
                            {
                                println!("{}", line.trim_end());
                            }
                        }
                        offset += even_len;
                    }
                }
            }
        }
        if let Some(status) = child.try_wait()? {
            break status.code().unwrap_or(-1);
        }
        if !ui && started.elapsed() > Duration::from_secs(300) {
            let _ = child.kill();
            let _ = child.wait();
            bail!("msiexec が 300 秒でタイムアウトしました");
        }
        thread::sleep(Duration::from_secs(1));
    };
    Ok(code)
}

/// 画面つき MSI ログから「Dialog created」行のダイアログ名を抽出する。
pub fn msi_dialogs(log: &Path) -> Vec<String> {
    let Ok(bytes) = fs::read(log) else {
        return Vec::new();
    };
    let u16s: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
    let text = String::from_utf16_lossy(&u16s);
    let mut names = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim_end();
        if trimmed.ends_with("Dialog created") {
            if let Some((_, rhs)) = trimmed.rsplit_once(':') {
                if let Some(name) = rhs.trim().split('.').next() {
                    names.push(name.to_owned());
                }
            }
        }
    }
    names
}

/// 実入力受入れ試験レポートの妥当性を検証する。
pub fn validate_acceptance(report: &Value, install: &Path) -> Result<()> {
    let checks = report["checks"].as_array();
    let expected: HashSet<&str> = ACCEPTANCE_CHECKS.iter().copied().collect();
    let ok_checks = checks.is_some_and(|arr| {
        let names: HashSet<&str> = arr
            .iter()
            .filter_map(|c| c.get("name").and_then(Value::as_str))
            .collect();
        arr.len() == ACCEPTANCE_CHECKS.len()
            && names == expected
            && arr
                .iter()
                .all(|c| c.get("passed").and_then(Value::as_bool) == Some(true))
    });
    let has_error = report
        .get("error")
        .is_some_and(|v| !v.is_null() && v.as_str() != Some(""));
    if report.get("complete").and_then(Value::as_bool) != Some(true)
        || report.get("native_tsf").and_then(Value::as_bool) != Some(true)
        || has_error
        || !ok_checks
    {
        bail!("実入力の追加検証が未完了: {report}");
    }
    let loaded = report
        .get("loaded_tip")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .unwrap_or("-");
    if !is_tip_module(Path::new(loaded), install, "64") {
        bail!("入力ホストが導入したTIPを読み込んでいません");
    }
    if report
        .get("secure_field_internal_ai_or_learning_verified")
        .and_then(Value::as_bool)
        != Some(false)
    {
        bail!("保護欄の内部AI/学習を、この表示試験で検証済みにしてはいけません");
    }
    if report
        .get("password_scope_learning_verified")
        .and_then(Value::as_bool)
        != Some(true)
    {
        bail!("IS_PASSWORD の欄で学習が止まることを確かめていません");
    }
    Ok(())
}

/// 二重入力ストレステストレポートの全試行完了を検証する。
pub fn validate_duplicate_stress(report: &Value) -> Result<()> {
    let stress = &report["duplicate_stress"];
    let Some(trials) = stress.get("trials").and_then(Value::as_array) else {
        bail!("二重入力の実TSF検査が未完了: {stress}");
    };
    let mut expected: HashSet<(&str, &str, i64)> = HashSet::new();
    for host in ["EDIT", "RichEdit"] {
        for mode in ["space_enter", "enter", "down_enter", "focus"] {
            for delay in [0, 30, 250] {
                expected.insert((host, mode, delay));
            }
        }
        for mode in ["click", "intentional_repeat"] {
            expected.insert((host, mode, 0));
        }
    }
    let actual: HashSet<(&str, &str, i64)> = trials
        .iter()
        .filter_map(|row| {
            Some((
                row.get("host")?.as_str()?,
                row.get("mode")?.as_str()?,
                row.get("space_to_action_ms")
                    .and_then(Value::as_i64)
                    .unwrap_or(0),
            ))
        })
        .collect();
    let all_passed = trials
        .iter()
        .all(|r| r.get("passed").and_then(Value::as_bool) == Some(true));
    if stress.get("complete").and_then(Value::as_bool) != Some(true)
        || trials.len() != expected.len()
        || actual != expected
        || !all_passed
    {
        bail!("二重入力の実TSF検査が未完了: {stress}");
    }
    Ok(())
}

/// `ImeBench.exe` を実行して `日本語` / `東京` の実入力確定を検証する。
pub fn verify_input(exe: &Path, cases: &Path, output: &Path) -> Result<HashMap<String, String>> {
    let exe_str = exe.to_string_lossy().into_owned();
    let cases_str = cases.to_string_lossy().into_owned();
    let out_str = output.to_string_lossy().into_owned();
    run_checked(
        &exe_str,
        &[
            "{2A1ADAE4-8061-4AC0-A7D4-713BF8DCB591}",
            "{455A87E7-275F-4582-92CB-6600917A436D}",
            &cases_str,
            &out_str,
            "3000",
        ],
    )?;
    let err_path = PathBuf::from(format!("{}.error.txt", output.display()));
    if err_path.exists() {
        bail!("{}", fs::read_to_string(&err_path)?);
    }
    let text = fs::read_to_string(output)?;
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
    let mut got = HashMap::new();
    for line in text.lines() {
        if let Some((k, v)) = line.split_once('\t') {
            got.insert(k.to_owned(), v.to_owned());
        }
    }
    if got.get("0").map(String::as_str) != Some("日本語")
        || got.get("1").map(String::as_str) != Some("東京")
    {
        bail!("実入力の結果が一致しません: {got:?}");
    }
    Ok(got)
}

/// `ImeAcceptance.exe` をビルド・実行しレポートを検証する。
pub fn run_acceptance(
    compiler: &Path,
    source: &Path,
    install: &Path,
    out: &Path,
    duplicate_stress: bool,
) -> Result<Value> {
    let exe = out.join("ImeAcceptance.exe");
    let output = out.join("acceptance.json");
    let csc = compiler.to_string_lossy().into_owned();
    let out_arg = format!("/out:{}", exe.display());
    let src1 = source.to_string_lossy().into_owned();
    let src2 = source
        .with_file_name("ImeAcceptance.cs")
        .to_string_lossy()
        .into_owned();
    let src3 = source
        .with_file_name("ImeDuplicateStress.cs")
        .to_string_lossy()
        .into_owned();
    run_checked(
        &csc,
        &[
            "/nologo",
            "/target:winexe",
            "/platform:x64",
            "/main:Acceptance",
            "/reference:System.Windows.Forms.dll",
            "/reference:System.Drawing.dll",
            "/reference:System.Web.Extensions.dll",
            &out_arg,
            &src1,
            &src2,
            &src3,
        ],
    )?;
    let mut cmd = Command::new(&exe);
    cmd.arg(install).arg(&output);
    if duplicate_stress {
        cmd.arg("--duplicate-stress");
    }
    let proc_out = cmd.output().context("ImeAcceptance 実行失敗")?;
    if !output.is_file() {
        bail!(
            "実入力レポートがありません: exit {:?}",
            proc_out.status.code()
        );
    }
    let report: Value = serde_json::from_str(&fs::read_to_string(&output)?)?;
    validate_acceptance(&report, install)?;
    if duplicate_stress {
        validate_duplicate_stress(&report)?;
    }
    if !proc_out.status.success() {
        bail!("実入力検証の終了コード: {:?}", proc_out.status.code());
    }
    Ok(report)
}

/// Edge の二重入力プローブ（`browser_duplicate_probe.py` は Playwright 依存のため外部起動）。
pub fn run_browser_duplicate_probe(
    compiler: &Path,
    source: &Path,
    install: &Path,
    out: &Path,
    repo_root: &Path,
) -> Result<Value> {
    let exe = out.join("BrowserDuplicateDriver.exe");
    let csc = compiler.to_string_lossy().into_owned();
    let out_arg = format!("/out:{}", exe.display());
    let src1 = source.to_string_lossy().into_owned();
    let src2 = source
        .with_file_name("ImeAcceptance.cs")
        .to_string_lossy()
        .into_owned();
    let src3 = source
        .with_file_name("ImeDuplicateStress.cs")
        .to_string_lossy()
        .into_owned();
    let src4 = source
        .with_file_name("BrowserDuplicateDriver.cs")
        .to_string_lossy()
        .into_owned();
    run_checked(
        &csc,
        &[
            "/nologo",
            "/target:exe",
            "/platform:x64",
            "/main:BrowserDuplicateDriver",
            "/reference:System.Windows.Forms.dll",
            "/reference:System.Drawing.dll",
            "/reference:System.Web.Extensions.dll",
            &out_arg,
            &src1,
            &src2,
            &src3,
            &src4,
        ],
    )?;
    let destination = out.join("browser-duplicate");
    let probe = repo_root.join("mozc/tools/browser_duplicate_probe.py");
    run_checked(
        "python",
        &[
            "-X",
            "utf8",
            &probe.to_string_lossy(),
            "--driver",
            &exe.to_string_lossy(),
            "--install",
            &install.to_string_lossy(),
            "--out",
            &destination.to_string_lossy(),
        ],
    )?;
    let report: Value =
        serde_json::from_str(&fs::read_to_string(destination.join("summary.json"))?)?;
    let trials = report["trials"].as_array();
    let ok = report.get("complete").and_then(Value::as_bool) == Some(true)
        && trials.is_some_and(|arr| {
            arr.len() == 14
                && arr
                    .iter()
                    .all(|r| r.get("passed").and_then(Value::as_bool) == Some(true))
        });
    if !ok {
        bail!("Edgeの二重入力試験が未完了");
    }
    Ok(report)
}

#[derive(Debug, Clone)]
pub struct SmokeOpts {
    pub msi: PathBuf,
    pub out: PathBuf,
    pub acceptance: bool,
    pub duplicate_stress: bool,
    pub browser_duplicate_stress: bool,
    pub reinstall: bool,
}

/// `installer_smoke` メイン処理（使い捨て GitHub Actions Windows ランナー専用）。
pub fn run_installer_smoke(opts: &SmokeOpts, repo_root: &Path) -> Result<i32> {
    if !cfg!(windows)
        || env::var("GITHUB_ACTIONS").as_deref() != Ok("true")
        || env::var("ASTERIA_DISPOSABLE_RUNNER").as_deref() != Ok("github-hosted")
    {
        bail!("使い捨てのGitHub Actions Windowsランナーでのみ実行できます");
    }
    let program_files_x86 = PathBuf::from(env::var("ProgramFiles(x86)")?);
    let program_files_x64 = env::var("ProgramFiles")
        .map(PathBuf::from)
        .unwrap_or_else(|_| program_files_x86.clone());
    if program_files_x86.join("Asteria").exists() || program_files_x64.join("Asteria").exists() {
        bail!("既存のAsteriaがあるので停止します");
    }
    let mut install = program_files_x64.join("Asteria");
    let msi_raw = fs::canonicalize(&opts.msi)?;
    let msi = PathBuf::from(msi_raw.to_string_lossy().trim_start_matches(r"\\?\"));
    if !msi.is_file() {
        bail!("MSI が見つかりません: {}", msi.display());
    }
    fs::create_dir_all(&opts.out)?;
    let out_raw = fs::canonicalize(&opts.out)?;
    let out = PathBuf::from(out_raw.to_string_lossy().trim_start_matches(r"\\?\"));
    let source = repo_root.join("eval/imebench/ImeBench.cs");
    let digest = sha256_file(&msi)?;
    let product_code = msi_property(&msi, "ProductCode")?;
    let product_version = msi_property(&msi, "ProductVersion")?;

    let mut result = json!({
        "msi": msi.file_name().and_then(|n| n.to_str()).unwrap_or(""),
        "sha256": digest,
        "source_commit": env::var("GITHUB_SHA").ok(),
        "run_id": env::var("GITHUB_RUN_ID").ok(),
        "install": false,
        "input": false,
        "uninstall": false,
        "product_code": product_code,
        "product_version": product_version,
    });
    let mut errors: Vec<String> = Vec::new();
    let checkpoint = |res: &mut Value, errs: &[String], stage: &str| {
        res["stage"] = json!(stage);
        res["errors"] = json!(errs);
        if let Ok(text) = serde_json::to_string_pretty(res) {
            let _ = fs::write(out.join("result.json"), &text);
            println!("{text}");
        }
    };

    let inner = (|| -> Result<()> {
        checkpoint(&mut result, &errors, "installing");
        let msi_s = msi.to_string_lossy().into_owned();
        let code = run_msi(&["/i", &msi_s], &out.join("install.log"), false)?;
        result["install_exit"] = json!(code);
        if code != 0 {
            bail!("導入の終了コード: {code}（3010は再起動が必要）");
        }
        install = install_dir();
        let installed: Vec<(&str, PathBuf)> = if install.join("asteria-server.exe").is_file() {
            vec![
                ("asteria-server.exe", install.join("asteria-server.exe")),
                ("asteria-renderer.exe", install.join("asteria-renderer.exe")),
                ("asteria_tip.dll", tip_path(&install, "64")),
            ]
        } else {
            vec![
                ("mozc_server.exe", install.join("mozc_server.exe")),
                ("mozc_renderer.exe", install.join("mozc_renderer.exe")),
                ("mozc_tip64.dll", tip_path(&install, "64")),
            ]
        };
        for (name, path) in &installed {
            if !path.is_file() {
                bail!("導入後のファイルがありません: {name}");
            }
        }
        result["install"] = json!(true);
        let mut hashes = serde_json::Map::new();
        for (name, path) in &installed {
            hashes.insert((*name).to_owned(), json!(sha256_file(path)?));
        }
        result["installed_sha256"] = Value::Object(hashes);
        checkpoint(&mut result, &errors, "installed");

        let exe = out.join("ImeBench.exe");
        let windir = PathBuf::from(env::var("WINDIR")?);
        let compiler = windir.join("Microsoft.NET/Framework64/v4.0.30319/csc.exe");
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
        let cases = out.join("input.tsv");
        fs::write(&cases, "warm\tnihonngo\n0\tnihonngo\n1\ttoukyou\n")?;
        let output = out.join("output.tsv");
        result["committed_text"] = json!(verify_input(&exe, &cases, &output)?);
        result["input"] = json!(true);
        checkpoint(&mut result, &errors, "input_verified");

        if opts.acceptance || opts.duplicate_stress {
            result["acceptance"] =
                run_acceptance(&compiler, &source, &install, &out, opts.duplicate_stress)?;
            checkpoint(&mut result, &errors, "acceptance_verified");
        }
        if opts.browser_duplicate_stress {
            result["browser_duplicate"] =
                run_browser_duplicate_probe(&compiler, &source, &install, &out, repo_root)?;
            checkpoint(&mut result, &errors, "browser_duplicate_verified");
        }
        if opts.reinstall {
            stop_installed_processes(&install)?;
            let code = run_msi(
                &["/x", &product_code],
                &out.join("before-reinstall-uninstall.log"),
                false,
            )?;
            result["before_reinstall_uninstall_exit"] = json!(code);
            result["before_reinstall_reboot_required"] = json!(code == 3010);
            if (code != 0 && code != 3010) || tip_path(&install, "64").exists() {
                bail!("再導入前の削除に失敗: {code}");
            }
            checkpoint(&mut result, &errors, "reinstalling");
            let code = run_msi(&["/i", &msi_s], &out.join("reinstall.log"), false)?;
            result["reinstall_exit"] = json!(code);
            if code != 0 {
                bail!("再導入の終了コード: {code}");
            }
            if let Some(obj) = result["installed_sha256"].as_object() {
                for (name, exp) in obj {
                    let path = if name == "mozc_tip64.dll" || name == "asteria_tip.dll" {
                        tip_path(&install, "64")
                    } else {
                        install.join(name)
                    };
                    if Some(sha256_file(&path)?.as_str()) != exp.as_str() {
                        bail!("再導入後のファイルが違います: {name}");
                    }
                }
            }
            result["reinstalled_committed_text"] = json!(verify_input(
                &exe,
                &cases,
                &out.join("reinstalled-output.tsv")
            )?);
            result["reinstall"] = json!(true);
            result["reinstall_scope"] = json!(
                "same MSI, same boot; reboot recovery and previous-version upgrade are unverified"
            );
            checkpoint(&mut result, &errors, "reinstall_verified");
        }
        Ok(())
    })();

    if let Err(err) = inner {
        errors.push(err.to_string());
    }

    checkpoint(&mut result, &errors, "uninstalling");
    let cleanup = (|| -> Result<()> {
        stop_installed_processes(&install)?;
        let code = run_msi(&["/x", &product_code], &out.join("uninstall.log"), false)?;
        result["uninstall_exit"] = json!(code);
        result["reboot_required"] = json!(code == 3010);
        let uninstalled = (code == 0 || code == 3010) && !tip_path(&install, "64").exists();
        result["uninstall"] = json!(uninstalled);
        if !uninstalled {
            errors.push("アンインストールの完了を確認できません".into());
        }
        Ok(())
    })();
    if let Err(err) = cleanup {
        errors.push(format!("削除確認: {err}"));
    }
    checkpoint(&mut result, &errors, "finished");
    Ok(if errors.is_empty() { 0 } else { 1 })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn sample_report(install: &Path) -> Value {
        let checks: Vec<Value> = ACCEPTANCE_CHECKS
            .iter()
            .map(|&name| json!({"name": name, "passed": true}))
            .collect();
        json!({
            "complete": true,
            "native_tsf": true,
            "loaded_tip": install.join("mozc_tip64.dll").to_string_lossy(),
            "secure_field_internal_ai_or_learning_verified": false,
            "password_scope_learning_verified": true,
            "checks": checks,
        })
    }

    #[test]
    fn acceptance_receipt_validation() {
        let dir = TempDir::new().unwrap();
        let install = dir.path().join("install");
        fs::create_dir_all(&install).unwrap();
        let report = sample_report(&install);
        validate_acceptance(&report, &install).unwrap();

        let mut versioned = report.clone();
        versioned["loaded_tip"] = json!(install
            .join("mozc_tip64-3.34.6448.100.dll")
            .to_string_lossy());
        validate_acceptance(&versioned, &install).unwrap();

        for bad_tip in [
            dir.path().join("mozc_tip64-3.34.6448.100.dll"),
            install.join("mozc_tip64-3.34.6448.100.dll.3.34.6448.100-1.asteria-old"),
            install.join("other.dll"),
        ] {
            let mut r = report.clone();
            r["loaded_tip"] = json!(bad_tip.to_string_lossy());
            assert!(validate_acceptance(&r, &install).is_err());
        }

        let mut sec_bad = report.clone();
        sec_bad["secure_field_internal_ai_or_learning_verified"] = json!(true);
        assert!(validate_acceptance(&sec_bad, &install).is_err());

        let mut pw_bad = report.clone();
        pw_bad["password_scope_learning_verified"] = json!(false);
        assert!(validate_acceptance(&pw_bad, &install).is_err());
    }

    #[test]
    fn duplicate_stress_requires_all_28_trials() {
        let mut rows = Vec::new();
        for host in ["EDIT", "RichEdit"] {
            for mode in ["space_enter", "enter", "down_enter", "focus"] {
                for delay in [0, 30, 250] {
                    rows.push(json!({"host": host, "mode": mode, "space_to_action_ms": delay, "passed": true}));
                }
            }
            for mode in ["click", "intentional_repeat"] {
                rows.push(json!({"host": host, "mode": mode, "passed": true}));
            }
        }
        let report = json!({"duplicate_stress": {"complete": true, "trials": rows}});
        validate_duplicate_stress(&report).unwrap();

        let mut partial = report.clone();
        partial["duplicate_stress"]["trials"]
            .as_array_mut()
            .unwrap()
            .pop();
        assert!(validate_duplicate_stress(&partial).is_err());

        let mut failed = report.clone();
        failed["duplicate_stress"]["trials"][0]["passed"] = json!(false);
        assert!(validate_duplicate_stress(&failed).is_err());
    }
}
