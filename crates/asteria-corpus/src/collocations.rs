//! 初期連語データ生成・用例展開・Wikipedia 固定区間ステージング。
//! (`build_fast_collocations.py`, `build_usage_collocations.py`, `training/collocations/stage.py`)

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{bail, Context, Result};
use rayon::prelude::*;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use unicode_normalization::UnicodeNormalization;

use asteria_common::hash::{save_fixed, sha256_bytes, sha256_file};
use asteria_common::http::{fetch_bytes, fetch_range};
use asteria_common::kana::{evaluation_readings, reading_key, Exclusions};

pub const REVISION: &str = "7d9c9ea36347b638a627c1d264da48ebc7eb38aa";
pub const CORPUS_SHA: &str = "3d47371a7704fa359d8baeed03b25f4d820e68b299fb75f3fe348bb583878552";
pub const CANONICAL_CORPUS_SHA: &str =
    "f886d411a7565171733edc9a3e6aa5f659affcc030eae277e5a6213165564255";
pub const AJIMEE_URL: &str = "https://raw.githubusercontent.com/azooKey/AJIMEE-Bench/\
    401666cd56d1a570c2021798b64b6da4396bfd45/JWTD_v2/v1/evaluation_items.json";
pub const AJIMEE_SHA: &str = "e9eb668fd6aa14b1e26436f429b5550108af0a1dfd443b8cea0bcb3ab3028fca";

pub const USAGE_SHA: &str = "9a5ff4ab0be399e01e6aeb8637a166d421299f92a3c39df93fab6bcbaf3e68b8";
pub const BASE_SHA: &str = "f576ffc696ad81f244fb5791a2571ac1edb320f2a2f20d0773ea6a044f01b29c";
pub const CFORMS_SHA: &str = "947d82de803181bb9df8bb4fede6d0d3c3677c7de88c946ff3a35dbfc2bce0ab";

const MIN_LENGTH: usize = 3;
const MAX_LENGTH: usize = 14;
const MIN_SENTENCES: usize = 5;
const MIN_GROUPS: usize = 2;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CorpusRow {
    pub input: String,
    pub output: String,
    #[serde(default)]
    pub split: String,
    #[serde(default)]
    pub source_group: String,
}

fn is_japanese_collocation_char(c: char) -> bool {
    matches!(c,
        '\u{3400}'..='\u{4dbf}'
        | '\u{4e00}'..='\u{9fff}'
        | '々'
        | 'ぁ'..='ゖ'
        | 'ァ'..='ヺ'
        | 'ー'
    )
}

fn is_kanji(c: char) -> bool {
    matches!(c, '\u{3400}'..='\u{4dbf}' | '\u{4e00}'..='\u{9fff}')
}

/// 1 文の中から短い日本語の連語候補（漢字を含み、先頭がひらがなでない 3〜14 文字）を重複なく抽出する。
pub fn fragments(text: &str) -> BTreeSet<String> {
    let mut result = BTreeSet::new();
    let normalized: String = text.nfkc().collect();
    let mut byte_start = None;
    let mut runs: Vec<&str> = Vec::new();
    for (idx, ch) in normalized.char_indices() {
        if is_japanese_collocation_char(ch) {
            if byte_start.is_none() {
                byte_start = Some(idx);
            }
        } else if let Some(s) = byte_start.take() {
            runs.push(&normalized[s..idx]);
        }
    }
    if let Some(s) = byte_start {
        runs.push(&normalized[s..]);
    }

    for run in runs {
        let chars: Vec<char> = run.chars().collect();
        for start in 0..chars.len() {
            if ('ぁ'..='ゖ').contains(&chars[start]) {
                continue;
            }
            let max_len = MAX_LENGTH.min(chars.len() - start);
            for len in MIN_LENGTH..=max_len {
                let slice = &chars[start..start + len];
                if slice.iter().any(|&c| is_kanji(c)) {
                    result.insert(slice.iter().collect());
                }
            }
        }
    }
    result
}

