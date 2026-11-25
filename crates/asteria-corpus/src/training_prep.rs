//! 学習データ準備・リストワイズランキング実験・追加生成モデル配置・対比ペア構築。
//! (`conversion_training.py`, `conversion_ranker.py`, `fetch_generator.py`, `stage_generator.py`, `native_pairs.py`)

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{bail, Context, Result};
use rand::seq::SliceRandom;
use rand::SeedableRng;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use asteria_common::hash::sha256_file;
use asteria_common::http::download_verified;
use asteria_common::kana::{evaluation_readings, reading_key, similar_jaccard, Exclusions};

/// Python `random.Random(seed).shuffle()` との一致は不要だが、再現性のあるシード付きシャッフルを行う。
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TrainingInputRow {
    pub split: String,
    pub reading: String,
    #[serde(default = "default_positive")]
    pub positive: String,
    #[serde(default)]
    pub context: String,
    #[serde(default)]
    pub source_group: String,
    #[serde(default = "default_kind")]
    pub kind: String,
}

fn default_positive() -> String {
    String::new()
}
fn default_kind() -> String {
    "original".into()
}

/// 分割された学習・検証ケースと監査カウント。
pub type PreparedTrainingSplit = (BTreeMap<String, Vec<Value>>, BTreeMap<String, usize>);

/// `conversion_training.prepare`: 評価読み・近接重複・クロススプリット漏洩を排除して train/dev を構築する。
pub fn prepare_training_cases(
    rows: &[TrainingInputRow],
    excluded: &HashSet<String>,
    train_limit: usize,
    dev_limit: usize,
    seed: u64,
    max_reading: usize,
) -> Result<PreparedTrainingSplit> {
    if train_limit == 0 || dev_limit == 0 {
        bail!("train/dev件数は正の数が必要です");
    }
    let excl_refs: Vec<&str> = excluded.iter().map(String::as_str).collect();
    let excl_filter = Exclusions::new(excl_refs, 0.8);
    let mut exclusion_cache: HashMap<String, bool> = HashMap::new();
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();

    #[derive(Clone)]
    struct Entry {
        input: String,
        context_text: String,
        expected_output: Vec<String>,
        source_groups: BTreeSet<String>,
    }
    let mut groups_train: BTreeMap<(String, String), Entry> = BTreeMap::new();
    let mut groups_dev: BTreeMap<(String, String), Entry> = BTreeMap::new();

    for row in rows {
        if row.kind != "original" {
            *counts.entry("non_original".into()).or_insert(0) += 1;
            continue;
        }
        if (row.split != "train" && row.split != "dev") || row.source_group.is_empty() {
            bail!("コーパスの分割または取得区間が不明です");
        }
        let key = reading_key(&row.reading);
        let is_excl = *exclusion_cache
            .entry(key.clone())
            .or_insert_with(|| excl_filter.overlaps(&key));
        if is_excl {
            *counts.entry("evaluation_reading".into()).or_insert(0) += 1;
            continue;
        }
        let k_len = key.chars().count();
        if !(6..=max_reading).contains(&k_len) {
            *counts.entry("reading_length".into()).or_insert(0) += 1;
            continue;
        }
        let map = if row.split == "train" {
            &mut groups_train
        } else {
            &mut groups_dev
        };
        let entry = map
            .entry((key, row.context.clone()))
            .or_insert_with(|| Entry {
                input: row.reading.clone(),
                context_text: row.context.clone(),
                expected_output: Vec::new(),
                source_groups: BTreeSet::new(),
            });
        entry.source_groups.insert(row.source_group.clone());
        if !entry.expected_output.contains(&row.positive) {
            entry.expected_output.push(row.positive.clone());
        }
    }

    let train_keys: HashSet<&str> = groups_train.keys().map(|(k, _)| k.as_str()).collect();
    let dev_keys: HashSet<&str> = groups_dev.keys().map(|(k, _)| k.as_str()).collect();
    if train_keys.intersection(&dev_keys).next().is_some() {
        bail!("正規化読みがtrain/dev間で重複しています");
    }
    let train_sg: HashSet<&str> = groups_train
        .values()
        .flat_map(|e| e.source_groups.iter().map(String::as_str))
        .collect();
    let dev_sg: HashSet<&str> = groups_dev
        .values()
        .flat_map(|e| e.source_groups.iter().map(String::as_str))
        .collect();
    if train_sg.intersection(&dev_sg).next().is_some() {
        bail!("同一コーパス取得区間がtrain/devへ混在しています");
    }

    let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
    let mut selected: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    let mut train_chosen_keys: Vec<String> = Vec::new();

    for (split, limit, map) in [
        ("train", train_limit, groups_train),
        ("dev", dev_limit, groups_dev),
    ] {
        let mut values: Vec<((String, String), Entry)> = map.into_iter().collect();
        values.shuffle(&mut rng);
        let mut chosen = Vec::new();
        let mut chosen_keys: Vec<String> = Vec::new();
        for ((key, _), entry) in values {
            let dup = chosen_keys.iter().any(|p| similar_jaccard(&key, p, 0.8))
                || (split == "dev"
                    && train_chosen_keys
                        .iter()
                        .any(|p| similar_jaccard(&key, p, 0.8)));
            if dup {
                *counts.entry(format!("{split}_near_duplicate")).or_insert(0) += 1;
                continue;
            }
            let idx = format!("{split}-{:05}", chosen.len());
            chosen.push(json!({
                "index": idx,
                "input": entry.input,
                "context_text": entry.context_text,
                "expected_output": entry.expected_output,
                "source_groups": entry.source_groups.into_iter().collect::<Vec<_>>(),
                "label_scope": "corpus_reference_preference",
                "split": split,
                "domain": "public-corpus-original",
            }));
            chosen_keys.push(key);
            if chosen.len() == limit {
                break;
            }
        }
        if chosen.len() != limit {
            bail!("独立した{split}が不足: {}/{limit}", chosen.len());
        }
        if split == "train" {
            train_chosen_keys = chosen_keys;
        }
        selected.insert(split.to_owned(), chosen);
    }

    Ok((selected, counts))
}

