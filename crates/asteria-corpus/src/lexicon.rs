//! 短文語彙・ソース検証・ステージング・頻度絞り込み・診断セット生成。
//! (`fetch_sources.py`, `stage_package.py`, `stage_tokenizer.py`, `build_lexicon.py`,
//!  `build_frequency_lexicon.py`, `build_frequency_collocations.py`, `build_diagnostic.py`)

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap, HashMap, HashSet};
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use regex::Regex;
use serde::Deserialize;
use serde_json::{json, Value};
use sudachi::analysis::Tokenize;
use zip::ZipArchive;

use asteria_common::hash::sha256_file;
use asteria_common::http::download_verified;
use asteria_common::kana::{evaluation_readings, kata_to_hira, nfkc, Exclusions};

pub const MIN_COMMON: usize = 100;
pub const KNOWN_READING_MARGIN: i64 = 1000;
pub const BASELINE_COLLOCATION_SHA: &str =
    "c615bb88d82db2bdb19f3cc401b785e252d4efebdbd71cf9403538eb6457acc0";

pub fn pos_mapping(pos2: &str, pos3: &str, pos4: &str) -> Option<&'static str> {
    match (pos2, pos3, pos4) {
        ("普通名詞", "一般", "*") => Some("名詞,一般,*,*,*,*,*"),
        ("普通名詞", "サ変可能", "*") => Some("名詞,サ変接続,*,*,*,*,*"),
        ("固有名詞", "一般", "*") => Some("名詞,固有名詞,一般,*,*,*,*"),
        ("固有名詞", "人名", "一般") => Some("名詞,固有名詞,人名,一般,*,*,*"),
        ("固有名詞", "人名", "名") => Some("名詞,固有名詞,人名,名,*,*,*"),
        ("固有名詞", "人名", "姓") => Some("名詞,固有名詞,人名,姓,*,*,*"),
        ("固有名詞", "地名", "一般") => Some("名詞,固有名詞,地域,一般,*,*,*"),
        ("固有名詞", "地名", "国") => Some("名詞,固有名詞,地域,国,*,*,*"),
        _ => None,
    }
}

pub fn all_pos_values() -> &'static [&'static str] {
    &[
        "名詞,一般,*,*,*,*,*",
        "名詞,サ変接続,*,*,*,*,*",
        "名詞,固有名詞,一般,*,*,*,*",
        "名詞,固有名詞,人名,一般,*,*,*",
        "名詞,固有名詞,人名,名,*,*,*",
        "名詞,固有名詞,人名,姓,*,*,*",
        "名詞,固有名詞,地域,一般,*,*,*",
        "名詞,固有名詞,地域,国,*,*,*",
    ]
}

/// 読み正規表現（ひらがな・長音 2〜40 文字）。
pub fn is_valid_reading(reading: &str) -> bool {
    let mut len = 0usize;
    for c in reading.chars() {
        if !(('ぁ'..='ゖ').contains(&c) || c == 'ー') {
            return false;
        }
        len += 1;
    }
    (2..=40).contains(&len)
}

/// 表層文字種が文字・記号(数・欧字・漢字・かな)または中黒であるか。
pub fn is_valid_surface_char(c: char) -> bool {
    c == '・' || c.is_alphabetic() || c.is_numeric() || is_unicode_mark(c)
}

fn is_unicode_mark(c: char) -> bool {
    // Combining Marks (Mn, Mc, Me)
    matches!(c as u32,
        0x0300..=0x036F | 0x3099..=0x309C | 0xFE00..=0xFE0F
    )
}

/// `fetch_sources.py`: ファイルのサイズ・SHA-256・ZIP メンバ一覧を検証する。
pub fn verify_source_file(path: &Path, spec: &Value) -> Result<()> {
    let name = spec["name"].as_str().context("spec.name")?;
    let expected_bytes = spec["bytes"].as_u64().context("spec.bytes")?;
    let expected_sha = spec["sha256"].as_str().context("spec.sha256")?;
    let meta = fs::metadata(path)?;
    if meta.len() != expected_bytes || sha256_file(path)? != expected_sha {
        bail!("固定したサイズ/SHAと一致しません: {name}");
    }
    if let Some(members) = spec.get("members").and_then(Value::as_array) {
        let mut archive = ZipArchive::new(File::open(path)?)?;
        if archive.len() != members.len() {
            bail!("固定したZIP内容と一致しません: {name}");
        }
        for (i, exp) in members.iter().enumerate() {
            let entry = archive.by_index(i)?;
            let exp_name = exp["name"].as_str().context("member.name")?;
            let exp_size = exp["bytes"].as_u64().context("member.bytes")?;
            if entry.name() != exp_name || entry.size() != exp_size {
                bail!("固定したZIP内容と一致しません: {name}");
            }
        }
    }
    Ok(())
}

/// `fetch_sources.py`: 単一ファイル名のみ許可し、キャッシュまたは HTTPS から取得・検証する。
pub fn fetch_source(spec: &Value, out: &Path, cache: Option<&Path>) -> Result<PathBuf> {
    let name = spec["name"].as_str().context("spec.name")?;
    if Path::new(name).file_name().and_then(|n| n.to_str()) != Some(name)
        || name.contains('/')
        || name.contains('\\')
    {
        bail!("取得名は単一のファイル名に限ります");
    }
    fs::create_dir_all(out)?;
    let target = out.join(name);
    if target.exists() {
        verify_source_file(&target, spec)?;
        return Ok(target);
    }
    if let Some(c) = cache {
        let source = c.join(name);
        if source.exists() {
            verify_source_file(&source, spec)?;
            fs::copy(&source, &target)?;
            return Ok(target);
        }
    }
    let url = spec["url"].as_str().context("spec.url")?;
    if !url.starts_with("https://") {
        bail!("公式HTTPS配布URLが必要です");
    }
    let exp_bytes = spec["bytes"].as_u64().context("spec.bytes")?;
    let exp_sha = spec["sha256"].as_str().context("spec.sha256")?;
    download_verified(
        url,
        &target,
        Some(exp_bytes),
        exp_sha,
        "Asteria-lexicon/1",
        Duration::from_secs(60),
    )?;
    verify_source_file(&target, spec)?;
    Ok(target)
}

pub fn load_sources(out: &Path, cache: Option<&Path>, config_path: &Path) -> Result<Value> {
    let config: Value = serde_json::from_str(&fs::read_to_string(config_path)?)?;
    fs::create_dir_all(out)?;
    let files = config["files"].as_array().context("config.files")?;
    let mut file_map = serde_json::Map::new();
    for spec in files {
        fetch_source(spec, out, cache)?;
        let n = spec["name"].as_str().unwrap_or("").to_owned();
        let s = spec["sha256"].as_str().unwrap_or("").to_owned();
        file_map.insert(n, json!(s));
    }
    let receipt = json!({
        "source_commit": config["commit"],
        "version": config["version"],
        "config_sha256": sha256_file(config_path)?,
        "files": file_map,
    });
    fs::write(
        out.join("verified.json"),
        format!("{}\n", serde_json::to_string_pretty(&receipt)?),
    )?;
    Ok(config)
}

/// `stage_package.py`: マニフェストの形式・URL・サイズ・SHA を検証する。
pub fn package_identity(manifest_path: &Path) -> Result<Value> {
    let spec: Value = serde_json::from_str(&fs::read_to_string(manifest_path)?)?;
    if spec.get("schema").and_then(Value::as_u64) != Some(1) {
        bail!("語彙配布の固定ファイル構成が一致しません");
    }
    let names = [
        "short_lexicon.tsv",
        "short_lexicon.json",
        "NOTICE-short-lexicon.txt",
    ];
    let files = spec["files"].as_array().context("files")?;
    let got_names: Vec<&str> = files.iter().filter_map(|f| f["name"].as_str()).collect();
    if got_names != names {
        bail!("語彙配布の固定ファイル構成が一致しません");
    }
    let tag = spec["tag"].as_str().unwrap_or("");
    let tag_re = Regex::new(r"^data-short-lexicon-[0-9]{8}$")?;
    if !tag_re.is_match(tag) {
        bail!("製品の版と分離したデータタグが必要です");
    }
    let sha_re = Regex::new(r"^[0-9a-f]{64}$")?;
    let prefix = "https://github.com/aruiki/Asteria-Japanese-IME/releases/download/";
    for f in files {
        let name = f["name"].as_str().unwrap_or("");
        let sha = f["sha256"].as_str().unwrap_or("");
        let bytes = f["bytes"].as_u64().unwrap_or(0);
        let url = f["url"].as_str().unwrap_or("");
        if !sha_re.is_match(sha)
            || bytes == 0
            || bytes > 16 * 1024 * 1024
            || url != format!("{prefix}{tag}/{name}")
        {
            bail!("語彙配布のURL/サイズ/SHAが不正です");
        }
    }
    Ok(spec)
}

