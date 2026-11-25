//! ローカル PC での段階的候補 MSI 更新・実入力・復帰検査（`mozc/tools/local_install_check.py` 移植）。

use std::collections::{BTreeMap, HashSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{bail, Context, Result};
use serde_json::{json, Map, Value};

use asteria_common::hash::{set_writable, sha256_file};
use asteria_common::paths::{install_dir, profile_dir, tip_path};

use crate::check_release::msi_property;
use crate::installer_smoke::{
    msi_dialogs, run_acceptance, run_checked, run_msi, stop_installed_processes,
    validate_acceptance, verify_input,
};

pub const WORD_CASES: &[(&str, &str, &str)] = &[
    ("w1", "nihongo", "日本語"),
    ("w2", "toukyou", "東京"),
    ("w3", "arigatougozaimasu", "ありがとうございます"),
    ("w4", "kaiginoshiryouwookurimasu", "会議の資料を送ります"),
    ("w5", "kyouhaiitenkidesune", "今日はいい天気ですね"),
];

pub fn core_paths(directory: &Path) -> BTreeMap<String, PathBuf> {
    let mut map = BTreeMap::new();
    map.insert("mozc_server.exe".into(), directory.join("mozc_server.exe"));
    map.insert(
        "mozc_renderer.exe".into(),
        directory.join("mozc_renderer.exe"),
    );
    map.insert("mozc_tip64.dll".into(), tip_path(directory, "64"));
    map.insert("mozc_tip32.dll".into(), tip_path(directory, "32"));
    map
}

pub fn is_runtime_file(path: &Path) -> bool {
    let ext = path.extension().and_then(|e| e.to_str());
    let name = path.file_name().and_then(|n| n.to_str());
    matches!(ext, Some("ipc" | "lock")) || name == Some("asteria_ai_status.txt")
}

/// プロファイル配下の非ランタイムファイルの相対パス → SHA-256 マップを返す。
pub fn tree_hashes(root: &Path) -> Result<BTreeMap<String, String>> {
    let mut out = BTreeMap::new();
    if !root.is_dir() {
        return Ok(out);
    }
    collect_hashes(root, root, &mut out)?;
    Ok(out)
}

fn collect_hashes(base: &Path, dir: &Path, out: &mut BTreeMap<String, String>) -> Result<()> {
    let mut entries: Vec<PathBuf> = fs::read_dir(dir)?
        .filter_map(|e| e.ok().map(|x| x.path()))
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            collect_hashes(base, &path, out)?;
        } else if path.is_file() && !is_runtime_file(&path) {
            let rel = path
                .strip_prefix(base)?
                .to_string_lossy()
                .replace('\\', "/");
            out.insert(rel, sha256_file(&path)?);
        }
    }
    Ok(())
}

/// ランタイムファイルを除外してディレクトリツリーを複製する。
pub fn copy_profile(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        if is_runtime_file(&path) {
            continue;
        }
        let target = dst.join(entry.file_name());
        if path.is_dir() {
            copy_profile(&path, &target)?;
        } else if path.is_file() {
            if target.exists() {
                let _ = set_writable(&target);
            }
            fs::copy(&path, &target)?;
        }
    }
    Ok(())
}

fn remove_tree_writable(root: &Path) -> Result<()> {
    if !root.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            remove_tree_writable(&path)?;
        } else {
            let _ = set_writable(&path);
            fs::remove_file(&path)?;
        }
    }
    fs::remove_dir(root)?;
    Ok(())
}

fn remove_non_runtime_files(dir: &Path) -> Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            remove_non_runtime_files(&path)?;
        } else if path.is_file() && !is_runtime_file(&path) {
            let _ = set_writable(&path);
            fs::remove_file(&path)?;
        }
    }
    Ok(())
}

/// プロファイルをスナップショットの状態へ戻す。
pub fn restore_profile(snapshot: &Path, prof: &Path) -> Result<()> {
    if prof.is_dir() {
        remove_non_runtime_files(prof)?;
    }
    if snapshot.is_dir() {
        copy_profile(snapshot, prof)?;
    }
    Ok(())
}