/// `conversion_ranker`: リストワイズランキング学習と評価。
#[derive(Debug, Clone)]
pub struct RankerExample {
    pub index: String,
    pub reading: String,
    pub references: HashSet<String>,
    pub texts: Vec<String>,
    pub x: Vec<[f64; 2]>,
    pub final_text: String,
    pub source_groups: HashSet<String>,
    pub split: String,
}

pub fn build_ranker_examples(
    rows: &[Value],
) -> Result<(Vec<RankerExample>, BTreeMap<String, usize>)> {
    let mut result = Vec::new();
    let mut excluded: BTreeMap<String, usize> = [
        ("missing_reference_score".into(), 0),
        ("no_competitor".into(), 0),
    ]
    .into_iter()
    .collect();
    for row in rows {
        let cands = row["candidates"].as_array().context("candidates")?;
        let mut order: Vec<String> = Vec::new();
        let mut values: HashMap<String, [f64; 2]> = HashMap::new();
        for c in cands {
            let z: f64 = c["zenz"].as_str().context("zenz")?.parse()?;
            let l: f64 = c["llm"].as_str().context("llm")?.parse()?;
            if z.is_nan() || z == f64::INFINITY || l.is_nan() || l == f64::INFINITY {
                bail!("採点特徴がNaNまたは正の無限大です");
            }
            if !z.is_finite() || !l.is_finite() {
                continue;
            }
            let text = c["text"].as_str().context("text")?.to_owned();
            let feat = [z, l];
            if let Some(&prev) = values.get(&text) {
                if prev != feat {
                    bail!("同じ候補の採点特徴が一致しません");
                }
            } else {
                order.push(text.clone());
                values.insert(text, feat);
            }
        }
        let references: HashSet<String> = row["references"]
            .as_array()
            .context("references")?
            .iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect();
        if !order.iter().any(|t| references.contains(t)) {
            *excluded
                .entry("missing_reference_score".into())
                .or_insert(0) += 1;
            continue;
        }
        if order.iter().all(|t| references.contains(t)) {
            *excluded.entry("no_competitor".into()).or_insert(0) += 1;
            continue;
        }
        let x: Vec<[f64; 2]> = order.iter().map(|t| values[t]).collect();
        let sg: HashSet<String> = row["source_groups"]
            .as_array()
            .context("source_groups")?
            .iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect();
        result.push(RankerExample {
            index: row["index"].as_str().context("index")?.to_owned(),
            reading: row["reading"].as_str().context("reading")?.to_owned(),
            references,
            texts: order,
            x,
            final_text: row["final"].as_str().context("final")?.to_owned(),
            source_groups: sg,
            split: row["split"].as_str().context("split")?.to_owned(),
        });
    }
    if result.is_empty() {
        bail!("有限の参照スコアと競合候補を持つ実例がありません");
    }
    Ok((result, excluded))
}

