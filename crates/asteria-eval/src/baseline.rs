//! `eval_baseline`, `eval_predict`, `session_eval`, `eval_all`, `eval_regression`, `imebench` 移植。

use std::collections::{BTreeMap, HashMap, HashSet};
use std::env;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use regex::Regex;
use serde_json::{json, Value};

use asteria_common::ai::{check_models, wait_ai};
use asteria_common::hash::{sha256_bytes, sha256_file};
use asteria_common::kana::{kata_to_hira, nfkc};
use asteria_common::romaji::imebench_to_romaji;

use crate::quality::{score_case, scoring_trace, split_scoring_requests, validate_items};
use crate::session_client::Session;

/// Wilson 95% 信頼区間 (%) を計算する。
pub fn wilson_ci(hit: usize, n: usize, z: f64) -> (f64, f64) {
    if n == 0 {
        return (0.0, 0.0);
    }
    let nf = n as f64;
    let p = (hit as f64) / nf;
    let d = 1.0 + z * z / nf;
    let c = p + z * z / (2.0 * nf);
    let m = z * ((p * (1.0 - p) + z * z / (4.0 * nf)) / nf).sqrt();
    (100.0 * (c - m) / d, 100.0 * (c + m) / d)
}

/// 同一読みで前文が異なる問題を別バッチへ分割する。
pub fn context_batches(items: &[Value], use_context: bool) -> Vec<Vec<usize>> {
    let mut batches = Vec::new();
    let mut batch = Vec::new();
    let mut contexts: HashMap<String, &str> = HashMap::new();
    for (i, item) in items.iter().enumerate() {
        let reading = kata_to_hira(item["input"].as_str().unwrap_or(""));
        let context = if use_context {
            item.get("context_text")
                .and_then(Value::as_str)
                .unwrap_or("")
        } else {
            ""
        };
        if let Some(&prev) = contexts.get(&reading) {
            if prev != context {
                batches.push(std::mem::take(&mut batch));
                contexts.clear();
            }
        }
        contexts.insert(reading, context);
        batch.push(i);
    }
    if !batch.is_empty() {
        batches.push(batch);
    }
    batches
}

#[derive(Debug, Clone)]
pub struct EvalBaselineOpts {
    pub converter_main: PathBuf,
    pub data: PathBuf,
    pub out: Option<PathBuf>,
    pub profile: Option<PathBuf>,
    pub context: bool,
    pub timeout_secs: f64,
    pub stderr_out: Option<PathBuf>,
    pub min_acc: Option<f64>,
    pub converter_source_commit: String,
    pub case_traces: Option<PathBuf>,
    pub request_traces: Option<PathBuf>,
}

