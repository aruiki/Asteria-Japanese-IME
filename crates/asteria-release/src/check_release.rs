//! 公開した MSI・ローカルビルド MSI の同梱物と版を検証する（`mozc/tools/check_release.py` 移植）。

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use asteria_common::hash::sha256_file;

pub const REPO: &str = "aruiki/Asteria-Japanese-IME";
pub const MAX_MSI_BYTES: u64 = (18 * (1u64 << 30)) / 10; // 1.8 GiB
pub const RELEASE_LIST_LIMIT: usize = 100;

pub const BUNDLED: &[&str] = &[
    "asteria-server.exe",
    "asteria-renderer.exe",
    "asteria-config.exe",
    "asteria_tip.dll",
    "x86/asteria_tip.dll",
    "data/system.dict",
    "llama.dll",
    "ggml.dll",
    "ggml-base.dll",
    "ggml-vulkan.dll",
    "zenz-v2.5-small-q8_0.gguf",
    "genmai-310M-q8_0.gguf",
    "tinyswallow-1.5b-q5_k_m.gguf",
    "NOTICE-zenz.txt",
    "NOTICE-tinyswallow.txt",
    "AsteriaEnglish.tsv",
    "NOTICE-katakana-english.txt",
    "THIRD_PARTY_NOTICES.txt",
    "LICENSE-APACHE",
    "LICENSE-MIT",
];

pub const ABSENT: &[&str] = &["ggml-rpc.dll"];

/// 検査結果の記録（OK / NG を出力して 1 つでも NG があれば `ok = false`）。
#[derive(Debug, Default)]
pub struct Checks {
    pub ok: bool,
    pub results: Vec<(bool, String)>,
}

impl Checks {
    /// 初期状態 (`ok = true`) の検証器を作成する。
    pub fn new() -> Self {
        Self {
            ok: true,
            results: Vec::new(),
        }
    }

    /// 1 項目の成否を記録し、標準出力へ表示する。
    pub fn record(&mut self, cond: bool, msg: impl Into<String>) {
        let msg = msg.into();
        println!("{}{msg}", if cond { "OK  " } else { "NG  " });
        self.ok = self.ok && cond;
        self.results.push((cond, msg));
    }
}

/// 5.1.0 以降の版つき配布名（例: `Asteria-5.1.0-x64.msi`）。
pub fn versioned_asset_name(tag: &str) -> String {
    let stripped = tag.strip_prefix('v').unwrap_or(tag);
    format!("Asteria-{stripped}-x64.msi")
}

/// 添付資産名一覧から検証対象の MSI 名を選ぶ（`Asteria64.msi`）。
pub fn choose_asset_name<'a>(names: &[&'a str]) -> Result<&'a str> {
    let set: HashSet<&str> = names.iter().copied().collect();
    let name = "Asteria64.msi";
    let sha_name = format!("{name}.sha256");
    if set.contains(name) && set.contains(sha_name.as_str()) {
        return Ok(name);
    }
    bail!("MSIと対応するsha256の添付がない");
}

#[derive(Debug, Clone, Deserialize)]
pub struct AssetMeta {
    pub name: String,
    #[serde(default)]
    pub digest: Option<String>,
}

