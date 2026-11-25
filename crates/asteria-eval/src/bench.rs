//! レイテンシ・負荷試験・一貫性・学習フローベンチマーク群。
//! (`cost_bench.py`, `space_latency.py`, `stress_test.py`, `typing_test.py`, `keystroke_latency.py`,
//!  `window_profile.py`, `idle_test.py`, `perf_table.py`, `consistency_check.py`, `learning_flow.py`, `qod_replay.py`)

use std::env;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use rand::seq::IndexedRandom;
use rand::Rng;
use rand::SeedableRng;
use serde_json::{json, Value};

use asteria_common::ai::{check_models, default_models};
use asteria_common::kana::kata_to_hira;
use asteria_common::paths::install_dir;
use asteria_common::romaji::imebench_to_romaji;

use crate::session_client::Session;

fn default_converter_exe() -> PathBuf {
    env::var_os("ASTERIA_CONVERTER_MAIN")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(r"C:\Users\aruik\mz\src\bazel-bin\converter\converter_main.exe")
        })
}

fn default_install() -> PathBuf {
    env::var_os("ASTERIA_INSTALL_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(install_dir)
}

struct ConverterPipe {
    child: Child,
    stdin: Option<ChildStdin>,
    rx: Receiver<String>,
}

impl ConverterPipe {
    fn spawn(exe: &Path, env_pairs: &[(String, String)]) -> Result<Self> {
        let cwd = PathBuf::from(format!("{}.runfiles/_main", exe.display()));
        let mut child = Command::new(exe)
            .current_dir(cwd)
            .envs(env_pairs.iter().map(|(k, v)| (k.as_str(), v.as_str())))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .with_context(|| format!("converter_main 起動失敗: {}", exe.display()))?;
        let stdin = child.stdin.take();
        let stdout = child.stdout.take().context("stdout missing")?;
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let mut rdr = BufReader::new(stdout);
            let mut buf = Vec::new();
            loop {
                buf.clear();
                match rdr.read_until(b'\n', &mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {
                        let s = String::from_utf8_lossy(&buf)
                            .trim_end_matches(['\r', '\n'])
                            .to_owned();
                        if tx.send(s).is_err() {
                            break;
                        }
                    }
                }
            }
        });
        Ok(Self { child, stdin, rx })
    }

    fn send(&mut self, cmd: &str, timeout_secs: u64) -> Result<(f64, Vec<String>)> {
        let t0 = Instant::now();
        {
            let stdin = self.stdin.as_mut().context("stdin closed")?;
            stdin.write_all(format!("{cmd}\nreset\nasteria_sep\n").as_bytes())?;
            stdin.flush()?;
        }
        let deadline = t0 + Duration::from_secs(timeout_secs);
        let mut lines = Vec::new();
        while Instant::now() < deadline {
            let rem = deadline.saturating_duration_since(Instant::now());
            match self.rx.recv_timeout(rem.min(Duration::from_millis(50))) {
                Ok(line) => {
                    let done = line.starts_with("ExecCommand() return false");
                    lines.push(line);
                    if done {
                        break;
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
        Ok((t0.elapsed().as_secs_f64() * 1000.0, lines))
    }

    fn close(mut self) -> Result<i32> {
        self.stdin.take();
        let status = self.child.wait()?;
        Ok(status.code().unwrap_or(-1))
    }
}

/// `stress_test.py`: 1〜300 文字・記号・絵文字の読みで suggest/start/predict 負荷試験を行う。
pub fn run_stress_test(extra_env: &[(String, String)]) -> Result<()> {
    let exe = default_converter_exe();
    let inst = default_install();
    let mut env = vec![
        (
            "ASTERIA_RUNTIME_DIR".into(),
            inst.to_string_lossy().into_owned(),
        ),
        (
            "ASTERIA_MODEL_DIR".into(),
            inst.to_string_lossy().into_owned(),
        ),
    ];
    env.extend(extra_env.iter().cloned());

    let kana: Vec<char> = "あいうえおかきくけこさしすせそたちつてとなにぬねのはひふへほまみむめもやゆよらりるれろわをんがぎぐげござじずぜぞだでどばびぶべぼぱぴぷぺぽゃゅょっー"
        .chars()
        .collect();
    let extra = [
        "、", "。", "！", "？", "「", "」", "1", "23", "abc", "A", "ｗ", "😀", " ", "・", "〜",
        "(", ")", "%", "\\",
    ];
    let lengths = [1usize, 2, 3, 5, 8, 15, 30, 60, 120, 250, 300];
    let mut rng = rand::rngs::StdRng::seed_from_u64(1);
    let mut readings = Vec::new();
    for _ in 0..120 {
        let &n = lengths.choose(&mut rng).unwrap_or(&10);
        let mut s = String::new();
        for _ in 0..n {
            if rng.random::<f64>() > 0.12 {
                if let Some(&c) = kana.choose(&mut rng) {
                    s.push(c);
                }
            } else if let Some(&e) = extra.choose(&mut rng) {
                s.push_str(e);
            }
        }
        readings.push(s);
    }
    for fixed in [
        "",
        "ー",
        "っっっ",
        "んんんんん",
        "ゃ",
        "12345678901234567890",
        "aaaaaaaaaaaaaaaaaaaa",
    ] {
        readings.push(fixed.to_owned());
    }

    let mut pipe = ConverterPipe::spawn(&exe, &env)?;
    thread::sleep(Duration::from_secs(6));
    let cmds = ["suggest", "start", "predict"];
    let mut lat: Vec<(f64, &str, usize)> = Vec::new();

    for r in &readings {
        for &c in &cmds {
            let (dt, _) = pipe.send(&format!("{c} {r}"), 20)?;
            lat.push((dt, c, r.chars().count()));
            if let Some(status) = pipe.child.try_wait()? {
                bail!("CRASH exit {:?} at {c} {:?}", status.code(), r);
            }
            if dt > 20_000.0 {
                println!("HANG {c} {}", r.chars().take(40).collect::<String>());
            }
        }
    }
    let code = pipe.close()?;
    println!("exit {code} commands {}", lat.len());
    for c in cmds {
        let mut xs: Vec<f64> = lat
            .iter()
            .filter(|(_, cc, _)| *cc == c)
            .map(|(d, _, _)| *d)
            .collect();
        xs.sort_by(f64::total_cmp);
        let p50 = xs[xs.len() / 2];
        let p95 = xs[(xs.len() * 95 / 100).min(xs.len() - 1)];
        let max_v = xs[xs.len() - 1];
        println!("{c:8} p50 {p50:.0} ms  p95 {p95:.0} ms  max {max_v:.0} ms");
    }
    Ok(())
}

/// `space_latency.py`: 打ち終わり→Space 応答時間（p50, p95, max）を測る。
pub fn run_space_latency(
    interval_secs: f64,
    n: usize,
    pause_secs: f64,
    data_path: &Path,
    out_path: Option<&Path>,
    warm: usize,
    extra_env: &[(String, String)],
) -> Result<()> {
    let exe = default_converter_exe();
    let inst = default_install();
    let mut env = vec![
        (
            "ASTERIA_RUNTIME_DIR".into(),
            inst.to_string_lossy().into_owned(),
        ),
        (
            "ASTERIA_MODEL_DIR".into(),
            inst.to_string_lossy().into_owned(),
        ),
    ];
    env.extend(extra_env.iter().cloned());

    let warmup_all: Vec<Value> =
        serde_json::from_str(&fs::read_to_string("eval/sets/asteria-daily.json")?)?;
    let warmup: Vec<Value> = warmup_all.into_iter().take(warm).collect();
    let mut items: Vec<Value> = serde_json::from_str(&fs::read_to_string(data_path)?)?;
    items.truncate(n);

    let mut pipe = ConverterPipe::spawn(&exe, &env)?;
    let t_start = Instant::now();
    let mut space_ms = Vec::new();
    let mut key_ms = Vec::new();
    let mut ok = 0usize;
    let mut rows = Vec::new();

    for (idx, it) in warmup.iter().chain(items.iter()).enumerate() {
        let r = kata_to_hira(it["input"].as_str().unwrap_or(""));
        let chars: Vec<char> = r.chars().collect();
        for len in 1..=chars.len() {
            let prefix: String = chars[..len].iter().collect();
            let (ms, _) = pipe.send(&format!("suggest {prefix}"), 60)?;
            if idx >= warmup.len() {
                key_ms.push(ms);
            }
            thread::sleep(Duration::from_secs_f64(interval_secs));
        }
        thread::sleep(Duration::from_secs_f64(pause_secs));
        let (ms, lines) = pipe.send(&format!("start {r}"), 60)?;
        if idx < warmup.len() {
            println!(
                "{:5.1}s  慣らし {ms:5.0} ms  {}",
                t_start.elapsed().as_secs_f64(),
                r
            );
            continue;
        }
        space_ms.push(ms);
        let mut top = String::new();
        for line in &lines {
            if line.starts_with("  0/") {
                if let Some((_, val)) = line.trim().split_once(' ') {
                    top.push_str(val.trim());
                }
            }
        }
        let is_ok = it["expected_output"]
            .as_array()
            .is_some_and(|a| a.iter().any(|v| v.as_str() == Some(top.as_str())));
        if is_ok {
            ok += 1;
        }
        rows.push(json!({"reading": r, "space_ms": (ms * 10.0).round() / 10.0}));
        println!(
            "{:5.1}s  Space {ms:5.0} ms  {r}  {top}",
            t_start.elapsed().as_secs_f64()
        );
    }
    pipe.close()?;
    let mut sorted_space = space_ms.clone();
    sorted_space.sort_by(f64::total_cmp);
    key_ms.sort_by(f64::total_cmp);
    let med = sorted_space
        .get(sorted_space.len() / 2)
        .copied()
        .unwrap_or(0.0);
    let p95 = if sorted_space.is_empty() {
        0.0
    } else {
        sorted_space[((sorted_space.len() * 95).div_ceil(100)).saturating_sub(1)]
    };
    let max_s = sorted_space.last().copied().unwrap_or(0.0);
    let max_k = key_ms.last().copied().unwrap_or(0.0);
    println!(
        "Space: 中央値 {med:.0} ms、p95 {p95:.0} ms、最大 {max_s:.0} ms / 入力中の候補: 最大 {max_k:.0} ms / 正解 {ok}/{}",
        items.len()
    );
    if let Some(op) = out_path {
        let summary = json!({
            "data": data_path.to_string_lossy(),
            "interval": interval_secs,
            "pause": pause_secs,
            "converter": exe.to_string_lossy(),
            "median_ms": med,
            "p95_ms": p95,
            "max_ms": max_s,
            "key_max_ms": max_k,
            "correct": ok,
            "n": items.len(),
            "items": rows,
        });
        fs::write(op, serde_json::to_string_pretty(&summary)?)?;
    }
    Ok(())
}

/// `keystroke_latency.py`: 1 打鍵ごとの応答時間を計測する。
#[allow(clippy::too_many_arguments)]
pub fn run_keystroke_latency(
    exe: &Path,
    data_path: &Path,
    out_path: &Path,
    models: Option<&Path>,
    config: &[(String, String)],
    warm: f64,
    gap_ms: f64,
    repo_root: &Path,
) -> Result<Value> {
    let default_m = default_models(repo_root);
    let models_dir = check_models(models.or(default_m.as_deref()))?;
    let items: Vec<Value> = serde_json::from_str(&fs::read_to_string(data_path)?)?;
    let env = vec![
        (
            "ASTERIA_MODEL_DIR".into(),
            models_dir.to_string_lossy().into_owned(),
        ),
        (
            "ASTERIA_RUNTIME_DIR".into(),
            models_dir.to_string_lossy().into_owned(),
        ),
    ];
    let exe = fs::canonicalize(exe)?;
    let profile = tempfile::Builder::new()
        .prefix("asteria-keystroke-")
        .tempdir()?;
    let mut session = Session::spawn(
        &exe,
        &env,
        profile.path(),
        &profile.path().join("err.txt"),
        false,
    )?;
    for (k, v) in config {
        session.send(&format!("SET_CONFIG\t{k}\t{v}"))?;
    }
    session.send("SEND_KEY\tON\nSEND_KEYS\tai")?;
    thread::sleep(Duration::from_secs_f64(warm));
    session.send("SEND_KEY\tESC")?;

    let mut times = Vec::new();
    for it in &items {
        let romaji = it
            .get("romaji")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .or_else(|| it["input"].as_str().and_then(imebench_to_romaji));
        let Some(romaji) = romaji else {
            continue;
        };
        for ch in romaji.chars() {
            let t0 = Instant::now();
            session.send(&format!("SEND_KEY\t{ch}"))?;
            times.push(t0.elapsed().as_secs_f64() * 1000.0);
            thread::sleep(Duration::from_secs_f64(gap_ms / 1000.0));
        }
        session.send("SEND_KEY\tESC")?;
        session.send("SEND_KEY\tESC")?;
    }
    session.close(false)?;

    times.sort_by(f64::total_cmp);
    let pick = |q: f64| -> f64 {
        if times.is_empty() {
            return 0.0;
        }
        let idx = ((times.len() as f64 * q) as usize).min(times.len() - 1);
        (times[idx] * 100.0).round() / 100.0
    };
    let summary = json!({
        "keys": times.len(),
        "median_ms": pick(0.5),
        "p95_ms": pick(0.95),
        "p99_ms": pick(0.99),
        "max_ms": (times.last().copied().unwrap_or(0.0) * 100.0).round() / 100.0,
    });
    fs::write(out_path, serde_json::to_string_pretty(&summary)?)?;
    Ok(summary)
}
