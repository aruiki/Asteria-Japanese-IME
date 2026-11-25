//! リリースの版(タグ)を決める（`mozc/tools/release_tag.py` 移植）。

use std::fs;
use std::path::Path;
use std::process::Command;

use anyhow::{bail, Context, Result};
use regex::Regex;

/// リリースチャネル。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Channel {
    Beta,
    Rc,
    Stable,
}

impl Channel {
    /// 文字列からチャネルを解析する。
    pub fn parse(s: &str) -> Result<Self> {
        match s {
            "beta" => Ok(Self::Beta),
            "rc" => Ok(Self::Rc),
            "stable" => Ok(Self::Stable),
            other => bail!("段階は beta, rc, stable のどれか: {other}"),
        }
    }

    /// 文字列表現。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Beta => "beta",
            Self::Rc => "rc",
            Self::Stable => "stable",
        }
    }
}

/// 算出されたリリースタグと GitHub Release フラグ。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseTag {
    pub tag: String,
    pub prerelease: bool,
    pub make_latest: bool,
}

/// 既存タグ一覧から次のリリースの版と公開設定を決定する。
pub fn next_release(base: &str, channel: Channel, tags: &[&str]) -> Result<ReleaseTag> {
    let base_re = Regex::new(r"^\d+\.\d+\.\d+$")?;
    if !base_re.is_match(base) {
        bail!("mozc/VERSION は X.Y.Z の形にする: {base}");
    }
    let stable_tag = format!("v{base}");
    if tags.contains(&stable_tag.as_str()) {
        bail!("{stable_tag} は正式版として出ている。mozc/VERSION を上げる");
    }
    if channel == Channel::Stable {
        return Ok(ReleaseTag {
            tag: stable_tag,
            prerelease: false,
            make_latest: true,
        });
    }
    let pattern = format!(r"^v{}-{}\.(\d+)$", regex::escape(base), channel.as_str());
    let re = Regex::new(&pattern)?;
    let max_num = tags
        .iter()
        .filter_map(|t| re.captures(t))
        .filter_map(|c| c.get(1)?.as_str().parse::<u64>().ok())
        .max()
        .unwrap_or(0);
    Ok(ReleaseTag {
        tag: format!("v{base}-{}.{}", channel.as_str(), max_num + 1),
        prerelease: channel == Channel::Rc,
        make_latest: channel != Channel::Rc,
    })
}

/// 正式版のリリースノートに「ベータ版」が残っていないか検証する。
pub fn check_notes(channel: Channel, notes: &str) -> Result<()> {
    if channel == Channel::Stable && notes.contains("ベータ版") {
        bail!("正式版のリリースノートに「ベータ版」が残っている(mozc/release-notes.md)");
    }
    Ok(())
}

/// `git ls-remote --tags` でリモートタグ一覧を取得する。
pub fn remote_tags(repo: &str) -> Result<Vec<String>> {
    let output = Command::new("git")
        .args(["ls-remote", "--tags", repo])
        .output()
        .with_context(|| format!("git ls-remote の実行に失敗: {repo}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("git ls-remote 失敗: {stderr}");
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut tags = Vec::new();
    for line in stdout.lines() {
        if let Some((_, tag)) = line.split_once("refs/tags/") {
            if !tag.ends_with("^{}") {
                tags.push(tag.to_owned());
            }
        }
    }
    Ok(tags)
}

/// CLI 実行エントリーポイント。
pub fn run(
    repo_root: &Path,
    channel_str: &str,
    repo_url: &str,
    notes_path: Option<&Path>,
) -> Result<ReleaseTag> {
    let channel = Channel::parse(channel_str)?;
    let base = fs::read_to_string(repo_root.join("mozc/VERSION"))
        .context("mozc/VERSION を読めません")?
        .trim()
        .to_owned();
    let default_notes = repo_root.join("mozc/release-notes.md");
    let notes_file = notes_path.unwrap_or(&default_notes);
    let notes = fs::read_to_string(notes_file)
        .with_context(|| format!("リリースノートを読めません: {}", notes_file.display()))?;
    check_notes(channel, &notes)?;
    let tags = remote_tags(repo_url)?;
    let tag_refs: Vec<&str> = tags.iter().map(String::as_str).collect();
    next_release(&base, channel, &tag_refs)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TAGS: &[&str] = &[
        "v0.1.0-beta.3",
        "v0.3.0-beta.7",
        "v0.3.0-beta.8",
        "v0.3.0-beta.10x",
    ];

    #[test]
    fn beta_follows_last_beta() {
        let got = next_release("0.3.0", Channel::Beta, TAGS).unwrap();
        assert_eq!(
            got,
            ReleaseTag {
                tag: "v0.3.0-beta.9".into(),
                prerelease: false,
                make_latest: true,
            }
        );
    }

    #[test]
    fn first_rc_is_prerelease_not_latest() {
        let got = next_release("1.0.0", Channel::Rc, TAGS).unwrap();
        assert_eq!(
            got,
            ReleaseTag {
                tag: "v1.0.0-rc.1".into(),
                prerelease: true,
                make_latest: false,
            }
        );
    }

    #[test]
    fn rc_counts_only_its_own_version() {
        let mut tags = TAGS.to_vec();
        tags.extend(["v1.0.0-rc.1", "v1.0.0-rc.2", "v1.1.0-rc.5", "v1.0.0-beta.4"]);
        let got = next_release("1.0.0", Channel::Rc, &tags).unwrap();
        assert_eq!(got.tag, "v1.0.0-rc.3");
    }

    #[test]
    fn stable_release() {
        let mut tags = TAGS.to_vec();
        tags.push("v1.0.0-rc.2");
        let got = next_release("1.0.0", Channel::Stable, &tags).unwrap();
        assert_eq!(
            got,
            ReleaseTag {
                tag: "v1.0.0".into(),
                prerelease: false,
                make_latest: true,
            }
        );
    }

    #[test]
    fn nothing_after_stable_with_same_version() {
        let mut tags = TAGS.to_vec();
        tags.push("v1.0.0");
        for ch in [Channel::Beta, Channel::Rc, Channel::Stable] {
            assert!(next_release("1.0.0", ch, &tags).is_err());
        }
    }

    #[test]
    fn rejects_bad_input() {
        assert!(next_release("1.0", Channel::Rc, TAGS).is_err());
        assert!(Channel::parse("alpha").is_err());
    }

    #[test]
    fn stable_rejects_beta_notes() {
        assert!(check_notes(Channel::Stable, "Windows 版です(ベータ版、未署名)。").is_err());
        check_notes(Channel::Rc, "Windows 版です(ベータ版、未署名)。").unwrap();
        check_notes(Channel::Stable, "Windows 版です。").unwrap();
    }
}