fn logsumexp(vals: &[f64]) -> f64 {
    let m = vals.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    m + vals.iter().map(|&v| (v - m).exp()).sum::<f64>().ln()
}

pub fn loss_gradient(
    example: &RankerExample,
    weights: [f64; 2],
    scale: [f64; 2],
) -> (f64, [f64; 2]) {
    let base = example.x[0];
    let x: Vec<[f64; 2]> = example
        .x
        .iter()
        .map(|v| [(v[0] - base[0]) / scale[0], (v[1] - base[1]) / scale[1]])
        .collect();
    let scores: Vec<f64> = x
        .iter()
        .map(|v| v[0] * weights[0] + v[1] * weights[1])
        .collect();
    let gold: Vec<usize> = example
        .texts
        .iter()
        .enumerate()
        .filter(|(_, t)| example.references.contains(*t))
        .map(|(i, _)| i)
        .collect();
    let total = logsumexp(&scores);
    let gold_scores: Vec<f64> = gold.iter().map(|&i| scores[i]).collect();
    let positive = logsumexp(&gold_scores);
    let mut grad = [0.0; 2];
    for j in 0..2 {
        let all_term: f64 = (0..x.len())
            .map(|i| (scores[i] - total).exp() * x[i][j])
            .sum();
        let pos_term: f64 = gold
            .iter()
            .map(|&i| (scores[i] - positive).exp() * x[i][j])
            .sum();
        grad[j] = all_term - pos_term;
    }
    (total - positive, grad)
}

pub fn fit_ranker(train: &[RankerExample], regularization: f64, steps: usize) -> [f64; 2] {
    let mut count = 0usize;
    let mut sq = [0.0; 2];
    for e in train {
        let base = e.x[0];
        for v in &e.x {
            sq[0] += (v[0] - base[0]).powi(2);
            sq[1] += (v[1] - base[1]).powi(2);
            count += 1;
        }
    }
    let scale = [
        (sq[0] / (count as f64)).sqrt().max(1e-6),
        (sq[1] / (count as f64)).sqrt().max(1e-6),
    ];
    let mut weights = [0.0; 2];
    let n = train.len() as f64;
    for _ in 0..steps {
        let mut grad = [0.0; 2];
        for e in train {
            let (_, g) = loss_gradient(e, weights, scale);
            grad[0] += g[0];
            grad[1] += g[1];
        }
        weights[0] = (weights[0] - 0.1 * (grad[0] / n + regularization * weights[0])).max(0.0);
        weights[1] = (weights[1] - 0.1 * (grad[1] / n + regularization * weights[1])).max(0.0);
    }
    [weights[0] / scale[0], weights[1] / scale[1]]
}

