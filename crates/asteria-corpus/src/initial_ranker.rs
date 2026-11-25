//! 初期統計順位のオフライン実験（`training/initial_ranker/*.py` 移植）。

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{bail, Context, Result};
use flate2::read::GzDecoder;
use flate2::{Compression, GzBuilder};
use rand::seq::SliceRandom;
use rand::SeedableRng;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use unicode_normalization::UnicodeNormalization;

use asteria_common::hash::sha256_file;
use asteria_common::kana::{kata_to_hira, reading_key, Exclusions};

pub const WEIGHTS: &[i64] = &[0, 25, 50, 100, 200, 400];
pub const MARGINS: &[i64] = &[0, 100, 250];
pub const MAX_COST_DELTA: i64 = 3453;

/// 文字 3-gram 用テキスト正規化（NFKC → 空白折りたたみ）。制御文字 `\x02`/`\x03` は拒否する。
pub fn normalize_text(text: &str) -> Result<String> {
    if text.contains('\x02') || text.contains('\x03') {
        bail!("Reserved boundary character");
    }
    let norm: String = text.nfkc().collect();
    Ok(norm.split_whitespace().collect::<Vec<_>>().join(" "))
}

/// 文字 3-gram 言語モデル（Witten-Bell / 加法スムージング相当）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharacterModel {
    pub counts: [BTreeMap<String, u64>; 3],
    pub total: u64,
    pub contexts: [HashMap<String, u64>; 2],
}

impl CharacterModel {
    /// 3 次のカウント配列からモデルを構築する。
    pub fn from_counts(counts: [BTreeMap<String, u64>; 3]) -> Result<Self> {
        let total: u64 = counts[0].values().sum();
        if total == 0 {
            bail!("Empty training text");
        }
        let mut contexts: [HashMap<String, u64>; 2] = [HashMap::new(), HashMap::new()];
        for n in 1..=2 {
            for (key, &count) in &counts[n] {
                let mut chars: Vec<char> = key.chars().collect();
                chars.pop();
                let ctx: String = chars.into_iter().collect();
                *contexts[n - 1].entry(ctx).or_insert(0) += count;
            }
        }
        Ok(Self {
            counts,
            total,
            contexts,
        })
    }

    /// テキスト群から 1〜3-gram を学習する。
    pub fn fit<'a>(texts: impl IntoIterator<Item = &'a str>) -> Result<Self> {
        let mut counts: [BTreeMap<String, u64>; 3] =
            [BTreeMap::new(), BTreeMap::new(), BTreeMap::new()];
        for text in texts {
            let norm = normalize_text(text)?;
            let mut chars: Vec<char> = Vec::with_capacity(norm.chars().count() + 3);
            chars.push('\x02');
            chars.push('\x02');
            chars.extend(norm.chars());
            chars.push('\x03');
            for i in 2..chars.len() {
                for order in 1..=3 {
                    let key: String = chars[(i + 1 - order)..=i].iter().collect();
                    *counts[order - 1].entry(key).or_insert(0) += 1;
                }
            }
        }
        Self::from_counts(counts)
    }

    /// 指定読み長に対する負の対数確率コストを計算する。
    pub fn cost(&self, text: &str, reading_length: usize) -> Result<f64> {
        let norm = normalize_text(text)?;
        let mut chars: Vec<char> = Vec::with_capacity(norm.chars().count() + 3);
        chars.push('\x02');
        chars.push('\x02');
        chars.extend(norm.chars());
        chars.push('\x03');
        let mut loss = 0.0;
        let denom0 = (self.total as f64) + 0.1 * ((self.counts[0].len() + 1) as f64);
        for i in 2..chars.len() {
            let ch_key = chars[i].to_string();
            let c0 = self.counts[0].get(&ch_key).copied().unwrap_or(0) as f64;
            let mut prob = (c0 + 0.1) / denom0;
            for order in 2..=3 {
                let key: String = chars[(i + 1 - order)..=i].iter().collect();
                let ctx: String = chars[(i + 1 - order)..i].iter().collect();
                let ck = self.counts[order - 1].get(&key).copied().unwrap_or(0) as f64;
                let cc = self.contexts[order - 2].get(&ctx).copied().unwrap_or(0) as f64;
                prob = (ck + 8.0 * prob) / (cc + 8.0);
            }
            loss -= prob.ln();
        }
        let body_len = (chars.len() - 2) as f64;
        Ok(loss / body_len * (reading_length.max(1) as f64))
    }

    /// 決定的な gzip (`mtime=0`, 空ファイル名) でモデルを保存する。
    pub fn save(&self, path: &Path) -> Result<()> {
        let payload = serde_json::to_vec(&self.counts)?;
        let file = OpenOptions::new().write(true).create_new(true).open(path)?;
        let mut gz = GzBuilder::new()
            .mtime(0)
            .write(file, Compression::default());
        gz.write_all(&payload)?;
        let mut f = gz.finish()?;
        f.flush()?;
        Ok(())
    }

    /// gzip JSON モデルを読み込む。
    pub fn load(path: &Path) -> Result<Self> {
        let mut decoder = GzDecoder::new(File::open(path)?);
        let mut buf = String::new();
        decoder.read_to_string(&mut buf)?;
        let counts: [BTreeMap<String, u64>; 3] = serde_json::from_str(&buf)?;
        Self::from_counts(counts)
    }
}

