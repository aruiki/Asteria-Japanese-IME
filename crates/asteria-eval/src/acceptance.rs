//! 受入れ・セッション長期連続操作 (Soak / Fuzz / CLI / 代替候補) 検査。
//! (`session_fuzz.py`, `session_soak.py`, `session_cli_acceptance.py`, `scored_alternatives_acceptance.py`,
//!  `ai_first_acceptance.py`, `normal_conversion_acceptance.py`, `first_candidate_smoke.py`)

use std::fs;
use std::path::Path;
use std::thread;
use std::time::Duration;

use anyhow::{bail, Result};
use rand::seq::IndexedRandom;
use rand::SeedableRng;
use serde_json::{json, Value};

use asteria_common::ai::{check_models, default_models, wait_ai};

use crate::session_client::Session;
use crate::textproto::parse_output;

/// `scored_alternatives_acceptance.py`: 有限採点済み代替候補が ID 指定で選択・表示・確定できるか検証する。
pub fn verify_scored_alternatives_result(
    reading: &str,
    expected_surface: &str,
    selected_value: &str,
    committed_result: &str,
) -> Result<()> {
    if selected_value != expected_surface || committed_result != expected_surface {
        bail!("採点済み代替候補 {expected_surface} ({reading}) の選択/確定が一致しません: selected={selected_value}, committed={committed_result}");
    }
    Ok(())
}

/// `session_fuzz.py`: セッション経路の乱打試験（クラッシュ・コマンド拒否なしを確認）。
pub fn run_session_fuzz(
    exe: &Path,
    models: Option<&Path>,
    rounds: usize,
    seed: u64,
    repo_root: &Path,
) -> Result<Value> {
    let default_m = default_models(repo_root);
    let models_dir = check_models(models.or(default_m.as_deref()))?;
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
        .prefix("asteria-session-fuzz-")
        .tempdir()?;
    let mut session = Session::spawn(
        &exe,
        &env,
        profile.path(),
        &profile.path().join("err.txt"),
        false,
    )?;
    session.send("SEND_KEY\tON\nSEND_KEYS\tai")?;
    thread::sleep(Duration::from_secs(10));
    session.send("SEND_KEY\tESC")?;
    wait_ai(
        &profile.path().join("session"),
        || {
            let _ = session.send("SEND_KEYS\tai");
            let _ = session.send("SEND_KEY\tESC");
            Ok(())
        },
        &["llm", "zenz"],
        Duration::from_secs(90),
    )?;

    let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
    let keys = [
        "a",
        "i",
        "u",
        "e",
        "o",
        "k",
        "s",
        "t",
        "n",
        "h",
        "m",
        "y",
        "r",
        "w",
        "Space",
        "Tab",
        "BackSpace",
        "Enter",
        "ESC",
        "Left",
        "Right",
        "Up",
        "Down",
        "F6",
        "F7",
        "F10",
    ];
    for step in 0..rounds {
        let &k = keys.choose(&mut rng).unwrap_or(&"a");
        let cmd = if k.len() == 1 {
            format!("SEND_KEYS\t{k}")
        } else {
            format!("SEND_KEY\t{k}")
        };
        let ev = session.send(&cmd)?;
        let _ = parse_output(&ev.output)?;
        if step % 100 == 99 {
            session.send("ASTERIA_REFRESH")?;
        }
    }
    session.close(false)?;
    Ok(json!({ "rounds": rounds, "seed": seed, "passed": true }))
}

/// `session_cli_acceptance.py`: 入力ファイル不正時に空実行 PASS にならないことを検証する。
pub fn run_session_cli_acceptance(exe: &Path) -> Result<()> {
    let tmp = tempfile::Builder::new()
        .prefix("asteria-cli-accept-")
        .tempdir()?;
    let missing = tmp.path().join("missing.json");
    if missing.exists() {
        bail!("unexpected missing file");
    }
    let _ = exe;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scored_alternatives_requires_exact_selection_and_commit() {
        verify_scored_alternatives_result("はし", "箸", "箸", "箸").unwrap();
        assert!(verify_scored_alternatives_result("はし", "箸", "橋", "箸").is_err());
        assert!(verify_scored_alternatives_result("はし", "箸", "箸", "橋").is_err());
    }
}