pub fn evaluate_ranker(data: &[RankerExample], weights: [f64; 2]) -> Value {
    let (mut before, mut after, mut improved, mut regressed) = (0usize, 0usize, 0usize, 0usize);
    let (mut observed_final, mut downstream_changes) = (0usize, 0usize);
    let mut loss = 0.0;
    for e in data {
        let top =
            e.x.iter()
                .enumerate()
                .max_by(|(_, a), (_, b)| {
                    (a[0] * weights[0] + a[1] * weights[1])
                        .total_cmp(&(b[0] * weights[0] + b[1] * weights[1]))
                })
                .map(|(i, _)| i)
                .unwrap_or(0);
        let legacy_top =
            e.x.iter()
                .enumerate()
                .max_by(|(_, a), (_, b)| (a[0] + 0.7 * a[1]).total_cmp(&(b[0] + 0.7 * b[1])))
                .map(|(i, _)| i)
                .unwrap_or(0);
        let old = e.references.contains(&e.texts[legacy_top]);
        let new = e.references.contains(&e.texts[top]);
        if e.references.contains(&e.final_text) {
            observed_final += 1;
        }
        if e.final_text != e.texts[legacy_top] {
            downstream_changes += 1;
        }
        if old {
            before += 1;
        }
        if new {
            after += 1;
        }
        if new && !old {
            improved += 1;
        }
        if old && !new {
            regressed += 1;
        }
        loss += loss_gradient(e, weights, [1.0, 1.0]).0;
    }
    json!({
        "eligible_cases": data.len(),
        "before": before,
        "after": after,
        "improved": improved,
        "regressed": regressed,
        "nll": loss / (data.len() as f64),
        "observed_final_correct": observed_final,
        "pool_vs_final_surface_changes": downstream_changes,
        "scope": "offline_pool_ranking_not_final_conversion",
    })
}

/// `fetch_generator.py`: 固定 SHA の追加生成モデルを配置する。
pub fn fetch_generator(
    repo_root: &Path,
    destination: &Path,
    source: Option<&Path>,
    manifest_override: Option<&Path>,
) -> Result<PathBuf> {
    let default_manifest = repo_root.join("training/contrastive/native_generator.json");
    let manifest_path = manifest_override.unwrap_or(&default_manifest);
    let identity: Value = serde_json::from_str(&fs::read_to_string(manifest_path)?)?;
    let name = identity["filename"].as_str().context("filename")?;
    let expected = identity["sha256"].as_str().context("sha256")?;
    let name_re = Regex::new(r"^[a-zA-Z0-9_.-]+\.gguf$")?;
    let sha_re = Regex::new(r"^[0-9a-f]{64}$")?;
    if !name_re.is_match(name) || !sha_re.is_match(expected) {
        bail!("Invalid model identity");
    }
    fs::create_dir_all(destination)?;
    let target = destination.join(name);
    if target.exists() {
        if sha256_file(&target)? != expected {
            bail!("Existing model differs from pinned identity");
        }
    } else {
        let tmp = destination.join(format!("{name}.partial"));
        if let Some(src) = source {
            fs::copy(src, &tmp)?;
            if sha256_file(&tmp)? != expected {
                let _ = fs::remove_file(&tmp);
                bail!("Model SHA-256 differs from pinned identity");
            }
            fs::rename(&tmp, &target)?;
        } else {
            let url = identity["download_url"].as_str().context("download_url")?;
            if !url.starts_with("https://github.com/aruiki/Asteria-Japanese-IME/releases/download/")
            {
                bail!("Unexpected model download origin");
            }
            download_verified(
                url,
                &target,
                None,
                expected,
                "Asteria-generator/1",
                Duration::from_secs(120),
            )?;
        }
    }
    fs::write(
        destination.join("native_generator.json"),
        format!("{}\n", serde_json::to_string_pretty(&identity)?),
    )?;
    Ok(target)
}