fn build_manifest(
    opts: &EvalBaselineOpts,
    exe: &Path,
    env: &HashMap<String, String>,
    acc: f64,
    n: usize,
    repo_root: &Path,
) -> Result<Value> {
    let run_git = |args: &[&str]| -> String {
        Command::new("git")
            .args(args)
            .current_dir(repo_root)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
            .unwrap_or_default()
    };
    let mut models = serde_json::Map::new();
    let mut paths: Vec<PathBuf> = [
        "ASTERIA_ZENZ_MODEL",
        "ASTERIA_LLM_MODEL",
        "ASTERIA_LM_CHOICE_MODEL",
    ]
    .iter()
    .filter_map(|k| env.get(*k).map(PathBuf::from))
    .collect();
    if let Some(md) = env.get("ASTERIA_MODEL_DIR").map(PathBuf::from) {
        if md.is_dir() {
            let mut ggufs: Vec<PathBuf> = fs::read_dir(&md)?
                .filter_map(|e| e.ok().map(|x| x.path()))
                .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("gguf"))
                .collect();
            ggufs.sort();
            paths.extend(ggufs);
        }
    }
    for p in paths {
        let key = p.to_string_lossy().into_owned();
        if p.is_file() && !models.contains_key(&key) {
            models.insert(key, json!(sha256_file(&p)?));
        }
    }
    let mut runtime = serde_json::Map::new();
    if let Some(rd) = env.get("ASTERIA_RUNTIME_DIR").map(PathBuf::from) {
        if rd.is_dir() {
            let mut dlls: Vec<PathBuf> = fs::read_dir(&rd)?
                .filter_map(|e| e.ok().map(|x| x.path()))
                .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("dll"))
                .collect();
            dlls.sort();
            for p in dlls {
                runtime.insert(p.to_string_lossy().into_owned(), json!(sha256_file(&p)?));
            }
        }
    }
    let asteria_env: BTreeMap<String, String> = env
        .iter()
        .filter(|(k, _)| k.starts_with("ASTERIA_"))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    let dict_path = exe.parent().context("exe parent")?.join(format!(
        "{}.runfiles/_main/data_manager/oss/mozc.data",
        exe.file_name().and_then(|n| n.to_str()).unwrap_or("")
    ));
    let dict_sha = if dict_path.is_file() {
        sha256_file(&dict_path)?
    } else {
        String::new()
    };
    let gpu = Command::new("nvidia-smi")
        .args(["--query-gpu=name,driver_version", "--format=csv,noheader"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
        .unwrap_or_default();

    Ok(json!({
        "data": {"path": opts.data.to_string_lossy(), "sha256": sha256_file(&opts.data)?, "items": n},
        "context": opts.context,
        "evidence_scope": "offline_converter_not_installed_tsf",
        "converter_source_commit": if opts.converter_source_commit.is_empty() { "unverified" } else { &opts.converter_source_commit },
        "profile": opts.profile.as_ref().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default(),
        "profile_policy": if opts.profile.is_some() { "explicit" } else { "isolated_temporary" },
        "tracked_working_changes": !run_git(&["status", "--porcelain", "--untracked-files=no"]).is_empty(),
        "acc": acc,
        "converter_main": {"path": exe.to_string_lossy(), "sha256": sha256_file(exe)?},
        "models": models,
        "runtime": runtime,
        "env": asteria_env,
        "commit": run_git(&["rev-parse", "HEAD"]),
        "gpu": gpu,
        "dictionary": {"path": dict_path.to_string_lossy(), "sha256": dict_sha},
    }))
}

pub fn run_conversions(
    opts: &EvalBaselineOpts,
    exe: &Path,
    cwd: &Path,
    items: &[Value],
    env_map: &HashMap<String, String>,
) -> Result<(Vec<Vec<String>>, Vec<u8>, usize)> {
    let mut batches = context_batches(items, opts.context);
    if opts.case_traces.is_some() {
        batches = (0..items.len()).map(|i| vec![i]).collect();
    }
    let tmp = tempfile::Builder::new().prefix("asteria-eval-").tempdir()?;
    let mut blocks = Vec::new();
    let mut all_stderr = Vec::new();
    let deadline = Instant::now() + Duration::from_secs_f64(opts.timeout_secs);

    for (batch_num, batch) in batches.iter().enumerate() {
        let mut batch_env = env_map.clone();
        batch_env.remove("ASTERIA_LM_CONTEXT_MAP");
        if opts.context {
            let ctx_path = tmp.path().join(format!("context-{batch_num}.tsv"));
            let mut contexts: BTreeMap<String, String> = BTreeMap::new();
            for &i in batch {
                let r = kata_to_hira(items[i]["input"].as_str().unwrap_or(""));
                let c = items[i]
                    .get("context_text")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_owned();
                contexts.insert(r, c);
            }
            let body: String = contexts
                .into_iter()
                .filter(|(_, c)| !c.is_empty())
                .map(|(r, c)| format!("{r}\t{c}\n"))
                .collect();
            fs::write(&ctx_path, body)?;
            batch_env.insert(
                "ASTERIA_LM_CONTEXT_MAP".into(),
                ctx_path.to_string_lossy().into_owned(),
            );
        }

        let mut cmd = Command::new(exe);
        if opts.request_traces.is_some() {
            cmd.arg("--evaluation_trace=true");
        }
        let profile = if let Some(p) = &opts.profile {
            let base = fs::canonicalize(p).unwrap_or_else(|_| p.clone());
            if batches.len() > 1 {
                base.join(format!("batch-{batch_num}"))
            } else {
                base
            }
        } else {
            tmp.path().join(format!("profile-{batch_num}"))
        };
        fs::create_dir_all(&profile)?;
        cmd.arg(format!("--user_profile_dir={}", profile.display()));
        cmd.current_dir(cwd)
            .env_clear()
            .envs(&batch_env)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let script: String = batch
            .iter()
            .map(|&i| {
                format!(
                    "start {}\nreset\nasteria_ime_eval_separator\n",
                    kata_to_hira(items[i]["input"].as_str().unwrap_or(""))
                )
            })
            .collect();
        if Instant::now() >= deadline {
            bail!("変換が制限時間 ({:.0}秒) を超えました", opts.timeout_secs);
        }
        let mut child = cmd.spawn()?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(script.as_bytes())?;
        }
        let out = child.wait_with_output()?;
        all_stderr.extend_from_slice(&out.stderr);
        all_stderr.push(b'\n');

        if let Some(td) = &opts.case_traces {
            fs::create_dir_all(td)?;
            fs::write(td.join(format!("{}.stdout", batch[0])), &out.stdout)?;
            fs::write(td.join(format!("{}.stderr", batch[0])), &out.stderr)?;
        }
        if !out.status.success() {
            let tail = String::from_utf8_lossy(&out.stderr);
            bail!("変換器の終了コード {:?}: {tail}", out.status.code());
        }
        if let Some(td) = &opts.request_traces {
            let req_logs = split_scoring_requests(&out.stderr, batch.len())?;
            fs::create_dir_all(td)?;
            fs::write(td.join(format!("batch-{batch_num}.stdout")), &out.stdout)?;
            fs::write(td.join(format!("batch-{batch_num}.stderr")), &out.stderr)?;
            let batch_err_sha = sha256_bytes(&out.stderr);
            let batch_out_sha = sha256_bytes(&out.stdout);
            for (&idx, (req_id, raw)) in batch.iter().zip(req_logs) {
                fs::write(td.join(format!("{idx}.stderr")), &raw)?;
                let link = json!({
                    "request_id": req_id,
                    "batch": batch_num,
                    "stderr_sha256": sha256_bytes(&raw),
                    "batch_stderr_sha256": batch_err_sha,
                    "batch_stdout_sha256": batch_out_sha,
                });
                fs::write(td.join(format!("{idx}.link.json")), link.to_string())?;
            }
        }

        let mut current: Vec<String> = Vec::new();
        let mut batch_blocks: Vec<Vec<String>> = Vec::new();
        let mut in_segment = false;
        for raw_line in out.stdout.split(|&b| b == b'\n') {
            let raw_line = raw_line.strip_suffix(b"\r").unwrap_or(raw_line);
            if raw_line.starts_with(b"ExecCommand() return false") {
                batch_blocks.push(std::mem::take(&mut current));
                in_segment = false;
            } else if raw_line.starts_with(b"---------- Segment ") {
                in_segment = true;
                current.push(std::str::from_utf8(raw_line)?.to_owned());
            } else if in_segment {
                current.push(std::str::from_utf8(raw_line)?.to_owned());
            }
        }
        if batch_blocks.len() != batch.len() {
            bail!("出力件数 {} != 問題数 {}", batch_blocks.len(), batch.len());
        }
        blocks.extend(batch_blocks);
    }
    Ok((blocks, all_stderr, batches.len()))
}

