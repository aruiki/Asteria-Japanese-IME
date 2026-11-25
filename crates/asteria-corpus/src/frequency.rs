//! Wikipedia 全量コーパスのシャード分割・Sudachi 並列集計・頻度マージ（`collect_frequency.py` 移植）。

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use anyhow::{bail, Context, Result};
use rayon::prelude::*;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sudachi::analysis::stateless_tokenizer::StatelessTokenizer;
use sudachi::analysis::Tokenize;
use sudachi::config::Config;
use sudachi::dic::dictionary::JapaneseDictionary;
use sudachi::prelude::*;
use unicode_normalization::UnicodeNormalization;

use asteria_common::hash::{hex_lower, sha256_bytes, sha256_file};
use asteria_common::kana::{evaluation_readings, reading_key, Exclusions};

use crate::lexicon::{
    is_valid_reading, is_valid_surface_char, pos_mapping, stage_tokenizer, verify_source_file,
};

fn is_kanji(c: char) -> bool {
    matches!(c, '\u{3400}'..='\u{4dbf}' | '\u{4e00}'..='\u{9fff}')
}

/// 予約済みバイト区間とレコード `[start, end)` が重なるか判定する。
pub fn reserved(start: u64, end: u64, policy: &Value) -> bool {
    let Some(ranges) = policy["reserved_byte_ranges"].as_array() else {
        return false;
    };
    ranges.iter().any(|item| {
        let off = item["offset"].as_u64().unwrap_or(0);
        let len = item["bytes"].as_u64().unwrap_or(0);
        start < off + len && end > off
    })
}

/// 固定パスの Sudachi システム辞書 (`system_full.dic`) を読み込む。
pub fn load_sudachi_dict(dict_path: &Path) -> Result<JapaneseDictionary> {
    let cfg = Config::new(None, None, Some(dict_path.to_path_buf()))
        .map_err(|e| anyhow::anyhow!("Sudachi Config 構築エラー: {e}"))?;
    JapaneseDictionary::from_cfg(&cfg)
        .map_err(|e| anyhow::anyhow!("Sudachi 辞書読込エラー ({}): {e}", dict_path.display()))
}

/// コーパスを本文 SHA-256 先頭 4 バイトで `shards` 個の JSONL ファイルへ分割する。
pub fn shard_corpus(
    corpus: &Path,
    out: &Path,
    shards: usize,
    policy: &Value,
    limit_bytes: u64,
) -> Result<Value> {
    let mut handles: Vec<BufWriter<File>> = (0..shards)
        .map(|i| {
            let p = out.join(format!("input-{i:02}.jsonl"));
            let f = OpenOptions::new().write(true).create_new(true).open(p)?;
            Ok(BufWriter::with_capacity(256 * 1024, f))
        })
        .collect::<Result<Vec<_>>>()?;

    let min_len = policy["min_text_length"].as_u64().unwrap_or(0) as usize;
    let max_len = policy["max_text_length"]
        .as_u64()
        .unwrap_or(usize::MAX as u64) as usize;

    let file = File::open(corpus)?;
    let corpus_size = file.metadata()?.len();
    let mut reader = BufReader::with_capacity(1024 * 1024, file);
    let mut source_digest = Sha256::new();
    let mut audit: BTreeMap<String, u64> = BTreeMap::new();
    let mut position: u64 = 0;
    let mut buf = Vec::with_capacity(4096);

    loop {
        buf.clear();
        let n = reader.read_until(b'\n', &mut buf)?;
        if n == 0 {
            break;
        }
        source_digest.update(&buf);
        let start = position;
        position += n as u64;
        if limit_bytes > 0 && start >= limit_bytes {
            break;
        }
        *audit.entry("records_read".into()).or_insert(0) += 1;
        if reserved(start, position, policy) {
            *audit.entry("reserved_interval".into()).or_insert(0) += 1;
            continue;
        }
        let item: Value = serde_json::from_slice(&buf)?;
        let Some(inp) = item.get("input").and_then(Value::as_str) else {
            bail!("固定コーパスの形式が一致しません");
        };
        let Some(outp) = item.get("output").and_then(Value::as_str) else {
            bail!("固定コーパスの形式が一致しません");
        };
        let text: String = outp.nfkc().collect();
        let t_len = text.chars().count();
        if !(min_len..=max_len).contains(&t_len) {
            *audit.entry("invalid_length".into()).or_insert(0) += 1;
            continue;
        }
        let id_bytes = Sha256::digest(text.as_bytes());
        let bucket_u32 = u32::from_le_bytes([id_bytes[0], id_bytes[1], id_bytes[2], id_bytes[3]]);
        let bucket = (bucket_u32 as usize) % shards;
        let id_hex = hex_lower(&id_bytes);
        let row = json!([inp, text, id_hex]);
        serde_json::to_writer(&mut handles[bucket], &row)?;
        handles[bucket].write_all(b"\n")?;
        *audit.entry("sharded_records".into()).or_insert(0) += 1;
    }
    for h in &mut handles {
        h.flush()?;
    }
    audit.insert("source_bytes_scanned".into(), position.min(corpus_size));
    let mut res = serde_json::Map::new();
    for (k, v) in audit {
        res.insert(k, json!(v));
    }
    let hash_key = if limit_bytes == 0 {
        "full_source_sha256"
    } else {
        "scanned_prefix_sha256"
    };
    res.insert(hash_key.into(), json!(hex_lower(&source_digest.finalize())));
    Ok(Value::Object(res))
}