/// 数字・英字・区切り記号を弱い文字統計で勝手に書き換えないガード。
pub fn preserves_explicit_forms(current: &str, candidate: &str) -> Result<bool> {
    let cur = normalize_text(current)?;
    let cand = normalize_text(candidate)?;
    let num_re = Regex::new(r"[0-9一二三四五六七八九十百千万億兆〇零壱弐参拾]+")?;
    let orig_nums: Vec<&str> = num_re.find_iter(&cur).map(|m| m.as_str()).collect();
    let cand_nums: Vec<&str> = num_re.find_iter(&cand).map(|m| m.as_str()).collect();
    if !orig_nums.is_empty() && orig_nums != cand_nums {
        return Ok(false);
    }
    let alpha_re = Regex::new(r"[A-Za-z]+")?;
    let orig_alpha: Vec<&str> = alpha_re.find_iter(&cur).map(|m| m.as_str()).collect();
    let cand_alpha: Vec<&str> = alpha_re.find_iter(&cand).map(|m| m.as_str()).collect();
    if orig_alpha != cand_alpha {
        return Ok(false);
    }
    let seps = |s: &str| -> String { s.chars().filter(|&c| is_punct_or_symbol(c)).collect() };
    Ok(seps(&cur) == seps(&cand))
}

fn is_punct_or_symbol(c: char) -> bool {
    // ASCII punctuation/symbols + Japanese punctuation/symbols
    c.is_ascii_punctuation()
        || matches!(
            c,
            '、'..='〜' | '・' | 'ー' | '！'..='／' | '：'..='＠' | '［'..='｀' | '｛'..='～'
        )
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ScoredCandidate {
    pub text: String,
    pub cost_delta: i64,
    pub stat_cost: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CalibrationRow {
    pub top1: String,
    pub references: Vec<String>,
    pub scored: Vec<ScoredCandidate>,
}

pub fn ranked_choice(row: &CalibrationRow, weight: i64, margin: i64) -> Result<String> {
    if weight == 0 {
        return Ok(row.top1.clone());
    }
    let current = row
        .scored
        .iter()
        .find(|c| c.text == row.top1)
        .context("current top1 not in scored")?;
    let mut eligible = Vec::new();
    for c in &row.scored {
        if preserves_explicit_forms(&row.top1, &c.text)? {
            eligible.push(c);
        }
    }
    let winner = eligible
        .into_iter()
        .min_by(|a, b| {
            let sa = (a.cost_delta as f64) + (weight as f64) * a.stat_cost;
            let sb = (b.cost_delta as f64) + (weight as f64) * b.stat_cost;
            sa.total_cmp(&sb)
        })
        .context("no eligible candidate")?;
    let gain = ((current.cost_delta as f64) + (weight as f64) * current.stat_cost)
        - ((winner.cost_delta as f64) + (weight as f64) * winner.stat_cost);
    Ok(if gain > (margin as f64) {
        winner.text.clone()
    } else {
        row.top1.clone()
    })
}

pub fn select_configuration(rows: &[CalibrationRow]) -> Result<(Value, Vec<Value>)> {
    if rows.is_empty() {
        bail!("Empty calibration");
    }
    #[derive(Clone)]
    struct Cfg {
        weight: i64,
        margin: i64,
        improvements: usize,
        regressions: usize,
        changed: usize,
    }
    let mut results = Vec::new();
    for &weight in WEIGHTS {
        for &margin in MARGINS {
            let mut improvements = 0usize;
            let mut regressions = 0usize;
            let mut changed = 0usize;
            for row in rows {
                let choice = ranked_choice(row, weight, margin)?;
                let old_ok = row.references.contains(&row.top1);
                let new_ok = row.references.contains(&choice);
                if !old_ok && new_ok {
                    improvements += 1;
                }
                if old_ok && !new_ok {
                    regressions += 1;
                }
                if choice != row.top1 {
                    changed += 1;
                }
            }
            results.push(Cfg {
                weight,
                margin,
                improvements,
                regressions,
                changed,
            });
        }
    }
    let chosen = results
        .iter()
        .min_by(|a, b| {
            a.regressions
                .cmp(&b.regressions)
                .then_with(|| b.improvements.cmp(&a.improvements))
                .then_with(|| a.weight.cmp(&b.weight))
                .then_with(|| b.margin.cmp(&a.margin))
        })
        .context("no results")?
        .clone();
    let to_val = |c: &Cfg| {
        json!({
            "weight": c.weight,
            "margin": c.margin,
            "improvements": c.improvements,
            "regressions": c.regressions,
            "changed": c.changed,
        })
    };
    let all: Vec<Value> = results.iter().map(to_val).collect();
    Ok((to_val(&chosen), all))
}

/// (学習用行, 較正用行, 監査カウント)。
pub type SplitRowsOutput = (Vec<Value>, Vec<Value>, BTreeMap<String, usize>);

/// `prepare.py`: Wikipedia 固定区間から train/calibration 分割を作成する。
pub fn split_rows(
    rows: &[crate::collocations::CorpusRow],
    excluded: &HashSet<String>,
    count: usize,
) -> Result<SplitRowsOutput> {
    let valid_groups: HashSet<String> = (1..=7)
        .map(|n| format!("train_wikipedia.jsonl:{n:02}"))
        .collect();
    let excl_refs: Vec<&str> = excluded.iter().map(String::as_str).collect();
    let overlaps = Exclusions::new(excl_refs, 0.8);
    let mut train_bucket = Vec::new();
    let mut dev_bucket = Vec::new();
    let mut seen = HashSet::new();
    let mut audit: BTreeMap<String, usize> = BTreeMap::new();

    for row in rows {
        if row.split != "train" || !valid_groups.contains(&row.source_group) {
            *audit.entry("wrong_source_group".into()).or_insert(0) += 1;
            continue;
        }
        let reading = reading_key(&row.input);
        let text = normalize_text(&row.output)?;
        let t_len = text.chars().count();
        if !(3..=512).contains(&t_len)
            || reading.is_empty()
            || reading
                .chars()
                .any(|c| c.is_whitespace() || (c as u32) < 32)
        {
            *audit.entry("invalid".into()).or_insert(0) += 1;
            continue;
        }
        if overlaps.overlaps(&reading) {
            *audit
                .entry("evaluation_reading_or_near".into())
                .or_insert(0) += 1;
            continue;
        }
        if !seen.insert((reading.clone(), text.clone())) {
            *audit.entry("duplicate".into()).or_insert(0) += 1;
            continue;
        }
        let item = json!({
            "input": reading,
            "output": text,
            "source_group": row.source_group,
        });
        if row.source_group.ends_with(":07") {
            dev_bucket.push(item);
        } else {
            train_bucket.push(item);
        }
    }

    let mut rng = rand::rngs::StdRng::seed_from_u64(20261018);
    dev_bucket.shuffle(&mut rng);
    let mut selected = Vec::new();
    let mut readings = HashSet::new();
    for row in dev_bucket {
        let inp = row["input"].as_str().unwrap_or("");
        let r_len = inp.chars().count();
        if (10..=80).contains(&r_len) && !readings.contains(inp) {
            readings.insert(inp.to_owned());
            selected.push(row);
            if selected.len() == count {
                break;
            }
        }
    }
    if selected.len() != count {
        bail!("Insufficient distinct calibration inputs");
    }
    let dev_refs: Vec<&str> = readings.iter().map(String::as_str).collect();
    let overlap_dev = Exclusions::new(dev_refs, 0.8);
    let texts: HashSet<&str> = selected
        .iter()
        .filter_map(|r| r["output"].as_str())
        .collect();
    let before_len = train_bucket.len();
    let train: Vec<Value> = train_bucket
        .into_iter()
        .filter(|r| {
            let outp = r["output"].as_str().unwrap_or("");
            let inp = r["input"].as_str().unwrap_or("");
            !texts.contains(outp) && !overlap_dev.overlaps(inp)
        })
        .collect();
    audit.insert("train_dev_overlap_removed".into(), before_len - train.len());
    Ok((train, selected, audit))
}

/// `collect.py`: `converter_main` の `ASTERIA_LATTICE` トレースを解析する。
pub fn native_reading(text: &str) -> String {
    reading_key(text).replace('、', ",").replace('。', ".")
}

pub fn decode_hex_utf8(hex: &str) -> Result<String> {
    if hex.len() % 2 != 0 {
        bail!("奇数長の hex 文字列です");
    }
    let mut bytes = Vec::with_capacity(hex.len() / 2);
    for i in (0..hex.len()).step_by(2) {
        let b = u8::from_str_radix(&hex[i..i + 2], 16)?;
        bytes.push(b);
    }
    Ok(String::from_utf8(bytes)?)
}

pub fn parse_lattice_blocks(raw: &[u8], readings: &[String]) -> Result<Vec<Value>> {
    let mut blocks: Vec<Vec<String>> = Vec::new();
    let mut lines: Vec<String> = Vec::new();
    let mut body = false;

    for line_bytes in raw.split(|&b| b == b'\n') {
        let line_bytes = line_bytes.strip_suffix(b"\r").unwrap_or(line_bytes);
        if line_bytes.starts_with(b"ExecCommand() return false") {
            blocks.push(std::mem::take(&mut lines));
            body = false;
        } else if line_bytes.starts_with(b"---------- Segment ") {
            body = true;
            lines.push(String::from_utf8(line_bytes.to_vec())?);
        } else if body {
            lines.push(String::from_utf8(line_bytes.to_vec())?);
        }
    }
    if blocks.len() != readings.len() {
        bail!("Native result count mismatch");
    }

    let seg_re = Regex::new(r"^---------- Segment \d+/\d+ \[.*\] ----------$")?;
    let cand_re = Regex::new(r"^\s+0/\d+ (.*)$")?;

    let mut rows = Vec::new();
    for (block_lines, reading) in blocks.iter().zip(readings.iter()) {
        let mut candidates = Vec::new();
        let mut parts = Vec::new();
        let mut want = false;
        for line in block_lines {
            if let Some(rest) = line.strip_prefix("ASTERIA_LATTICE\t") {
                let f: Vec<&str> = rest.split('\t').collect();
                if f.len() != 5 {
                    bail!("Malformed or duplicated lattice trace");
                }
                let rank: usize = f[0].parse()?;
                if rank != candidates.len() {
                    bail!("Malformed or duplicated lattice trace");
                }
                let cost: i64 = f[1].parse()?;
                let attributes: i64 = f[2].parse()?;
                let key = decode_hex_utf8(f[3])?;
                let text = decode_hex_utf8(f[4])?;
                if native_reading(&key) != native_reading(reading) || text.is_empty() {
                    bail!("Lattice reading differs from request");
                }
                candidates.push(json!({
                    "rank": rank,
                    "cost": cost,
                    "attributes": attributes,
                    "text": text,
                }));
            } else if seg_re.is_match(line) {
                want = true;
            } else if want {
                if let Some(caps) = cand_re.captures(line) {
                    parts.push(caps[1].to_owned());
                    want = false;
                }
            }
        }
        let top: String = parts.concat();
        if top.is_empty() || candidates.is_empty() {
            bail!("Empty current output or lattice");
        }
        rows.push(json!({
            "reading": reading,
            "top1": top,
            "candidates": candidates,
        }));
    }
    Ok(rows)
}

/// `collect.py` のフル実行。
pub fn collect_lattice(exe: &Path, data: &Path, out: &Path, source_commit: &str) -> Result<Value> {
    let exe = fs::canonicalize(exe)?;
    let data = fs::canonicalize(data)?;
    if out.exists() {
        bail!("出力先が既に存在します: {}", out.display());
    }
    let items: Vec<Value> = if data.extension().and_then(|e| e.to_str()) == Some("jsonl") {
        fs::read_to_string(&data)?
            .lines()
            .filter(|l| !l.trim().is_empty())
            .enumerate()
            .map(|(i, line)| {
                let r: Value = serde_json::from_str(line)?;
                Ok(json!({
                    "index": format!("calibration-{i}"),
                    "input": r["input"],
                    "expected_output": [r["output"]],
                }))
            })
            .collect::<Result<Vec<_>>>()?
    } else {
        serde_json::from_str(&fs::read_to_string(&data)?)?
    };

    let readings: Vec<String> = items
        .iter()
        .filter_map(|it| it["input"].as_str().map(kata_to_hira))
        .collect();
    if readings.is_empty()
        || readings
            .iter()
            .any(|r| r.is_empty() || r.chars().any(|c| c.is_whitespace() || (c as u32) < 32))
    {
        bail!("Empty/unsafe native argument");
    }

    fs::create_dir_all(out)?;
    let empty = out.join("empty-runtime");
    fs::create_dir_all(&empty)?;

    let temp_dir = tempfile::Builder::new()
        .prefix("asteria-initial-lattice-")
        .tempdir()?;
    let temp_s = temp_dir.path().to_string_lossy().into_owned();
    if !temp_s.is_ascii() {
        bail!("Native profile path requires ASCII");
    }
    let mut script = String::new();
    for r in &readings {
        script.push_str(&format!("start {r}\nreset\ninitial_stat_separator\n"));
    }
    let cwd = exe.parent().context("exe parent")?.join(format!(
        "{}.runfiles/_main",
        exe.file_name().and_then(|n| n.to_str()).unwrap_or("")
    ));
    let mut cmd = Command::new(&exe);
    cmd.arg("--asteria_lattice_trace=true")
        .arg(format!("--user_profile_dir={temp_s}"))
        .current_dir(&cwd)
        .env_clear();
    for (k, v) in std::env::vars() {
        if !k.starts_with("ASTERIA_") {
            cmd.env(k, v);
        }
    }
    cmd.env("ASTERIA_MODEL_DIR", &empty)
        .env("ASTERIA_RUNTIME_DIR", &empty)
        .env("ASTERIA_LM_PRELOAD", "0")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd.spawn()?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(script.as_bytes())?;
    }
    let output = child.wait_with_output()?;
    fs::write(out.join("native.stdout"), &output.stdout)?;
    fs::write(out.join("native.stderr"), &output.stderr)?;
    if !output.status.success() {
        bail!("Native converter exit {:?}", output.status.code());
    }

    let mut rows = parse_lattice_blocks(&output.stdout, &readings)?;
    for (row, item) in rows.iter_mut().zip(items.iter()) {
        if let Some(obj) = row.as_object_mut() {
            let idx = item["index"].to_string();
            let idx_str = item["index"].as_str().map(str::to_owned).unwrap_or(idx);
            obj.insert("index".into(), json!(idx_str));
            let refs = &item["expected_output"];
            if refs.is_string() {
                obj.insert("references".into(), json!([refs]));
            } else {
                obj.insert("references".into(), refs.clone());
            }
        }
    }
    let obs_path = out.join("observations.jsonl");
    let mut obs_body = String::new();
    for row in &rows {
        obs_body.push_str(&serde_json::to_string(row)?);
        obs_body.push('\n');
    }
    fs::write(&obs_path, obs_body.as_bytes())?;
    let correct = rows
        .iter()
        .filter(|r| {
            let top = r["top1"].as_str().unwrap_or("");
            r["references"]
                .as_array()
                .is_some_and(|a| a.iter().any(|x| x.as_str() == Some(top)))
        })
        .count();
    let lattice_hit = rows
        .iter()
        .filter(|r| {
            let refs: HashSet<&str> = r["references"]
                .as_array()
                .map(|a| a.iter().filter_map(Value::as_str).collect())
                .unwrap_or_default();
            r["candidates"].as_array().is_some_and(|a| {
                a.iter()
                    .any(|c| c["text"].as_str().is_some_and(|t| refs.contains(t)))
            })
        })
        .count();
    let manifest = json!({
        "source_commit": source_commit,
        "identities": {
            exe.to_string_lossy().into_owned(): sha256_file(&exe)?,
            data.to_string_lossy().into_owned(): sha256_file(&data)?,
        },
        "observations_sha256": sha256_file(&obs_path)?,
        "cases": rows.len(),
        "correct": correct,
        "original_lattice_candidate_hit": lattice_hit,
        "scope": "offline native converter, no preceding context, no AI; not TSF",
    });
    fs::write(
        out.join("manifest.json"),
        serde_json::to_string_pretty(&manifest)?,
    )?;
    Ok(manifest)
}

/// `prepare.py`: コーパス分割と特徴モデル学習・校正データ準備を行う。
pub fn run_prepare(
    corpus: &Path,
    out: &Path,
    eval_paths: &[&Path],
    source_commit: &str,
    train_limit: usize,
) -> Result<Value> {
    fs::create_dir_all(out)?;
    let mut rows: Vec<crate::collocations::CorpusRow> = Vec::new();
    for line in fs::read_to_string(corpus)?
        .lines()
        .filter(|l| !l.trim().is_empty())
    {
        rows.push(serde_json::from_str(line)?);
    }
    let mut excl_readings: HashSet<String> = HashSet::new();
    let mut excl_meta = Vec::new();
    for &p in eval_paths {
        let data: Value = serde_json::from_str(&fs::read_to_string(p)?)?;
        let rset = asteria_common::kana::evaluation_readings(&data);
        excl_readings.extend(rset);
        excl_meta.push(json!({"path": p.to_string_lossy(), "sha256": sha256_file(p)?}));
    }
    let (train_rows, cal_rows, counts) = split_rows(&rows, &excl_readings, train_limit)?;
    let train_texts: Vec<&str> = train_rows
        .iter()
        .filter_map(|r| r["output"].as_str())
        .collect();
    let model = CharacterModel::fit(train_texts)?;
    let model_path = out.join("char_3gram.model.json.gz");
    model.save(&model_path)?;
    let mut cal_jsonl = String::new();
    for r in &cal_rows {
        cal_jsonl.push_str(&serde_json::to_string(r)?);
        cal_jsonl.push('\n');
    }
    let cal_path = out.join("calibration.jsonl");
    fs::write(&cal_path, cal_jsonl.as_bytes())?;
    let manifest = json!({
        "source_commit": source_commit,
        "corpus": {"path": corpus.to_string_lossy(), "sha256": sha256_file(corpus)?},
        "exclusions": excl_meta,
        "model_path": model_path.to_string_lossy(),
        "model_sha256": sha256_file(&model_path)?,
        "calibration_path": cal_path.to_string_lossy(),
        "calibration_sha256": sha256_file(&cal_path)?,
        "counts": counts,
    });
    fs::write(
        out.join("prepare_manifest.json"),
        serde_json::to_string_pretty(&manifest)?,
    )?;
    Ok(manifest)
}

/// `calibrate.py`: `observations.jsonl` + `char_3gram.model.json.gz` から最適な重み・余裕幅を選ぶ。
pub fn run_calibrate(observations: &Path, model_path: &Path, out: &Path) -> Result<Value> {
    let model = CharacterModel::load(model_path)?;
    let mut rows = Vec::new();
    for line in fs::read_to_string(observations)?
        .lines()
        .filter(|l| !l.trim().is_empty())
    {
        let obs: Value = serde_json::from_str(line)?;
        let top1 = obs["top1"].as_str().unwrap_or("").to_owned();
        let refs: Vec<String> = obs["references"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default();
        let reading_len = obs["reading"].as_str().unwrap_or("").chars().count().max(1);
        let mut scored = Vec::new();
        if let Some(arr) = obs["candidates"].as_array() {
            for c in arr {
                let text = c["text"].as_str().unwrap_or("").to_owned();
                let cost_delta = c["cost_delta"]
                    .as_i64()
                    .or_else(|| c["cost"].as_i64())
                    .unwrap_or(0);
                let stat_cost = model.cost(&text, reading_len).unwrap_or(100.0);
                scored.push(ScoredCandidate {
                    text,
                    cost_delta,
                    stat_cost,
                });
            }
        }
        rows.push(CalibrationRow {
            top1,
            references: refs,
            scored,
        });
    }
    let (selected, grid) = select_configuration(&rows)?;
    let out_val = json!({ "selected": selected, "grid": grid, "rows": rows.len() });
    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(out, serde_json::to_string_pretty(&out_val)?)?;
    Ok(out_val)
}

/// `evaluate.py`: 選ばれた構成と特徴モデルを別フォルダの観測結果へ適用しスコアを比較する。
pub fn run_evaluate(
    observations: &Path,
    model_path: &Path,
    weight: i64,
    margin: i64,
    out: &Path,
) -> Result<Value> {
    let model = CharacterModel::load(model_path)?;
    let mut base_ok = 0usize;
    let mut new_ok = 0usize;
    let mut improved = 0usize;
    let mut regressed = 0usize;
    let mut total = 0usize;
    for line in fs::read_to_string(observations)?
        .lines()
        .filter(|l| !l.trim().is_empty())
    {
        let obs: Value = serde_json::from_str(line)?;
        let top1 = obs["top1"].as_str().unwrap_or("").to_owned();
        let refs: Vec<String> = obs["references"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default();
        let reading_len = obs["reading"].as_str().unwrap_or("").chars().count().max(1);
        let mut scored = Vec::new();
        if let Some(arr) = obs["candidates"].as_array() {
            for c in arr {
                let text = c["text"].as_str().unwrap_or("").to_owned();
                let cost_delta = c["cost_delta"]
                    .as_i64()
                    .or_else(|| c["cost"].as_i64())
                    .unwrap_or(0);
                let stat_cost = model.cost(&text, reading_len).unwrap_or(100.0);
                scored.push(ScoredCandidate {
                    text,
                    cost_delta,
                    stat_cost,
                });
            }
        }
        let row = CalibrationRow {
            top1: top1.clone(),
            references: refs.clone(),
            scored,
        };
        let picked = ranked_choice(&row, weight, margin)?;
        let b_hit = refs.iter().any(|r| r == &top1);
        let n_hit = refs.iter().any(|r| r == &picked);
        total += 1;
        if b_hit {
            base_ok += 1;
        }
        if n_hit {
            new_ok += 1;
        }
        if !b_hit && n_hit {
            improved += 1;
        }
        if b_hit && !n_hit {
            regressed += 1;
        }
    }
    let summary = json!({
        "rows": total,
        "base_correct": base_ok,
        "after_correct": new_ok,
        "improved": improved,
        "regressed": regressed,
        "weight": weight,
        "margin": margin,
    });
    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(out, serde_json::to_string_pretty(&summary)?)?;
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn model_cost_and_deterministic_roundtrip() {
        let texts: Vec<&str> = std::iter::repeat("猫が庭で遊ぶ。")
            .take(10)
            .chain(std::iter::repeat("犬が庭で遊ぶ。").take(10))
            .collect();
        let model = CharacterModel::fit(texts).unwrap();
        assert!(
            model.cost("猫が庭で遊ぶ。", 10).unwrap() < model.cost("猫が庭で泳ぐ。", 10).unwrap()
        );
        assert!(model.cost("未知の𠀀", 4).unwrap().is_finite());

        let dir = TempDir::new().unwrap();
        let a = dir.path().join("a.gz");
        let b = dir.path().join("b.gz");
        model.save(&a).unwrap();
        model.save(&b).unwrap();
        assert_eq!(fs::read(&a).unwrap(), fs::read(&b).unwrap());
        assert_eq!(
            model.cost("猫が眠る。", 5).unwrap(),
            CharacterModel::load(&a)
                .unwrap()
                .cost("猫が眠る。", 5)
                .unwrap()
        );
    }

    #[test]
    fn preserves_explicit_forms_and_rejects_bad_inputs() {
        assert!(CharacterModel::fit(std::iter::empty::<&str>()).is_err());
        assert!(CharacterModel::fit(["秘密\x02境界"]).is_err());
        assert!(select_configuration(&[]).is_err());

        for (cur, other) in [
            ("第二弾。", "第2弾。"),
            ("Officeを使う", "OFFICEを使う"),
            ("東京・大阪", "東京大阪"),
        ] {
            let row = CalibrationRow {
                top1: cur.into(),
                references: vec![cur.into()],
                scored: vec![
                    ScoredCandidate {
                        text: cur.into(),
                        cost_delta: 0,
                        stat_cost: 100.0,
                    },
                    ScoredCandidate {
                        text: other.into(),
                        cost_delta: 0,
                        stat_cost: 1.0,
                    },
                ],
            };
            assert_eq!(ranked_choice(&row, 400, 0).unwrap(), cur);
        }
    }

    #[test]
    fn parse_lattice_blocks_roundtrips_hex_and_rejects_mismatch() {
        let hex = |s: &str| -> String { s.as_bytes().iter().map(|b| format!("{b:02x}")).collect() };
        let trace = |key: &str, text: &str, rank: usize| -> Vec<u8> {
            format!(
                "---------- Segment 0/1 [FREE] ----------\nねこ\n  0/1 猫\n\
                 ASTERIA_LATTICE\t{rank}\t1000\t0\t{}\t{}\nExecCommand() return false\n",
                hex(key),
                hex(text)
            )
            .into_bytes()
        };
        let rows = parse_lattice_blocks(&trace("ねこ,", "猫、", 0), &["ねこ、".into()]).unwrap();
        assert_eq!(rows[0]["top1"], "猫");
        assert_eq!(rows[0]["candidates"][0]["cost"], 1000);
        assert!(parse_lattice_blocks(&trace("いぬ,", "犬、", 0), &["ねこ、".into()]).is_err());
        assert!(parse_lattice_blocks(b"", &["ねこ".into()]).is_err());
        assert!(parse_lattice_blocks(&trace("ねこ", "猫", 1), &["ねこ".into()]).is_err());
    }
}