/// 固定 Wikipedia 学習区間から 5 文以上・2 グループ以上に現れる連語を並列抽出する。
pub fn collect(
    rows: &[CorpusRow],
    excluded: &HashSet<String>,
) -> (Vec<String>, BTreeMap<String, usize>) {
    let train_groups: HashSet<String> = (1..=7)
        .map(|i| format!("train_wikipedia.jsonl:{i:02}"))
        .collect();
    let excl_refs: Vec<&str> = excluded.iter().map(String::as_str).collect();
    let overlaps = Exclusions::new(excl_refs, 0.8);
    let mut audit: BTreeMap<String, usize> = BTreeMap::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut accepted: Vec<(String, String)> = Vec::new();

    for row in rows {
        if row.split != "train" || !train_groups.contains(&row.source_group) {
            *audit.entry("not_wikipedia_train".into()).or_insert(0) += 1;
            continue;
        }
        let key = reading_key(&row.input);
        if overlaps.overlaps(&key) {
            *audit
                .entry("evaluation_reading_or_near_duplicate".into())
                .or_insert(0) += 1;
            continue;
        }
        let len = row.output.chars().count();
        if !(3..=512).contains(&len) {
            *audit.entry("invalid_text".into()).or_insert(0) += 1;
            continue;
        }
        let identity: String = row.output.nfkc().collect();
        if !seen.insert(identity) {
            *audit.entry("duplicate_sentence".into()).or_insert(0) += 1;
            continue;
        }
        *audit.entry("accepted_sentences".into()).or_insert(0) += 1;
        accepted.push((row.output.clone(), row.source_group.clone()));
    }

    // 断片抽出を rayon で並列実行し、頻度・グループ集合へ集約する。
    let per_row: Vec<(String, BTreeSet<String>)> = accepted
        .par_iter()
        .map(|(text, group)| (group.clone(), fragments(text)))
        .collect();

    let mut counts: HashMap<String, usize> = HashMap::new();
    let mut groups: HashMap<String, HashSet<String>> = HashMap::new();
    for (group, frags) in per_row {
        for value in frags {
            *counts.entry(value.clone()).or_insert(0) += 1;
            groups.entry(value).or_default().insert(group.clone());
        }
    }

    let mut phrases: Vec<String> = counts
        .iter()
        .filter(|(v, &c)| {
            c >= MIN_SENTENCES && groups.get(*v).is_some_and(|g| g.len() >= MIN_GROUPS)
        })
        .map(|(v, _)| v.clone())
        .collect();
    phrases.sort();
    audit.insert("unique_fragments".into(), counts.len());
    audit.insert("selected_phrases".into(), phrases.len());
    (phrases, audit)
}