/// 版つき添付が存在する場合、SHA-256 と `.sha256` 添付を検査する。
pub fn check_versioned_asset(assets: &[AssetMeta], tag: &str, sha256: &str, checks: &mut Checks) {
    let name = versioned_asset_name(tag);
    let by_name: HashMap<&str, &AssetMeta> = assets.iter().map(|a| (a.name.as_str(), a)).collect();
    let Some(asset) = by_name.get(name.as_str()) else {
        println!("--  版つきの配布名 {name} はない(5.1.0 より前の版)");
        return;
    };
    let digest = asset
        .digest
        .as_deref()
        .unwrap_or("")
        .strip_prefix("sha256:")
        .unwrap_or("");
    checks.record(
        digest == sha256,
        format!("版つきの配布名 {name} の中身が Asteria64.msi と同じ"),
    );
    let sha_name = format!("{name}.sha256");
    checks.record(
        by_name.contains_key(sha_name.as_str()),
        format!("版つきの配布名 {name}.sha256 の添付"),
    );
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseItem {
    pub tag_name: String,
    #[serde(default)]
    pub is_latest: bool,
    #[serde(default)]
    pub is_draft: bool,
}

/// モデル専用リリースや draft を除外して対象タグと直前製品タグの組を返す。
pub fn product_release_pair(
    releases: &[ReleaseItem],
    requested: &str,
    listing_truncated: bool,
) -> Result<(String, String)> {
    let re = Regex::new(r"^v\d+\.\d+\.\d+(?:-(?:beta|rc)\.\d+)?$")?;
    let products: Vec<&ReleaseItem> = releases
        .iter()
        .filter(|r| !r.is_draft && re.is_match(&r.tag_name))
        .collect();
    let tag = if requested.is_empty() {
        products
            .iter()
            .find(|r| r.is_latest)
            .map(|r| r.tag_name.clone())
            .unwrap_or_default()
    } else {
        requested.to_owned()
    };
    let idx = products.iter().position(|r| r.tag_name == tag);
    let Some(index) = idx else {
        bail!("公開済みの製品タグが取得一覧にない。モデル専用release/draftはMSI検査の対象外");
    };
    let previous = products
        .get(index + 1)
        .map(|r| r.tag_name.clone())
        .unwrap_or_default();
    if previous.is_empty() && listing_truncated {
        bail!("取得範囲の末尾に達したため、前の製品releaseがないとは確認できない");
    }
    Ok((tag, previous))
}

/// バージョン文字列 `A.B.C.D` の先頭 3 要素を比較用タプルへ変換する。
pub fn parse_version_prefix3(v: &str) -> Result<(u64, u64, u64)> {
    let parts: Vec<u64> = v
        .split('.')
        .map(|s| s.parse::<u64>().context("不正なバージョン数値"))
        .collect::<Result<Vec<_>>>()?;
    if parts.len() < 3 {
        bail!("バージョン文字列が短すぎます: {v}");
    }
    Ok((parts[0], parts[1], parts[2]))
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct GeneratorIdentity {
    pub filename: String,
    pub sha256: String,
    #[serde(default)]
    pub base_models: HashMap<String, String>,
}

/// 同梱された追加生成モデルと `native_generator.json` の整合性を検査する。
pub fn check_generator(
    root: &Path,
    checks: &mut Checks,
    required: bool,
    identity: Option<&Value>,
    repo_root: Option<&Path>,
) -> Result<()> {
    let owned_identity: Value;
    let id_val = match identity {
        Some(v) => v,
        None => {
            let repo = repo_root.context("リポジトリルートが必要です")?;
            let path = repo.join("training/contrastive/native_generator.json");
            owned_identity = serde_json::from_str(&fs::read_to_string(&path)?)?;
            &owned_identity
        }
    };
    let filename = id_val["filename"]
        .as_str()
        .context("identity.filename がありません")?;
    let expected_sha = id_val["sha256"]
        .as_str()
        .context("identity.sha256 がありません")?;
    let model = root.join(filename);
    let metadata = root.join("native_generator.json");
    if !required && !model.exists() && !metadata.exists() {
        return Ok(());
    }
    checks.record(model.is_file(), format!("同梱: {filename}"));
    checks.record(metadata.is_file(), "同梱: native_generator.json");
    if metadata.is_file() {
        let packaged: Option<Value> = fs::read_to_string(&metadata)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok());
        checks.record(
            packaged.as_ref() == Some(id_val),
            "追加生成モデルの固定情報が検証ソースと一致",
        );
    }
    let mut models: Vec<(String, String)> = vec![(filename.to_owned(), expected_sha.to_owned())];
    if let Some(obj) = id_val.get("base_models").and_then(Value::as_object) {
        let mut keys: Vec<&String> = obj.keys().collect();
        keys.sort();
        for k in keys {
            if let Some(v) = obj[k].as_str() {
                models.push((k.clone(), v.to_owned()));
            }
        }
    }
    for (name, exp) in models {
        let path = root.join(&name);
        checks.record(path.is_file(), format!("固定モデルの同梱: {name}"));
        if path.is_file() {
            let got = sha256_file(&path)?;
            checks.record(got == exp, format!("モデルSHA-256が評価対象と一致: {name}"));
        }
    }
    Ok(())
}

/// 同梱された初期連語 (`fast_collocations.*`) と帰属表示を検査する。
pub fn check_collocations(
    root: &Path,
    checks: &mut Checks,
    required: bool,
    identity: Option<&Value>,
    repo_root: Option<&Path>,
) -> Result<()> {
    let owned_identity: Value;
    let id_val = match identity {
        Some(v) => v,
        None => {
            let repo = repo_root.context("リポジトリルートが必要です")?;
            let path = repo.join("training/collocations/sources.json");
            owned_identity = serde_json::from_str(&fs::read_to_string(&path)?)?;
            &owned_identity
        }
    };
    let names = [
        "fast_collocations.txt",
        "fast_collocations.json",
        "NOTICE-fast-collocations.txt",
    ];
    if !required && !names.iter().any(|n| root.join(n).exists()) {
        return Ok(());
    }
    for name in names {
        checks.record(root.join(name).is_file(), format!("連語データ同梱: {name}"));
    }
    if !names.iter().all(|n| root.join(n).is_file()) {
        return Ok(());
    }
    let data = fs::read(root.join(names[0]))?;
    let expected = id_val["result_sha256"]
        .as_str()
        .context("identity.result_sha256 がありません")?;
    let actual_sha = asteria_common::hash::sha256_bytes(&data);
    checks.record(actual_sha == expected, "連語SHA-256が比較対象と一致");

    let line_count = data
        .split(|&b| b == b'\n')
        .filter(|l| !l.is_empty())
        .count();
    let (mut valid, mut attribution) = (false, false);
    if let (Ok(meta_raw), Ok(notice)) = (
        fs::read_to_string(root.join(names[1])),
        fs::read_to_string(root.join(names[2])),
    ) {
        if let Ok(metadata) = serde_json::from_str::<Value>(&meta_raw) {
            let source = id_val["source"].as_str().unwrap_or("");
            let revision = id_val["revision"].as_str().unwrap_or("");
            let sample_sha = id_val["sample_sha256"].as_str().unwrap_or("");
            valid = metadata.get("result_sha256").and_then(Value::as_str) == Some(expected)
                && metadata.get("filename").and_then(Value::as_str) == Some(names[0])
                && metadata.get("source").and_then(Value::as_str) == Some(source)
                && metadata.get("revision").and_then(Value::as_str) == Some(revision)
                && metadata.get("corpus_sha256").and_then(Value::as_str) == Some(sample_sha)
                && metadata
                    .get("evaluation_labels_used")
                    .and_then(Value::as_bool)
                    == Some(false)
                && metadata.get("combined_entries").and_then(Value::as_u64)
                    == Some(line_count as u64);

            attribution = ["CC BY-SA 4.0", source, revision, expected]
                .iter()
                .all(|needle| !needle.is_empty() && notice.contains(needle));

            if let Some(usage) = id_val.get("usage_source") {
                valid = valid && metadata.get("usage_source") == Some(usage);
                let repo = usage["repository"].as_str().unwrap_or("");
                let tag = usage["tag"].as_str().unwrap_or("");
                let u_sha = usage["usage_sha256"].as_str().unwrap_or("");
                let c_sha = usage["cforms_sha256"].as_str().unwrap_or("");
                attribution = attribution
                    && [
                        repo,
                        tag,
                        u_sha,
                        c_sha,
                        "BSD-2-Clause",
                        "Hiroyuki Komatsu",
                        "Shoji Ogura",
                        "Tsuyoshi Horo",
                    ]
                    .iter()
                    .all(|needle| !needle.is_empty() && notice.contains(needle));
            }
        }
    }
    checks.record(
        valid,
        "連語manifestの出典・学習区間・結果SHAが固定情報と一致",
    );
    checks.record(attribution, "連語の帰属・ライセンス・変更対象の表示");
    Ok(())
}

/// 同梱された短文語彙 (`short_lexicon.*`) のサイズと SHA-256 を検査する。
pub fn check_short_lexicon(
    root: &Path,
    checks: &mut Checks,
    required: bool,
    identity: Option<&Value>,
    repo_root: Option<&Path>,
) -> Result<()> {
    let owned_identity: Value;
    let id_val = match identity {
        Some(v) => v,
        None => {
            let repo = repo_root.context("リポジトリルートが必要です")?;
            let path = repo.join("training/lexicon/package.json");
            owned_identity = serde_json::from_str(&fs::read_to_string(&path)?)?;
            &owned_identity
        }
    };
    let files = id_val["files"]
        .as_array()
        .context("identity.files が配列ではありません")?;
    if !required
        && !files
            .iter()
            .filter_map(|f| f["name"].as_str())
            .any(|n| root.join(n).exists())
    {
        return Ok(());
    }
    for f in files {
        let name = f["name"].as_str().context("file.name がありません")?;
        let exp_bytes = f["bytes"].as_u64().context("file.bytes がありません")?;
        let exp_sha = f["sha256"].as_str().context("file.sha256 がありません")?;
        let path = root.join(name);
        checks.record(path.is_file(), format!("短文語彙の同梱: {name}"));
        if path.is_file() {
            let len = fs::metadata(&path)?.len();
            let got_sha = sha256_file(&path)?;
            checks.record(
                len == exp_bytes && got_sha == exp_sha,
                format!("短文語彙の固定サイズ/SHA: {name}"),
            );
        }
    }
    Ok(())
}

/// PowerShell 経由で Windows Installer COM から MSI のプロパティを取得する。
pub fn msi_property(msi: &Path, name: &str) -> Result<String> {
    let msi_escaped = msi.to_string_lossy().replace('\'', "''");
    let script = format!(
        "[Console]::OutputEncoding=[Text.Encoding]::UTF8;\
         $wi=New-Object -ComObject WindowsInstaller.Installer;\
         $db=$wi.GetType().InvokeMember('OpenDatabase','InvokeMethod',$null,$wi,@('{msi_escaped}',0));\
         $v=$db.OpenView(\"SELECT Value FROM Property WHERE Property='{name}'\");\
         $v.Execute();$r=$v.Fetch();$r.StringData(1)"
    );
    let out = Command::new("powershell")
        .args(["-NoProfile", "-Command", &script])
        .output()
        .context("powershell 実行失敗")?;
    if !out.status.success() {
        bail!(
            "MSI プロパティ {name} 取得失敗: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    let val = String::from_utf8_lossy(&out.stdout).trim().to_owned();
    if val.is_empty() {
        bail!("MSIの{name}が空です");
    }
    Ok(val)
}

/// PowerShell 経由で EXE/DLL の `FileVersion` を取得する。
pub fn file_version(path: &Path) -> Result<String> {
    let escaped = path.to_string_lossy().replace('\'', "''");
    let script = format!("(Get-Item '{escaped}').VersionInfo.FileVersion");
    let out = Command::new("powershell")
        .args(["-NoProfile", "-Command", &script])
        .output()
        .context("powershell 実行失敗")?;
    if !out.status.success() {
        bail!(
            "FileVersion 取得失敗: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

fn run_cmd(args: &[&str]) -> Result<String> {
    let out = Command::new(args[0])
        .args(&args[1..])
        .output()
        .with_context(|| format!("コマンド起動失敗: {}", args[0]))?;
    if !out.status.success() {
        bail!(
            "{} 失敗 (exit {:?}): {}",
            args[0],
            out.status.code(),
            String::from_utf8_lossy(&out.stderr)
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

fn download_release(tag: &str, dest: &Path) -> Result<PathBuf> {
    fs::create_dir_all(dest)?;
    let json_raw = run_cmd(&[
        "gh", "release", "view", tag, "--repo", REPO, "--json", "assets",
    ])?;
    let val: Value = serde_json::from_str(&json_raw)?;
    let assets = val["assets"]
        .as_array()
        .context("gh release view の assets が配列ではありません")?;
    let names: Vec<&str> = assets.iter().filter_map(|a| a["name"].as_str()).collect();
    let name = choose_asset_name(&names)?;
    let pattern = format!("{name}*");
    run_cmd(&[
        "gh",
        "release",
        "download",
        tag,
        "--repo",
        REPO,
        "--pattern",
        &pattern,
        "--dir",
        &dest.to_string_lossy(),
        "--clobber",
    ])?;
    Ok(dest.join(name))
}

#[derive(Debug, Clone)]
pub struct CheckReleaseOpts {
    pub tag: String,
    pub work: PathBuf,
    pub local: Option<PathBuf>,
    pub previous_local: Option<PathBuf>,
    pub require_generator: bool,
    pub require_collocations: bool,
    pub require_short_lexicon: bool,
}

/// MSI の中身（管理展開・同梱物・除外物・ファイル版）を検証する。
pub fn check_contents(
    msi: &Path,
    work: &Path,
    checks: &mut Checks,
    opts: &CheckReleaseOpts,
    repo_root: &Path,
) -> Result<()> {
    let size = fs::metadata(msi)?.len();
    let gib = (size as f64) / ((1u64 << 30) as f64);
    let max_gib = (MAX_MSI_BYTES as f64) / ((1u64 << 30) as f64);
    checks.record(
        size <= MAX_MSI_BYTES,
        format!("大きさ {gib:.2} GiB ≦ {max_gib:.1} GiB(上限 2 GiB)"),
    );
    let ver = msi_property(msi, "ProductVersion")?;
    fs::create_dir_all(work)?;
    let ext = work.join("ext");
    if ext.exists() {
        let _ = fs::remove_dir_all(&ext);
    }
    let msi_str = msi.to_string_lossy().into_owned();
    let target_arg = format!("TARGETDIR={}", ext.display());
    run_cmd(&["msiexec", "/a", &msi_str, "/qn", &target_arg])?;

    let root = [
        ext.join("PFiles64").join("Asteria"),
        ext.join("Program Files").join("Asteria"),
        ext.join("PFiles").join("Asteria"),
    ]
    .into_iter()
    .find(|p| p.exists())
    .unwrap_or_else(|| ext.join("PFiles64").join("Asteria"));
    let is_rust_edition = root.join("asteria-server.exe").is_file();
    let is_asteria = root.file_name().and_then(|n| n.to_str()) == Some("Asteria");
    for &name in BUNDLED {
        if !is_rust_edition && name.starts_with("asteria") {
            continue;
        }
        if is_rust_edition && (name == "asteria_tip.dll" || name == "x86/asteria_tip.dll") {
            continue;
        }
        checks.record(root.join(name).exists(), format!("同梱: {name}"));
    }
    if is_rust_edition {
        let x64_ver = format!("asteria_tip64-{ver}.dll");
        let x86_ver = format!("x86/asteria_tip32-{ver}.dll");
        checks.record(
            root.join(&x64_ver).is_file(),
            format!("同梱(版ごとの名前): {x64_ver}"),
        );
        checks.record(
            root.join(&x86_ver).is_file(),
            format!("同梱(版ごとの名前): {x86_ver}"),
        );
        checks.record(
            !root.join("asteria_tip.dll").exists(),
            "版のない名前の DLL を入れない: asteria_tip.dll",
        );
        checks.record(
            !root.join("x86/asteria_tip.dll").exists(),
            "版のない名前の DLL を入れない: x86/asteria_tip.dll",
        );
    }
    if is_asteria && !is_rust_edition {
        for bits in ["64", "32"] {
            let name = format!("mozc_tip{bits}-{ver}.dll");
            checks.record(
                root.join(&name).is_file(),
                format!("同梱(版ごとの名前): {name}"),
            );
            let unversioned = format!("mozc_tip{bits}.dll");
            checks.record(
                !root.join(&unversioned).exists(),
                format!("版のない名前の DLL を入れない: {unversioned}"),
            );
        }
    }
    check_generator(&root, checks, opts.require_generator, None, Some(repo_root))?;
    check_collocations(
        &root,
        checks,
        opts.require_collocations,
        None,
        Some(repo_root),
    )?;
    check_short_lexicon(
        &root,
        checks,
        opts.require_short_lexicon,
        None,
        Some(repo_root),
    )?;
    for &name in ABSENT {
        checks.record(!root.join(name).exists(), format!("入れない: {name}"));
    }
    if is_rust_edition {
        checks.record(
            parse_version_prefix3(&ver).is_ok(),
            format!("ProductVersion {ver} が有効な 3 要素版番号"),
        );
    } else {
        let fv = file_version(&root.join("mozc_server.exe"))?;
        checks.record(
            fv == ver,
            format!("mozc_server.exe の版 {fv} = ProductVersion"),
        );
    }
    Ok(())
}

fn clean_canonical(path: &Path) -> PathBuf {
    let p = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let s = p.to_string_lossy();
    if let Some(stripped) = s.strip_prefix(r"\\?\") {
        PathBuf::from(stripped)
    } else {
        p
    }
}

/// `check_release` メイン処理（終了コード 0 = 成功, 1 = 検証失敗）。
pub fn run_check_release(opts: &CheckReleaseOpts, repo_root: &Path) -> Result<i32> {
    if opts.previous_local.is_some() && opts.local.is_none() {
        bail!("--previous-local requires --local");
    }
    let mut checks = Checks::new();
    fs::create_dir_all(&opts.work)?;
    let work = clean_canonical(&opts.work);
    if let Some(local) = &opts.local {
        let msi = clean_canonical(local);
        if let Some(prev_path) = &opts.previous_local {
            let prev = clean_canonical(prev_path);
            let ver = msi_property(&msi, "ProductVersion")?;
            let previous = msi_property(&prev, "ProductVersion")?;
            checks.record(
                parse_version_prefix3(&ver)? > parse_version_prefix3(&previous)?,
                format!("ProductVersion {ver} > 公開済みMSI({previous})(先頭 3 つで比べる)"),
            );
        }
        if checks.ok {
            check_contents(&msi, &work, &mut checks, opts, repo_root)?;
        }
    } else {
        let limit_str = RELEASE_LIST_LIMIT.to_string();
        let raw = run_cmd(&[
            "gh",
            "release",
            "list",
            "--repo",
            REPO,
            "--limit",
            &limit_str,
            "--json",
            "tagName,isLatest,isDraft",
        ])?;
        let releases: Vec<ReleaseItem> = serde_json::from_str(&raw)?;
        let (tag, prev) =
            product_release_pair(&releases, &opts.tag, releases.len() == RELEASE_LIST_LIMIT)?;
        if work.exists() {
            let _ = fs::remove_dir_all(&work);
        }
        let msi = download_release(&tag, &work.join(&tag))?;
        let sha_path = msi.with_file_name(format!(
            "{}.sha256",
            msi.file_name().and_then(|n| n.to_str()).unwrap_or("")
        ));
        let want = fs::read_to_string(&sha_path)?
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_owned();
        let got = sha256_file(&msi)?;
        checks.record(
            got == want,
            format!("SHA-256 {}…", &got[..got.len().min(16)]),
        );

        let api_path = format!("repos/{REPO}/releases/tags/{tag}");
        let rel_json: Value = serde_json::from_str(&run_cmd(&["gh", "api", &api_path])?)?;
        let assets: Vec<AssetMeta> = serde_json::from_value(rel_json["assets"].clone())?;
        check_versioned_asset(&assets, &tag, &got, &mut checks);

        let ver = msi_property(&msi, "ProductVersion")?;
        if !prev.is_empty() {
            let prev_msi = download_release(&prev, &work.join(&prev))?;
            let prev_ver = msi_property(&prev_msi, "ProductVersion")?;
            checks.record(
                parse_version_prefix3(&ver)? > parse_version_prefix3(&prev_ver)?,
                format!("ProductVersion {ver} > {prev}({prev_ver})(先頭 3 つで比べる)"),
            );
        } else {
            println!("--  ProductVersion {ver}(比べる前の版がない)");
        }
        check_contents(&msi, &work, &mut checks, opts, repo_root)?;
    }
    println!(
        "{}",
        if checks.ok {
            "すべて OK"
        } else {
            "確かめに失敗した項目がある"
        }
    );
    Ok(if checks.ok { 0 } else { 1 })
}

#[cfg(test)]
mod tests {
    use super::*;
    use asteria_common::hash::sha256_bytes;
    use serde_json::json;
    use tempfile::TempDir;

    #[test]
    fn asset_name_selection() {
        assert_eq!(
            choose_asset_name(&["Asteria64.msi", "Asteria64.msi.sha256"]).unwrap(),
            "Asteria64.msi"
        );
        assert!(choose_asset_name(&["Asteria64.msi", "Other64.msi.sha256"]).is_err());
        assert!(choose_asset_name(&["Asteria64.msi"]).is_err());
    }

    #[test]
    fn product_release_pair_skips_models_and_drafts() {
        let releases = vec![
            ReleaseItem {
                tag_name: "v1.1.4-beta.1".into(),
                is_latest: false,
                is_draft: true,
            },
            ReleaseItem {
                tag_name: "model-native-generator-20261017".into(),
                is_latest: false,
                is_draft: false,
            },
            ReleaseItem {
                tag_name: "v1.1.3".into(),
                is_latest: true,
                is_draft: false,
            },
            ReleaseItem {
                tag_name: "v1.1.2".into(),
                is_latest: false,
                is_draft: false,
            },
        ];
        assert_eq!(
            product_release_pair(&releases, "", false).unwrap(),
            ("v1.1.3".into(), "v1.1.2".into())
        );
        assert!(product_release_pair(&releases, "model-native-generator-20261017", false).is_err());
        assert!(product_release_pair(
            &[ReleaseItem {
                tag_name: "v1.1.3".into(),
                is_latest: true,
                is_draft: false
            }],
            "",
            true
        )
        .is_err());
    }

    #[test]
    fn generator_payload_checks() {
        let dir = TempDir::new().unwrap();
        let identity = json!({"filename": "native.gguf", "sha256": "0".repeat(64)});
        let mut legacy = Checks::new();
        check_generator(dir.path(), &mut legacy, false, Some(&identity), None).unwrap();
        assert!(legacy.ok);

        let mut req = Checks::new();
        check_generator(dir.path(), &mut req, true, Some(&identity), None).unwrap();
        assert!(!req.ok);

        let orig_sha = sha256_bytes(b"original");
        let nat_sha = sha256_bytes(b"native");
        let id2 = json!({"filename": "native.gguf", "sha256": nat_sha, "base_models": {"original.gguf": orig_sha}});
        fs::write(dir.path().join("native.gguf"), b"native").unwrap();
        fs::write(dir.path().join("native_generator.json"), id2.to_string()).unwrap();
        fs::write(dir.path().join("original.gguf"), b"substituted").unwrap();
        let mut sub = Checks::new();
        check_generator(dir.path(), &mut sub, true, Some(&id2), None).unwrap();
        assert!(!sub.ok);
    }

    #[test]
    fn collocations_payload_checks() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        let data = "司会を務め\n猫を飼う\n".as_bytes();
        let r_sha = sha256_bytes(data);
        let usage = json!({
            "repository": "fixed/usage", "tag": "2025-01-25",
            "usage_sha256": "a".repeat(64), "license_sha256": "b".repeat(64), "cforms_sha256": "c".repeat(64)
        });
        let identity = json!({
            "source": "public/wiki", "revision": "pinned-revision",
            "sample_sha256": "fixed-corpus", "result_sha256": r_sha,
            "usage_source": usage
        });
        let metadata = json!({
            "source": "public/wiki", "revision": "pinned-revision",
            "corpus_sha256": "fixed-corpus", "result_sha256": r_sha,
            "filename": "fast_collocations.txt", "evaluation_labels_used": false,
            "combined_entries": 2, "usage_source": usage
        });
        fs::write(root.join("fast_collocations.txt"), data).unwrap();
        fs::write(root.join("fast_collocations.json"), metadata.to_string()).unwrap();
        fs::write(
            root.join("NOTICE-fast-collocations.txt"),
            format!(
                "CC BY-SA 4.0 public/wiki pinned-revision {r_sha} BSD-2-Clause Hiroyuki Komatsu Shoji Ogura Tsuyoshi Horo fixed/usage 2025-01-25 {} {} {}",
                "a".repeat(64),
                "b".repeat(64),
                "c".repeat(64)
            ),
        )
        .unwrap();

        let mut ok_c = Checks::new();
        check_collocations(root, &mut ok_c, true, Some(&identity), None).unwrap();
        assert!(ok_c.ok);

        fs::write(root.join("fast_collocations.json"), b"{").unwrap();
        let mut bad_c = Checks::new();
        check_collocations(root, &mut bad_c, true, Some(&identity), None).unwrap();
        assert!(!bad_c.ok);
    }

    #[test]
    fn short_lexicon_package_checks() {
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        let mut files = Vec::new();
        for name in [
            "short_lexicon.tsv",
            "short_lexicon.json",
            "NOTICE-short-lexicon.txt",
        ] {
            let bytes = name.as_bytes();
            fs::write(root.join(name), bytes).unwrap();
            files.push(json!({"name": name, "bytes": bytes.len(), "sha256": sha256_bytes(bytes)}));
        }
        let spec = json!({ "files": files });
        let mut c = Checks::new();
        check_short_lexicon(root, &mut c, true, Some(&spec), None).unwrap();
        assert_eq!(c.results.len(), 6);
        assert!(c.ok);

        fs::write(root.join("short_lexicon.tsv"), b"short_lexicon.tsvx").unwrap();
        let mut c2 = Checks::new();
        check_short_lexicon(root, &mut c2, true, Some(&spec), None).unwrap();
        assert!(!c2.ok);
    }

    #[test]
    fn versioned_asset_checks() {
        assert_eq!(versioned_asset_name("v5.1.0"), "Asteria-5.1.0-x64.msi");
        assert_eq!(
            versioned_asset_name("v5.1.0-rc.2"),
            "Asteria-5.1.0-rc.2-x64.msi"
        );
        let assets = vec![
            AssetMeta {
                name: "Asteria-5.1.0-x64.msi".into(),
                digest: Some("sha256:abc".into()),
            },
            AssetMeta {
                name: "Asteria-5.1.0-x64.msi.sha256".into(),
                digest: Some("sha256:x".into()),
            },
        ];
        let mut c = Checks::new();
        check_versioned_asset(&assets, "v5.1.0", "abc", &mut c);
        assert!(c.ok && c.results.len() == 2);

        let mut c2 = Checks::new();
        check_versioned_asset(&assets, "v5.1.0", "different", &mut c2);
        assert!(!c2.results[0].0);
    }
}