#[derive(Debug, Default, Clone, Copy)]
pub struct FreqEntry {
    pub occurrences: u64,
    pub documents: u64,
    pub buckets: u64,
}

impl FreqEntry {
    fn add(&mut self, occ: u64, docs: u64, bucket_mask: u64) {
        self.occurrences += occ;
        self.documents += docs;
        self.buckets |= bucket_mask;
    }
}

/// 1 本文の形態素特徴量 (単語頻度, 連語頻度, 表層頻度)。
pub type TokenFeatureMaps = (
    HashMap<(String, String, String), u64>,
    HashMap<String, u64>,
    HashMap<String, u64>,
);

/// 1 本文を Sudachi で形態素解析し、(単語頻度, 連語頻度, 表層頻度) を返す。
pub fn token_features(
    tokenizer: &StatelessTokenizer<&JapaneseDictionary>,
    text: &str,
    policy: &Value,
) -> Result<TokenFeatureMaps> {
    let morphemes = tokenizer.tokenize(text, Mode::C, false)?;
    let mut words: HashMap<(String, String, String), u64> = HashMap::new();
    let mut surfaces: HashMap<String, u64> = HashMap::new();
    let mut tokens: Vec<(String, String, bool)> = Vec::with_capacity(morphemes.len());

    for m in morphemes.iter() {
        let surface = m.surface();
        let pos = m.part_of_speech();
        let valid = !surface.is_empty() && surface.chars().all(is_valid_surface_char);
        let pos0 = pos.first().map(String::as_str).unwrap_or("");
        tokens.push((surface.to_string(), pos0.to_owned(), valid));
        if valid {
            *surfaces.entry(surface.to_string()).or_insert(0) += 1;
        }
        let mapped = if pos0 == "名詞" && pos.len() >= 4 {
            pos_mapping(&pos[1], &pos[2], &pos[3])
        } else {
            None
        };
        let reading = reading_key(m.reading_form());
        if let Some(mpos) = mapped {
            if valid && is_valid_reading(&reading) && surface.chars().count() <= 32 {
                *words
                    .entry((reading, surface.to_string(), mpos.to_owned()))
                    .or_insert(0) += 1;
            }
        }
    }

    let mut phrases: HashMap<String, u64> = HashMap::new();
    let content: HashSet<&str> = policy["content_pos"]
        .as_array()
        .map(|a| a.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    let sizes: Vec<usize> = policy["phrase_tokens"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_u64().map(|n| n as usize))
                .collect()
        })
        .unwrap_or_default();
    let min_plen = policy["min_phrase_length"].as_u64().unwrap_or(0) as usize;
    let max_plen = policy["max_phrase_length"].as_u64().unwrap_or(100) as usize;

    for start in 0..tokens.len() {
        for &size in &sizes {
            if start + size > tokens.len() {
                continue;
            }
            let win = &tokens[start..start + size];
            if !win.iter().all(|t| t.2)
                || !content.contains(win[0].1.as_str())
                || !content.contains(win[win.len() - 1].1.as_str())
            {
                continue;
            }
            let value: String = win.iter().map(|t| t.0.as_str()).collect();
            let v_len = value.chars().count();
            if (min_plen..=max_plen).contains(&v_len) && value.chars().any(is_kanji) {
                *phrases.entry(value).or_insert(0) += 1;
            }
        }
    }
    Ok((words, phrases, surfaces))
}