/// `build_fast_collocations.py` 相当のビルドを実行する。
pub fn run_build_fast_collocations(
    corpus: &Path,
    corpus_manifest: &Path,
    baseline: &Path,
    exclude_paths: &[PathBuf],
    out: &Path,
) -> Result<Value> {
    let manifest: Value = serde_json::from_str(&fs::read_to_string(corpus_manifest)?)?;
    let corpus_sha = sha256_file(corpus)?;
    if manifest.get("source").and_then(Value::as_str) != Some("Miwa-Keita/zenz-v2.5-dataset")
        || manifest.get("revision").and_then(Value::as_str) != Some(REVISION)
        || (corpus_sha != CORPUS_SHA && corpus_sha != CANONICAL_CORPUS_SHA)
        || manifest.get("sample_sha256").and_then(Value::as_str) != Some(corpus_sha.as_str())
    {
        bail!("固定コーパスの識別またはSHAが一致しません");
    }
    let chunks = manifest["chunks"]
        .as_array()
        .context("chunks が配列ではありません")?;
    for i in 1..=7 {
        let g = format!("train_wikipedia.jsonl:{i:02}");
        let ok = chunks.iter().any(|c| {
            c.get("group").and_then(Value::as_str) == Some(g.as_str())
                && c.get("license").and_then(Value::as_str) == Some("CC-BY-SA-4.0")
        });
        if !ok {
            bail!("Wikipedia学習区間のライセンス識別が不足しています");
        }
    }

    let mut exclusions = HashSet::new();
    let mut excl_hashes: BTreeMap<String, String> = BTreeMap::new();
    for path in exclude_paths {
        let val: Value = serde_json::from_str(&fs::read_to_string(path)?)?;
        for r in evaluation_readings(&val) {
            exclusions.insert(r);
        }
        let abs = fs::canonicalize(path).unwrap_or_else(|_| path.clone());
        excl_hashes.insert(abs.to_string_lossy().into_owned(), sha256_file(path)?);
    }
    if exclusions.is_empty() {
        bail!("評価入力の除外集合が空です");
    }

    let file = fs::File::open(corpus)?;
    let mut rows = Vec::new();
    for line in BufReader::new(file).lines() {
        let line = line?;
        if !line.trim().is_empty() {
            rows.push(serde_json::from_str::<CorpusRow>(&line)?);
        }
    }
    let (phrases, audit) = collect(&rows, &exclusions);
    let baseline_text = fs::read_to_string(baseline)?;
    let baseline_set: BTreeSet<String> = baseline_text
        .lines()
        .filter(|l| !l.is_empty())
        .map(str::to_owned)
        .collect();
    let mut combined: BTreeSet<String> = baseline_set.clone();
    for p in &phrases {
        combined.insert(p.clone());
    }
    if phrases.is_empty() || combined.len() > 100_000 {
        bail!("連語抽出が空または想定サイズを超えました");
    }
    if out.exists() {
        bail!("出力先が既に存在します: {}", out.display());
    }
    fs::create_dir_all(out)?;
    let output_path = out.join("fast_collocations.txt");
    let mut out_body = String::new();
    for v in &combined {
        out_body.push_str(v);
        out_body.push('\n');
    }
    fs::write(&output_path, out_body.as_bytes())?;

    let train_groups: Vec<String> = (1..=7)
        .map(|i| format!("train_wikipedia.jsonl:{i:02}"))
        .collect();
    let receipt = json!({
        "source": manifest["source"],
        "revision": REVISION,
        "corpus_sha256": corpus_sha,
        "corpus_manifest_sha256": sha256_file(corpus_manifest)?,
        "baseline_sha256": sha256_file(baseline)?,
        "exclusion_files": excl_hashes,
        "exclusion_readings": exclusions.len(),
        "used_groups": train_groups,
        "license": "CC-BY-SA-4.0 (additional phrases); BSD-3-Clause (upstream baseline)",
        "min_length": MIN_LENGTH,
        "max_length": MAX_LENGTH,
        "min_distinct_sentences": MIN_SENTENCES,
        "min_source_groups": MIN_GROUPS,
        "audit": audit,
        "baseline_entries": baseline_set.len(),
        "combined_entries": combined.len(),
        "result_sha256": sha256_file(&output_path)?,
        "evaluation_labels_used": false,
    });
    fs::write(
        out.join("fast_collocations.json"),
        serde_json::to_string_pretty(&receipt)?,
    )?;
    Ok(receipt)
}

#[derive(Debug, Default)]
pub struct ConjugationRule {
    pub base_form: String,
    pub suffixes: BTreeSet<String>,
}

/// Mozc の `cforms.def` をパースする。
pub fn parse_cforms(text: &str) -> Result<HashMap<String, ConjugationRule>> {
    let mut result: HashMap<String, ConjugationRule> = HashMap::new();
    for line in text.lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 4 {
            bail!("Malformed Mozc conjugation rule");
        }
        let pos = fields[0];
        let kind = fields[1];
        let suffix = if fields[2] == "*" { "" } else { fields[2] };
        let entry = result.entry(pos.to_owned()).or_default();
        entry.suffixes.insert(suffix.to_owned());
        if kind == "基本形" {
            entry.base_form = suffix.to_owned();
        }
    }
    Ok(result)
}