/// 状態ファイル (`state.json`) 管理。
pub struct RunState {
    pub out: PathBuf,
    pub path: PathBuf,
    pub state: Value,
}

impl RunState {
    pub fn open(out: &Path) -> Result<Self> {
        fs::create_dir_all(out)?;
        let out = fs::canonicalize(out).unwrap_or_else(|_| out.to_path_buf());
        let path = out.join("state.json");
        let state = if path.exists() {
            serde_json::from_str(&fs::read_to_string(&path)?)?
        } else {
            json!({})
        };
        Ok(Self { out, path, state })
    }

    pub fn save(&mut self, stage: &str, values: Map<String, Value>) -> Result<()> {
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let mut entry = Map::new();
        entry.insert("stage".into(), json!(stage));
        entry.insert("at_unix".into(), json!(secs));
        for (k, v) in &values {
            entry.insert(k.clone(), v.clone());
        }
        if !self.state["stages"].is_array() {
            self.state["stages"] = json!([]);
        }
        if let Some(arr) = self.state["stages"].as_array_mut() {
            arr.push(Value::Object(entry));
        }
        if let Some(obj) = self.state.as_object_mut() {
            for (k, v) in &values {
                obj.insert(k.clone(), v.clone());
            }
        }
        fs::create_dir_all(&self.out)?;
        let text = serde_json::to_string_pretty(&self.state)?;
        fs::write(&self.path, text)?;
        let mut printed = Map::new();
        printed.insert("stage".into(), json!(stage));
        for (k, v) in values {
            printed.insert(k, v);
        }
        println!("{}", serde_json::to_string_pretty(&Value::Object(printed))?);
        Ok(())
    }

    pub fn require(&self, stage: &str) -> Result<()> {
        let done: HashSet<&str> = self.state["stages"]
            .as_array()
            .map(|arr| arr.iter().filter_map(|s| s["stage"].as_str()).collect())
            .unwrap_or_default();
        if !done.contains(stage) {
            bail!("先に {stage} を実行してください");
        }
        Ok(())
    }
}

/// 検査ブロック実行中にプロファイルを退避し、成否に関わらず終了時に完全一致へ戻す。
pub fn with_profile_restored<F, R>(
    run: &mut RunState,
    stage: &str,
    prof: &Path,
    stop_fn: &dyn Fn() -> Result<()>,
    restore_fn: &dyn Fn(&Path, &Path) -> Result<()>,
    body: F,
) -> Result<R>
where
    F: FnOnce(&mut RunState) -> Result<R>,
{
    stop_fn()?;
    let snapshot = run.out.join(format!("profile-before-{stage}"));
    remove_tree_writable(&snapshot)?;
    if prof.is_dir() {
        copy_profile(prof, &snapshot)?;
    } else {
        fs::create_dir_all(&snapshot)?;
    }
    let expected = tree_hashes(&snapshot)?;
    let res = body(run);

    let _ = stop_fn();
    let restore_res = restore_fn(&snapshot, prof);
    let same = tree_hashes(prof)? == expected;
    let mut vals = Map::new();
    vals.insert("profile_restored".into(), json!(same));
    vals.insert("profile_files".into(), json!(expected.len()));
    run.save(&format!("{stage}_profile"), vals)?;
    restore_res?;
    if !same {
        bail!(
            "{stage}の後に学習履歴を元へ戻せませんでした（保存先 {}）",
            snapshot.display()
        );
    }
    res
}

fn powershell_json(script: &str) -> Result<Value> {
    let out = Command::new("powershell")
        .args(["-NoProfile", "-Command", script])
        .output()
        .context("powershell 実行失敗")?;
    if !out.status.success() {
        bail!(
            "PowerShell exit {:?}: {}",
            out.status.code(),
            String::from_utf8_lossy(&out.stderr)
        );
    }
    let text = String::from_utf8_lossy(&out.stdout).trim().to_owned();
    if text.is_empty() {
        return Ok(json!([]));
    }
    let val: Value = serde_json::from_str(&text)?;
    Ok(if val.is_array() { val } else { json!([val]) })
}