#[derive(Default)]
pub struct ShardCounts {
    pub words: HashMap<(String, String, String), FreqEntry>,
    pub phrases: HashMap<String, FreqEntry>,
    pub surfaces: HashMap<String, FreqEntry>,
    pub summary: Value,
}

/// 1 シャードの JSONL をネイティブ Sudachi で並列処理する。
pub fn process_shard_native(
    shard: usize,
    out: &Path,
    dict: &JapaneseDictionary,
    policy: &Value,
    overlaps: &Exclusions,
) -> Result<ShardCounts> {
    let started = Instant::now();
    let tokenizer = StatelessTokenizer::new(dict);
    let max_unique = policy["max_unique_records_per_shard"]
        .as_u64()
        .unwrap_or(u64::MAX) as usize;
    let bucket_count = policy["bucket_count"].as_u64().unwrap_or(16) as u8;

    let input_path = out.join(format!("input-{shard:02}.jsonl"));
    let mut seen: HashSet<[u8; 32]> = HashSet::new();
    let mut sc = ShardCounts::default();
    let mut audit: BTreeMap<String, u64> = BTreeMap::new();

    for line in BufReader::new(File::open(&input_path)?).lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let arr: (String, String, String) = serde_json::from_str(&line)?;
        let (reading, text, id_hex) = arr;
        *audit.entry("records".into()).or_insert(0) += 1;
        if overlaps.overlaps(&reading_key(&reading)) {
            *audit
                .entry("evaluation_reading_or_near_duplicate".into())
                .or_insert(0) += 1;
            continue;
        }
        let id_bytes = decode_32_hex(&id_hex)?;
        if !seen.insert(id_bytes) {
            *audit.entry("duplicate_text".into()).or_insert(0) += 1;
            continue;
        }
        if seen.len() > max_unique {
            bail!("重複除去のメモリ上限に到達しました");
        }
        let bucket = 1u64 << (id_bytes[8] % bucket_count);
        let (w_map, p_map, s_map) = token_features(&tokenizer, &text, policy)?;
        for (k, occ) in w_map {
            sc.words.entry(k).or_default().add(occ, 1, bucket);
        }
        for (k, occ) in p_map {
            sc.phrases.entry(k).or_default().add(occ, 1, bucket);
        }
        for (k, occ) in s_map {
            sc.surfaces.entry(k).or_default().add(occ, 1, bucket);
        }
        *audit.entry("unique_accepted_texts".into()).or_insert(0) += 1;
    }

    let total_keys = sc.words.len() + sc.phrases.len() + sc.surfaces.len();
    audit.insert("database_keys".into(), total_keys as u64);
    sc.summary = json!({
        "shard": shard,
        "audit": audit,
        "seconds": started.elapsed().as_secs_f64(),
        "input_sha256": sha256_file(&input_path)?,
    });
    fs::write(
        out.join(format!("shard-{shard:02}.json")),
        format!("{}\n", serde_json::to_string_pretty(&sc.summary)?),
    )?;
    Ok(sc)
}