/// Japanese Usage Dictionary から語幹を含む連語を抽出・活用展開する。
pub fn extract_usage_collocations(
    text: &str,
    conjugations: &HashMap<String, ConjugationRule>,
) -> Result<(Vec<String>, BTreeMap<String, usize>)> {
    let content_re = Regex::new(r"[\u3400-\u4dbf\u4e00-\u9fffァ-ヺ]")?;
    let quote_re = Regex::new(r"「([^「」]+)」")?;
    let mut values: BTreeSet<String> = BTreeSet::new();
    let mut audit: BTreeMap<String, usize> = BTreeMap::new();

    for line in text.lines() {
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() != 4 {
            bail!("Malformed usage dictionary row");
        }
        let (_reading, word, pos, meaning) = (fields[0], fields[1], fields[2], fields[3]);
        let rule = conjugations.get(pos);
        let base = rule.map(|r| r.base_form.as_str()).unwrap_or("");
        let stem = if !base.is_empty() && word.ends_with(base) {
            let trimmed = &word[..word.len() - base.len()];
            if trimmed.is_empty() {
                word
            } else {
                trimmed
            }
        } else {
            word
        };

        for caps in quote_re.captures_iter(meaning) {
            let example = &caps[1];
            let ex_len = example.chars().count();
            if !(3..=96).contains(&ex_len) || !example.contains(stem) {
                continue;
            }
            let norm_example: String = example.nfkc().collect();
            let mut variants: BTreeSet<String> = BTreeSet::new();
            variants.insert(norm_example);
            if !base.is_empty() && stem != word && example.ends_with(word) {
                if let Some(r) = rule {
                    let prefix = &example[..example.len() - word.len()];
                    for suffix in &r.suffixes {
                        variants.insert(format!("{prefix}{stem}{suffix}"));
                    }
                    *audit.entry("ending_conjugations".into()).or_insert(0) +=
                        variants.len().saturating_sub(1);
                }
            }
            for variant in variants {
                for val in fragments(&variant) {
                    if val.contains(stem) && content_re.is_match(&val.replacen(stem, "", 1)) {
                        values.insert(val);
                    }
                }
            }
            *audit.entry("eligible_quotes".into()).or_insert(0) += 1;
        }
    }
    audit.insert("phrases".into(), values.len());
    Ok((values.into_iter().collect(), audit))
}

/// `build_usage_collocations.py` 相当のビルドを実行する。
pub fn run_build_usage_collocations(
    baseline: &Path,
    usage: &Path,
    cforms: &Path,
    out: &Path,
) -> Result<Value> {
    if sha256_file(baseline)? != BASE_SHA
        || sha256_file(usage)? != USAGE_SHA
        || sha256_file(cforms)? != CFORMS_SHA
    {
        bail!("Fixed baseline/usage SHA mismatch");
    }
    let conjugations = parse_cforms(&fs::read_to_string(cforms)?)?;
    let (phrases, audit) = extract_usage_collocations(&fs::read_to_string(usage)?, &conjugations)?;
    let mut combined: BTreeSet<String> = fs::read_to_string(baseline)?
        .lines()
        .map(str::to_owned)
        .collect();
    let baseline_len = combined.len();
    for p in &phrases {
        combined.insert(p.clone());
    }
    if phrases.is_empty() || combined.len() > 50_000 {
        bail!("Empty/excessive extracted collocations");
    }
    if out.exists() {
        bail!("出力先が既に存在します: {}", out.display());
    }
    fs::create_dir_all(out)?;
    let output_path = out.join("fast_collocations.txt");
    let mut body = String::new();
    for v in &combined {
        body.push_str(v);
        body.push('\n');
    }
    fs::write(&output_path, body.as_bytes())?;
    let manifest = json!({
        "source": "hiroyuki-komatsu/japanese-usage-dictionary",
        "tag": "2025-01-25",
        "usage_sha256": sha256_file(usage)?,
        "cforms_sha256": sha256_file(cforms)?,
        "baseline_sha256": sha256_file(baseline)?,
        "baseline_entries": baseline_len,
        "extracted_entries": phrases.len(),
        "added_entries": combined.len() - baseline_len,
        "combined_entries": combined.len(),
        "audit": audit,
        "result_sha256": sha256_file(&output_path)?,
        "bytes": fs::metadata(&output_path)?.len(),
        "license": "BSD-2-Clause additional usage phrases; previous baseline licenses retained",
        "evaluation_labels_used": false,
        "product_adopted": false,
    });
    fs::write(
        out.join("fast_collocations.json"),
        serde_json::to_string_pretty(&manifest)?,
    )?;
    Ok(manifest)
}