/// `eval_baseline.py` メイン実行（戻り値: 0=合格, 1=異常, 2=Acc@1 未満）。
pub fn run_eval_baseline(opts: &EvalBaselineOpts, repo_root: &Path) -> Result<i32> {
    let exe = fs::canonicalize(&opts.converter_main)?;
    let runfiles = exe.parent().context("exe parent")?.join(format!(
        "{}.runfiles/_main",
        exe.file_name().and_then(|n| n.to_str()).unwrap_or("")
    ));
    let cwd = if runfiles.is_dir() {
        runfiles
    } else {
        exe.parent().context("exe parent")?.to_path_buf()
    };
    let data_bytes = fs::read(&opts.data)?;
    let items: Vec<Value> = serde_json::from_slice(&data_bytes)?;
    validate_items(&items)?;

    if let Some(rt) = &opts.request_traces {
        if rt.exists() && fs::read_dir(rt)?.next().is_some() {
            bail!("要求原票の保存先が空ではありません。過去の原票を保持してください");
        }
    }
    let mut env_map: HashMap<String, String> = env::vars().collect();
    env_map.remove("ASTERIA_LM_CONTEXT_MAP");
    env_map.remove("ASTERIA_LM_PRECEDING");

    let before_meta = if opts.out.is_some() {
        Some(build_manifest(
            opts,
            &exe,
            &env_map,
            0.0,
            items.len(),
            repo_root,
        )?)
    } else {
        None
    };

    let t0 = Instant::now();
    let (blocks, stderr, batch_count) = run_conversions(opts, &exe, &cwd, &items, &env_map)?;
    let elapsed = t0.elapsed().as_secs_f64();

    let mut metadata = if opts.out.is_some() {
        Some(build_manifest(
            opts,
            &exe,
            &env_map,
            0.0,
            items.len(),
            repo_root,
        )?)
    } else {
        None
    };
    if let (Some(bm), Some(am)) = (&before_meta, &metadata) {
        for key in ["data", "converter_main", "dictionary", "models", "runtime"] {
            if bm.get(key) != am.get(key) {
                bail!("測定中にデータ・変換器・辞書・モデル・評価器が変更されました");
            }
        }
    }
    if let Some(se_path) = &opts.stderr_out {
        fs::write(se_path, &stderr)?;
    }

    let time_re = Regex::new(r"\[time\].* total=(\d+)ms")?;
    let stderr_str = String::from_utf8_lossy(&stderr);
    let mut times: Vec<f64> = time_re
        .captures_iter(&stderr_str)
        .filter_map(|c| c[1].parse::<f64>().ok())
        .collect();
    times.sort_by(f64::total_cmp);

    let mut rows = Vec::with_capacity(items.len());
    let mut hit = 0usize;
    let trace_dir_opt = opts.case_traces.as_ref().or(opts.request_traces.as_ref());

    for (i, it) in items.iter().enumerate() {
        let mut row = score_case(it, &blocks[i], 5)?;
        if let Some(td) = trace_dir_opt {
            let trace = td.join(format!("{i}.stderr"));
            let exp: Vec<String> = it["expected_output"]
                .as_array()
                .unwrap_or(&Vec::new())
                .iter()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect();
            row["scoring_trace"] = scoring_trace(&fs::read(&trace)?, &exp)?;
            if opts.request_traces.is_some() {
                let link: Value =
                    serde_json::from_str(&fs::read_to_string(td.join(format!("{i}.link.json")))?)?;
                if Some(sha256_file(&trace)?.as_str()) != link["stderr_sha256"].as_str() {
                    bail!("要求原票が測定中に変更されました");
                }
                row["trace"] = json!({
                    "case_index": i,
                    "request_id": link["request_id"],
                    "batch": link["batch"],
                    "stderr_sha256": link["stderr_sha256"],
                    "batch_stderr_sha256": link["batch_stderr_sha256"],
                    "batch_stdout_sha256": link["batch_stdout_sha256"],
                });
            } else {
                row["trace"] = json!({
                    "case_index": i,
                    "stderr_sha256": sha256_file(&trace)?,
                    "stdout_sha256": sha256_file(&td.join(format!("{i}.stdout")))?,
                });
            }
        }
        if row["ok"].as_bool() == Some(true) {
            hit += 1;
        }
        rows.push(row);
    }

    let (lo, hi) = wilson_ci(hit, items.len(), 1.96);
    let acc = 100.0 * (hit as f64) / (items.len() as f64);
    println!(
        "Acc@1 {hit}/{} = {acc:.1}%(95% 区間 {lo:.1}〜{hi:.1}%)  ({elapsed:.0} 秒)",
        items.len()
    );
    if !times.is_empty() {
        let med = times[times.len() / 2];
        let p95 = times[((times.len() as f64) * 0.95) as usize];
        let max_t = times[times.len() - 1];
        println!("AI の変換 中央値 {med:.0} ms、p95 {p95:.0} ms、最大 {max_t:.0} ms");
    }

    if let Some(out_path) = &opts.out {
        fs::write(out_path, serde_json::to_string_pretty(&rows)?)?;
        if let Some(m) = metadata.as_mut().and_then(Value::as_object_mut) {
            m.insert("acc".into(), json!(acc));
            m.insert("result_sha256".into(), json!(sha256_file(out_path)?));
            m.insert("context_batches".into(), json!(batch_count));
            m.insert("case_isolated".into(), json!(opts.case_traces.is_some()));
            m.insert(
                "request_traced".into(),
                json!(opts.request_traces.is_some()),
            );
            let td_str = trace_dir_opt
                .and_then(|p| fs::canonicalize(p).ok())
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default();
            m.insert("trace_directory".into(), json!(td_str));
        }
        let m_path = format!("{}.manifest.json", out_path.display());
        fs::write(m_path, serde_json::to_string_pretty(&metadata)?)?;
    }

    if let Some(min) = opts.min_acc {
        if acc < min {
            eprintln!("失敗: Acc@1 {acc:.1}% が基準 {min:.1}% を下回った");
            return Ok(2);
        }
    }
    Ok(0)
}

