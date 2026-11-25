//! 単体運用・比較・回帰管理・セッション診断ツールの統合ランナー群。
//! (`lenient_score.py`, `adoption_check.py`, `compare_quality.py`, `compare_runs.py`,
//!  `quality_triage.py`, `make_same_mistake_set.py`, `regression_add.py`, `regression_matrix.py`,
//!  `check_startup_quality.py`, `perf_table.py`, `cost_bench.py`, `typing_test.py`, `idle_test.py`,
//!  `candidate_order.py`, `consistency_check.py`, `oracle_check.py`, `window_profile.py`,
//!  `commit_replay.py`, `qod_replay.py`, `learning_flow.py`, `imebench/imebench.py`,
//!  `imebench/to_rows.py`, `eval/tools/dump_cands.py`)

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use regex::Regex;
use serde_json::{json, Value};
use sudachi::analysis::stateless_tokenizer::StatelessTokenizer;
use sudachi::analysis::Tokenize;
use sudachi::dic::dictionary::JapaneseDictionary;
use sudachi::prelude::Mode;

use asteria_common::ai::default_models;
use asteria_common::hash::sha256_file;
use asteria_common::kana::{kata_to_hira, nfkc};
use asteria_common::paths::install_dir;
use asteria_common::romaji::imebench_to_romaji;
use asteria_corpus::frequency::load_sudachi_dict;

use crate::baseline::{run_session_eval, SessionEvalOpts};
use crate::quality::{compare_rows, load_result, validate_conditions};
use crate::session_client::Session;
use crate::textproto::parse_output;

/// `lenient_score.py`: Sudachi 正規化表記と読みによる表記ゆれ一致を判定する。
pub fn lenient_ok_with_tokenizer(
    tok: &StatelessTokenizer<&JapaneseDictionary>,
    top1: &str,
    expected: &[String],
) -> Result<bool> {
    if expected.iter().any(|e| e == top1) {
        return Ok(true);
    }
    let key = |s: &str| -> Result<(Vec<String>, String)> {
        let norm = nfkc(s);
        let ms = tok.tokenize(&norm, Mode::A, false)?;
        let mut forms = Vec::with_capacity(ms.len());
        let mut readings = String::new();
        for i in 0..ms.len() {
            let m = ms.get(i);
            forms.push(m.normalized_form().to_owned());
            readings.push_str(m.reading_form());
        }
        Ok((forms, readings))
    };
    let tk = key(top1)?;
    for e in expected {
        if key(e)? == tk {
            return Ok(true);
        }
    }
    Ok(false)
}