/// `training/collocations/stage.py`: 固定 Wikipedia 学習区間 7 MiB を検証取得し初期連語を再現する。
pub fn load_chunk(
    config: &Value,
    chunk: &Value,
    out: &Path,
    cache: Option<&Path>,
) -> Result<Vec<u8>> {
    let group = chunk["group"].as_str().context("chunk.group")?;
    let suffix = group.rsplit(':').next().unwrap_or("");
    let file = chunk["file"].as_str().context("chunk.file")?;
    let bytes = chunk["bytes"].as_u64().context("chunk.bytes")? as usize;
    let expected_sha = chunk["sha256"].as_str().context("chunk.sha256")?;
    let filename = format!("{file}.{suffix}.raw");

    let mut existing = vec![out.join(&filename)];
    if let Some(c) = cache {
        existing.push(c.join(&filename));
    }
    for path in existing {
        if path.exists() {
            let data = fs::read(&path)?;
            if data.len() != bytes || sha256_bytes(&data) != expected_sha {
                bail!("既存の範囲データSHAが一致しません: {}", path.display());
            }
            return Ok(data);
        }
    }
    let source = config["source"].as_str().context("config.source")?;
    let revision = config["revision"].as_str().context("config.revision")?;
    let offset = chunk["offset"].as_u64().context("chunk.offset")?;
    let total_size = config["source_file_size"]
        .as_u64()
        .context("config.source_file_size")?;
    let url = format!(
        "https://huggingface.co/datasets/{source}/resolve/{revision}/{file}?sample_offset={offset}"
    );
    let data = fetch_range(
        &url,
        offset,
        bytes,
        total_size,
        "Asteria-collocations/1",
        Duration::from_secs(45),
    )?;
    if sha256_bytes(&data) != expected_sha {
        bail!("取得した範囲のSHAが一致しません");
    }
    save_fixed(&out.join(&filename), &data)?;
    Ok(data)
}

pub fn fixed_file(url: &str, path: &Path, expected: &str, cache: Option<&Path>) -> Result<PathBuf> {
    let existing = if path.exists() {
        Some(path)
    } else {
        cache.filter(|c| c.exists())
    };
    let data = if let Some(p) = existing {
        fs::read(p)?
    } else {
        fetch_bytes(
            url,
            1_048_576,
            "Asteria-collocations/1",
            Duration::from_secs(45),
        )?
    };
    if data.len() > 1_048_576 || sha256_bytes(&data) != expected {
        bail!("固定原票のサイズまたはSHAが一致しません");
    }
    save_fixed(path, &data)?;
    Ok(path.to_path_buf())
}