pub fn verify_package_dir(directory: &Path, spec: &Value) -> Result<Value> {
    let files = spec["files"].as_array().context("files")?;
    for f in files {
        let name = f["name"].as_str().context("name")?;
        let p = directory.join(name);
        if !p.is_file() {
            bail!("語彙ファイルがありません: {name}");
        }
        let exp_bytes = f["bytes"].as_u64().context("bytes")?;
        let exp_sha = f["sha256"].as_str().context("sha256")?;
        if fs::metadata(&p)?.len() != exp_bytes || sha256_file(&p)? != exp_sha {
            bail!("語彙ファイルのサイズ/SHA不一致: {name}");
        }
    }
    let metadata: Value =
        serde_json::from_str(&fs::read_to_string(directory.join("short_lexicon.json"))?)?;
    let tsv = fs::read_to_string(directory.join("short_lexicon.tsv"))?;
    let lines: Vec<&str> = tsv.lines().collect();
    let entries = spec["entries"].as_u64().context("entries")? as usize;
    if lines.len() != entries
        || metadata.get("output_entries").and_then(Value::as_u64) != Some(entries as u64)
        || metadata.get("sha256").and_then(Value::as_str) != files[0]["sha256"].as_str()
        || metadata
            .get("evaluation_labels_used")
            .and_then(Value::as_bool)
            != Some(false)
        || metadata.get("source_commit") != spec.get("sudachi_commit")
        || metadata.get("frequency_data_sha256") != spec.get("frequency_data_sha256")
    {
        bail!("語彙manifestの出典/件数/参照未使用が不一致です");
    }
    let mut prev: Option<(String, String, u64)> = None;
    for line in lines {
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() != 5
            || fields[0].is_empty()
            || fields[4].is_empty()
            || fields[1] != fields[2]
        {
            bail!("語彙TSVの形式/接続ID/コストが不正です");
        }
        let conn_id: u64 = fields[1]
            .parse()
            .context("語彙TSVの形式/接続ID/コストが不正です")?;
        let cost: u64 = fields[3]
            .parse()
            .context("語彙TSVの形式/接続ID/コストが不正です")?;
        if cost > 32767 {
            bail!("語彙TSVの形式/接続ID/コストが不正です");
        }
        let key = (fields[0].to_owned(), fields[4].to_owned(), conn_id);
        if let Some(p) = &prev {
            if &key < p {
                bail!("語彙TSVの固定順序が不正です");
            }
        }
        prev = Some(key);
    }
    Ok(metadata)
}

pub fn stage_package(out: &Path, source: Option<&Path>, manifest: &Path) -> Result<Value> {
    let spec = package_identity(manifest)?;
    if let Some(src) = source {
        verify_package_dir(src, &spec)?;
    }
    fs::create_dir_all(out)?;
    let files = spec["files"].as_array().context("files")?;
    for f in files {
        let name = f["name"].as_str().context("name")?;
        let target = out.join(name);
        if target.exists() {
            let exp_bytes = f["bytes"].as_u64().context("bytes")?;
            let exp_sha = f["sha256"].as_str().context("sha256")?;
            if fs::metadata(&target)?.len() != exp_bytes || sha256_file(&target)? != exp_sha {
                bail!("語彙ファイルのサイズ/SHA不一致: {name}");
            }
        }
    }
    for f in files {
        let name = f["name"].as_str().context("name")?;
        let target = out.join(name);
        if target.exists() {
            continue;
        }
        let partial = out.join(format!("{name}.partial"));
        let exp_bytes = f["bytes"].as_u64().context("bytes")?;
        let exp_sha = f["sha256"].as_str().context("sha256")?;
        if let Some(src) = source {
            let mut dst = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&partial)?;
            let mut src_f = File::open(src.join(name))?;
            std::io::copy(&mut src_f, &mut dst)?;
            dst.flush()?;
            drop(dst);
            if fs::metadata(&partial)?.len() != exp_bytes || sha256_file(&partial)? != exp_sha {
                bail!("語彙ファイルのサイズ/SHA不一致: {name}");
            }
            fs::rename(&partial, &target)?;
        } else {
            if partial.exists() {
                bail!(
                    "既存の途中ダウンロードファイルがあります: {}",
                    partial.display()
                );
            }
            let url = f["url"].as_str().context("url")?;
            download_verified(
                url,
                &target,
                Some(exp_bytes),
                exp_sha,
                "Asteria-lexicon/1",
                Duration::from_secs(120),
            )?;
        }
    }
    verify_package_dir(out, &spec)?;
    Ok(spec)
}

/// `stage_tokenizer.py`: ZIP 展開先パスがルート配下から脱出しないことを検証する。
pub fn safe_zip_member_path(root: &Path, name: &str, is_symlink: bool) -> Result<PathBuf> {
    if is_symlink || name.starts_with('/') || name.contains('\\') || name.contains(':') {
        bail!("展開先の外へ出るZIP要素を拒否します: {name}");
    }
    let rel = Path::new(name);
    for comp in rel.components() {
        if matches!(
            comp,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        ) {
            bail!("展開先の外へ出るZIP要素を拒否します: {name}");
        }
    }
    Ok(root.join(rel))
}

/// ZIP 内の各ファイルを検証展開する（シンボリックリンク・パストラバーサル・上書き禁止）。
pub fn extract_verified_zip(archive_path: &Path, root: &Path) -> Result<BTreeMap<String, String>> {
    let mut archive = ZipArchive::new(File::open(archive_path)?)?;
    let total_uncompressed: u64 = (0..archive.len())
        .filter_map(|i| archive.by_index(i).ok().map(|e| e.size()))
        .sum();
    if total_uncompressed > (1u64 << 30) {
        bail!("展開サイズが上限を超えています");
    }
    let mut files = BTreeMap::new();
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let name = entry.name().to_owned();
        let is_symlink = entry
            .unix_mode()
            .is_some_and(|m| (m & 0o170000) == 0o120000);
        let target = safe_zip_member_path(root, &name, is_symlink)?;
        if entry.is_dir() {
            continue;
        }
        let expected_size = entry.size();
        let mut buf = Vec::with_capacity(expected_size.min(16 * 1024 * 1024) as usize);
        entry.read_to_end(&mut buf)?;
        let expected = asteria_common::hash::sha256_bytes(&buf);
        if target.exists() {
            if fs::metadata(&target)?.len() != expected_size || sha256_file(&target)? != expected {
                bail!("既存の解析用ファイルを上書きしません: {name}");
            }
        } else {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut out_f = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&target)?;
            out_f.write_all(&buf)?;
            out_f.flush()?;
            if sha256_file(&target)? != expected {
                bail!("展開後のSHAが一致しません");
            }
        }
        let rel = target
            .strip_prefix(root)?
            .to_string_lossy()
            .replace('\\', "/");
        files.insert(rel, expected);
    }
    Ok(files)
}

/// `stage_tokenizer.py`: `tokenizer_sources.json` から SudachiDict を検証・展開する。
pub fn stage_tokenizer(repo_root: &Path, out: &Path, cache: Option<&Path>) -> Result<Value> {
    let config_path = repo_root.join("training/lexicon/tokenizer_sources.json");
    let config = load_sources(&out.join("source"), cache, &config_path)?;
    let mut files_map = serde_json::Map::new();
    for spec in config["files"].as_array().context("files")? {
        let name = spec["name"].as_str().context("name")?;
        let subdir = if name.ends_with(".whl") {
            "python"
        } else {
            "dictionary"
        };
        let extracted = extract_verified_zip(&out.join("source").join(name), &out.join(subdir))?;
        files_map.insert(subdir.to_owned(), json!(extracted));
    }
    let mut dicts = Vec::new();
    find_files_named(&out.join("dictionary"), "system_full.dic", &mut dicts)?;
    dicts.sort();
    if dicts.len() != 1 {
        bail!("full辞書の展開結果を一意に決められません");
    }
    let dict_path = fs::canonicalize(&dicts[0])?;
    let py_root = fs::canonicalize(out.join("python")).unwrap_or_else(|_| out.join("python"));
    let result = json!({
        "version": config["version"],
        "config_sha256": sha256_file(&config_path)?,
        "python_root": py_root.to_string_lossy(),
        "dictionary": dict_path.to_string_lossy(),
        "files": files_map,
        "dictionary_sha256": sha256_file(&dict_path)?,
        "training_only": true,
    });
    fs::write(
        out.join("runtime.json"),
        format!("{}\n", serde_json::to_string_pretty(&result)?),
    )?;
    Ok(result)
}