fn installed_products() -> Result<Value> {
    powershell_json(
        "[Console]::OutputEncoding=[Text.Encoding]::UTF8;\
         Get-ItemProperty HKLM:\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\*,\
         HKLM:\\SOFTWARE\\WOW6432Node\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\* -ErrorAction SilentlyContinue \
         | Where-Object { $_.DisplayName -match 'Asteria|Asteria' } \
         | ForEach-Object { [pscustomobject]@{code=$_.PSChildName; name=$_.DisplayName; version=$_.DisplayVersion} } \
         | ConvertTo-Json -Compress",
    )
}

fn language_list() -> Result<Value> {
    powershell_json(
        "[Console]::OutputEncoding=[Text.Encoding]::UTF8;\
         Get-WinUserLanguageList | ForEach-Object { [pscustomobject]@{tag=$_.LanguageTag; tips=@($_.InputMethodTips)} } \
         | ConvertTo-Json -Compress -Depth 3",
    )
}

fn core_hashes(install: &Path) -> Result<BTreeMap<String, String>> {
    let mut out = BTreeMap::new();
    for (name, path) in core_paths(install) {
        if path.is_file() {
            out.insert(name, sha256_file(&path)?);
        }
    }
    Ok(out)
}

#[derive(Debug, Clone)]
pub struct LocalInstallOpts {
    pub stage: String,
    pub out: PathBuf,
    pub candidate: Option<PathBuf>,
    pub rollback: Option<PathBuf>,
    pub expected_dir: Option<PathBuf>,
    pub restore_profile_flag: bool,
    pub skip_click: bool,
    pub python: Option<String>,
    pub ui: bool,
}