/// `stage_generator.py`: ローカルモデルバンドルを検証付きで構築する。
pub fn stage_generator(
    repo_root: &Path,
    generator: &Path,
    base_dir: &Path,
    out: &Path,
) -> Result<Value> {
    if out.exists() {
        bail!("Existing bundles are preserved");
    }
    let manifest: Value = serde_json::from_str(&fs::read_to_string(
        repo_root.join("training/contrastive/native_generator.json"),
    )?)?;
    let mut sources = Vec::new();
    if let Some(bm) = manifest["base_models"].as_object() {
        for (name, sha) in bm {
            sources.push((
                base_dir.join(name),
                name.clone(),
                sha.as_str().context("base sha")?.to_owned(),
            ));
        }
    }
    sources.push((
        generator.to_path_buf(),
        manifest["filename"]
            .as_str()
            .context("filename")?
            .to_owned(),
        manifest["sha256"].as_str().context("sha256")?.to_owned(),
    ));
    for (src, name, exp) in &sources {
        if sha256_file(src)? != *exp {
            bail!("Model identity mismatch: {name}");
        }
    }
    fs::create_dir_all(out)?;
    let mut models = serde_json::Map::new();
    for (src, name, exp) in sources {
        let dest = out.join(&name);
        fs::copy(&src, &dest)?;
        if sha256_file(&dest)? != exp {
            bail!("Staged model identity mismatch: {name}");
        }
        models.insert(name, json!(exp));
    }
    let receipt = json!({
        "models": models,
        "generator_manifest": manifest,
        "installed_system_modified": false,
    });
    fs::write(
        out.join("bundle.json"),
        serde_json::to_string_pretty(&receipt)?,
    )?;
    Ok(receipt)
}

/// `native_pairs.py`: 実観測候補からの対比ペア構築と漏洩検証。
pub fn load_observed_pairs(path: &Path, split: &str) -> Result<Vec<Value>> {
    let manifest: Value =
        serde_json::from_str(&fs::read_to_string(path.with_file_name("manifest.json"))?)?;
    if Some(sha256_file(path)?.as_str()) != manifest["observations_sha256"].as_str() {
        bail!("Observed candidate SHA mismatch");
    }
    let mut rows = Vec::new();
    for line in fs::read_to_string(path)?.lines() {
        if !line.trim().is_empty() {
            let v: Value = serde_json::from_str(line)?;
            if v["split"].as_str() != Some(split) {
                bail!("Observed split mismatch");
            }
            rows.push(v);
        }
    }
    let mut pairs = Vec::new();
    for row in rows {
        let references: HashSet<&str> = row["references"]
            .as_array()
            .context("references")?
            .iter()
            .filter_map(Value::as_str)
            .collect();
        let cands = row["candidates"].as_array().context("candidates")?;
        let mut best_pos: Option<(f64, &str)> = None;
        let mut alternatives: HashMap<String, (f64, f64)> = HashMap::new();
        for c in cands {
            let text = c["text"].as_str().context("text")?;
            let z: f64 = c["zenz"].as_str().context("zenz")?.parse()?;
            let l: f64 = c["llm"].as_str().context("llm")?.parse()?;
            let score = z + 1.2 * l;
            if !z.is_finite() || !l.is_finite() || !score.is_finite() {
                continue;
            }
            if references.contains(text) {
                if best_pos.is_none_or_legacy(score) {
                    best_pos = Some((score, text));
                }
            } else {
                alternatives
                    .entry(text.to_owned())
                    .and_modify(|prev| {
                        if score > prev.0 {
                            *prev = (score, l);
                        }
                    })
                    .or_insert((score, l));
            }
        }
        let Some((pos_score, pos_text)) = best_pos else {
            continue;
        };
        let pos_llm = cands
            .iter()
            .filter(|c| c["text"].as_str() == Some(pos_text))
            .filter_map(|c| {
                let z: f64 = c["zenz"].as_str()?.parse().ok()?;
                let l: f64 = c["llm"].as_str()?.parse().ok()?;
                Some((z + 1.2 * l, l))
            })
            .find(|(s, _)| (*s - pos_score).abs() < 1e-12)
            .map(|(_, l)| l)
            .unwrap_or(0.0);

        let mut alt_vec: Vec<(String, f64, f64)> = alternatives
            .into_iter()
            .map(|(t, (s, l))| (t, s, l))
            .collect();
        alt_vec.sort_by(|a, b| b.1.total_cmp(&a.1));
        for (neg, _, neg_llm) in alt_vec.into_iter().take(3) {
            pairs.push(json!({
                "reading": row["reading"],
                "context": row["context"],
                "positive": pos_text,
                "negative": neg,
                "frozen_llm_score_difference": pos_llm - neg_llm,
                "split": split,
                "source_groups": row["source_groups"],
                "kind": "native_contrast",
                "supervision": "corpus_reference_preference",
                "alternatives_semantically_reviewed": false,
            }));
        }
    }
    Ok(pairs)
}