fn decode_32_hex(s: &str) -> Result<[u8; 32]> {
    if s.len() != 64 {
        bail!("SHA-256 hex 長さが不正です");
    }
    let mut out = [0u8; 32];
    for i in 0..32 {
        out[i] = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16)?;
    }
    Ok(out)
}

/// 複数シャードの集計マップを結合し、ソート済み TSV 3 ファイルへ書き出す。
pub fn merge_shard_counts(
    out: &Path,
    shards: Vec<ShardCounts>,
    min_phrase_docs: u64,
) -> Result<Vec<Value>> {
    let mut words: BTreeMap<(String, String, String), FreqEntry> = BTreeMap::new();
    let mut phrases: BTreeMap<String, FreqEntry> = BTreeMap::new();
    let mut surfaces: BTreeMap<String, FreqEntry> = BTreeMap::new();

    for sc in shards {
        for (k, v) in sc.words {
            words
                .entry(k)
                .or_default()
                .add(v.occurrences, v.documents, v.buckets);
        }
        for (k, v) in sc.phrases {
            phrases
                .entry(k)
                .or_default()
                .add(v.occurrences, v.documents, v.buckets);
        }
        for (k, v) in sc.surfaces {
            surfaces
                .entry(k)
                .or_default()
                .add(v.occurrences, v.documents, v.buckets);
        }
    }

    let mut summaries = Vec::new();
    {
        let p = out.join("word_frequency.tsv");
        let mut w = BufWriter::new(File::create(&p)?);
        let mut count = 0usize;
        for ((r, s, pos), e) in &words {
            writeln!(
                w,
                "{r}\t{s}\t{pos}\t{}\t{}\t{}",
                e.occurrences, e.documents, e.buckets
            )?;
            count += 1;
        }
        w.flush()?;
        summaries.push(json!({
            "file": "word_frequency.tsv", "entries": count,
            "bytes": fs::metadata(&p)?.len(), "sha256": sha256_file(&p)?,
        }));
    }
    {
        let p = out.join("phrase_frequency.tsv");
        let mut w = BufWriter::new(File::create(&p)?);
        let mut count = 0usize;
        for (k, e) in &phrases {
            if e.documents < min_phrase_docs {
                continue;
            }
            writeln!(w, "{k}\t{}\t{}\t{}", e.occurrences, e.documents, e.buckets)?;
            count += 1;
        }
        w.flush()?;
        summaries.push(json!({
            "file": "phrase_frequency.tsv", "entries": count,
            "bytes": fs::metadata(&p)?.len(), "sha256": sha256_file(&p)?,
        }));
    }
    {
        let p = out.join("surface_frequency.tsv");
        let mut w = BufWriter::new(File::create(&p)?);
        let mut count = 0usize;
        for (k, e) in &surfaces {
            writeln!(w, "{k}\t{}\t{}\t{}", e.occurrences, e.documents, e.buckets)?;
            count += 1;
        }
        w.flush()?;
        summaries.push(json!({
            "file": "surface_frequency.tsv", "entries": count,
            "bytes": fs::metadata(&p)?.len(), "sha256": sha256_file(&p)?,
        }));
    }
    Ok(summaries)
}