#[derive(Debug, Clone)]
pub struct SessionEvalOpts {
    pub exe: PathBuf,
    pub data: PathBuf,
    pub out: PathBuf,
    pub context: bool,
    pub models: Option<PathBuf>,
    pub ai_state: String,
    pub warm: f64,
    pub limit: usize,
    pub config: Vec<(String, String)>,
    pub immediate_space: bool,
    pub space_gap_ms: Option<f64>,
    pub live: bool,
    pub timeline: bool,
    pub extra_env: Vec<(String, String)>,
}

/// `session_eval.py`: 実際の IME と同じ `session_handler_main` 経路で評価する。
pub fn run_session_eval(opts: &SessionEvalOpts, repo_root: &Path) -> Result<()> {
    let default_m = asteria_common::ai::default_models(repo_root);
    let models_dir = check_models(opts.models.as_deref().or(default_m.as_deref()))?;
    let mut items: Vec<Value> = serde_json::from_str(&fs::read_to_string(&opts.data)?)?;
    if opts.limit > 0 && opts.limit < items.len() {
        items.truncate(opts.limit);
    }

    let mut env: Vec<(String, String)> = env::vars().collect();
    env.push((
        "ASTERIA_MODEL_DIR".into(),
        models_dir.to_string_lossy().into_owned(),
    ));
    env.push((
        "ASTERIA_RUNTIME_DIR".into(),
        models_dir.to_string_lossy().into_owned(),
    ));
    if opts.timeline {
        env.push(("ASTERIA_LM_TIMELINE".into(), "1".into()));
    }
    for (k, v) in &opts.extra_env {
        env.push((k.clone(), v.clone()));
    }

    let exe = fs::canonicalize(&opts.exe)?;
    let profile = tempfile::Builder::new()
        .prefix("asteria-session-eval-")
        .tempdir()?;
    let stderr_path = profile.path().join("stderr.txt");
    let mut session = Session::spawn(&exe, &env, profile.path(), &stderr_path, false)?;

    for (k, v) in &opts.config {
        session.send(&format!("SET_CONFIG\t{k}\t{v}"))?;
    }
    session.send("SEND_KEY\tON\nSEND_KEYS\tai")?;
    thread::sleep(Duration::from_secs_f64(opts.warm));
    session.send("SEND_KEY\tESC")?;

    if opts.ai_state != "any" {
        let allow: Vec<&str> = opts.ai_state.split(',').collect();
        let sess_dir = profile.path().join("session");
        wait_ai(
            &sess_dir,
            || {
                let _ = session.send("SEND_KEYS\tai");
                let _ = session.send("SEND_KEY\tESC");
                Ok(())
            },
            &allow,
            Duration::from_secs(90),
        )?;
    }

    let delay_re = Regex::new(r"delay_millisec:\s*(\d+)")?;
    let get_cb =
        |txt: &str| -> Option<u64> { delay_re.captures(txt).and_then(|c| c[1].parse().ok()) };
    let get_preedit = |txt: &str| -> String {
        crate::textproto::parse_output(txt)
            .map(|p| p.value)
            .unwrap_or_default()
    };

    let mut rows = Vec::new();
    for it in &items {
        let romaji = it
            .get("romaji")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .or_else(|| it["input"].as_str().and_then(imebench_to_romaji));
        let Some(romaji) = romaji else {
            rows.push(json!({"index": it["index"], "skipped": true}));
            continue;
        };

        let started = Instant::now();
        let context = if opts.context {
            it.get("context_text").and_then(Value::as_str).unwrap_or("")
        } else {
            ""
        };
        let escaped_ctx = context.replace('\\', "\\\\").replace('"', "\\\"");
        let option = format!("context.preceding_text=\"{escaped_ctx}\"");

        let mut text = String::new();
        if !context.is_empty() {
            for ch in romaji.chars() {
                let ev = session.send(&format!("SEND_KEY_WITH_OPTION\t{ch}\t{option}"))?;
                text = ev.output;
            }
        } else {
            text = session.send(&format!("SEND_KEYS\t{romaji}"))?.output;
        }

        let live_info = if opts.live {
            let waited_from = Instant::now();
            let mut seen = vec![(0.0, get_preedit(&text))];
            while let Some(d) = get_cb(&text) {
                if waited_from.elapsed().as_secs_f64() >= 5.0 {
                    break;
                }
                thread::sleep(Duration::from_millis(d));
                text = session.send("ASTERIA_REFRESH")?.output;
                seen.push((
                    waited_from.elapsed().as_secs_f64() * 1000.0,
                    get_preedit(&text),
                ));
            }
            let final_s = seen.last().map(|x| x.1.clone()).unwrap_or_default();
            let first_ms = seen
                .iter()
                .enumerate()
                .find(|(idx, _)| seen[*idx..].iter().all(|(_, w)| w == &final_s))
                .map(|(_, (t, _))| (*t * 10.0).round() / 10.0)
                .unwrap_or(0.0);
            Some((final_s, first_ms))
        } else {
            None
        };

        let gap_s = if opts.immediate_space || opts.live {
            0.0
        } else if let Some(ms) = opts.space_gap_ms {
            ms / 1000.0
        } else {
            0.7
        };
        let typed_at = Instant::now();
        while let Some(d) = get_cb(&text) {
            if typed_at.elapsed().as_secs_f64() + (d as f64) / 1000.0 > gap_s {
                break;
            }
            thread::sleep(Duration::from_millis(d));
            text = session.send("ASTERIA_REFRESH")?.output;
        }
        if opts.space_gap_ms.is_some() {
            let rem = gap_s - typed_at.elapsed().as_secs_f64();
            if rem > 0.0 {
                thread::sleep(Duration::from_secs_f64(rem));
            }
        }

        let space_sent = Instant::now();
        let space_cmd = if !context.is_empty() {
            format!("SEND_KEY_WITH_OPTION\tSpace\t{option}")
        } else {
            "SEND_KEY\tSpace".into()
        };
        text = session.send(&space_cmd)?.output;
        let mut waited = 0usize;
        while let Some(d) = get_cb(&text) {
            if space_sent.elapsed().as_secs_f64() >= 10.0 {
                break;
            }
            thread::sleep(Duration::from_millis(d));
            text = session.send("ASTERIA_REFRESH")?.output;
            waited += 1;
        }
        let space_to_cand_ms = (space_sent.elapsed().as_secs_f64() * 10000.0).round() / 10.0;
        let top1 = get_preedit(&text);
        let exp = &it["expected_output"];
        let ok = exp
            .as_array()
            .is_some_and(|a| a.iter().any(|v| v.as_str() == Some(top1.as_str())));
        let mut row = json!({
            "index": it["index"],
            "input": it["input"],
            "context_text": it.get("context_text").and_then(Value::as_str).unwrap_or(""),
            "top1": top1,
            "expected": exp,
            "ok": ok,
            "refreshes_after_space": waited,
            "space_to_candidate_ms": space_to_cand_ms,
            "ms": (started.elapsed().as_secs_f64() * 10000.0).round() / 10.0,
        });
        if let Some((live_txt, live_ms)) = live_info {
            let live_ok = exp
                .as_array()
                .is_some_and(|a| a.iter().any(|v| v.as_str() == Some(live_txt.as_str())));
            row["live"] = json!(live_txt);
            row["live_ok"] = json!(live_ok);
            row["live_ms"] = json!(live_ms);
            row["live_same_as_space"] = json!(live_txt == top1);
        }
        rows.push(row);
        session.send("SEND_KEY\tESC")?;
        session.send("SEND_KEY\tESC")?;
    }

    session.close(false)?;
    fs::write(&opts.out, serde_json::to_string_pretty(&rows)?)?;

    let scored: Vec<&Value> = rows
        .iter()
        .filter(|r| r.get("skipped").and_then(Value::as_bool) != Some(true))
        .collect();
    let ok_count = scored
        .iter()
        .filter(|r| r["ok"].as_bool() == Some(true))
        .count();
    let mut times: Vec<f64> = scored
        .iter()
        .filter_map(|r| r["space_to_candidate_ms"].as_f64())
        .collect();
    times.sort_by(f64::total_cmp);
    let med = times.get(times.len() / 2).copied().unwrap_or(0.0);
    let p95 = if times.is_empty() {
        0.0
    } else {
        let idx = ((times.len() * 95).div_ceil(100)).saturating_sub(1);
        times[idx]
    };
    let max_t = times.last().copied().unwrap_or(0.0);
    println!(
        "{}: {ok_count}/{} (skipped {}) Space→候補 中央値 {med:.0}ms p95 {p95:.0}ms 最大 {max_t:.0}ms",
        opts.data.display(),
        scored.len(),
        rows.len() - scored.len()
    );
    Ok(())
}