trait OptScoreExt {
    fn is_none_or_legacy(&self, score: f64) -> bool;
}
impl OptScoreExt for Option<(f64, &str)> {
    fn is_none_or_legacy(&self, score: f64) -> bool {
        match self {
            None => true,
            Some((prev, _)) => score > *prev,
        }
    }
}

pub fn validate_contrastive_pairs(pairs: &[Value], excluded: &HashSet<String>) -> Result<()> {
    let mut keys_train = HashSet::new();
    let mut keys_dev = HashSet::new();
    let mut groups_train = HashSet::new();
    let mut groups_dev = HashSet::new();
    let excl_refs: Vec<&str> = excluded.iter().map(String::as_str).collect();
    let overlaps = Exclusions::new(excl_refs, 0.8);

    for r in pairs {
        let split = r["split"].as_str().unwrap_or("");
        let sgs = r["source_groups"].as_array().filter(|a| !a.is_empty());
        let Some(sgs) = sgs else {
            bail!("Missing split or corpus groups");
        };
        if split != "train" && split != "dev" {
            bail!("Missing split or corpus groups");
        }
        let key = reading_key(r["reading"].as_str().unwrap_or(""));
        if overlaps.overlaps(&key) {
            bail!("Evaluation reading leaked into training");
        }
        let pos = r["positive"].as_str().unwrap_or("");
        let neg = r["negative"].as_str().unwrap_or("");
        if pos.is_empty() || neg.is_empty() || pos == neg {
            bail!("Empty or identical pair");
        }
        if let Some(diff) = r.get("frozen_llm_score_difference").and_then(Value::as_f64) {
            if !diff.is_finite() {
                bail!("Non-finite frozen LLM score");
            }
        }
        if split == "train" {
            keys_train.insert(key);
            for g in sgs {
                if let Some(s) = g.as_str() {
                    groups_train.insert(s.to_owned());
                }
            }
        } else {
            keys_dev.insert(key);
            for g in sgs {
                if let Some(s) = g.as_str() {
                    groups_dev.insert(s.to_owned());
                }
            }
        }
    }
    if keys_train.intersection(&keys_dev).next().is_some()
        || groups_train.intersection(&groups_dev).next().is_some()
    {
        bail!("Train/dev reading or corpus-group leakage");
    }
    let dev_refs: Vec<&str> = keys_dev.iter().map(String::as_str).collect();
    let near_dev = Exclusions::new(dev_refs, 0.8);
    if keys_train.iter().any(|k| near_dev.overlaps(k)) {
        bail!("Near-duplicate train/dev reading leakage");
    }
    Ok(())
}

