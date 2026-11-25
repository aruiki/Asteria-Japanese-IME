//! `mozc/tools/test_make_patches.py` 移植: `make_patches.sh` の MSYS パス変換禁止ガード。

use std::path::Path;
use std::process::Command;

use tempfile::TempDir;

#[test]
fn bazel_only_path_settings_stop_before_any_file_operation() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("repo root");
    let script = repo_root.join("mozc/tools/make_patches.sh");
    let bash = if cfg!(windows) {
        let git_bash = Path::new("C:/Program Files/Git/bin/bash.exe");
        if !git_bash.is_file() {
            return;
        }
        git_bash.to_path_buf()
    } else {
        Path::new("bash").to_path_buf()
    };

    for (key, value) in [("MSYS_NO_PATHCONV", "1"), ("MSYS2_ARG_CONV_EXCL", "*")] {
        let tmp = TempDir::new().unwrap();
        let mut cmd = Command::new(&bash);
        cmd.arg(&script)
            .current_dir(tmp.path())
            .env("TMPDIR", tmp.path())
            .env_remove("MSYS_NO_PATHCONV")
            .env_remove("MSYS2_ARG_CONV_EXCL")
            .env(key, value);
        let Ok(out) = cmd.output() else {
            continue;
        };
        assert!(
            !out.status.success(),
            "expected non-zero exit for {key}={value}"
        );
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(
            stderr.contains("パッチ生成ではMSYS_NO_PATHCONV"),
            "unexpected stderr for {key}: {stderr}"
        );
        assert_eq!(std::fs::read_dir(tmp.path()).unwrap().count(), 0);
    }
}