/// `eval/imebench/to_rows.py`: `imebench` TSV 結果を共通 row JSON 形式へ変換する。
pub fn imebench_to_rows(data_path: &Path, tsv_path: &Path, out_path: &Path) -> Result<()> {
    let items: Vec<Value> = serde_json::from_str(&fs::read_to_string(data_path)?)?;
    let mut got: HashMap<String, String> = HashMap::new();
    for line in BufReader::new(fs::File::open(tsv_path)?).lines() {
        let line = line?;
        if let Some((k, v)) = line.split_once('\t') {
            got.insert(k.to_owned(), v.to_owned());
        }
    }
    let norm = |s: &str| -> String { nfkc(s).chars().filter(|c| !c.is_whitespace()).collect() };
    let mut rows = Vec::new();
    for (i, it) in items.iter().enumerate() {
        let top1 = got.get(&i.to_string()).cloned().unwrap_or_default();
        let top1_n = norm(&top1);
        let exp: Vec<String> = it["expected_output"]
            .as_array()
            .unwrap_or(&Vec::new())
            .iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect();
        let ok = exp.iter().any(|e| norm(e) == top1_n);
        rows.push(json!({
            "index": it["index"],
            "input": it["input"],
            "context_text": it.get("context_text").and_then(Value::as_str).unwrap_or(""),
            "top1": top1,
            "expected": exp,
            "ok": ok,
        }));
    }
    fs::write(out_path, serde_json::to_string_pretty(&rows)?)?;
    Ok(())
}

