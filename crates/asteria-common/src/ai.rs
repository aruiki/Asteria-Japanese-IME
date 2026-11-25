//! AI モデルの場所解決と状態ファイル (`asteria_ai_status.txt`) の待機。

use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{bail, Result};

const REQUIRED: &[&str] = &["zenz-v2.5-small-q8_0.gguf", "tinyswallow-1.5b-q5_k_m.gguf"];

/// 必要なモデルがそろった最初の候補フォルダを返す。
pub fn default_models(repo_root: &Path) -> Option<PathBuf> {
    let candidates = [
        repo_root.join("target/accuracy-guide/eval-models"),
        repo_root.join("target/accuracy-guide/v310-ci-msi-6459/extract/PFiles/Asteria"),
        repo_root
            .join("target/accuracy-guide/short-word-package-check-v7/payload/ext/PFiles/Asteria"),
    ];
    candidates
        .into_iter()
        .find(|dir| REQUIRED.iter().all(|name| dir.join(name).is_file()))
}

/// モデルディレクトリの妥当性を検証する。
pub fn check_models(path: Option<&Path>) -> Result<PathBuf> {
    let Some(dir) = path else {
        bail!(
            "AI のモデルがない: (候補なし) に {} がない。--models で指定する",
            REQUIRED.join(", ")
        );
    };
    let missing: Vec<&str> = REQUIRED
        .iter()
        .copied()
        .filter(|name| !dir.join(name).is_file())
        .collect();
    if !missing.is_empty() {
        bail!(
            "AI のモデルがない: {} に {} がない。--models で指定する",
            dir.display(),
            missing.join(", ")
        );
    }
    Ok(dir.to_path_buf())
}

/// プロファイル内の `asteria_ai_status.txt` から `state=` 値を読み取る。
pub fn ai_state(profile: &Path) -> String {
    let path = profile.join("asteria_ai_status.txt");
    let Ok(bytes) = fs::read(path) else {
        return String::new();
    };
    let text = String::from_utf8_lossy(&bytes);
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("state=") {
            return rest.trim().to_owned();
        }
    }
    String::new()
}

/// `asteria_ai_status.txt` が `allow` のいずれかの状態になるまで `poke` を呼びつつ待機する。
pub fn wait_ai<F>(profile: &Path, mut poke: F, allow: &[&str], timeout: Duration) -> Result<String>
where
    F: FnMut() -> Result<()>,
{
    let deadline = Instant::now() + timeout;
    let mut state = String::new();
    while Instant::now() < deadline {
        state = ai_state(profile);
        if allow.iter().any(|&a| a == state) {
            return Ok(state);
        }
        poke()?;
        thread::sleep(Duration::from_secs(1));
    }
    let shown = if state.is_empty() { "(なし)" } else { &state };
    bail!(
        "AI が動いていない(asteria_ai_status の state={shown})。AI なしの結果は数えないので止める"
    )
}