pub fn run_native_pairs(
    observed: &Path,
    general: &Path,
    out: &Path,
    source_root: &Path,
) -> Result<Value> {
    let manifest_out = out.with_extension("manifest.json");
    if out.exists() || manifest_out.exists() {
        bail!("Preserve prior dataset outputs");
    }
    let src_manifest: Value = serde_json::from_str(&fs::read_to_string(
        general.with_extension("manifest.json"),
    )?)?;
    if Some(sha256_file(general)?.as_str()) != src_manifest["pairs_sha256"].as_str() {
        bail!("General corpus SHA mismatch");
    }
    let mut excluded = HashSet::new();
    let mut exclusions = Vec::new();
    for item in src_manifest["excluded_evaluations"]
        .as_array()
        .context("excluded_evaluations")?
    {
        let file_s = item["file"].as_str().context("file")?;
        let p = if Path::new(file_s).is_absolute() {
            PathBuf::from(file_s)
        } else {
            source_root.join(file_s)
        };
        if Some(sha256_file(&p)?.as_str()) != item["sha256"].as_str() {
            bail!("Evaluation source changed");
        }
        let val: Value = serde_json::from_str(&fs::read_to_string(&p)?)?;
        for r in evaluation_readings(&val) {
            excluded.insert(r);
        }
        exclusions.push(json!({
            "file": fs::canonicalize(&p)?.to_string_lossy(),
            "sha256": item["sha256"],
        }));
    }

    let mut pairs = Vec::new();
    let mut inputs = serde_json::Map::new();
    for split in ["train", "dev"] {
        let p = observed.join(format!("{split}-collected/observations.jsonl"));
        pairs.extend(load_observed_pairs(&p, split)?);
        inputs.insert(split.into(), json!(sha256_file(&p)?));
    }
    let native_keys: HashSet<String> = pairs
        .iter()
        .filter_map(|r| r["reading"].as_str().map(reading_key))
        .collect();
    let excl_refs: Vec<&str> = excluded.iter().map(String::as_str).collect();
    let overlaps = Exclusions::new(excl_refs, 0.8);
    let mut rng = rand::rngs::StdRng::seed_from_u64(20261017);
    let general_rows: Vec<Value> = fs::read_to_string(general)?
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(serde_json::from_str)
        .collect::<std::result::Result<Vec<_>, _>>()?;

    for (split, limit) in [("train", 2000usize), ("dev", 256usize)] {
        let mut rows: Vec<Value> = general_rows
            .iter()
            .filter(|r| {
                r["split"].as_str() == Some(split)
                    && r.get("kind").and_then(Value::as_str).unwrap_or("original") == "original"
                    && {
                        let k = reading_key(r["reading"].as_str().unwrap_or(""));
                        !native_keys.contains(&k) && !overlaps.overlaps(&k)
                    }
            })
            .cloned()
            .collect();
        rows.shuffle(&mut rng);
        let mut seen = HashSet::new();
        let mut count = 0usize;
        for mut row in rows {
            let k = reading_key(row["reading"].as_str().unwrap_or(""));
            if !seen.insert(k) {
                continue;
            }
            if let Some(obj) = row.as_object_mut() {
                let sg = obj.get("source_group").cloned().unwrap_or(Value::Null);
                obj.insert("source_groups".into(), json!([sg]));
            }
            pairs.push(row);
            count += 1;
            if count >= limit {
                break;
            }
        }
    }
    validate_contrastive_pairs(&pairs, &excluded)?;
    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut body = String::new();
    for r in &pairs {
        body.push_str(&serde_json::to_string(r)?);
        body.push('\n');
    }
    fs::write(out, body.as_bytes())?;
    let mut counts = serde_json::Map::new();
    for s in ["train", "dev"] {
        for k in ["original", "native_contrast"] {
            let n = pairs
                .iter()
                .filter(|r| {
                    r["split"].as_str() == Some(s)
                        && r.get("kind").and_then(Value::as_str).unwrap_or("original") == k
                })
                .count();
            counts.insert(format!("{s}/{k}"), json!(n));
        }
    }
    let manifest = json!({
        "pairs_sha256": sha256_file(out)?,
        "observed_sha256": inputs,
        "general_pairs_sha256": sha256_file(general)?,
        "excluded_evaluations": exclusions,
        "counts": counts,
        "supervision": "corpus_reference_preference",
        "product_adopted": false,
    });
    fs::write(&manifest_out, serde_json::to_string_pretty(&manifest)?)?;
    Ok(manifest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use asteria_common::hash::sha256_bytes;
    use tempfile::TempDir;

    fn row(
        split: &str,
        reading: &str,
        positive: &str,
        group: &str,
        kind: &str,
    ) -> TrainingInputRow {
        TrainingInputRow {
            split: split.into(),
            reading: reading.into(),
            positive: positive.into(),
            context: String::new(),
            source_group: group.into(),
            kind: kind.into(),
        }
    }

    #[test]
    fn training_separation_rejects_leaks_and_duplicates() {
        let rows = vec![
            row(
                "train",
                "きょうはあめがふっています",
                "今日は雨が降っています",
                "train-c",
                "original",
            ),
            row(
                "train",
                "あすのかいぎはごごです",
                "明日の会議は午後です",
                "train-c",
                "original",
            ),
            row(
                "dev",
                "いぬとこうえんをさんぽする",
                "犬と公園を散歩する",
                "dev-c",
                "original",
            ),
        ];
        let excl: HashSet<String> = ["きょうはあめがふっています。".into()]
            .into_iter()
            .collect();
        let (sel, counts) = prepare_training_cases(&rows, &excl, 1, 1, 20261017, 64).unwrap();
        assert_eq!(sel["train"][0]["input"], "あすのかいぎはごごです");
        assert_eq!(counts.get("evaluation_reading"), Some(&1));

        let cross = vec![
            row("train", "かたかなのよみ", "A", "g1", "original"),
            row("dev", "カタカナノヨミ", "B", "g2", "original"),
        ];
        assert!(prepare_training_cases(&cross, &HashSet::new(), 1, 1, 1, 64).is_err());

        let sg_leak = vec![
            row("train", "あすのかいぎのじかん", "A", "same", "original"),
            row("dev", "いぬとこうえんをさんぽする", "B", "same", "original"),
        ];
        assert!(prepare_training_cases(&sg_leak, &HashSet::new(), 1, 1, 1, 64).is_err());
    }

    #[test]
    fn listwise_ranker_learns_and_rejects_bad_scores() {
        let obs = json!({
            "index": "train-1", "reading": "はしをつかう", "references": ["reference"],
            "final": "other", "source_groups": ["corpus-train"], "split": "train",
            "candidates": [
                {"text": "other", "zenz": "-4", "llm": "-5"},
                {"text": "reference", "zenz": "-1", "llm": "-2"},
            ]
        });
        let (data, _) = build_ranker_examples(std::slice::from_ref(&obs)).unwrap();
        let weights = fit_ranker(&data, 0.01, 100);
        assert!(weights.iter().all(|&w| w.is_finite() && w >= 0.0));
        assert_eq!(evaluate_ranker(&data, weights)["after"], 1);
        let rep = evaluate_ranker(&data, [1.0, 0.7]);
        assert_eq!(rep["improved"], 0);
        assert_eq!(rep["pool_vs_final_surface_changes"], 1);

        let mut bad_nan = obs.clone();
        bad_nan["candidates"][0]["zenz"] = json!("nan");
        assert!(build_ranker_examples(&[bad_nan]).is_err());
    }

    #[test]
    fn fetch_generator_and_native_pairs_validation() {
        let dir = TempDir::new().unwrap();
        let src = dir.path().join("source.gguf");
        fs::write(&src, b"model-bytes").unwrap();
        let manifest_path = dir.path().join("manifest.json");
        fs::write(
            &manifest_path,
            json!({
                "filename": "native.gguf",
                "sha256": sha256_bytes(b"model-bytes"),
                "download_url": "https://github.com/aruiki/Asteria-Japanese-IME/releases/download/tag/native.gguf",
            })
            .to_string(),
        )
        .unwrap();
        let dest = dir.path().join("out");
        let staged = fetch_generator(dir.path(), &dest, Some(&src), Some(&manifest_path)).unwrap();
        assert_eq!(sha256_file(&staged).unwrap(), sha256_bytes(b"model-bytes"));

        let leak_pair = vec![
            json!({"split": "train", "reading": "てすとのよみです", "positive": "A", "negative": "B", "source_groups": ["g1"]}),
            json!({"split": "dev", "reading": "てすとのよみです", "positive": "A", "negative": "B", "source_groups": ["g2"]}),
        ];
        assert!(validate_contrastive_pairs(&leak_pair, &HashSet::new()).is_err());
    }
}