/// `lenient_score.py`: 結果 JSON 群の完全一致・表記ゆれ一致を集計する。
pub fn run_lenient_score(
    paths: &[&Path],
    sudachi_dict: &Path,
    show_misses: bool,
) -> Result<Vec<Value>> {
    let dict = load_sudachi_dict(sudachi_dict)?;
    let tok = StatelessTokenizer::new(&dict);
    let mut reports = Vec::new();
    for &p in paths {
        let rows: Vec<Value> = serde_json::from_str(&fs::read_to_string(p)?)?;
        let active: Vec<&Value> = rows
            .iter()
            .filter(|r| !r["skipped"].as_bool().unwrap_or(false))
            .collect();
        let strict = active
            .iter()
            .filter(|r| r["ok"].as_bool().unwrap_or(false))
            .count();
        let mut lenient = 0usize;
        for r in &active {
            let top1 = r["top1"].as_str().unwrap_or("");
            let exp: Vec<String> = r["expected"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(Value::as_str)
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default();
            let hit = lenient_ok_with_tokenizer(&tok, top1, &exp)?;
            if hit {
                lenient += 1;
            } else if show_misses {
                println!(
                    "   {top1} | {}",
                    exp.first().map(String::as_str).unwrap_or("")
                );
            }
        }
        println!(
            "{}: 完全一致 {strict}/{}  表記ゆれ一致 {lenient}/{}",
            p.display(),
            active.len(),
            active.len()
        );
        reports.push(json!({
            "path": p.to_string_lossy(),
            "total": active.len(),
            "strict": strict,
            "lenient": lenient,
        }));
    }
    Ok(reports)
}

/// `adoption_check.py`: 5 分野の加重平均と回帰制約を判定する。
pub fn run_adoption_check(
    base_dir: &Path,
    cand_dir: &Path,
    split: &str,
    sudachi_dict: &Path,
) -> Result<Value> {
    const WEIGHTS: &[(&str, f64)] = &[
        ("ctxpair", 0.30),
        ("short", 0.25),
        ("doc", 0.15),
        ("tech", 0.15),
        ("names", 0.15),
    ];
    let dict = load_sudachi_dict(sudachi_dict)?;
    let tok = StatelessTokenizer::new(&dict);
    let load_domain = |dir: &Path, dom: &str| -> Result<BTreeMap<String, bool>> {
        let rows: Vec<Value> = serde_json::from_str(&fs::read_to_string(
            dir.join(format!("{dom}-{split}.json")),
        )?)?;
        let mut m = BTreeMap::new();
        for r in rows {
            if r["skipped"].as_bool().unwrap_or(false) {
                continue;
            }
            let idx = r["index"]
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| r["index"].to_string());
            let top1 = r["top1"].as_str().unwrap_or("");
            let exp: Vec<String> = r["expected"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(Value::as_str)
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default();
            m.insert(idx, lenient_ok_with_tokenizer(&tok, top1, &exp)?);
        }
        Ok(m)
    };

    let (mut total_base, mut total_cand, mut worst_drop) = (0.0f64, 0.0f64, 0.0f64);
    let (mut improved, mut regressed) = (0usize, 0usize);
    let mut report = serde_json::Map::new();
    for &(domain, w) in WEIGHTS {
        let a = load_domain(base_dir, domain)?;
        let b = load_domain(cand_dir, domain)?;
        let keys: Vec<String> = a.keys().filter(|k| b.contains_key(*k)).cloned().collect();
        if keys.is_empty() {
            bail!("分野 {domain} の共通問題が空です");
        }
        let ra = 100.0 * keys.iter().filter(|k| a[*k]).count() as f64 / keys.len() as f64;
        let rb = 100.0 * keys.iter().filter(|k| b[*k]).count() as f64 / keys.len() as f64;
        let imp = keys.iter().filter(|k| !a[*k] && b[*k]).count();
        let reg = keys.iter().filter(|k| a[*k] && !b[*k]).count();
        improved += imp;
        regressed += reg;
        worst_drop = worst_drop.max(ra - rb);
        total_base += w * ra;
        total_cand += w * rb;
        report.insert(
            domain.into(),
            json!({
                "n": keys.len(),
                "base": (ra * 100.0).round() / 100.0,
                "cand": (rb * 100.0).round() / 100.0,
                "improved": imp,
                "regressed": reg,
            }),
        );
    }
    let ok1 = total_cand > total_base;
    let ok2 = improved >= 2 * regressed && improved > 0;
    let ok3 = worst_drop <= 1.0;
    Ok(json!({
        "split": split,
        "domains": report,
        "composite_base": (total_base * 100.0).round() / 100.0,
        "composite_cand": (total_cand * 100.0).round() / 100.0,
        "improved": improved,
        "regressed": regressed,
        "worst_domain_drop": (worst_drop * 100.0).round() / 100.0,
        "conditions": {
            "composite_up": ok1,
            "improved_at_least_2x_regressed": ok2,
            "no_domain_drop_over_1pt": ok3,
        },
        "adopt": ok1 && ok2 && ok3,
    }))
}

/// `compare_quality.py`: 2 つのオフライン評価結果と manifest を突き合わせて比較 JSON を書き出す。
pub fn run_compare_quality(before_path: &Path, after_path: &Path, out: &Path) -> Result<Value> {
    let (before, a) = load_result(before_path)?;
    let (after, b) = load_result(after_path)?;
    validate_conditions(&a, &b)?;
    let mut report = compare_rows(&before, &after)?;
    let summarize = |rows: &[Value]| -> Value {
        let mut stages: BTreeMap<String, usize> = BTreeMap::new();
        let mut failures: BTreeMap<String, usize> = BTreeMap::new();
        let mut domains: BTreeMap<String, (usize, usize)> = BTreeMap::new();
        for r in rows {
            let st = r["diagnostic_stage"]
                .as_str()
                .unwrap_or("unverified")
                .to_owned();
            *stages.entry(st).or_default() += 1;
            let dom = r["domain"].as_str().unwrap_or("").to_owned();
            let e = domains.entry(dom).or_insert((0, 0));
            e.0 += 1;
            if r["ok"].as_bool().unwrap_or(false) {
                e.1 += 1;
            } else {
                let trace = &r["scoring_trace"];
                let reason = if trace["status"].as_str() != Some("complete_logged_pool") {
                    "unverified"
                } else if !trace["expected_in_pool"].as_bool().unwrap_or(false) {
                    "expected_missing_from_pool"
                } else if !trace["expected_jointly_selectable"]
                    .as_bool()
                    .unwrap_or(false)
                {
                    "expected_excluded_from_joint_score"
                } else if trace["expected_has_max_logged_score"]
                    .as_bool()
                    .unwrap_or(false)
                {
                    "expected_near_logged_max_final_mismatch"
                } else {
                    "expected_scored_below_winner"
                };
                *failures.entry(reason.into()).or_default() += 1;
            }
        }
        let dom_json: BTreeMap<String, Value> = domains
            .into_iter()
            .map(|(k, (n, c))| (k, json!({"items": n, "correct": c})))
            .collect();
        json!({
            "items": rows.len(),
            "top1_correct": rows.iter().filter(|r| r["ok"].as_bool().unwrap_or(false)).count(),
            "observed_candidate_coverage": rows.iter().filter(|r| r["candidate_coverage"].as_bool().unwrap_or(false)).count(),
            "reachable_at_per_segment_k": rows.iter().filter(|r| r["reachable_at_per_segment_k"].as_bool().unwrap_or(false)).count(),
            "truncated_items": rows.iter().filter(|r| r["candidates_truncated"].as_bool().unwrap_or(false)).count(),
            "stages": stages,
            "scoring_failures": failures,
            "domains": dom_json,
        })
    };
    if let Some(obj) = report.as_object_mut() {
        obj.insert("before".into(), summarize(&before));
        obj.insert("after".into(), summarize(&after));
        obj.insert("before_manifest".into(), a);
        obj.insert("after_manifest".into(), b);
        obj.insert(
            "evidence_scope".into(),
            json!("offline_converter_not_installed_tsf"),
        );
    }
    fs::write(out, serde_json::to_string_pretty(&report)?)?;
    Ok(report)
}

/// `compare_runs.py`: 2 フォルダ間の同名 JSON 結果を 1 問ずつ比較して改善/回帰を集計する。
pub fn run_compare_runs(
    before_dir: &Path,
    after_dir: &Path,
    names: &[String],
    before_prefix: &str,
    after_prefix: &str,
) -> Result<usize> {
    let mut total_lost = 0usize;
    for name in names {
        let load = |dir: &Path, prefix: &str| -> Result<BTreeMap<String, Value>> {
            let rows: Vec<Value> = serde_json::from_str(&fs::read_to_string(
                dir.join(format!("{prefix}{name}.json")),
            )?)?;
            Ok(rows
                .into_iter()
                .map(|r| {
                    let idx = r["index"]
                        .as_str()
                        .map(str::to_owned)
                        .unwrap_or_else(|| r["index"].to_string());
                    (idx, r)
                })
                .collect())
        };
        let a = load(before_dir, before_prefix)?;
        let b = load(after_dir, after_prefix)?;
        let lost: Vec<&String> = a
            .keys()
            .filter(|k| {
                a[*k]["ok"].as_bool().unwrap_or(false)
                    && b.get(*k)
                        .is_some_and(|r| !r["ok"].as_bool().unwrap_or(false))
            })
            .collect();
        let won_count = a
            .keys()
            .filter(|k| {
                !a[*k]["ok"].as_bool().unwrap_or(false)
                    && !a[*k]["skipped"].as_bool().unwrap_or(false)
                    && b.get(*k)
                        .is_some_and(|r| r["ok"].as_bool().unwrap_or(false))
            })
            .count();
        let a_ok = a
            .values()
            .filter(|r| r["ok"].as_bool().unwrap_or(false))
            .count();
        let b_ok = b
            .values()
            .filter(|r| r["ok"].as_bool().unwrap_or(false))
            .count();
        total_lost += lost.len();
        println!(
            "{name}: {a_ok} → {b_ok}(外れた {}、正しくなった {won_count})",
            lost.len()
        );
        for k in lost {
            println!("  外れた {k}: {} → {}", a[k]["top1"], b[k]["top1"]);
        }
    }
    println!("回帰の合計: {total_lost}");
    Ok(total_lost)
}

/// `quality_triage.py`: 修正前後 + Mozc 基準結果の 3 者照合と失敗例抽出。
pub fn run_quality_triage(
    before_dir: &Path,
    after_dir: &Path,
    baseline_dir: &Path,
    out: &Path,
    require_ai: bool,
    repo_root: &Path,
) -> Result<Value> {
    const SETS: &[(&str, &str)] = &[
        ("daily", "eval/sets/asteria-daily.json"),
        ("ajimee", "eval/data/ajimee-bench.json"),
        ("idioms", "eval/sets/asteria-idioms.json"),
        ("nuance", "eval/sets/asteria-nuance.json"),
        ("notation", "eval/sets/asteria-notation.json"),
    ];
    let index = |rows: Vec<Value>| -> Result<BTreeMap<String, Value>> {
        let mut m = BTreeMap::new();
        for r in rows {
            let k = r["index"]
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| r["index"].to_string());
            if m.insert(k.clone(), r).is_some() {
                bail!("duplicate case index: {k}");
            }
        }
        Ok(m)
    };
    let mut results = serde_json::Map::new();
    for &(name, dataset) in SETS {
        let p_before = before_dir.join(format!("Standard_{name}.json"));
        let p_after = after_dir.join(format!("Standard_{name}.json"));
        let p_base = baseline_dir.join(format!("Mozc_{name}.json"));
        let manifests: Vec<Value> = [&p_before, &p_after, &p_base]
            .iter()
            .map(|p| {
                let mp = p.with_file_name(format!(
                    "{}.manifest.json",
                    p.file_name().and_then(|s| s.to_str()).unwrap_or("")
                ));
                Ok(serde_json::from_str(&fs::read_to_string(mp)?)?)
            })
            .collect::<Result<_>>()?;
        let id0 = (
            &manifests[0]["data"]["sha256"],
            &manifests[0]["data"]["items"],
            &manifests[0]["context"],
        );
        for m in &manifests[1..] {
            if (&m["data"]["sha256"], &m["data"]["items"], &m["context"]) != id0 {
                bail!("dataset/context differs: {name}");
            }
        }
        if require_ai {
            for p in [&p_before, &p_after] {
                let log = p.with_extension("stderr");
                if !log.is_file() || !fs::read_to_string(&log)?.contains("[time] chars=") {
                    bail!("AI scoring was not observed: {}", p.display());
                }
            }
        }
        let b = index(serde_json::from_str(&fs::read_to_string(&p_before)?)?)?;
        let a = index(serde_json::from_str(&fs::read_to_string(&p_after)?)?)?;
        let m = index(serde_json::from_str(&fs::read_to_string(&p_base)?)?)?;
        let mut failures = Vec::new();
        let mut improved = Vec::new();
        let mut regressed = Vec::new();
        let ds_path = repo_root.join(dataset);
        if Some(sha256_file(&ds_path)?.as_str()) != manifests[0]["data"]["sha256"].as_str() {
            bail!("source dataset changed: {name}");
        }
        let originals = index(serde_json::from_str(&fs::read_to_string(&ds_path)?)?)?;
        for (k, row) in &a {
            let br = b.get(k).context("missing before key")?;
            let mr = m.get(k).context("missing base key")?;
            if row["input"] != br["input"]
                || row["expected"] != br["expected"]
                || row["input"] != mr["input"]
            {
                bail!("reading or expected outputs differ: {k}");
            }
            let top = row["top1"].as_str().unwrap_or("");
            let btop = br["top1"].as_str().unwrap_or("");
            let mtop = mr["top1"].as_str().unwrap_or("");
            let exp: Vec<&str> = row["expected"]
                .as_array()
                .map(|arr| arr.iter().filter_map(Value::as_str).collect())
                .unwrap_or_default();
            let ok = exp.contains(&top);
            let old_ok = exp.contains(&btop);
            if ok && !old_ok {
                improved.push(row["index"].clone());
            } else if old_ok && !ok {
                regressed.push(row["index"].clone());
            }
            if !ok {
                let mut f = row.clone();
                if let Some(obj) = f.as_object_mut() {
                    obj.insert("ok".into(), json!(false));
                    obj.insert("before".into(), json!(btop));
                    obj.insert("mozc".into(), json!(mtop));
                    obj.insert("mozc_was_correct".into(), json!(exp.contains(&mtop)));
                    obj.insert(
                        "context_text".into(),
                        json!(originals
                            .get(k)
                            .and_then(|o| o["context_text"].as_str())
                            .unwrap_or("")),
                    );
                    obj.insert("source_dataset".into(), json!(dataset));
                }
                failures.push(f);
            }
        }
        let items_n = a.len();
        results.insert(
            name.into(),
            json!({
                "items": items_n,
                "correct": items_n - failures.len(),
                "improved": improved,
                "regressed": regressed,
                "failures": failures,
            }),
        );
    }
    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent)?;
    }
    let val = Value::Object(results);
    fs::write(out, format!("{}\n", serde_json::to_string_pretty(&val)?))?;
    Ok(val)
}