/// `collect_frequency.py` の全体パイプラインを Rust ネイティブで並列実行する。
pub fn collect_frequency(
    repo_root: &Path,
    source: &Path,
    runtime_dir: &Path,
    out: &Path,
    exclude_paths: &[PathBuf],
    shards: usize,
    limit_bytes: u64,
) -> Result<Value> {
    if !(1..=16).contains(&shards) {
        bail!("shard数が範囲外です");
    }
    let config_path = repo_root.join("training/lexicon/corpus_sources.json");
    let policy_path = repo_root.join("training/lexicon/frequency_policy.json");
    let config: Value = serde_json::from_str(&fs::read_to_string(&config_path)?)?;
    let policy: Value = serde_json::from_str(&fs::read_to_string(&policy_path)?)?;

    for spec in config["files"].as_array().context("files")? {
        let name = spec["name"].as_str().context("name")?;
        verify_source_file(&source.join(name), spec)?;
    }

    let corpus = source.join("train_wikipedia.jsonl");
    let mut reserved_readings = HashSet::new();
    {
        let mut stream = File::open(&corpus)?;
        for item in policy["reserved_byte_ranges"]
            .as_array()
            .context("reserved")?
        {
            let offset = item["offset"].as_u64().context("offset")?;
            let bytes = item["bytes"].as_u64().context("bytes")? as usize;
            let expected_sha = item["sha256"].as_str().context("sha256")?;
            stream.seek(SeekFrom::Start(offset))?;
            let mut data = vec![0u8; bytes];
            stream.read_exact(&mut data)?;
            if sha256_bytes(&data) != expected_sha {
                bail!("予約済み開発区間のSHAが一致しません");
            }
            let mut lines: Vec<&[u8]> = data.split(|&b| b == b'\n').collect();
            if lines.len() >= 2 {
                lines = lines[1..lines.len() - 1].to_vec();
            } else {
                lines.clear();
            }
            for line in lines {
                let v: Value = serde_json::from_slice(line)?;
                if let Some(inp) = v["input"].as_str() {
                    reserved_readings.insert(reading_key(inp));
                }
            }
        }
    }

    let mut excluded = reserved_readings.clone();
    let mut excl_hashes = BTreeMap::new();
    for p in exclude_paths {
        let val: Value = serde_json::from_str(&fs::read_to_string(p)?)?;
        for r in evaluation_readings(&val) {
            excluded.insert(r);
        }
        let abs = fs::canonicalize(p).unwrap_or_else(|_| p.clone());
        excl_hashes.insert(abs.to_string_lossy().into_owned(), sha256_file(p)?);
    }
    if excluded.is_empty() {
        bail!("評価読みの除外集合が空です");
    }

    let runtime = stage_tokenizer(repo_root, runtime_dir, None)?;
    if out.exists() {
        bail!("出力先が既に存在します: {}", out.display());
    }
    fs::create_dir_all(out)?;
    let started = Instant::now();

    let mut identity = json!({
        "corpus": config,
        "corpus_config_sha256": sha256_file(&config_path)?,
        "policy": policy,
        "policy_sha256": sha256_file(&policy_path)?,
        "exclusion_files": excl_hashes,
        "exclusion_readings": excluded.len(),
        "reserved_interval_readings": reserved_readings.len(),
        "runtime": runtime,
        "shards": shards,
        "limit_bytes": limit_bytes,
        "evaluation_labels_used": false,
        "product_adopted": false,
    });
    fs::write(
        out.join("plan.json"),
        format!("{}\n", serde_json::to_string_pretty(&identity)?),
    )?;

    let sharding = shard_corpus(&corpus, out, shards, &policy, limit_bytes)?;
    if limit_bytes == 0
        && sharding["full_source_sha256"].as_str() != config["files"][0]["sha256"].as_str()
    {
        bail!("集計中にコーパスのSHAが変わりました");
    }
    identity["sharding"] = sharding;

    let dict_path = PathBuf::from(runtime["dictionary"].as_str().context("dictionary")?);
    let dict = Arc::new(load_sudachi_dict(&dict_path)?);
    let excl_refs: Vec<&str> = excluded.iter().map(String::as_str).collect();
    let overlaps = Exclusions::new(
        excl_refs,
        policy["near_duplicate_jaccard"].as_f64().unwrap_or(0.8),
    );

    let shard_results: Vec<ShardCounts> = (0..shards)
        .into_par_iter()
        .map(|s| process_shard_native(s, out, &dict, &policy, &overlaps))
        .collect::<Result<Vec<_>>>()?;

    let shard_summaries: Vec<Value> = shard_results.iter().map(|s| s.summary.clone()).collect();
    identity["shard_results"] = Value::Array(shard_summaries);
    let min_phrase_docs = policy["min_output_phrase_documents"].as_u64().unwrap_or(5);
    identity["outputs"] = Value::Array(merge_shard_counts(out, shard_results, min_phrase_docs)?);
    identity["seconds"] = json!(started.elapsed().as_secs_f64());

    let repo_str = config["repository"].as_str().unwrap_or("");
    let commit_str = config["commit"].as_str().unwrap_or("");
    let notice = format!(
        "Asteriaの語彙・連語頻度候補 / 製品採用前\n\
         CC BY-SA 4.0 https://creativecommons.org/licenses/by-sa/4.0/\n\
         出典: Wikipedia日本語版の執筆者、Keita Miwa / zenz-v2.5-dataset\n\
         https://huggingface.co/datasets/{repo_str}/tree/{commit_str}\n\
         変更: NFKC本文を重複除去し、評価入力/近似読み/開発区間を除外。\n\
         固定Sudachiで名詞と2〜3形態素の連語を数えた頻度データ。CC BY-SA 4.0で提供。\n\
         読みはSudachiの優先読みであり、実コーパスの発音を照合した頻度ではない。\n\
         本文hashの16区分は独立サイト/記事を表さない。\n"
    );
    fs::write(out.join("NOTICE-frequency.txt"), notice)?;
    fs::write(
        out.join("frequency.json"),
        format!("{}\n", serde_json::to_string_pretty(&identity)?),
    )?;
    Ok(identity)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn reserved_interval_excludes_partly_overlapping_records() {
        let policy = json!({"reserved_byte_ranges": [{"offset": 10, "bytes": 10}]});
        assert!(!reserved(0, 10, &policy));
        assert!(reserved(5, 11, &policy));
        assert!(reserved(19, 21, &policy));
        assert!(!reserved(20, 25, &policy));
    }

    #[test]
    fn normalized_duplicate_text_goes_to_same_shard() {
        let dir = TempDir::new().unwrap();
        let src = dir.path().join("corpus.jsonl");
        let mut content = String::new();
        for text in ["ＡＩ技術", "AI技術", "日本語変換"] {
            content.push_str(
                &serde_json::to_string(&json!({"input": "エーアイ", "output": text})).unwrap(),
            );
            content.push('\n');
        }
        fs::write(&src, content).unwrap();
        let policy =
            json!({"reserved_byte_ranges": [], "min_text_length": 2, "max_text_length": 512});
        shard_corpus(&src, dir.path(), 3, &policy, 0).unwrap();

        let mut containing = Vec::new();
        for i in 0..3 {
            let p = dir.path().join(format!("input-{i:02}.jsonl"));
            let mut matches = Vec::new();
            for line in fs::read_to_string(p).unwrap().lines() {
                let row: (String, String, String) = serde_json::from_str(line).unwrap();
                if row.1 == "AI技術" {
                    matches.push(row);
                }
            }
            if !matches.is_empty() {
                containing.push(matches);
            }
        }
        assert_eq!(containing.len(), 1);
        assert_eq!(containing[0].len(), 2);
        assert_eq!(containing[0][0].2, containing[0][1].2);
    }

    #[test]
    fn merge_shard_counts_sums_counts_and_ors_buckets() {
        let dir = TempDir::new().unwrap();
        let mut s0 = ShardCounts::default();
        s0.phrases.insert(
            "日本語入力".into(),
            FreqEntry {
                occurrences: 7,
                documents: 5,
                buckets: 5,
            },
        );
        s0.phrases.insert(
            "少数句".into(),
            FreqEntry {
                occurrences: 1,
                documents: 1,
                buckets: 1,
            },
        );
        let mut s1 = ShardCounts::default();
        s1.phrases.insert(
            "日本語入力".into(),
            FreqEntry {
                occurrences: 9,
                documents: 4,
                buckets: 2,
            },
        );
        merge_shard_counts(dir.path(), vec![s0, s1], 5).unwrap();
        assert_eq!(
            fs::read_to_string(dir.path().join("phrase_frequency.tsv")).unwrap(),
            "日本語入力\t16\t9\t7\n"
        );
    }
}