pub fn run_collocations_stage(
    repo_root: &Path,
    baseline: &Path,
    out: &Path,
    cache_dir: Option<&Path>,
    ajimee_path: Option<&Path>,
    usage_dir: Option<&Path>,
    cforms_path: Option<&Path>,
) -> Result<String> {
    let config_path = repo_root.join("training/collocations/sources.json");
    let config: Value = serde_json::from_str(&fs::read_to_string(&config_path)?)?;
    let baseline_norm = fs::read(baseline)?;
    let baseline_lf: Vec<u8> = baseline_norm
        .windows(2)
        .enumerate()
        .filter_map(|(i, w)| {
            if w == b"\r\n" {
                None
            } else {
                Some(baseline_norm[i])
            }
        })
        .chain(baseline_norm.last().copied())
        .collect();
    if sha256_bytes(&baseline_lf)
        != config["baseline_sha256"]
            .as_str()
            .context("baseline_sha256")?
    {
        bail!("元のMozc連語データSHAが一致しません");
    }

    let source = out.join("source");
    let chunks = config["chunks"].as_array().context("chunks")?;
    let expected_groups: Vec<String> = (1..=7)
        .map(|i| format!("train_wikipedia.jsonl:{i:02}"))
        .collect();
    let got_groups: Vec<String> = chunks
        .iter()
        .filter_map(|c| c["group"].as_str().map(str::to_owned))
        .collect();
    if got_groups != expected_groups
        || chunks.iter().any(|c| {
            c["file"].as_str() != Some("train_wikipedia.jsonl")
                || c["license"].as_str() != Some("CC-BY-SA-4.0")
                || c["bytes"].as_u64() != Some(1_048_576)
        })
    {
        bail!("固定Wikipedia学習区間以外が指定されました");
    }

    let mut raw_jsonl = String::new();
    let mut row_count = 0usize;
    for chunk in chunks {
        let data = load_chunk(&config, chunk, &source, cache_dir)?;
        let mut lines: Vec<&[u8]> = data.split(|&b| b == b'\n').collect();
        if lines.len() >= 2 {
            lines = lines[1..lines.len() - 1].to_vec();
        } else {
            lines.clear();
        }
        let expected_records = chunk["records"].as_u64().context("chunk.records")? as usize;
        if lines.len() != expected_records {
            bail!("範囲中の完全レコード数が一致しません");
        }
        let group = chunk["group"].as_str().context("group")?;
        for line in lines {
            let item: Value = serde_json::from_slice(line)?;
            let inp = item["input"]
                .as_str()
                .context("コーパスの形式が一致しません (input)")?;
            let outp = item["output"]
                .as_str()
                .context("コーパスの形式が一致しません (output)")?;
            // Python json.dumps(sort_keys=True, separators=(',', ':'), ensure_ascii=False) と同一形式
            let row = json!({
                "input": inp,
                "output": outp,
                "source_group": group,
                "split": "train",
            });
            raw_jsonl.push_str(&serde_json::to_string(&row)?);
            raw_jsonl.push('\n');
            row_count += 1;
        }
        println!("{group} {} SHA verified", expected_records);
    }
    if sha256_bytes(raw_jsonl.as_bytes()) != config["sample_sha256"].as_str().unwrap_or("")
        || row_count != config["records"].as_u64().unwrap_or(0) as usize
    {
        bail!("正規化した学習区間のSHAが一致しません");
    }
    let sample_path = source.join("sample.jsonl");
    save_fixed(&sample_path, raw_jsonl.as_bytes())?;
    let sample_manifest = json!({
        "source": config["source"],
        "revision": config["revision"],
        "chunks": config["chunks"],
        "records": config["records"],
        "sample_sha256": config["sample_sha256"],
    });
    let manifest_path = source.join("sample.manifest.json");
    save_fixed(
        &manifest_path,
        format!("{}\n", serde_json::to_string_pretty(&sample_manifest)?).as_bytes(),
    )?;

    let eval_path = if let Some(p) = ajimee_path {
        let raw = fs::read(p)?;
        if sha256_bytes(&raw) != AJIMEE_SHA {
            bail!("除外に使う固定AJIMEEのSHAが一致しません");
        }
        save_fixed(p, &raw)?;
        p.to_path_buf()
    } else {
        let p = source.join("ajimee-bench.json");
        let raw = if p.exists() {
            fs::read(&p)?
        } else {
            fetch_bytes(
                AJIMEE_URL,
                2_097_152,
                "Asteria-collocations/1",
                Duration::from_secs(45),
            )?
        };
        if sha256_bytes(&raw) != AJIMEE_SHA {
            bail!("除外に使う固定AJIMEEのSHAが一致しません");
        }
        save_fixed(&p, &raw)?;
        p
    };

    let mut exclusions: Vec<PathBuf> = fs::read_dir(repo_root.join("eval/sets"))?
        .filter_map(|e| e.ok().map(|x| x.path()))
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("json"))
        .collect();
    exclusions.sort();
    exclusions.push(eval_path);

    let generated = out.join("generated");
    run_build_fast_collocations(
        &sample_path,
        &manifest_path,
        baseline,
        &exclusions,
        &generated,
    )?;
    let wiki_data = generated.join("fast_collocations.txt");
    if sha256_file(&wiki_data)? != config["wikipedia_result_sha256"].as_str().unwrap_or("") {
        bail!("比較済みのWikipedia連語を再現できません");
    }

    let usage = &config["usage_source"];
    let repo_u = usage["repository"].as_str().context("usage.repository")?;
    let tag_u = usage["tag"].as_str().context("usage.tag")?;
    let base_url = format!("https://raw.githubusercontent.com/{repo_u}/{tag_u}/");
    let usage_txt = fixed_file(
        &format!("{base_url}usage_dict.txt"),
        &source.join("usage/usage_dict.txt"),
        usage["usage_sha256"].as_str().context("usage_sha256")?,
        usage_dir.map(|d| d.join("usage_dict.txt")).as_deref(),
    )?;
    let license_txt = fixed_file(
        &format!("{base_url}LICENSE"),
        &source.join("usage/LICENSE"),
        usage["license_sha256"].as_str().context("license_sha256")?,
        usage_dir.map(|d| d.join("LICENSE")).as_deref(),
    )?;

    let default_cforms = baseline
        .parent()
        .and_then(Path::parent)
        .map(|p| p.join("rules/cforms.def"))
        .context("cforms.def の既定パスを決められません")?;
    let cforms = cforms_path.unwrap_or(&default_cforms);
    if sha256_file(cforms)? != usage["cforms_sha256"].as_str().unwrap_or("") {
        bail!("固定したMozc活用規則のSHAが一致しません");
    }

    let usage_generated = out.join("usage-generated");
    let usage_receipt =
        run_build_usage_collocations(&wiki_data, &usage_txt, cforms, &usage_generated)?;
    let final_txt = usage_generated.join("fast_collocations.txt");
    let final_bytes = fs::read(&final_txt)?;
    let result_sha = config["result_sha256"].as_str().context("result_sha256")?;
    if sha256_bytes(&final_bytes) != result_sha {
        bail!("比較済みの連語データSHAを再現できません");
    }

    let mut metadata: Value = serde_json::from_str(&fs::read_to_string(
        generated.join("fast_collocations.json"),
    )?)?;
    if let Some(obj) = metadata.as_object_mut() {
        obj.insert("result_sha256".into(), json!(result_sha));
        obj.insert(
            "combined_entries".into(),
            usage_receipt["combined_entries"].clone(),
        );
        obj.insert("usage_source".into(), usage.clone());
        obj.insert("usage_generation".into(), usage_receipt.clone());
        obj.insert(
            "wikipedia_result_sha256".into(),
            config["wikipedia_result_sha256"].clone(),
        );
        let lic = obj
            .get("license")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned();
        obj.insert(
            "license".into(),
            json!(format!("{lic}; BSD-2-Clause (additional usage phrases)")),
        );
        let mut excl_map = serde_json::Map::new();
        for p in &exclusions {
            let key = if let Ok(rel) = p.strip_prefix(repo_root) {
                rel.to_string_lossy().replace('\\', "/")
            } else {
                "AJIMEE-Bench/evaluation_items.json".into()
            };
            excl_map.insert(key, json!(sha256_file(p)?));
        }
        obj.insert("exclusion_files".into(), Value::Object(excl_map));
        obj.insert(
            "source_manifest_sha256".into(),
            json!(sha256_file(&config_path)?),
        );
        obj.insert("filename".into(), json!("fast_collocations.txt"));
        obj.insert(
            "distribution_status".into(),
            json!("generated_from_pinned_public_source"),
        );
    }

    let license_body = fs::read_to_string(&license_txt)?;
    let notice = format!(
        "Asteriaの初期変換用連語データ\n\n\
         追加データ: CC BY-SA 4.0 https://creativecommons.org/licenses/by-sa/4.0/\n\
         出典: Wikipedia日本語版の執筆者、Keita Miwa / zenz-v2.5-dataset\n\
         https://huggingface.co/datasets/{}/tree/{}\n\
         Wikipedia由来の学習区間01〜07だけを使用。dev区間とCommon Crawl部分は使用していません。\n\
         変更: 複数の文・取得区間に共通する短い日本語の連語を抽出し、評価読みを除外しました。\n\
         Wikipedia由来の追加連語もCC BY-SA 4.0で提供します。元のMozc40件はGoogle Inc.のBSD-3-Clauseデータです。\n\n\
         用例由来の追加データ: Japanese Usage Dictionary、BSD-2-Clause。\n\
         https://github.com/{repo_u}/tree/{tag_u}\n\
         変更: 引用用例から対象語の語幹を含む連語を抽出し、末尾だけを固定Mozc活用規則で展開。\n\
         用例辞書SHA-256: {}\n\
         活用規則SHA-256: {}\n\n\
         {license_body}\n\n\
         結果SHA-256: {result_sha}\n\
         実行時はコンパイル済みデータを使い、モデルやネットワークへのアクセスは追加しません。\n\
         fast_collocations.txtは再構築と出典確認用です。生成規則と取得原票は以下を参照。\n\
         https://github.com/aruiki/Asteria-Japanese-IME/tree/main/training/collocations\n",
        config["source"].as_str().unwrap_or(""),
        config["revision"].as_str().unwrap_or(""),
        usage["usage_sha256"].as_str().unwrap_or(""),
        usage["cforms_sha256"].as_str().unwrap_or(""),
    );

    save_fixed(&out.join("fast_collocations.txt"), &final_bytes)?;
    save_fixed(
        &out.join("fast_collocations.json"),
        format!("{}\n", serde_json::to_string_pretty(&metadata)?).as_bytes(),
    )?;
    save_fixed(&out.join("NOTICE-fast-collocations.txt"), notice.as_bytes())?;
    println!("fixed collocations reproduced {result_sha}");
    Ok(result_sha.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn sample_rows() -> Vec<CorpusRow> {
        ["。", "！", "？", "、", "です。", "のです。"]
            .into_iter()
            .enumerate()
            .map(|(i, suffix)| CorpusRow {
                input: format!("ごれいをかいせつする{i}"),
                output: format!("新しい語例を解説する{suffix}"),
                split: "train".into(),
                source_group: format!("train_wikipedia.jsonl:{:02}", 1 + (i % 2)),
            })
            .collect()
    }

    #[test]
    fn held_out_groups_and_non_wikipedia_do_not_count() {
        let mut rows = sample_rows();
        for (i, row) in rows.iter_mut().enumerate() {
            if i < 3 {
                row.split = "dev".into();
            } else {
                row.source_group = "train_llm-jp-corpus-v3.jsonl:01".into();
            }
        }
        let (phrases, audit) = collect(&rows, &HashSet::new());
        assert!(phrases.is_empty());
        assert_eq!(audit.get("not_wikipedia_train"), Some(&6));
    }

    #[test]
    fn repeated_sentence_cannot_create_rule() {
        let row = sample_rows().into_iter().next().unwrap();
        let rows = vec![row; 50];
        let (phrases, audit) = collect(&rows, &HashSet::new());
        assert!(phrases.is_empty());
        assert_eq!(audit.get("duplicate_sentence"), Some(&49));
    }

    #[test]
    fn evaluation_readings_and_near_duplicates_are_excluded() {
        let mut rows = sample_rows();
        for row in &mut rows {
            row.input = "これはひじょうにながいひょうかようのにゅうりょくです".into();
        }
        let excl: HashSet<String> =
            ["これはひじょうにながいひょうかようのにゅうりょくです。".into()]
                .into_iter()
                .collect();
        let (phrases, audit) = collect(&rows, &excl);
        assert!(phrases.is_empty());
        assert_eq!(audit.get("evaluation_reading_or_near_duplicate"), Some(&6));
    }

    #[test]
    fn multiple_independent_sentences_and_groups_required() {
        let rows = sample_rows();
        let (phrases, _) = collect(&rows, &HashSet::new());
        assert!(phrases.iter().any(|p| p == "語例を解説"));

        let mut single_group = rows;
        for r in &mut single_group {
            r.source_group = "train_wikipedia.jsonl:01".into();
        }
        assert!(collect(&single_group, &HashSet::new()).0.is_empty());
    }

    #[test]
    fn ascii_numbers_and_punctuation_do_not_bridge_phrases() {
        assert!(!fragments("語例を1解説").contains("語例を解説"));
        assert!(!fragments("語例を、解説").contains("語例を解説"));
    }

    #[test]
    fn mismatched_cache_is_preserved_and_not_replaced() {
        let dir = TempDir::new().unwrap();
        let cache = dir.path().join("cache");
        fs::create_dir_all(&cache).unwrap();
        let cached = cache.join("train_wikipedia.jsonl.01.raw");
        fs::write(&cached, b"wrong").unwrap();
        let chunk = json!({
            "file": "train_wikipedia.jsonl",
            "group": "train_wikipedia.jsonl:01",
            "bytes": 3,
            "sha256": sha256_bytes(b"abc"),
        });
        assert!(load_chunk(&json!({}), &chunk, &dir.path().join("out"), Some(&cache)).is_err());
        assert_eq!(fs::read(&cached).unwrap(), b"wrong");
    }
}