/// `make_same_mistake_set.py`: 誤選択事例から学習再現シナリオ (`learning_flow` 用) を作成する。
pub fn run_make_same_mistake_set(
    set_path: &Path,
    results_path: &Path,
    out: &Path,
) -> Result<usize> {
    let items: Vec<Value> = serde_json::from_str(&fs::read_to_string(set_path)?)?;
    let mut by_idx = BTreeMap::new();
    let mut by_pair: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    for x in items {
        let idx = x["index"].as_str().unwrap_or("").to_owned();
        if let Some(pair) = x["pair"].as_str() {
            by_pair.entry(pair.to_owned()).or_default().push(x.clone());
        }
        by_idx.insert(idx, x);
    }
    let rows: Vec<Value> = serde_json::from_str(&fs::read_to_string(results_path)?)?;
    let mut scenes = Vec::new();
    for r in rows {
        let idx = r["index"].as_str().unwrap_or("");
        let Some(it) = by_idx.get(idx) else {
            continue;
        };
        let target = it["target"].as_str().unwrap_or("");
        if r["ok"].as_bool().unwrap_or(false) || target.is_empty() {
            continue;
        }
        let top = r["top1"].as_str().unwrap_or("");
        let rival = it["rival"].as_array().and_then(|arr| {
            arr.iter()
                .filter_map(Value::as_str)
                .find(|v| top.contains(*v))
        });
        let Some(rival) = rival else {
            continue;
        };
        if top.contains(target) || top.matches(rival).count() != 1 {
            continue;
        }
        let input = it["input"].as_str().unwrap_or("");
        let mut check = vec![json!({"kind": "same_cue", "input": input, "expect": target})];
        if let Some(pair) = it["pair"].as_str() {
            if let Some(other) = by_pair
                .get(pair)
                .and_then(|arr| arr.iter().find(|x| x["index"].as_str() != Some(idx)))
            {
                check.push(json!({"kind": "no_cue", "input": other["input"]}));
            }
        }
        scenes.push(json!({
            "name": format!("{idx} {rival}→{target}"),
            "teach": {"input": input, "rejected": rival, "chosen": target},
            "check": check,
        }));
    }
    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(out, serde_json::to_string_pretty(&scenes)?)?;
    Ok(scenes.len())
}