pub fn run_local_install_check(opts: &LocalInstallOpts, repo_root: &Path) -> Result<()> {
    if !cfg!(windows) {
        bail!("Windows専用です");
    }
    let install = install_dir();
    let prof = profile_dir();
    let mut run = RunState::open(&opts.out)?;

    match opts.stage.as_str() {
        "precheck" => {
            if run.path.exists() {
                bail!(
                    "既に記録があります。新しい --out を使ってください: {}",
                    run.path.display()
                );
            }
            let (Some(cand), Some(rb), Some(exp)) =
                (&opts.candidate, &opts.rollback, &opts.expected_dir)
            else {
                bail!("precheckには --candidate --rollback --expected-dir が必要です");
            };
            let products = installed_products()?;
            let arr = products
                .as_array()
                .context("products が配列ではありません")?;
            if arr.len() != 1 {
                bail!("導入済みAsteriaがちょうど1件ではありません: {products}");
            }
            let cand = fs::canonicalize(cand)?;
            let rb = fs::canonicalize(rb)?;
            let exp = fs::canonicalize(exp)?;
            let cand_code = msi_property(&cand, "ProductCode")?;
            let rb_code = msi_property(&rb, "ProductCode")?;
            let installed_code = arr[0]["code"].as_str().unwrap_or("");
            if rb_code != installed_code {
                bail!("復帰用MSIが導入済みの版と一致しません: {rb_code} != {installed_code}");
            }
            if cand_code == installed_code {
                bail!("候補MSIが導入済みと同じProductCodeです");
            }
            let mut expected_core = BTreeMap::new();
            for (name, path) in core_paths(&exp) {
                expected_core.insert(name, sha256_file(&path)?);
            }
            let mut vals = Map::new();
            vals.insert("baseline_product".into(), arr[0].clone());
            vals.insert(
                "msi".into(),
                json!({
                    "candidate": {
                        "path": cand.to_string_lossy(),
                        "sha256": sha256_file(&cand)?,
                        "product_code": cand_code,
                        "product_version": msi_property(&cand, "ProductVersion")?,
                    },
                    "rollback": {
                        "path": rb.to_string_lossy(),
                        "sha256": sha256_file(&rb)?,
                        "product_code": rb_code,
                        "product_version": msi_property(&rb, "ProductVersion")?,
                    },
                }),
            );
            vals.insert("expected_dir".into(), json!(exp.to_string_lossy()));
            vals.insert("expected_core".into(), json!(expected_core));
            vals.insert("baseline_core".into(), json!(core_hashes(&install)?));
            vals.insert("baseline_profile".into(), json!(tree_hashes(&prof)?));
            vals.insert("baseline_languages".into(), language_list()?);
            run.save("precheck", vals)?;
        }
        "install" => {
            run.require("precheck")?;
            let backup = run.out.join("profile-backup");
            if !backup.exists() {
                copy_profile(&prof, &backup)?;
            }
            let mut bvals = Map::new();
            bvals.insert("backup_profile".into(), json!(tree_hashes(&backup)?));
            run.save("profile_backup", bvals)?;
            let msi = PathBuf::from(
                run.state["msi"]["candidate"]["path"]
                    .as_str()
                    .context("candidate path")?,
            );
            if Some(sha256_file(&msi)?.as_str()) != run.state["msi"]["candidate"]["sha256"].as_str()
            {
                bail!("候補MSIがprecheck後に変わりました");
            }
            let log = run.out.join("install.log");
            let code = run_msi(&["/i", &msi.to_string_lossy()], &log, opts.ui)?;
            let mut vals = Map::new();
            vals.insert("ui".into(), json!(opts.ui));
            if opts.ui {
                vals.insert("dialogs".into(), json!(msi_dialogs(&log)));
            }
            vals.insert("install_exit".into(), json!(code));
            if code != 0 && code != 3010 {
                run.save("install_failed", vals)?;
                bail!("導入の終了コード: {code}");
            }
            vals.insert("install_reboot_required".into(), json!(code == 3010));
            vals.insert("products_after_install".into(), installed_products()?);
            run.save("install", vals)?;
        }
        "verify" => {
            run.require("install")?;
            let products = installed_products()?;
            let want_code = run.state["msi"]["candidate"]["product_code"]
                .as_str()
                .unwrap_or("");
            let codes: Vec<&str> = products
                .as_array()
                .map(|a| a.iter().filter_map(|p| p["code"].as_str()).collect())
                .unwrap_or_default();
            let mut errors = Vec::new();
            if codes != [want_code] {
                errors.push(format!("登録が候補1件ではありません: {products}"));
            }
            let core = core_hashes(&install)?;
            if json!(core) != run.state["expected_core"] {
                errors.push(format!("導入ファイルが候補と一致しません: {}", json!(core)));
            }
            if !errors.is_empty() {
                let mut fvals = Map::new();
                fvals.insert("verify_errors".into(), json!(errors));
                run.save("verify_failed", fvals)?;
                bail!("{}", errors.join("\n"));
            }
            let inst_clone = install.clone();
            let stop = move || stop_installed_processes(&inst_clone);
            with_profile_restored(&mut run, "verify", &prof, &stop, &restore_profile, |r| {
                let compiler = PathBuf::from(env::var("WINDIR")?)
                    .join("Microsoft.NET/Framework64/v4.0.30319/csc.exe");
                let source = repo_root.join("eval/imebench/ImeBench.cs");
                let exe = r.out.join("ImeBench.exe");
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
                let cases = r.out.join("input.tsv");
                fs::write(&cases, "warm\tnihonngo\n0\tnihonngo\n1\ttoukyou\n")?;
                let committed = verify_input(&exe, &cases, &r.out.join("output.tsv"))?;
                let mut ivals = Map::new();
                ivals.insert("committed_text".into(), json!(committed));
                r.save("input", ivals)?;
                env::set_var("ASTERIA_LOCAL_ACCEPTANCE", "owner-approved");
                if opts.skip_click {
                    env::set_var("ASTERIA_SKIP_CLICK", "environment");
                }
                let report = run_acceptance(&compiler, &source, &install, &r.out, true)?;
                validate_acceptance(&report, &install)?;
                let mut vvals = Map::new();
                vvals.insert(
                    "acceptance_checks".into(),
                    json!(report["checks"].as_array().map(|a| a.len()).unwrap_or(0)),
                );
                vvals.insert(
                    "scope".into(),
                    json!("this PC, upgrade from installed baseline; EDIT/RichEdit only"),
                );
                r.save("verify", vvals)?;
                Ok(())
            })?;
        }
        "word" => {
            run.require("install")?;
            let inst_clone = install.clone();
            let stop = move || stop_installed_processes(&inst_clone);
            with_profile_restored(&mut run, "word", &prof, &stop, &restore_profile, |r| {
                let compiler = PathBuf::from(env::var("WINDIR")?)
                    .join("Microsoft.NET/Framework64/v4.0.30319/csc.exe");
                let src = repo_root.join("eval/imebench/WordInputProbe.cs");
                let exe = r.out.join("WordInputProbe.exe");
                let output = r.out.join("word-input.json");
                let shots = r.out.join("word-shots");
                run_checked(
                    &compiler.to_string_lossy(),
                    &[
                        "/nologo",
                        "/target:winexe",
                        "/platform:x64",
                        "/reference:System.Windows.Forms.dll",
                        "/reference:System.Drawing.dll",
                        "/reference:System.Web.Extensions.dll",
                        "/reference:Microsoft.CSharp.dll",
                        &format!("/out:{}", exe.display()),
                        &src.to_string_lossy(),
                    ],
                )?;
                let cases = r.out.join("word-cases.tsv");
                let mut tsv = String::new();
                for (id, romaji, expected) in WORD_CASES {
                    tsv.push_str(&format!("{id}\t{romaji}\t{expected}\n"));
                }
                fs::write(&cases, tsv)?;
                env::set_var("ASTERIA_LOCAL_ACCEPTANCE", "owner-approved");
                let proc_out = Command::new(&exe)
                    .args([&cases, &output, &shots])
                    .output()?;
                let report: Value = if output.is_file() {
                    serde_json::from_str(&fs::read_to_string(&output)?)?
                } else {
                    json!({})
                };
                let ok = proc_out.status.success()
                    && report["complete"].as_bool() == Some(true)
                    && report["passed"].as_u64() == Some(WORD_CASES.len() as u64);
                let mut wvals = Map::new();
                wvals.insert("word_passed".into(), report["passed"].clone());
                wvals.insert("word_trials".into(), report["trials"].clone());
                r.save(if ok { "word" } else { "word_failed" }, wvals)?;
                if !ok {
                    bail!("Wordの実入力が未完了");
                }
                Ok(())
            })?;
        }
        "rollback" => {
            run.require("precheck")?;
            let cand_code = run.state["msi"]["candidate"]["product_code"]
                .as_str()
                .unwrap_or("")
                .to_owned();
            let rb_code = run.state["msi"]["rollback"]["product_code"]
                .as_str()
                .unwrap_or("")
                .to_owned();
            let products = installed_products()?;
            let has_cand = products.as_array().is_some_and(|a| {
                a.iter()
                    .any(|p| p["code"].as_str() == Some(cand_code.as_str()))
            });
            if has_cand {
                stop_installed_processes(&install)?;
                let code = run_msi(&["/x", &cand_code], &run.out.join("uninstall.log"), false)?;
                if code != 0 && code != 3010 {
                    bail!("候補の削除に失敗: {code}");
                }
                let mut vals = Map::new();
                vals.insert("uninstall_exit".into(), json!(code));
                vals.insert("uninstall_reboot_required".into(), json!(code == 3010));
                run.save("uninstall", vals)?;
            }
            let products = installed_products()?;
            let has_rb = products.as_array().is_some_and(|a| {
                a.iter()
                    .any(|p| p["code"].as_str() == Some(rb_code.as_str()))
            });
            if !has_rb {
                let rb_msi = PathBuf::from(
                    run.state["msi"]["rollback"]["path"]
                        .as_str()
                        .context("rb path")?,
                );
                if Some(sha256_file(&rb_msi)?.as_str())
                    != run.state["msi"]["rollback"]["sha256"].as_str()
                {
                    bail!("復帰用MSIがprecheck後に変わりました");
                }
                let code = run_msi(
                    &["/i", &rb_msi.to_string_lossy()],
                    &run.out.join("rollback-install.log"),
                    false,
                )?;
                if code != 0 && code != 3010 {
                    bail!("元の版の導入に失敗: {code}");
                }
                let mut vals = Map::new();
                vals.insert("rollback_install_exit".into(), json!(code));
                vals.insert("rollback_reboot_required".into(), json!(code == 3010));
                run.save("rollback_install", vals)?;
            }
            let mut restored_profile = false;
            let backup = run.out.join("profile-backup");
            if opts.restore_profile_flag && backup.exists() {
                stop_installed_processes(&install)?;
                restore_profile(&backup, &prof)?;
                restored_profile = json!(tree_hashes(&prof)?) == run.state["backup_profile"];
            }
            let mut vals = Map::new();
            vals.insert("profile_restored".into(), json!(restored_profile));
            run.save("rollback", vals)?;
        }
        "restored" => {
            run.require("rollback")?;
            let products = installed_products()?;
            let codes: Vec<&str> = products
                .as_array()
                .map(|a| a.iter().filter_map(|p| p["code"].as_str()).collect())
                .unwrap_or_default();
            let base_code = run.state["baseline_product"]["code"].as_str().unwrap_or("");
            let backup_kept = run.state["backup_profile"]
                .as_object()
                .map_or(true, |m| m.keys().all(|k| prof.join(k).is_file()));
            let checks = json!({
                "product": codes == [base_code],
                "core_files": json!(core_hashes(&install)?) == run.state["baseline_core"],
                "profile_files_kept": backup_kept,
                "languages": language_list()? == run.state["baseline_languages"],
            });
            let all_ok = checks
                .as_object()
                .is_some_and(|m| m.values().all(|v| v.as_bool() == Some(true)));
            let mut vals = Map::new();
            vals.insert("restored_checks".into(), checks.clone());
            vals.insert("restored_all".into(), json!(all_ok));
            run.save("restored", vals)?;
            if !all_ok {
                bail!("元の状態と違う項目があります: {checks}");
            }
        }
        other => bail!("未対応の段階です: {other}"),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn profile_is_restored_even_with_readonly_and_errors() {
        let dir = TempDir::new().unwrap();
        let prof = dir.path().join("Asteria");
        fs::create_dir_all(prof.join("sub")).unwrap();
        let hist = prof.join("history.db");
        fs::write(&hist, b"user history").unwrap();
        fs::write(prof.join("sub/segment.db"), b"segments").unwrap();
        let mut perms = fs::metadata(&hist).unwrap().permissions();
        perms.set_readonly(true);
        fs::set_permissions(&hist, perms).unwrap();
        fs::write(prof.join("session.ipc"), b"runtime").unwrap();

        let before = tree_hashes(&prof).unwrap();
        let mut run = RunState::open(&dir.path().join("out")).unwrap();

        let mutate = |p: &Path| {
            set_writable(&p.join("history.db")).unwrap();
            fs::write(p.join("history.db"), b"user history + nihongo").unwrap();
            fs::write(p.join("new_learning.db"), b"learned").unwrap();
            fs::remove_file(p.join("sub/segment.db")).unwrap();
        };

        with_profile_restored(
            &mut run,
            "verify",
            &prof,
            &|| Ok(()),
            &restore_profile,
            |_| {
                mutate(&prof);
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(tree_hashes(&prof).unwrap(), before);
        assert_eq!(run.state["profile_restored"], json!(true));
        assert_eq!(run.state["profile_files"], json!(2));

        let err = with_profile_restored(
            &mut run,
            "browser",
            &prof,
            &|| Ok(()),
            &restore_profile,
            |_| -> Result<()> {
                mutate(&prof);
                bail!("test failed")
            },
        );
        assert!(err.is_err());
        assert_eq!(tree_hashes(&prof).unwrap(), before);

        let err2 = with_profile_restored(
            &mut run,
            "verify2",
            &prof,
            &|| Ok(()),
            &|_, _| Ok(()),
            |_| {
                mutate(&prof);
                Ok(())
            },
        );
        assert!(err2.is_err());
        assert_eq!(run.state["profile_restored"], json!(false));

        let _ = set_writable(&hist);
    }
}