/// `eval_predict.py`: Tab 予測 / 入力中候補の当たり・節約文字数を測る。
pub fn run_eval_predict(
    converter_main: &Path,
    data_path: &Path,
    typed_len: usize,
    top_k: usize,
    min_len: usize,
    out_path: Option<&Path>,
) -> Result<()> {
    let exe = fs::canonicalize(converter_main)?;
    let cwd = exe.parent().context("exe parent")?.join(format!(
        "{}.runfiles/_main",
        exe.file_name().and_then(|n| n.to_str()).unwrap_or("")
    ));
    let items: Vec<Value> = serde_json::from_str(&fs::read_to_string(data_path)?)?;
    let mut seen = HashSet::new();
    let mut rows = Vec::new();
    for it in &items {
        if let Some(t) = it.get("typed").and_then(Value::as_str) {
            if seen.insert(t.to_owned()) {
                rows.push(json!({
                    "typed": t,
                    "context": it["context"],
                    "truth": it["truth"],
                    "conv": it["conv"],
                }));
            }
            continue;
        }
        let r = kata_to_hira(it["input"].as_str().unwrap_or(""));
        let chars: Vec<char> = r.chars().collect();
        if chars.is_empty() || !('ぁ'..='ゖ').contains(&chars[0]) || chars.len() < typed_len + 3 {
            continue;
        }
        let t: String = chars[..typed_len].iter().collect();
        if !seen.insert(t.clone()) {
            continue;
        }
        rows.push(json!({
            "typed": t,
            "context": it.get("context_text").and_then(Value::as_str).unwrap_or(""),
            "truth": it["expected_output"][0],
        }));
    }
    let ctx_tmp = tempfile::NamedTempFile::new()?;
    let ctx_body: String = rows
        .iter()
        .filter_map(|r| {
            let c = r["context"].as_str().unwrap_or("");
            if c.is_empty() {
                None
            } else {
                Some(format!("{}\t{c}\n", r["typed"].as_str().unwrap_or("")))
            }
        })
        .collect();
    fs::write(ctx_tmp.path(), ctx_body)?;

    let script: String = rows
        .iter()
        .map(|r| {
            format!(
                "predict {}\nreset\nasteria_ime_eval_separator\n",
                r["typed"].as_str().unwrap_or("")
            )
        })
        .chain(std::iter::once("quit\n".to_owned()))
        .collect();

    let t0 = Instant::now();
    let mut child = Command::new(&exe)
        .current_dir(&cwd)
        .env("ASTERIA_LM_CONTEXT_MAP", ctx_tmp.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(script.as_bytes())?;
    }
    let out = child.wait_with_output()?;
    let elapsed = t0.elapsed().as_secs_f64();
    let stdout_s = String::from_utf8_lossy(&out.stdout);

    let mut blocks: Vec<Vec<&str>> = Vec::new();
    let mut cur: Vec<&str> = Vec::new();
    for line in stdout_s.lines() {
        if line.starts_with("ExecCommand() return false") {
            blocks.push(std::mem::take(&mut cur));
        } else {
            cur.push(line);
        }
    }

    let (mut hit, mut shown, mut off, mut full) = (0usize, 0usize, 0usize, 0usize);
    let mut saved: Vec<usize> = Vec::new();
    let mut gained: Vec<usize> = Vec::new();
    for (i, r) in rows.iter_mut().enumerate() {
        let mut cands = Vec::new();
        if let Some(blk) = blocks.get(i) {
            for line in blk {
                if line.starts_with("  ") {
                    if let Some((head, val)) = line.trim().split_once(' ') {
                        if head
                            .split_once('/')
                            .is_some_and(|(a, _)| a.chars().all(|c| c.is_ascii_digit()))
                        {
                            cands.push(val.to_owned());
                        }
                    }
                }
            }
        }
        let top_cands: Vec<String> = cands.into_iter().take(top_k).collect();
        let truth = r["truth"].as_str().unwrap_or("");
        if top_cands.iter().any(|c| c == truth) {
            full += 1;
        }
        let conv_len = r
            .get("conv")
            .and_then(Value::as_str)
            .map(|s| s.chars().count())
            .unwrap_or(0);
        let least = if r.get("conv").is_some() {
            conv_len + 1
        } else {
            min_len
        };
        let good: Vec<&String> = top_cands
            .iter()
            .filter(|c| c.chars().count() >= least && truth.starts_with(c.as_str()))
            .collect();
        if !good.is_empty() {
            hit += 1;
            let best_len = good.iter().map(|c| c.chars().count()).max().unwrap_or(0);
            saved.push(best_len);
            gained.push(best_len.saturating_sub(conv_len));
        }
        for c in &top_cands {
            if c.chars().count() > conv_len {
                shown += 1;
                if !truth.starts_with(c.as_str()) {
                    off += 1;
                }
            }
        }
        r["cands"] = json!(top_cands);
    }

    let n = rows.len().max(1);
    let avg_saved = (saved.iter().sum::<usize>() as f64) / (saved.len().max(1) as f64);
    let total_gained: usize = gained.iter().sum();
    println!(
        "文まで当たり {full}/{}  当たり {hit}/{} = {:.1}%  当たった候補の長さ {avg_saved:.1} 文字  ({elapsed:.0} 秒)",
        rows.len(),
        rows.len(),
        100.0 * (hit as f64) / (n as f64)
    );
    println!(
        "節約できた文字数 合計 {total_gained}(1 問あたり {:.1})  外れ候補 {off}/{shown} = {:.1}%",
        (total_gained as f64) / (n as f64),
        100.0 * (off as f64) / (shown.max(1) as f64)
    );
    if let Some(op) = out_path {
        fs::write(op, serde_json::to_string_pretty(&rows)?)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_batches_splits_conflicting_readings() {
        let items = vec![
            json!({"index": 1, "input": "はし", "context_text": "食事", "expected_output": ["箸"]}),
            json!({"index": 2, "input": "ハシ", "context_text": "渡河", "expected_output": ["橋"]}),
            json!({"index": 3, "input": "はし", "context_text": "", "expected_output": ["箸"]}),
        ];
        assert_eq!(
            context_batches(&items, true),
            vec![vec![0], vec![1], vec![2]]
        );
        assert_eq!(context_batches(&items, false), vec![vec![0, 1, 2]]);
    }
}