/// `regression_add.py`: 回帰ゴールデン (`asteria-regression-golden.json`) の検証・一覧・追記。
pub fn run_regression_add(
    file: &Path,
    input: Option<&str>,
    expected: &[String],
    context: &str,
    check_only: bool,
    list_only: bool,
    dry_run: bool,
) -> Result<i32> {
    let load_and_validate = |p: &Path| -> Result<Vec<Value>> {
        let items: Vec<Value> = serde_json::from_str(&fs::read_to_string(p)?)?;
        if items.is_empty() {
            bail!("評価セットは空でない配列が必要です");
        }
        let mut seen_idx = HashSet::new();
        let mut seen_pairs = HashSet::new();
        for item in &items {
            let idx = item["index"].to_string();
            if !seen_idx.insert(idx.clone()) {
                bail!("問題IDが重複しています: {idx}");
            }
            let inp = item["input"].as_str().unwrap_or("").trim();
            let ctx = item["context_text"].as_str().unwrap_or("");
            if inp.is_empty()
                || inp
                    .chars()
                    .chain(ctx.chars())
                    .any(|c| matches!(c, '\r' | '\n' | '\t' | '\0'))
            {
                bail!("読み/前文が不正です: {idx}");
            }
            if !item["expected_output"].as_array().is_some_and(|a| {
                !a.is_empty() && a.iter().all(|v| v.as_str().is_some_and(|s| !s.is_empty()))
            }) {
                bail!("空でない正解表記集合が必要です: {idx}");
            }
            if !seen_pairs.insert((kata_to_hira(inp), ctx.to_owned())) {
                bail!("同じ読み・同じ前の文が重複しています: {idx}");
            }
        }
        Ok(items)
    };

    if check_only || list_only {
        let items = load_and_validate(file)?;
        if check_only {
            println!("OK: {} 問  {}", items.len(), file.display());
        } else {
            for item in &items {
                let ctx = item["context_text"].as_str().unwrap_or("");
                let exp: Vec<&str> = item["expected_output"]
                    .as_array()
                    .map(|a| a.iter().filter_map(Value::as_str).collect())
                    .unwrap_or_default();
                let ctx_note = if ctx.is_empty() {
                    String::new()
                } else {
                    format!("  [前: {ctx}]")
                };
                println!(
                    "{}  {}{ctx_note}  =>  {}",
                    item["index"].as_str().unwrap_or(""),
                    item["input"].as_str().unwrap_or(""),
                    exp.join(" / ")
                );
            }
        }
        return Ok(0);
    }

    let Some(raw_inp) = input.map(str::trim).filter(|s| !s.is_empty()) else {
        bail!("追加には --input と --expected が必要です");
    };
    if expected.is_empty() || expected.iter().any(|e| e.trim().is_empty()) {
        bail!("空でない正解を1つ以上指定してください");
    }
    if raw_inp
        .chars()
        .chain(context.chars())
        .any(|c| matches!(c, '\r' | '\n' | '\t' | '\0'))
    {
        bail!("読みと前の文に改行・タブ・NUL は使えません");
    }
    let mut items = if file.is_file() {
        load_and_validate(file)?
    } else {
        Vec::new()
    };
    let pair = (kata_to_hira(raw_inp), context.to_owned());
    if items.iter().any(|it| {
        (
            kata_to_hira(it["input"].as_str().unwrap_or("").trim()),
            it["context_text"].as_str().unwrap_or("").to_owned(),
        ) == pair
    }) {
        eprintln!("重複: 同じ読み・同じ前の文が既にあります(重複)");
        return Ok(3);
    }
    let idx_re = Regex::new(r"^rg-(\d+)$")?;
    let next_n = items
        .iter()
        .filter_map(|it| {
            it["index"]
                .as_str()
                .and_then(|s| idx_re.captures(s))
                .and_then(|c| c.get(1)?.as_str().parse::<usize>().ok())
        })
        .max()
        .unwrap_or(0)
        + 1;
    let exp_clean: Vec<String> = expected.iter().map(|s| s.trim().to_owned()).collect();
    items.push(json!({
        "index": format!("rg-{next_n:04}"),
        "context_text": context,
        "input": raw_inp,
        "expected_output": exp_clean,
    }));
    if dry_run {
        println!(
            "追加します(まだ書いていません): {}",
            items.last().unwrap_or(&Value::Null)
        );
        return Ok(0);
    }
    if let Some(parent) = file.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = file.with_extension("json.tmp");
    fs::write(&tmp, format!("{}\n", serde_json::to_string_pretty(&items)?))?;
    fs::rename(&tmp, file)?;
    println!("追加しました: rg-{next_n:04}");
    Ok(0)
}