fn find_files_named(dir: &Path, target: &str, out: &mut Vec<PathBuf>) -> Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let p = entry.path();
        if p.is_dir() {
            find_files_named(&p, target, out)?;
        } else if p.file_name().and_then(|n| n.to_str()) == Some(target) {
            out.push(p);
        }
    }
    Ok(())
}

/// `build_lexicon.py`: SudachiDict の 1 行（ヘッダマップによる検索）を Mozc 候補へ変換する。
#[derive(Debug, Clone, Deserialize)]
pub struct SudachiCsvRow {
    #[serde(rename = "IndexForm")]
    pub index_form: String,
    #[serde(rename = "LeftId")]
    pub left_id: String,
    #[serde(rename = "RightId")]
    pub right_id: String,
    #[serde(rename = "Cost")]
    pub cost: String,
    #[serde(rename = "POS1")]
    pub pos1: String,
    #[serde(rename = "POS2")]
    pub pos2: String,
    #[serde(rename = "POS3")]
    pub pos3: String,
    #[serde(rename = "POS4")]
    pub pos4: String,
    #[serde(rename = "ReadingForm")]
    pub reading_form: String,
    #[serde(rename = "Headword")]
    pub headword: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LexiconCandidate {
    pub reading: String,
    pub surface: String,
    pub pos: &'static str,
    pub cost: i64,
}

pub fn lexicon_candidate(row: &SudachiCsvRow) -> Result<(Option<LexiconCandidate>, &'static str)> {
    if row.pos1 != "名詞" {
        return Ok((None, "not_noun"));
    }
    let Some(pos) = pos_mapping(&row.pos2, &row.pos3, &row.pos4) else {
        return Ok((None, "unsupported_pos"));
    };
    let lid: i64 = row.left_id.parse().unwrap_or(-1);
    let rid: i64 = row.right_id.parse().unwrap_or(-1);
    if lid < 0 || rid < 0 {
        return Ok((None, "not_indexed"));
    }
    let raw_reading = if row.reading_form.is_empty() {
        &row.index_form
    } else {
        &row.reading_form
    };
    let reading = kata_to_hira(raw_reading);
    let surface = if row.headword.is_empty() {
        &row.index_form
    } else {
        &row.headword
    };
    let s_chars: Vec<char> = surface.chars().collect();
    if !is_valid_reading(&reading)
        || !(1..=32).contains(&s_chars.len())
        || !s_chars.iter().all(|&c| is_valid_surface_char(c))
    {
        return Ok((None, "invalid_reading_or_surface"));
    }
    let cost: i64 = row.cost.parse().context("Cost 数値エラー")?;
    if !(-32768..=32767).contains(&cost) {
        bail!("元辞書の単語コストが範囲外です");
    }
    Ok((
        Some(LexiconCandidate {
            reading,
            surface: surface.to_owned(),
            pos,
            cost,
        }),
        "",
    ))
}

/// Python `bisect_left` / `bisect_right` + `round` (banker's rounding) 互換の分位点コスト変換。
pub fn common_quantile(raw_cost: i64, raw_common: &[i64], mozc_common: &[i64]) -> i64 {
    let left = raw_common.partition_point(|&x| x < raw_cost);
    let right = raw_common.partition_point(|&x| x <= raw_cost);
    let q = ((left + right) as f64) / (2.0 * (raw_common.len() as f64));
    let q = q.clamp(0.1, 0.95);
    let idx = py_round(q * ((mozc_common.len() - 1) as f64)) as usize;
    mozc_common[idx.min(mozc_common.len() - 1)]
}

/// Python 3 の `round()`（偶数丸め: round half to even）。
fn py_round(v: f64) -> i64 {
    let f = v.floor();
    let frac = v - f;
    if (frac - 0.5).abs() < 1e-12 {
        let fi = f as i64;
        if fi % 2 == 0 {
            fi
        } else {
            fi + 1
        }
    } else {
        v.round() as i64
    }
}

/// `build_lexicon.py`: SudachiDict ZIP群から `short_lexicon.{tsv,json}` と NOTICE を生成する。
pub fn build_short_lexicon(
    source: &Path,
    mozc: &Path,
    out: &Path,
    config: &Value,
    repo_root: Option<&Path>,
) -> Result<Value> {
    let started = Instant::now();
    if out.exists() {
        bail!("出力先が既に存在します: {}", out.display());
    }
    fs::create_dir_all(out)?;
    let directory = mozc.join("src/data/dictionary_oss");

    let mut ids: HashMap<String, i64> = HashMap::new();
    for line in fs::read_to_string(directory.join("id.def"))?.lines() {
        if let Some((idx_str, pos_str)) = line.split_once(' ') {
            ids.insert(pos_str.to_owned(), idx_str.parse()?);
        }
    }
    for &pos in all_pos_values() {
        if !ids.contains_key(pos) {
            bail!("Mozcに品詞がありません: {pos}");
        }
    }
    let noun_ids: HashSet<i64> = all_pos_values()
        .iter()
        .filter_map(|p| ids.get(*p).copied())
        .collect();

    let mut dict_files = Vec::new();
    for i in 0..=9 {
        for j in 0..=9 {
            let p = directory.join(format!("dictionary{i}{j}.txt"));
            if p.is_file() {
                dict_files.push(p);
            }
        }
    }
    if dict_files.len() != 10 {
        bail!("固定Mozcの辞書10ファイルが必要です");
    }

    let mut known: HashMap<(String, String), i64> = HashMap::new();
    let mut matching_pos: HashMap<(String, String, i64), i64> = HashMap::new();
    let mut reading_min: HashMap<String, i64> = HashMap::new();
    let mut reading_pos_min: HashMap<(String, i64), i64> = HashMap::new();

    for file in &dict_files {
        let reader = BufReader::new(File::open(file)?);
        for line in reader.lines() {
            let line = line?;
            let fields: Vec<&str> = line.split('\t').collect();
            if fields.len() < 5 {
                continue;
            }
            let reading = fields[0];
            let lid: i64 = fields[1].parse()?;
            let rid: i64 = fields[2].parse()?;
            let cost: i64 = fields[3].parse()?;
            let surface = fields[4];
            let pair = (reading.to_owned(), surface.to_owned());
            known
                .entry(pair)
                .and_modify(|c| *c = (*c).min(cost))
                .or_insert(cost);
            if lid == rid && noun_ids.contains(&lid) {
                matching_pos
                    .entry((reading.to_owned(), surface.to_owned(), lid))
                    .and_modify(|c| *c = (*c).min(cost))
                    .or_insert(cost);
                reading_pos_min
                    .entry((reading.to_owned(), lid))
                    .and_modify(|c| *c = (*c).min(cost))
                    .or_insert(cost);
            }
            reading_min
                .entry(reading.to_owned())
                .and_modify(|c| *c = (*c).min(cost))
                .or_insert(cost);
        }
    }

    type CommonPairMap = HashMap<(String, String), (i64, i64)>;
    let mut pairs: BTreeMap<(String, String, &'static str), i64> = BTreeMap::new();
    let mut common: HashMap<&'static str, CommonPairMap> = HashMap::new();
    let mut audit: BTreeMap<String, usize> = BTreeMap::new();

    for group in ["small", "core", "notcore"] {
        let zip_path = source.join(format!("{group}_lex.zip"));
        let mut archive = ZipArchive::new(File::open(&zip_path)?)?;
        let csv_entry = archive.by_name(&format!("{group}_lex.csv"))?;
        let mut rdr = csv::Reader::from_reader(csv_entry);
        for rec in rdr.deserialize::<SudachiCsvRow>() {
            let row = rec?;
            *audit.entry("source_rows".into()).or_insert(0) += 1;
            let (cand_opt, reason) = lexicon_candidate(&row)?;
            let Some(cand) = cand_opt else {
                *audit.entry(reason.into()).or_insert(0) += 1;
                continue;
            };
            let pair = (cand.reading.clone(), cand.surface.clone());
            if known.contains_key(&pair) {
                let pos_id = ids[cand.pos];
                if let Some(&m_cost) = matching_pos.get(&(cand.reading, cand.surface, pos_id)) {
                    common
                        .entry(cand.pos)
                        .or_default()
                        .entry(pair)
                        .and_modify(|e| e.0 = e.0.min(cand.cost))
                        .or_insert((cand.cost, m_cost));
                }
                *audit.entry("already_in_mozc".into()).or_insert(0) += 1;
                continue;
            }
            pairs
                .entry((cand.reading, cand.surface, cand.pos))
                .and_modify(|c| *c = (*c).min(cand.cost))
                .or_insert(cand.cost);
        }
    }

    let mut distributions: HashMap<&'static str, (Vec<i64>, Vec<i64>)> = HashMap::new();
    for (&pos, values) in &common {
        if values.len() >= MIN_COMMON {
            let mut raw_vec: Vec<i64> = values.values().map(|&(r, _)| r).collect();
            let mut mozc_vec: Vec<i64> = values.values().map(|&(_, c)| c).collect();
            raw_vec.sort_unstable();
            mozc_vec.sort_unstable();
            distributions.insert(pos, (raw_vec, mozc_vec));
        }
    }

    let mut count: BTreeMap<String, usize> = BTreeMap::new();
    let out_tsv = out.join("short_lexicon.tsv");
    {
        let mut tsv_body = String::new();
        for ((reading, surface, pos), raw) in &pairs {
            let Some((raw_common, mozc_common)) = distributions.get(pos) else {
                *audit.entry("insufficient_common_pos".into()).or_insert(0) += 1;
                continue;
            };
            let mut mapped = common_quantile(*raw, raw_common, mozc_common);
            if let Some(&r_min) = reading_min.get(reading) {
                mapped = mapped.max(r_min + KNOWN_READING_MARGIN);
            }
            let pos_id = ids[*pos];
            if let Some(&rp_min) = reading_pos_min.get(&(reading.clone(), pos_id)) {
                mapped = mapped.max(rp_min + KNOWN_READING_MARGIN);
            }
            if mapped > 32767 {
                *audit.entry("cost_overflow".into()).or_insert(0) += 1;
                continue;
            }
            tsv_body.push_str(&format!(
                "{reading}\t{pos_id}\t{pos_id}\t{mapped}\t{surface}\n"
            ));
            *count.entry((*pos).to_owned()).or_insert(0) += 1;
        }
        fs::write(&out_tsv, tsv_body.as_bytes())?;
    }

    let mut mozc_inputs = BTreeMap::new();
    for p in &dict_files {
        let n = p
            .file_name()
            .and_then(|x| x.to_str())
            .unwrap_or("")
            .to_owned();
        mozc_inputs.insert(n, sha256_file(p)?);
    }
    let id_def = directory.join("id.def");
    mozc_inputs.insert("id.def".into(), sha256_file(&id_def)?);

    let cal_common: BTreeMap<String, usize> = common
        .iter()
        .map(|(&k, v)| (k.to_owned(), v.len()))
        .collect();
    let output_entries: usize = count.values().sum();
    let src_cfg_sha = repo_root
        .map(|r| sha256_file(&r.join("training/lexicon/sources.json")))
        .transpose()?
        .unwrap_or_default();

    let summary = json!({
        "source": config["repository"],
        "version": config["version"],
        "source_commit": config["commit"],
        "source_config_sha256": src_cfg_sha,
        "audit": audit,
        "output_entries": output_entries,
        "by_pos": count,
        "calibration_common": cal_common,
        "min_common": MIN_COMMON,
        "known_reading_margin": KNOWN_READING_MARGIN,
        "same_pos_reading_floor": true,
        "mozc_source_inputs": mozc_inputs,
        "bytes": fs::metadata(&out_tsv)?.len(),
        "sha256": sha256_file(&out_tsv)?,
        "seconds": started.elapsed().as_secs_f64(),
        "evaluation_labels_used": false,
        "product_adopted": false,
    });
    fs::write(
        out.join("short_lexicon.json"),
        format!("{}\n", serde_json::to_string_pretty(&summary)?),
    )?;
    let legal = fs::read_to_string(source.join("LEGAL"))?;
    let license2 = fs::read_to_string(source.join("LICENSE-2.0.txt"))?;
    let notice = format!(
        "Asteria短文向け語彙候補 / SudachiDict {}\n\
         出典: Works Applications Co., Ltd. / https://github.com/WorksApplications/SudachiDict\n\
         変更: 名詞の読み/表記を抽出し、Mozc品詞・共通語彙コスト分布へ対応。既存語は上書きしない。\n\
         元の接続行列・形態素解析エンジンは同梱しない。\n\n\
         {legal}\n\n{license2}",
        config["version"].as_str().unwrap_or("")
    );
    fs::write(out.join("NOTICE-short-lexicon.txt"), notice)?;
    Ok(summary)
}

/// `build_frequency_lexicon.py`: 頻度集計に基づいて語彙候補を選別しコスト加算する。
pub fn select_by_frequency(
    cost: i64,
    reading: &str,
    pos: &str,
    frequency: Option<(u64, u64, u64)>,
    policy: &Value,
) -> Result<(Option<i64>, &'static str)> {
    let Some((occurrences, documents, buckets)) = frequency else {
        return Ok((None, "unobserved"));
    };
    let minimum = if pos.contains(",人名,") {
        policy["min_person_documents"]
            .as_u64()
            .context("min_person_documents")?
    } else {
        policy["min_documents"].as_u64().context("min_documents")?
    };
    if documents < minimum {
        return Ok((None, "insufficient_distinct_texts"));
    }
    let min_buckets = policy["min_content_buckets"]
        .as_u64()
        .context("min_content_buckets")? as u32;
    if buckets.count_ones() < min_buckets {
        return Ok((None, "insufficient_content_buckets"));
    }
    if buckets == 0 || buckets >= 65536 || documents == 0 || documents > occurrences {
        bail!("頻度集計値が不正です");
    }
    let rarity_margin = policy["rarity_margin"].as_f64().context("rarity_margin")?;
    let reduction = policy["cost_reduction_per_frequency_doubling"]
        .as_f64()
        .context("cost_reduction_per_frequency_doubling")?;
    let rarity =
        py_round(rarity_margin - reduction * ((documents as f64) / (minimum as f64)).log2()).max(0);
    let start_len = policy["reading_length_margin_start"]
        .as_i64()
        .context("reading_length_margin_start")?;
    let per_char = policy["cost_per_additional_reading_character"]
        .as_i64()
        .context("cost_per_additional_reading_character")?;
    let r_len = reading.chars().count() as i64;
    let length_pen = (r_len - start_len).max(0) * per_char;
    let result = cost + rarity + length_pen;
    let max_cost = policy["max_cost"].as_i64().context("max_cost")?;
    if result > max_cost {
        return Ok((None, "cost_overflow"));
    }
    Ok((Some(result), ""))
}

pub fn build_frequency_lexicon(
    repo_root: &Path,
    candidate_dir: &Path,
    frequency_dir: &Path,
    mozc: &Path,
    out: &Path,
) -> Result<Value> {
    let started = Instant::now();
    let policy_path = repo_root.join("training/lexicon/frequency_lexicon_policy.json");
    let policy: Value = serde_json::from_str(&fs::read_to_string(&policy_path)?)?;
    let lexical: Value = serde_json::from_str(&fs::read_to_string(
        candidate_dir.join("short_lexicon.json"),
    )?)?;
    if Some(sha256_file(&candidate_dir.join("short_lexicon.tsv"))?.as_str())
        != lexical["sha256"].as_str()
    {
        bail!("元の語彙候補SHAが一致しません");
    }
    let counts: Value =
        serde_json::from_str(&fs::read_to_string(frequency_dir.join("frequency.json"))?)?;
    let corpus_cfg: Value = serde_json::from_str(&fs::read_to_string(
        repo_root.join("training/lexicon/corpus_sources.json"),
    )?)?;
    let freq_policy_sha = sha256_file(&repo_root.join("training/lexicon/frequency_policy.json"))?;
    if counts["limit_bytes"].as_u64() != Some(0)
        || counts["corpus"] != corpus_cfg
        || counts["sharding"]["full_source_sha256"].as_str()
            != corpus_cfg["files"][0]["sha256"].as_str()
        || counts["policy_sha256"].as_str() != Some(freq_policy_sha.as_str())
        || counts["exclusion_files"]
            .as_object()
            .map_or(true, |m| m.is_empty())
        || counts["reserved_interval_readings"].as_u64().unwrap_or(0) == 0
    {
        bail!("固定した全量・評価除外済みの頻度集計が必要です");
    }
    let data_file = frequency_dir.join("word_frequency.tsv");
    let exp_out = counts["outputs"]
        .as_array()
        .context("outputs")?
        .iter()
        .find(|s| s["file"].as_str() == Some("word_frequency.tsv"))
        .context("word_frequency.tsv missing")?;
    if Some(fs::metadata(&data_file)?.len()) != exp_out["bytes"].as_u64()
        || Some(sha256_file(&data_file)?.as_str()) != exp_out["sha256"].as_str()
    {
        bail!("頻度TSVのSHAが一致しません");
    }
    let directory = mozc.join("src/data/dictionary_oss");
    if let Some(obj) = lexical["mozc_source_inputs"].as_object() {
        for (name, exp_sha) in obj {
            if Some(sha256_file(&directory.join(name))?.as_str()) != exp_sha.as_str() {
                bail!("比較基準のMozc辞書が変更されています: {name}");
            }
        }
    }
    let mut ids: HashMap<i64, String> = HashMap::new();
    for line in fs::read_to_string(directory.join("id.def"))?.lines() {
        if let Some((idx_s, pos)) = line.split_once(' ') {
            ids.insert(idx_s.parse()?, pos.to_owned());
        }
    }
    let mut frequencies: HashMap<(String, String, String), (u64, u64, u64)> = HashMap::new();
    for line in BufReader::new(File::open(&data_file)?).lines() {
        let line = line?;
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() != 6 {
            continue;
        }
        let key = (f[0].to_owned(), f[1].to_owned(), f[2].to_owned());
        let val = (f[3].parse()?, f[4].parse()?, f[5].parse()?);
        if frequencies.insert(key, val).is_some() {
            bail!("頻度TSVのキーが重複しています");
        }
    }

    if out.exists() {
        bail!("出力先が既に存在します: {}", out.display());
    }
    fs::create_dir_all(out)?;
    let output_path = out.join("short_lexicon.tsv");
    let mut out_body = String::new();
    let mut audit: BTreeMap<String, usize> = BTreeMap::new();
    let mut selected_pos: BTreeMap<String, usize> = BTreeMap::new();

    for line in BufReader::new(File::open(candidate_dir.join("short_lexicon.tsv"))?).lines() {
        let line = line?;
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() != 5 {
            continue;
        }
        let (reading, left, right, cost_s, surface) = (f[0], f[1], f[2], f[3], f[4]);
        let lid: i64 = left.parse()?;
        if left != right || !ids.contains_key(&lid) {
            bail!("元の語彙候補品詞が一致しません");
        }
        let pos = &ids[&lid];
        let key = (reading.to_owned(), nfkc(surface), pos.clone());
        let (mapped_opt, reason) = select_by_frequency(
            cost_s.parse()?,
            reading,
            pos,
            frequencies.get(&key).copied(),
            &policy,
        )?;
        *audit.entry("input_entries".into()).or_insert(0) += 1;
        let Some(mapped) = mapped_opt else {
            *audit.entry(reason.into()).or_insert(0) += 1;
            continue;
        };
        out_body.push_str(&format!(
            "{reading}\t{left}\t{right}\t{mapped}\t{surface}\n"
        ));
        *audit.entry("selected_entries".into()).or_insert(0) += 1;
        *selected_pos.entry(pos.clone()).or_insert(0) += 1;
    }
    fs::write(&output_path, out_body.as_bytes())?;

    let mut summary = lexical;
    if let Some(obj) = summary.as_object_mut() {
        obj.insert(
            "parent_candidate_sha256".into(),
            obj.get("sha256").cloned().unwrap_or(Value::Null),
        );
        obj.insert("policy".into(), policy);
        obj.insert("policy_sha256".into(), json!(sha256_file(&policy_path)?));
        obj.insert(
            "frequency_manifest_sha256".into(),
            json!(sha256_file(&frequency_dir.join("frequency.json"))?),
        );
        obj.insert(
            "frequency_data_sha256".into(),
            json!(sha256_file(&data_file)?),
        );
        obj.insert("selection_audit".into(), json!(audit));
        obj.insert(
            "output_entries".into(),
            json!(audit.get("selected_entries").copied().unwrap_or(0)),
        );
        obj.insert("by_pos".into(), json!(selected_pos));
        obj.insert("bytes".into(), json!(fs::metadata(&output_path)?.len()));
        obj.insert("sha256".into(), json!(sha256_file(&output_path)?));
        obj.insert("seconds".into(), json!(started.elapsed().as_secs_f64()));
        obj.insert("evaluation_labels_used".into(), json!(false));
        obj.insert("product_adopted".into(), json!(false));
        obj.insert(
            "license".into(),
            json!("CC-BY-SA-4.0 frequency-derived selection; SudachiDict notices retained"),
        );
    }
    fs::write(
        out.join("short_lexicon.json"),
        format!("{}\n", serde_json::to_string_pretty(&summary)?),
    )?;
    let mut notice = fs::read_to_string(candidate_dir.join("NOTICE-short-lexicon.txt"))?;
    notice.push_str("\n\n");
    notice.push_str(&fs::read_to_string(
        frequency_dir.join("NOTICE-frequency.txt"),
    )?);
    notice.push_str("\n追加変更: 全量頻度の最小本文数で選択し、希少性と読みの長さでコストを加算。頻度由来の加工データはCC BY-SA 4.0で提供。\n");
    fs::write(out.join("NOTICE-short-lexicon.txt"), notice)?;
    Ok(summary)
}

/// `build_frequency_collocations.py`: PMI / NPMI による連語関連度スコア。
pub fn collocation_association(
    joint: u64,
    first: u64,
    last: u64,
    total: u64,
    policy: &Value,
) -> Option<f64> {
    let min_docs = policy["min_documents"].as_u64()?;
    if joint < min_docs || joint > first.min(last) || first.min(last) == 0 || joint >= total {
        return None;
    }
    let jf = joint as f64;
    let tf = total as f64;
    let pmi = (jf * tf / ((first as f64) * (last as f64))).log2();
    let npmi = pmi / -(jf / tf).log2();
    if pmi < policy["min_pmi_bits"].as_f64()? || npmi < policy["min_npmi"].as_f64()? {
        return None;
    }
    Some(npmi * jf.ln_1p())
}

/// 2〜3 形態素の品詞境界チェック。
pub fn collocation_endpoints<'a>(
    tokens: &[(&'a str, &[&str])],
    policy: &Value,
) -> Option<(&'a str, &'a str)> {
    if tokens.len() != 2 && tokens.len() != 3 {
        return None;
    }
    let allowed: HashSet<&str> = policy["content_pos"]
        .as_array()?
        .iter()
        .filter_map(Value::as_str)
        .collect();
    for &(_, pos) in [&tokens[0], &tokens[tokens.len() - 1]] {
        if pos.len() < 3 || !allowed.contains(pos[0]) || pos[1] == "数詞" || pos[2] == "人名" {
            return None;
        }
    }
    if tokens.len() == 3 {
        let (middle, pos) = tokens[1];
        if pos.is_empty() {
            return None;
        }
        if pos[0] == "助詞" {
            let ok_mid = policy["allowed_middle_particles"]
                .as_array()?
                .iter()
                .filter_map(Value::as_str)
                .any(|p| p == middle);
            if !ok_mid {
                return None;
            }
        } else if !allowed.contains(pos[0]) {
            return None;
        }
    }
    Some((tokens[0].0, tokens[tokens.len() - 1].0))
}

#[derive(Debug, Clone, PartialEq)]
struct HeapItem {
    score: f64,
    docs: u64,
    text: String,
}

impl Eq for HeapItem {}
impl Ord for HeapItem {
    fn cmp(&self, other: &Self) -> Ordering {
        // Min-heap 化のため逆順
        other
            .score
            .total_cmp(&self.score)
            .then_with(|| other.docs.cmp(&self.docs))
            .then_with(|| other.text.cmp(&self.text))
    }
}
impl PartialOrd for HeapItem {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

pub fn build_frequency_collocations(
    repo_root: &Path,
    frequency_dir: &Path,
    baseline: &Path,
    runtime_path: &Path,
    out: &Path,
) -> Result<Value> {
    let started = Instant::now();
    let policy_path = repo_root.join("training/lexicon/collocation_policy.json");
    let policy: Value = serde_json::from_str(&fs::read_to_string(&policy_path)?)?;
    let metadata: Value =
        serde_json::from_str(&fs::read_to_string(frequency_dir.join("frequency.json"))?)?;
    let config: Value = serde_json::from_str(&fs::read_to_string(
        repo_root.join("training/lexicon/corpus_sources.json"),
    )?)?;
    if metadata["limit_bytes"].as_u64().unwrap_or(1) != 0
        || metadata["corpus"] != config
        || metadata["sharding"]["full_source_sha256"].as_str()
            != config["files"][0]["sha256"].as_str()
    {
        bail!("固定した全量コーパスの集計が必要です");
    }
    if sha256_file(baseline)? != BASELINE_COLLOCATION_SHA {
        bail!("公開v2.0.0の連語基準SHAが一致しません");
    }
    for name in ["phrase_frequency.tsv", "surface_frequency.tsv"] {
        let item = metadata["outputs"]
            .as_array()
            .context("outputs")?
            .iter()
            .find(|x| x["file"].as_str() == Some(name))
            .context("output missing")?;
        if Some(sha256_file(&frequency_dir.join(name))?.as_str()) != item["sha256"].as_str() {
            bail!("集計済み頻度のSHAが一致しません");
        }
    }

    let runtime = stage_tokenizer(repo_root, runtime_path, None)?;
    let dict_path = PathBuf::from(runtime["dictionary"].as_str().context("dictionary")?);
    let dict = crate::frequency::load_sudachi_dict(&dict_path)?;
    let tokenizer = sudachi::analysis::stateless_tokenizer::StatelessTokenizer::new(&dict);

    let total: u64 = metadata["shard_results"]
        .as_array()
        .context("shard_results")?
        .iter()
        .filter_map(|r| r["audit"]["unique_accepted_texts"].as_u64())
        .sum();
    let mut surfaces: HashMap<String, u64> = HashMap::new();
    for line in BufReader::new(File::open(frequency_dir.join("surface_frequency.tsv"))?).lines() {
        let line = line?;
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() == 4 {
            surfaces.insert(f[0].to_owned(), f[2].parse()?);
        }
    }
    let existing: BTreeSet<String> = fs::read_to_string(baseline)?
        .lines()
        .filter(|l| !l.is_empty())
        .map(str::to_owned)
        .collect();

    let min_len = policy["min_length"].as_u64().unwrap_or(0) as usize;
    let max_len = policy["max_length"].as_u64().unwrap_or(100) as usize;
    let min_docs = policy["min_documents"].as_u64().unwrap_or(0);
    let min_buckets = policy["min_content_buckets"].as_u64().unwrap_or(0) as u32;
    let max_new = policy["max_new_phrases"].as_u64().unwrap_or(1000) as usize;
    let max_combined = policy["max_combined_phrases"].as_u64().unwrap_or(100_000) as usize;

    let jp_re = Regex::new(r"^[\u3400-\u4dbf\u4e00-\u9fff々ぁ-ゖァ-ヺー]+$")?;
    let mut heap: BinaryHeap<HeapItem> = BinaryHeap::new();
    let mut audit: BTreeMap<String, usize> = BTreeMap::new();

    for line in BufReader::new(File::open(frequency_dir.join("phrase_frequency.tsv"))?).lines() {
        let line = line?;
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() != 4 {
            continue;
        }
        let text = f[0];
        let docs: u64 = f[2].parse()?;
        let buckets: u64 = f[3].parse()?;
        *audit.entry("input_phrases".into()).or_insert(0) += 1;
        if existing.contains(text) {
            *audit.entry("already_in_baseline".into()).or_insert(0) += 1;
            continue;
        }
        let tlen = text.chars().count();
        if !(min_len..=max_len).contains(&tlen)
            || !jp_re.is_match(text)
            || docs < min_docs
            || buckets.count_ones() < min_buckets
        {
            *audit.entry("insufficient_or_invalid".into()).or_insert(0) += 1;
            continue;
        }
        use sudachi::prelude::*;
        let morphemes = tokenizer.tokenize(text, Mode::C, false)?;
        let tok_vecs: Vec<(String, Vec<String>)> = morphemes
            .iter()
            .map(|m| {
                (
                    m.surface().to_owned(),
                    m.part_of_speech().iter().map(|s| (*s).to_owned()).collect(),
                )
            })
            .collect();
        let pos_slices: Vec<Vec<&str>> = tok_vecs
            .iter()
            .map(|(_, p)| p.iter().map(String::as_str).collect())
            .collect();
        let refs: Vec<(&str, &[&str])> = tok_vecs
            .iter()
            .zip(pos_slices.iter())
            .map(|((s, _), p)| (s.as_str(), p.as_slice()))
            .collect();
        let Some((p0, p1)) = collocation_endpoints(&refs, &policy) else {
            *audit
                .entry("unsupported_token_structure".into())
                .or_insert(0) += 1;
            continue;
        };
        let first = surfaces.get(p0).copied().unwrap_or(0);
        let last = surfaces.get(p1).copied().unwrap_or(0);
        let Some(score) = collocation_association(docs, first, last, total, &policy) else {
            *audit
                .entry("insufficient_association_or_boundary_mismatch".into())
                .or_insert(0) += 1;
            continue;
        };
        *audit.entry("eligible_phrases".into()).or_insert(0) += 1;
        let item = HeapItem {
            score,
            docs,
            text: text.to_owned(),
        };
        if heap.len() < max_new {
            heap.push(item);
        } else if let Some(top) = heap.peek() {
            if item.score > top.score || (item.score == top.score && item.docs > top.docs) {
                heap.pop();
                heap.push(item);
            }
        }
    }

    let mut combined = existing.clone();
    let heap_len = heap.len();
    for item in heap {
        combined.insert(item.text);
    }
    if heap_len == 0 || combined.len() > max_combined {
        bail!("追加連語が空または上限を超えています");
    }
    if out.exists() {
        bail!("出力先が既に存在します: {}", out.display());
    }
    fs::create_dir_all(out)?;
    let out_txt = out.join("fast_collocations.txt");
    let mut body = String::new();
    for v in &combined {
        body.push_str(v);
        body.push('\n');
    }
    fs::write(&out_txt, body.as_bytes())?;

    let old: Value = serde_json::from_str(&fs::read_to_string(baseline.with_extension("json"))?)?;
    let mut receipt = old;
    if let Some(obj) = receipt.as_object_mut() {
        obj.insert("baseline_sha256".into(), json!(BASELINE_COLLOCATION_SHA));
        obj.insert("baseline_entries".into(), json!(existing.len()));
        obj.insert(
            "source".into(),
            json!("Miwa-Keita/zenz-v2.5-dataset Wikipedia full; existing source notices retained"),
        );
        obj.insert(
            "frequency_manifest_sha256".into(),
            json!(sha256_file(&frequency_dir.join("frequency.json"))?),
        );
        obj.insert("policy".into(), policy);
        obj.insert("policy_sha256".into(), json!(sha256_file(&policy_path)?));
        obj.insert("runtime".into(), runtime);
        obj.insert(
            "added_entries".into(),
            json!(combined.len() - existing.len()),
        );
        obj.insert("combined_entries".into(), json!(combined.len()));
        obj.insert("total_unique_texts".into(), json!(total));
        obj.insert("selection_audit".into(), json!(audit));
        obj.insert("seconds".into(), json!(started.elapsed().as_secs_f64()));
        obj.insert("result_sha256".into(), json!(sha256_file(&out_txt)?));
        obj.insert("bytes".into(), json!(fs::metadata(&out_txt)?.len()));
        obj.insert("evaluation_labels_used".into(), json!(false));
        obj.insert("product_adopted".into(), json!(false));
        obj.insert("distribution_status".into(), json!("candidate_not_adopted"));
        obj.insert(
            "license".into(),
            json!("CC-BY-SA-4.0 additional full-corpus phrases; previous notices retained"),
        );
    }
    fs::write(
        out.join("fast_collocations.json"),
        format!("{}\n", serde_json::to_string_pretty(&receipt)?),
    )?;
    let old_notice = fs::read_to_string(baseline.with_file_name("NOTICE-fast-collocations.txt"))?;
    let freq_notice = fs::read_to_string(frequency_dir.join("NOTICE-frequency.txt"))?;
    let notice = format!(
        "{old_notice}\n\n追加する全量コーパス連語候補（製品採用前）\n{freq_notice}\
         \n変更: 文書頻度とPMI/NPMI、形態素境界で連語を選択。既存の連語を保持。\n\
         今回の結合結果SHA-256: {}\n",
        sha256_file(&out_txt)?
    );
    fs::write(out.join("NOTICE-fast-collocations.txt"), notice)?;
    Ok(receipt)
}

/// `build_diagnostic.py`: ASCII・全角幅展開。
pub fn diagnostic_width_variants(surface: &str) -> BTreeSet<String> {
    let normalized = nfkc(surface);
    let full: String = normalized
        .chars()
        .map(|c| {
            let cp = c as u32;
            if (33..=126).contains(&cp) {
                char::from_u32(cp + 0xfee0).unwrap_or(c)
            } else {
                c
            }
        })
        .collect();
    [surface.to_owned(), normalized, full].into_iter().collect()
}

#[derive(Debug, Default)]
pub struct DiagnosticGroup {
    pub targets: BTreeSet<String>,
    pub expected: BTreeSet<String>,
    pub positions: BTreeSet<String>,
}

pub fn choose_diagnostic(
    groups: &BTreeMap<String, DiagnosticGroup>,
    excluded: &HashSet<String>,
    policy: &Value,
) -> Result<Vec<Value>> {
    let excl_refs: Vec<&str> = excluded.iter().map(String::as_str).collect();
    let overlap = Exclusions::new(excl_refs, 0.8);
    let min_r = policy["min_reading_length"]
        .as_u64()
        .context("min_reading_length")? as usize;
    let max_r = policy["max_reading_length"]
        .as_u64()
        .context("max_reading_length")? as usize;
    let max_aliases = policy["max_target_aliases"]
        .as_u64()
        .context("max_target_aliases")? as usize;
    let max_forms = policy["max_expected_canonical_forms"]
        .as_u64()
        .context("max_expected_canonical_forms")? as usize;
    let seed = policy["seed"].as_str().context("seed")?;
    let per_pos_limit = policy["per_pos_limit"].as_u64().context("per_pos_limit")? as usize;

    let mut by_pos: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
    for (reading, item) in groups {
        let r_len = reading.chars().count();
        if !(min_r..=max_r).contains(&r_len)
            || overlap.overlaps(reading)
            || item.targets.len() > max_aliases
            || item.expected.len() > max_forms
        {
            continue;
        }
        for pos in &item.positions {
            let key = asteria_common::hash::sha256_bytes(format!("{seed}\0{reading}").as_bytes());
            by_pos
                .entry(pos.clone())
                .or_default()
                .push((key, reading.clone()));
        }
    }

    let mut selected = Vec::new();
    let mut seen = HashSet::new();
    for (_, mut values) in by_pos {
        values.sort();
        let mut count = 0usize;
        for (_, reading) in values {
            if !seen.insert(reading.clone()) {
                continue;
            }
            let item = &groups[&reading];
            let expected: BTreeSet<String> = item
                .expected
                .iter()
                .flat_map(|s| diagnostic_width_variants(s))
                .collect();
            selected.push(json!({
                "index": format!("lexical-probe-{:03}", selected.len() + 1),
                "input": reading,
                "expected_output": expected.into_iter().collect::<Vec<_>>(),
                "target_surfaces": item.targets.iter().cloned().collect::<Vec<_>>(),
                "context_text": "",
                "domain": item.positions.iter().next().cloned().unwrap_or_default(),
                "scope": "dictionary-derived lexical diagnostic, not independent evaluation",
            }));
            count += 1;
            if count >= per_pos_limit {
                break;
            }
        }
    }
    Ok(selected)
}

pub fn build_diagnostic(
    repo_root: &Path,
    candidate_dir: &Path,
    frequency_dir: &Path,
    mozc: &Path,
    exclusions_paths: &[PathBuf],
    out: &Path,
) -> Result<Value> {
    let policy_path = repo_root.join("training/lexicon/diagnostic_policy.json");
    let policy: Value = serde_json::from_str(&fs::read_to_string(&policy_path)?)?;
    let lexical: Value = serde_json::from_str(&fs::read_to_string(
        candidate_dir.join("short_lexicon.json"),
    )?)?;
    if Some(sha256_file(&candidate_dir.join("short_lexicon.tsv"))?.as_str())
        != lexical["sha256"].as_str()
    {
        bail!("診断対象の辞書候補SHAが一致しません");
    }
    let counts: Value =
        serde_json::from_str(&fs::read_to_string(frequency_dir.join("frequency.json"))?)?;
    let exp_out = counts["outputs"]
        .as_array()
        .context("outputs")?
        .iter()
        .find(|item| item["file"].as_str() == Some("word_frequency.tsv"))
        .context("word_frequency.tsv")?;
    if counts["limit_bytes"].as_u64().unwrap_or(1) != 0
        || Some(sha256_file(&frequency_dir.join("word_frequency.tsv"))?.as_str())
            != exp_out["sha256"].as_str()
    {
        bail!("固定全量頻度が必要です");
    }
    let directory = mozc.join("src/data/dictionary_oss");
    let mozc_inputs = lexical["mozc_source_inputs"]
        .as_object()
        .context("mozc_source_inputs")?;
    for (name, exp_sha) in mozc_inputs {
        if Some(sha256_file(&directory.join(name))?.as_str()) != exp_sha.as_str() {
            bail!("Mozcの基準辞書が一致しません");
        }
    }
    let mut ids: HashMap<i64, String> = HashMap::new();
    for line in fs::read_to_string(directory.join("id.def"))?.lines() {
        if let Some((idx_s, pos)) = line.split_once(' ') {
            ids.insert(idx_s.parse()?, pos.to_owned());
        }
    }
    let valid_pos: HashSet<&str> = all_pos_values().iter().copied().collect();
    let noun_ids: HashSet<i64> = ids
        .iter()
        .filter(|(_, pos)| valid_pos.contains(pos.as_str()))
        .map(|(&idx, _)| idx)
        .collect();

    let min_r = policy["min_reading_length"].as_u64().unwrap_or(0) as usize;
    let max_r = policy["max_reading_length"].as_u64().unwrap_or(100) as usize;
    let min_s = policy["min_surface_length"].as_u64().unwrap_or(0) as usize;
    let min_docs = policy["min_documents"].as_u64().unwrap_or(0);

    let mut eligible: HashMap<(String, String, String), String> = HashMap::new();
    for line in BufReader::new(File::open(candidate_dir.join("short_lexicon.tsv"))?).lines() {
        let line = line?;
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() != 5 {
            continue;
        }
        let (reading, left, surface) = (f[0], f[1], f[4]);
        let r_len = reading.chars().count();
        let s_len = surface.chars().count();
        if (min_r..=max_r).contains(&r_len) && s_len >= min_s {
            let lid: i64 = left.parse()?;
            eligible.insert(
                (reading.to_owned(), nfkc(surface), ids[&lid].clone()),
                surface.to_owned(),
            );
        }
    }

    let mut groups: BTreeMap<String, DiagnosticGroup> = BTreeMap::new();
    for line in BufReader::new(File::open(frequency_dir.join("word_frequency.tsv"))?).lines() {
        let line = line?;
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() != 6 {
            continue;
        }
        let docs: u64 = f[4].parse()?;
        let key = (f[0].to_owned(), f[1].to_owned(), f[2].to_owned());
        if let Some(orig) = eligible.get(&key) {
            if docs >= min_docs {
                let g = groups.entry(f[0].to_owned()).or_default();
                g.targets.insert(orig.clone());
                g.expected.insert(orig.clone());
                g.positions.insert(f[2].to_owned());
            }
        }
    }

    let mut dict_names: Vec<&String> = mozc_inputs
        .keys()
        .filter(|n| n.starts_with("dictionary"))
        .collect();
    dict_names.sort();
    for name in dict_names {
        for line in BufReader::new(File::open(directory.join(name))?).lines() {
            let line = line?;
            let f: Vec<&str> = line.split('\t').collect();
            if f.len() < 5 {
                continue;
            }
            if let Some(g) = groups.get_mut(f[0]) {
                let lid: i64 = f[1].parse()?;
                let rid: i64 = f[2].parse()?;
                if noun_ids.contains(&lid) && noun_ids.contains(&rid) {
                    g.expected.insert(f[4].to_owned());
                }
            }
        }
    }

    let mut excluded = HashSet::new();
    let mut excl_hashes = BTreeMap::new();
    for path in exclusions_paths {
        let val: Value = serde_json::from_str(&fs::read_to_string(path)?)?;
        for r in evaluation_readings(&val) {
            excluded.insert(r);
        }
        let abs = fs::canonicalize(path).unwrap_or_else(|_| path.clone());
        excl_hashes.insert(abs.to_string_lossy().into_owned(), sha256_file(path)?);
    }
    if excluded.is_empty() {
        bail!("既存評価の除外集合が空です");
    }

    let result = choose_diagnostic(&groups, &excluded, &policy)?;
    if result.len() < 100 {
        bail!("十分な分野数の診断を作れませんでした");
    }
    if out.exists() {
        bail!("出力先が既に存在します: {}", out.display());
    }
    fs::create_dir_all(out)?;
    let data_path = out.join("lexical-diagnostic.json");
    fs::write(
        &data_path,
        format!("{}\n", serde_json::to_string_pretty(&result)?),
    )?;
    let metadata = json!({
        "policy": policy,
        "policy_sha256": sha256_file(&policy_path)?,
        "lexical_sha256": lexical["sha256"],
        "frequency_sha256": exp_out["sha256"],
        "exclusion_files": excl_hashes,
        "mozc_source_inputs": lexical["mozc_source_inputs"],
        "items": result.len(),
        "sha256": sha256_file(&data_path)?,
        "independent_evaluation": false,
        "representative_general_accuracy": false,
        "evaluation_labels_used_for_selection": false,
        "source_lexical_references_used": true,
    });
    fs::write(
        out.join("manifest.json"),
        format!("{}\n", serde_json::to_string_pretty(&metadata)?),
    )?;
    fs::copy(
        candidate_dir.join("NOTICE-short-lexicon.txt"),
        out.join("NOTICE.txt"),
    )?;
    Ok(metadata)
}

#[cfg(test)]
mod tests {
    use super::*;
    use asteria_common::hash::sha256_bytes;
    use tempfile::TempDir;
    use zip::write::SimpleFileOptions;
    use zip::ZipWriter;

    fn sample_word(overrides: &[(&str, &str)]) -> SudachiCsvRow {
        let mut row = SudachiCsvRow {
            index_form: "新語".into(),
            left_id: "1".into(),
            right_id: "1".into(),
            cost: "5000".into(),
            pos1: "名詞".into(),
            pos2: "普通名詞".into(),
            pos3: "一般".into(),
            pos4: "*".into(),
            reading_form: "シンゴ".into(),
            headword: "".into(),
        };
        for &(k, v) in overrides {
            match k {
                "IndexForm" => row.index_form = v.into(),
                "Headword" => row.headword = v.into(),
                "ReadingForm" => row.reading_form = v.into(),
                "Cost" => row.cost = v.into(),
                "POS1" => row.pos1 = v.into(),
                "POS2" => row.pos2 = v.into(),
                "POS3" => row.pos3 = v.into(),
                "POS4" => row.pos4 = v.into(),
                "LeftId" => row.left_id = v.into(),
                "RightId" => row.right_id = v.into(),
                _ => {}
            }
        }
        row
    }

    #[test]
    fn lexicon_candidate_preserves_case_and_rejects_bad_rows() {
        let (c, _) = lexicon_candidate(&sample_word(&[
            ("IndexForm", "example"),
            ("Headword", "Example"),
            ("ReadingForm", "エグザンプル"),
        ]))
        .unwrap();
        let c = c.unwrap();
        assert_eq!(
            (c.reading.as_str(), c.surface.as_str()),
            ("えぐざんぷる", "Example")
        );

        for bad in [
            vec![("ReadingForm", "しん\nご")],
            vec![("ReadingForm", "*")],
            vec![("ReadingForm", "ABC")],
            vec![("Headword", "新\t語")],
            vec![("Headword", "空 白")],
            vec![("Headword", r"\u002c")],
            vec![("LeftId", "-1")],
            vec![("POS1", "動詞")],
            vec![("POS3", "未知の品詞")],
        ] {
            assert!(lexicon_candidate(&sample_word(&bad)).unwrap().0.is_none());
        }
    }

    #[test]
    fn cost_mapping_is_monotone() {
        let raw = [1, 1, 4, 9, 10];
        let target = [3000, 5000, 7000, 9000, 11000];
        let costs: Vec<i64> = [-10, 1, 4, 8, 9, 10, 30]
            .iter()
            .map(|&c| common_quantile(c, &raw, &target))
            .collect();
        let mut sorted = costs.clone();
        sorted.sort_unstable();
        assert_eq!(costs, sorted);
        assert_eq!(common_quantile(1, &raw, &target), 5000);
        assert!(*costs.iter().min().unwrap() >= 3000);
    }

    #[test]
    fn corrupt_cache_and_unexpected_zip_member_rejected() {
        let dir = TempDir::new().unwrap();
        let cache = dir.path().join("cache");
        let out = dir.path().join("out");
        fs::create_dir_all(&cache).unwrap();
        fs::create_dir_all(&out).unwrap();
        fs::write(cache.join("data"), b"wrong").unwrap();
        let spec = json!({
            "name": "data",
            "bytes": 5,
            "sha256": sha256_bytes(b"right"),
            "url": "https://invalid.example/data",
        });
        assert!(fetch_source(&spec, &out, Some(&cache)).is_err());
        assert_eq!(fs::read(cache.join("data")).unwrap(), b"wrong");
        assert!(!out.join("data").exists());

        let zip_path = dir.path().join("data.zip");
        {
            let mut zw = ZipWriter::new(File::create(&zip_path).unwrap());
            zw.start_file("../unexpected.csv", SimpleFileOptions::default())
                .unwrap();
            zw.write_all(b"word").unwrap();
            zw.finish().unwrap();
        }
        let zip_spec = json!({
            "name": "data.zip",
            "bytes": fs::metadata(&zip_path).unwrap().len(),
            "sha256": sha256_file(&zip_path).unwrap(),
            "members": [{"name": "lex.csv", "bytes": 4}],
        });
        assert!(verify_source_file(&zip_path, &zip_spec).is_err());
    }

    #[test]
    fn zip_escape_and_package_validation() {
        let dir = TempDir::new().unwrap();
        for bad in ["../escape", "/absolute", r"..\escape", "C:/absolute"] {
            assert!(safe_zip_member_path(dir.path(), bad, false).is_err());
        }
        let zip_path = dir.path().join("valid.zip");
        {
            let mut zw = ZipWriter::new(File::create(&zip_path).unwrap());
            zw.start_file("package/main.py", SimpleFileOptions::default())
                .unwrap();
            zw.write_all(b"original").unwrap();
            zw.finish().unwrap();
        }
        let out = dir.path().join("ext");
        extract_verified_zip(&zip_path, &out).unwrap();
        fs::write(out.join("package/main.py"), b"changed").unwrap();
        assert!(extract_verified_zip(&zip_path, &out).is_err());
        assert_eq!(fs::read(out.join("package/main.py")).unwrap(), b"changed");
    }

    #[test]
    fn collocation_association_and_endpoints_match_policy() {
        let policy = json!({
            "min_documents": 20, "min_pmi_bits": 2.0, "min_npmi": 0.1,
            "content_pos": ["名詞", "動詞", "形容詞"],
            "allowed_middle_particles": ["を", "に", "が", "で", "へ", "と", "から", "より", "まで"]
        });
        assert!(collocation_association(30, 100, 200, 100000, &policy).is_some());
        assert!(collocation_association(30, 1000, 3000, 100000, &policy).is_none());
        assert!(collocation_association(10, 100, 200, 100000, &policy).is_none());
        assert!(collocation_association(300, 100, 200, 100000, &policy).is_none());

        let noun = ["名詞", "普通名詞", "一般"];
        let verb = ["動詞", "一般", "*"];
        let part = ["助詞", "格助詞", "*"];
        assert_eq!(
            collocation_endpoints(&[("本", &noun), ("を", &part), ("読む", &verb)], &policy),
            Some(("本", "読む"))
        );
        assert!(
            collocation_endpoints(&[("本", &noun), ("の", &part), ("値段", &noun)], &policy)
                .is_none()
        );
        assert!(collocation_endpoints(
            &[("人名", &["名詞", "固有名詞", "人名"]), ("作品", &noun)],
            &policy
        )
        .is_none());
    }

    #[test]
    fn diagnostic_width_and_choose() {
        assert_eq!(
            diagnostic_width_variants("日本AI2"),
            ["日本AI2", "日本ＡＩ２"]
                .into_iter()
                .map(str::to_owned)
                .collect()
        );
        let policy = json!({
            "min_reading_length": 4, "max_reading_length": 20,
            "max_target_aliases": 2, "max_expected_canonical_forms": 4,
            "seed": "seed", "per_pos_limit": 10
        });
        let mut groups = BTreeMap::new();
        groups.insert(
            "じんこうことばあつめ".into(),
            DiagnosticGroup {
                targets: ["新しい語".into()].into_iter().collect(),
                expected: ["新しい語".into(), "従来の同音語".into()]
                    .into_iter()
                    .collect(),
                positions: ["名詞1".into(), "名詞2".into()].into_iter().collect(),
            },
        );
        groups.insert(
            "じんこうことばしらべ".into(),
            DiagnosticGroup {
                targets: ["除外語".into()].into_iter().collect(),
                expected: ["除外語".into()].into_iter().collect(),
                positions: ["名詞1".into()].into_iter().collect(),
            },
        );
        let excl: HashSet<String> = ["じんこうことばしらべ".into()].into_iter().collect();
        let res = choose_diagnostic(&groups, &excl, &policy).unwrap();
        assert_eq!(res.len(), 1);
        assert_eq!(res[0]["input"], "じんこうことばあつめ");
    }
}