/// `eval/imebench/imebench.py`: `ImeBench.exe` を呼び出して実 IME の変換結果を測定する。
pub fn run_imebench(
    ime: &str,
    data_paths: &[&Path],
    limit: Option<usize>,
    wait_ms: u64,
    label: &str,
    repo_root: &Path,
) -> Result<()> {
    let (clsid, profile) = match ime {
        "google" => (
            "{D5A86FD5-5308-47EA-AD16-9C4EB160EC3C}",
            "{773EB24E-CA1D-4B1B-B420-FA985BB0B80D}",
        ),
        "msime" => (
            "{03B5835F-F03C-411B-9CE2-AA23E1171E36}",
            "{A76C93D9-5523-4E90-AAFA-4DB112F9AC76}",
        ),
        "asteria" => (
            "{2A1ADAE4-8061-4AC0-A7D4-713BF8DCB591}",
            "{455A87E7-275F-4582-92CB-6600917A436D}",
        ),
        other => bail!("未知の IME: {other}"),
    };
    let here = repo_root.join("eval/imebench");
    let exe = here.join("ImeBench.exe");
    let out_dir = here.join("out");
    fs::create_dir_all(&out_dir)?;

    for &data in data_paths {
        let mut items: Vec<Value> = serde_json::from_str(&fs::read_to_string(data)?)?;
        if let Some(n) = limit {
            items.truncate(n);
        }
        let stem = data.file_stem().and_then(|s| s.to_str()).unwrap_or("data");
        let who = if label.is_empty() {
            ime.to_owned()
        } else {
            format!("{ime}-{label}")
        };
        let in_tsv = out_dir.join(format!("{who}_{stem}.in.tsv"));
        let res_tsv = out_dir.join(format!("{who}_{stem}.tsv"));
        let mut body = String::from("w0\tkyouhaiitennkidesune\nw1\tyoroshikuonegaishimasu\n");
        let mut rows: Vec<(String, Value)> = Vec::new();
        let mut skipped = 0usize;
        for (i, it) in items.into_iter().enumerate() {
            let inp = it["input"].as_str().unwrap_or("");
            if let Some(rom) = imebench_to_romaji(inp) {
                body.push_str(&format!("{i}\t{rom}\n"));
                rows.push((i.to_string(), it));
            } else {
                skipped += 1;
            }
        }
        fs::write(&in_tsv, body.as_bytes())?;
        let status = Command::new(&exe)
            .args([
                clsid,
                profile,
                &in_tsv.to_string_lossy(),
                &res_tsv.to_string_lossy(),
                &wait_ms.to_string(),
            ])
            .status()?;
        let err_p = PathBuf::from(format!("{}.error.txt", res_tsv.display()));
        if err_p.is_file() {
            bail!("{}", fs::read_to_string(err_p)?);
        }
        let _ = status;
        let mut got: HashMap<String, String> = HashMap::new();
        for line in fs::read_to_string(&res_tsv)?.lines() {
            if let Some((k, v)) = line.split_once('\t') {
                got.insert(k.to_owned(), v.to_owned());
            }
        }
        let ok = rows
            .iter()
            .filter(|(id, it)| {
                let g = got
                    .get(id)
                    .map(|s| nfkc(s).replace(' ', ""))
                    .unwrap_or_default();
                it["expected_output"].as_array().is_some_and(|arr| {
                    arr.iter()
                        .filter_map(Value::as_str)
                        .any(|e| nfkc(e).replace(' ', "") == g)
                })
            })
            .count();
        println!(
            "{ime} {stem}: {ok}/{} = {:.1}%(打てない文字 {skipped})",
            rows.len(),
            100.0 * ok as f64 / rows.len().max(1) as f64
        );
    }
    Ok(())
}

/// `eval/imebench/to_rows.py`: `imebench` TSV を `top1`/`expected`/`ok` JSON 配列に変換する。
pub fn run_imebench_to_rows(data_path: &Path, result_tsv: &Path, out_path: &Path) -> Result<usize> {
    let items: Vec<Value> = serde_json::from_str(&fs::read_to_string(data_path)?)?;
    let mut got = HashMap::new();
    for line in fs::read_to_string(result_tsv)?.lines() {
        if let Some((k, v)) = line.split_once('\t') {
            got.insert(k.to_owned(), v.to_owned());
        }
    }
    let mut rows = Vec::with_capacity(items.len());
    for (i, it) in items.iter().enumerate() {
        let key = i.to_string();
        let Some(top1) = got.get(&key) else {
            rows.push(json!({"index": it["index"], "skipped": true}));
            continue;
        };
        let norm_top = nfkc(top1);
        let ok = it["expected_output"].as_array().is_some_and(|arr| {
            arr.iter()
                .filter_map(Value::as_str)
                .any(|e| nfkc(e) == norm_top)
        });
        rows.push(json!({
            "index": it["index"],
            "input": it["input"],
            "top1": top1,
            "expected": it["expected_output"],
            "ok": ok,
        }));
    }
    fs::write(out_path, serde_json::to_string_pretty(&rows)?)?;
    Ok(rows.len())
}

/// `eval/tools/dump_cands.py`: `converter_main` から文節ごとの候補を JSON 出力する。
pub fn run_dump_cands(exe: &Path, data: &Path, out: &Path, k: usize) -> Result<usize> {
    let items: Vec<Value> = serde_json::from_str(&fs::read_to_string(data)?)?;
    let cwd = exe.parent().context("exe parent")?.join(format!(
        "{}.runfiles/_main",
        exe.file_name().and_then(|n| n.to_str()).unwrap_or("")
    ));
    let seg_re = Regex::new(r"^-{10} Segment \d+/\d+ \[.*\] -{10}$")?;
    let cand_re = Regex::new(r"^\s+(\d+)/\d+ (.*)$")?;
    let mut res = Vec::with_capacity(items.len());
    for it in &items {
        let reading = kata_to_hira(it["input"].as_str().unwrap_or(""));
        let mut cmd = Command::new(exe);
        cmd.current_dir(&cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let mut child = cmd.spawn()?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(format!("start {reading}\nquit\n").as_bytes())?;
        }
        let output = child.wait_with_output()?;
        let text = String::from_utf8_lossy(&output.stdout);
        let lines: Vec<&str> = text.lines().collect();
        let mut segs: Vec<Value> = Vec::new();
        for (i, &line) in lines.iter().enumerate() {
            if seg_re.is_match(line) {
                let key = lines.get(i + 1).copied().unwrap_or("").trim().to_owned();
                segs.push(json!({"key": key, "cands": []}));
            } else if let Some(caps) = cand_re.captures(line) {
                let rank: usize = caps
                    .get(1)
                    .and_then(|m| m.as_str().parse().ok())
                    .unwrap_or(usize::MAX);
                let val = caps.get(2).map(|m| m.as_str()).unwrap_or("");
                if rank < k {
                    if let Some(cur) = segs.last_mut().and_then(|s| s["cands"].as_array_mut()) {
                        if !cur.iter().any(|c| c.as_str() == Some(val)) {
                            cur.push(json!(val));
                        }
                    }
                }
            }
        }
        res.push(json!({
            "index": it["index"],
            "context": it["context_text"].as_str().unwrap_or(""),
            "segments": segs,
            "expected": it["expected_output"],
        }));
    }
    fs::write(out, serde_json::to_string(&res)?)?;
    Ok(res.len())
}

/// `check_startup_quality.py`: 隔離 `converter_main` で AI 準備前 (`immediate`) と準備後 (`ready`) を比較する。
pub fn run_check_startup_quality(
    converter: &Path,
    model_dir: &Path,
    runtime_dir: &Path,
    out: &Path,
    reading: &str,
    expected: &str,
    ready_timeout_secs: f64,
) -> Result<Value> {
    if reading.is_empty() || reading.chars().any(|c| (c as u32) < 32 || c as u32 == 127) {
        bail!("reading must not contain control characters");
    }
    let exe = fs::canonicalize(converter)?;
    fs::create_dir_all(out)?;
    let profile = tempfile::Builder::new()
        .prefix("asteria-startup-quality-")
        .tempdir()?;
    let cwd = exe.parent().context("exe parent")?.join(format!(
        "{}.runfiles/_main",
        exe.file_name().and_then(|n| n.to_str()).unwrap_or("")
    ));
    let mut cmd = Command::new(&exe);
    cmd.arg("--evaluation_trace=true")
        .arg(format!("--user_profile_dir={}", profile.path().display()))
        .current_dir(&cwd)
        .env_clear();
    for (k, v) in std::env::vars() {
        if !k.starts_with("ASTERIA_") {
            cmd.env(k, v);
        }
    }
    cmd.env("ASTERIA_MODEL_DIR", fs::canonicalize(model_dir)?)
        .env("ASTERIA_RUNTIME_DIR", fs::canonicalize(runtime_dir)?)
        .env("ASTERIA_LM_TIME", "1")
        .env("ASTERIA_LM_DEBUG", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn()?;
    let stdout = child.stdout.take().context("stdout")?;
    let (tx, rx) = mpsc::channel::<Option<String>>();
    thread::spawn(move || {
        let mut r = BufReader::new(stdout);
        let mut line = String::new();
        while r.read_line(&mut line).unwrap_or(0) > 0 {
            let _ = tx.send(Some(std::mem::take(&mut line)));
        }
        let _ = tx.send(None);
    });
    let mut stdin = child.stdin.take().context("stdin")?;
    let status_file = profile.path().join("asteria_ai_status.txt");
    let state_text = || fs::read_to_string(&status_file).unwrap_or_default();

    let mut convert = |label: &str| -> Result<Value> {
        let before = state_text();
        let started = Instant::now();
        stdin.write_all(format!("start {reading}\nreset\nasteria_sep\n").as_bytes())?;
        stdin.flush()?;
        let mut raw = String::new();
        loop {
            let elapsed = started.elapsed().as_secs_f64();
            if elapsed > 30.0 {
                bail!("converter timeout");
            }
            let Some(line) = rx.recv_timeout(Duration::from_secs_f64((30.0 - elapsed).max(0.1)))?
            else {
                bail!("converter exited");
            };
            raw.push_str(&line);
            if line.starts_with("ExecCommand() return false") {
                break;
            }
        }
        fs::write(out.join(format!("{label}.stdout")), raw.as_bytes())?;
        let mut top = String::new();
        for l in raw.lines() {
            let l = l.trim_end_matches('\r');
            if l.starts_with("  0/") {
                if let Some((_, v)) = l.trim().split_once(' ') {
                    top.push_str(v);
                }
            }
        }
        Ok(json!({
            "label": label,
            "status_before": before,
            "top1": top,
            "ok": top == expected,
            "elapsed_ms": started.elapsed().as_secs_f64() * 1000.0,
        }))
    };

    let first = convert("immediate")?;
    let deadline = Instant::now() + Duration::from_secs_f64(ready_timeout_secs);
    loop {
        let st = state_text();
        if st.lines().any(|l| l == "state=llm" || l == "state=zenz") {
            break;
        }
        if st.contains("state=error") || st.contains("state=off") {
            bail!("AI readiness failed: {st}");
        }
        if Instant::now() > deadline {
            bail!("AI readiness timeout: {st}");
        }
        thread::sleep(Duration::from_millis(50));
    }
    let warm = convert("ready")?;
    drop(stdin);
    let _ = child.wait()?;
    let report = json!({"immediate": first, "ready": warm, "expected": expected});
    fs::write(
        out.join("result.json"),
        serde_json::to_string_pretty(&report)?,
    )?;
    Ok(report)
}

/// `regression_matrix.py`: 複数データセット × 3 タイミングで `session_eval` と `compare_runs` を順次実行する。
pub fn run_regression_matrix(
    exe: &Path,
    out: &Path,
    before: &Path,
    ajimee: Option<&Path>,
    only: Option<&[String]>,
    extra_env: &[(String, String)],
    repo_root: &Path,
) -> Result<()> {
    const TIMINGS: &[(&str, bool, u64)] = &[
        ("default", false, 200),
        ("immediate", true, 200),
        ("gap150", false, 150),
    ];
    const SETS: &[&str] = &[
        "asteria-typo",
        "asteria-typo-keys-20261026",
        "asteria-daily",
        "asteria-short-dev-20261022",
        "asteria-homophone",
    ];
    fs::create_dir_all(out)?;
    let mut env = extra_env.to_vec();
    env.push(("ASTERIA_LM_DEBUG".into(), "1".into()));
    let mut names = Vec::new();
    let mut specs: Vec<(String, PathBuf, bool, u64)> = Vec::new();
    for s in SETS {
        let data = repo_root.join("eval/sets").join(format!("{s}.json"));
        for &(t, imm, gap) in TIMINGS {
            specs.push((format!("{s}-{t}"), data.clone(), imm, gap));
        }
    }
    specs.push((
        "user-reports".into(),
        repo_root.join("eval/sets/asteria-user-reports-20261027.json"),
        false,
        200,
    ));
    if let Some(a) = ajimee {
        specs.push(("ajimee-default".into(), a.to_path_buf(), false, 200));
    }

    for (name, data, imm, gap) in specs {
        if let Some(wanted) = only {
            if !wanted.iter().any(|w| w == &name) {
                continue;
            }
        }
        names.push(name.clone());
        let opts = SessionEvalOpts {
            exe: exe.to_path_buf(),
            data,
            out: out.join(format!("{name}.json")),
            models: default_models(repo_root),
            context: true,
            ai_state: "llm".into(),
            warm: 15.0,
            limit: 0,
            config: Vec::new(),
            immediate_space: imm,
            space_gap_ms: Some(gap as f64),
            live: false,
            timeline: true,
            extra_env: env.clone(),
        };
        run_session_eval(&opts, repo_root)?;
    }
    run_compare_runs(before, out, &names, "", "")?;
    Ok(())
}

/// `commit_replay.py`: `Session` でキー間隔・Space 後待ちを再現して Enter 確定文字列を集計する。
#[allow(clippy::too_many_arguments)]
pub fn run_commit_replay(
    exe: &Path,
    data: &Path,
    out: &Path,
    models: Option<&Path>,
    limit: usize,
    key_ms: f64,
    pause_ms: f64,
    after_space_ms: f64,
    repo_root: &Path,
) -> Result<Value> {
    let model_dir = models
        .map(Path::to_path_buf)
        .or_else(|| default_models(repo_root))
        .unwrap_or_else(install_dir);
    let items: Vec<Value> = serde_json::from_str(&fs::read_to_string(data)?)?;
    let profile = tempfile::Builder::new()
        .prefix("asteria-commit-replay-")
        .tempdir()?;
    let env = vec![
        (
            "ASTERIA_MODEL_DIR".into(),
            model_dir.to_string_lossy().into_owned(),
        ),
        (
            "ASTERIA_RUNTIME_DIR".into(),
            model_dir.to_string_lossy().into_owned(),
        ),
    ];
    let mut session = Session::spawn(
        exe,
        &env,
        profile.path(),
        &profile.path().join("err.txt"),
        false,
    )?;
    session.send("SEND_KEY\tON\nSEND_KEYS\tai")?;
    thread::sleep(Duration::from_secs(15));
    session.send("SEND_KEY\tESC")?;

    let mut rows = Vec::new();
    for it in items.into_iter().take(limit) {
        let Some(romaji) = imebench_to_romaji(it["input"].as_str().unwrap_or("")) else {
            continue;
        };
        let mut last = String::new();
        for ch in romaji.chars() {
            last = session.send(&format!("SEND_KEY\t{ch}"))?.output;
            thread::sleep(Duration::from_secs_f64(key_ms / 1000.0));
        }
        let _ = last;
        thread::sleep(Duration::from_secs_f64(pause_ms / 1000.0));
        let mut raw = session.send("SEND_KEY\tSpace")?.output;
        let deadline = Instant::now() + Duration::from_secs_f64(after_space_ms / 1000.0);
        while Instant::now() < deadline {
            if let Ok(p) = parse_output(&raw) {
                if let Some(d) = p.callback_ms {
                    thread::sleep(Duration::from_millis(d));
                    raw = session.send("ASTERIA_REFRESH")?.output;
                    continue;
                }
            }
            thread::sleep(Duration::from_millis(50));
        }
        let enter_raw = session.send("SEND_KEY\tEnter")?.output;
        let committed = parse_output(&enter_raw)
            .map(|p| p.result)
            .unwrap_or_default();
        rows.push(json!({"index": it["index"], "expected": it["expected_output"], "committed": committed}));
        session.send("SEND_KEY\tESC")?;
    }
    session.close(false)?;
    let val = Value::Array(rows);
    fs::write(out, serde_json::to_string_pretty(&val)?)?;
    Ok(val)
}

/// `qod_replay.py`: `query_of_death.log` を `Session` へ再生してクラッシュしないことを確かめる。
pub fn run_qod_replay(
    exe: &Path,
    models: &Path,
    log_path: &Path,
    configs: &[(String, String)],
) -> Result<usize> {
    let raw = fs::read_to_string(log_path)?;
    let ty_re = Regex::new(r"(?m)^type: (\w+)")?;
    let kc_re = Regex::new(r"key_code: (\d+)")?;
    let sk_re = Regex::new(r"special_key: (\w+)")?;
    let mod_re = Regex::new(r"modifier_keys: (\w+)")?;
    let mut steps: Vec<(&'static str, String)> = Vec::new();
    for b in raw.split("goo.gle/debugproto").skip(1) {
        let Some(ty) = ty_re.captures(b).and_then(|c| c.get(1)).map(|m| m.as_str()) else {
            continue;
        };
        if ty == "SEND_KEY" {
            let mut parts: Vec<String> = mod_re
                .captures_iter(b)
                .filter_map(|c| c.get(1).map(|m| m.as_str().to_lowercase()))
                .collect();
            if let Some(kc) = kc_re
                .captures(b)
                .and_then(|c| c.get(1)?.as_str().parse::<u32>().ok())
                .and_then(char::from_u32)
            {
                parts.push(kc.to_string());
            } else if let Some(sk) = sk_re.captures(b).and_then(|c| c.get(1)) {
                parts.push(sk.as_str().to_lowercase());
            }
            steps.push(("key", parts.join(" ")));
        } else if b.contains("ASTERIA_REFRESH_SUGGESTION") {
            steps.push(("refresh", String::new()));
        }
    }
    let profile = tempfile::Builder::new().prefix("qod-").tempdir()?;
    let env = vec![
        (
            "ASTERIA_MODEL_DIR".into(),
            models.to_string_lossy().into_owned(),
        ),
        (
            "ASTERIA_RUNTIME_DIR".into(),
            models.to_string_lossy().into_owned(),
        ),
    ];
    let mut session = Session::spawn(
        exe,
        &env,
        profile.path(),
        &profile.path().join("err.txt"),
        false,
    )?;
    for (n, v) in configs {
        session.send(&format!("SET_CONFIG\t{n}\t{v}"))?;
    }
    session.send("SEND_KEY\tON")?;
    thread::sleep(Duration::from_secs(15));
    for (kind, arg) in &steps {
        if *kind == "key" {
            session.send(&format!("SEND_KEY\t{arg}"))?;
            thread::sleep(Duration::from_millis(120));
        } else {
            session.send("ASTERIA_REFRESH")?;
            thread::sleep(Duration::from_millis(30));
        }
    }
    thread::sleep(Duration::from_secs(1));
    session.send("ASTERIA_REFRESH")?;
    session.close(false)?;
    Ok(steps.len())
}

/// `window_profile.py`: 入力中の候補窓が出現するまでの遅延と一致率を計測する。
pub fn run_window_profile(
    exe: &Path,
    models: &Path,
    data: &Path,
    out: &Path,
    configs: &[(String, String)],
) -> Result<Value> {
    let items: Vec<Value> = serde_json::from_str(&fs::read_to_string(data)?)?;
    let profile = tempfile::Builder::new().prefix("winprof-").tempdir()?;
    let env = vec![
        (
            "ASTERIA_MODEL_DIR".into(),
            models.to_string_lossy().into_owned(),
        ),
        (
            "ASTERIA_RUNTIME_DIR".into(),
            models.to_string_lossy().into_owned(),
        ),
    ];
    let mut session = Session::spawn(
        exe,
        &env,
        profile.path(),
        &profile.path().join("err.txt"),
        false,
    )?;
    for (k, v) in configs {
        session.send(&format!("SET_CONFIG\t{k}\t{v}"))?;
    }
    session.send("SEND_KEY\tON\nSEND_KEYS\tai")?;
    thread::sleep(Duration::from_secs(15));
    session.send("SEND_KEY\tESC")?;

    let mut rows = Vec::new();
    for it in &items {
        let Some(romaji) = it["romaji"]
            .as_str()
            .map(str::to_owned)
            .or_else(|| imebench_to_romaji(it["input"].as_str().unwrap_or("")))
        else {
            continue;
        };
        let mut last = String::new();
        for ch in romaji.chars() {
            last = session.send(&format!("SEND_KEY\t{ch}"))?.output;
            thread::sleep(Duration::from_millis(40));
        }
        let typed = Instant::now();
        let mut first_ms: Option<f64> = None;
        let mut shown: Vec<String> = Vec::new();
        while typed.elapsed() < Duration::from_secs(3) {
            let parsed = parse_output(&last)?;
            let Some(d) = parsed.callback_ms else {
                break;
            };
            thread::sleep(Duration::from_millis(d));
            last = session.send("ASTERIA_REFRESH")?.output;
            let p2 = parse_output(&last)?;
            if first_ms.is_none() && !p2.candidate_window.is_empty() {
                first_ms = Some(typed.elapsed().as_secs_f64() * 1000.0);
                shown = p2.candidate_window.into_iter().map(|c| c.value).collect();
            }
        }
        let mut sp = session.send("SEND_KEY\tSpace")?.output;
        while let Some(d) = parse_output(&sp)?.callback_ms {
            thread::sleep(Duration::from_millis(d));
            sp = session.send("ASTERIA_REFRESH")?.output;
        }
        let space: String = parse_output(&sp)?
            .segments
            .into_iter()
            .map(|s| s.value)
            .collect();
        let exp: Vec<&str> = it["expected_output"]
            .as_array()
            .map(|a| a.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();
        rows.push(json!({
            "index": it["index"],
            "input": it["input"],
            "window_ms": first_ms,
            "n": shown.len(),
            "top": shown.first(),
            "shown": shown.iter().take(9).collect::<Vec<_>>(),
            "space": space,
            "top_is_space": shown.first().is_some_and(|t| t == &space),
            "space_ok": exp.contains(&space.as_str()),
            "in_window_ok": shown.iter().any(|s| exp.contains(&s.as_str())),
        }));
        session.send("SEND_KEY\tESC")?;
        session.send("SEND_KEY\tESC")?;
    }
    session.close(false)?;
    let val = Value::Array(rows);
    fs::write(out, serde_json::to_string_pretty(&val)?)?;
    Ok(val)
}

#[cfg(test)]
mod tests {
    use super::*;
    use asteria_common::hash::sha256_bytes;
    use tempfile::TempDir;

    #[test]
    fn regression_add_validates_duplicates_and_appends() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("golden.json");
        assert_eq!(
            run_regression_add(&file, Some("はし"), &["橋".into()], "", false, false, false)
                .unwrap(),
            0
        );
        assert_eq!(
            run_regression_add(&file, Some("ハシ"), &["箸".into()], "", false, false, false)
                .unwrap(),
            3
        );
        assert_eq!(
            run_regression_add(&file, None, &[], "", true, false, false).unwrap(),
            0
        );
        let rows: Vec<Value> = serde_json::from_str(&fs::read_to_string(&file).unwrap()).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["index"], "rg-0001");
    }

    #[test]
    fn compare_runs_and_same_mistake_set_work() {
        let dir = TempDir::new().unwrap();
        let before = dir.path().join("before");
        let after = dir.path().join("after");
        fs::create_dir_all(&before).unwrap();
        fs::create_dir_all(&after).unwrap();
        let b_rows = json!([
            {"index": "c1", "top1": "誤", "ok": false},
            {"index": "c2", "top1": "正", "ok": true},
        ]);
        let a_rows = json!([
            {"index": "c1", "top1": "正", "ok": true},
            {"index": "c2", "top1": "誤", "ok": false},
        ]);
        fs::write(before.join("s.json"), b_rows.to_string()).unwrap();
        fs::write(after.join("s.json"), a_rows.to_string()).unwrap();
        assert_eq!(
            run_compare_runs(&before, &after, &["s".into()], "", "").unwrap(),
            1
        );

        let set_p = dir.path().join("set.json");
        let out_p = dir.path().join("scenes.json");
        fs::write(
            &set_p,
            json!([
                {"index": "c1", "input": "はし", "target": "箸", "rival": ["橋"], "pair": "p1"},
                {"index": "c2", "input": "はし", "target": "橋", "rival": ["箸"], "pair": "p1"},
            ])
            .to_string(),
        )
        .unwrap();
        fs::write(
            before.join("res.json"),
            json!([{"index": "c1", "top1": "橋", "ok": false}]).to_string(),
        )
        .unwrap();
        let n = run_make_same_mistake_set(&set_p, &before.join("res.json"), &out_p).unwrap();
        assert_eq!(n, 1);
    }

    #[test]
    fn compare_quality_end_to_end() {
        let dir = TempDir::new().unwrap();
        let before_p = dir.path().join("b.json");
        let after_p = dir.path().join("a.json");
        let rows_b = json!([{"index": 1, "input": "a", "expected": ["B"], "top1": "A", "ok": false, "context_text": "", "domain": "d", "diagnostic_stage": "unverified"}]).to_string();
        let rows_a = json!([{"index": 1, "input": "a", "expected": ["B"], "top1": "B", "ok": true, "context_text": "", "domain": "d", "diagnostic_stage": "unverified"}]).to_string();
        fs::write(&before_p, rows_b.as_bytes()).unwrap();
        fs::write(&after_p, rows_a.as_bytes()).unwrap();
        let mk_man = |raw: &str| {
            json!({
                "result_sha256": sha256_bytes(raw.as_bytes()),
                "data": {"sha256": "d1", "items": 1},
                "dictionary": {"sha256": "dict1"},
                "context": false,
                "case_isolated": true,
                "profile_policy": "temp",
            })
            .to_string()
        };
        fs::write(dir.path().join("b.json.manifest.json"), mk_man(&rows_b)).unwrap();
        fs::write(dir.path().join("a.json.manifest.json"), mk_man(&rows_a)).unwrap();
        let out_p = dir.path().join("cmp.json");
        let cmp = run_compare_quality(&before_p, &after_p, &out_p).unwrap();
        assert_eq!(cmp["improved"], 1);
        assert_eq!(cmp["regressed"], 0);
    }
}
