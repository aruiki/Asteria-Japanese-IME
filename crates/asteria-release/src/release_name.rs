//! GitHub の表示名を「版・段階・日本時間の日付」の順にそろえる（`mozc/tools/release_name.py` 移植）。

use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{bail, Result};
use regex::Regex;

/// YYYY-MM-DD の日付表現。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SimpleDate {
    pub year: i32,
    pub month: u8,
    pub day: u8,
}

impl SimpleDate {
    /// `YYYY-MM-DD` 形式の文字列をパースする。
    pub fn parse(s: &str) -> Result<Self> {
        let parts: Vec<&str> = s.split('-').collect();
        if parts.len() != 3 || parts[0].len() != 4 || parts[1].len() != 2 || parts[2].len() != 2 {
            bail!("日付は YYYY-MM-DD 形式で指定してください: {s}");
        }
        let year: i32 = parts[0].parse()?;
        let month: u8 = parts[1].parse()?;
        let day: u8 = parts[2].parse()?;
        if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
            bail!("不正な日付です: {s}");
        }
        Ok(Self { year, month, day })
    }

    /// 現在の日本標準時（UTC+9）の日付を返す。
    pub fn today_jst() -> Self {
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let jst_secs = secs + 9 * 3600;
        let days = jst_secs.div_euclid(86_400);
        civil_from_days(days)
    }
}

impl fmt::Display for SimpleDate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

/// Howard Hinnant のアルゴリズムで 1970-01-01 からの日数からグレゴリオ暦を計算する。
fn civil_from_days(z: i64) -> SimpleDate {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = (y + if m <= 2 { 1 } else { 0 }) as i32;
    SimpleDate {
        year,
        month: m as u8,
        day: d as u8,
    }
}

/// タグと公開日から GitHub Releases のタイトルを生成する。
pub fn release_name(tag: &str, published_on: SimpleDate) -> Result<String> {
    let re = Regex::new(r"^v\d+\.\d+\.\d+(?:-(?:(beta|rc)\.([1-9]\d*)|pr([1-9]\d*)))?$")?;
    let Some(caps) = re.captures(tag) else {
        bail!("対応していないリリースタグ: {tag}");
    };
    let stage = if caps.get(3).is_some() {
        "開発確認用".to_owned()
    } else if let (Some(ch), Some(num)) = (caps.get(1), caps.get(2)) {
        match ch.as_str() {
            "beta" => format!("ベータ版{}", num.as_str()),
            "rc" => format!("リリース候補{}", num.as_str()),
            _ => unreachable!(),
        }
    } else {
        "正式版".to_owned()
    };
    Ok(format!("Asteria {tag} — {stage}（{published_on}）"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brand_version_and_stage_for_each_stage() {
        let d = SimpleDate {
            year: 2026,
            month: 10,
            day: 9,
        };
        for (tag, stage) in [
            ("v1.0.0", "正式版"),
            ("v0.3.0-beta.8", "ベータ版8"),
            ("v1.1.0-rc.3", "リリース候補3"),
            ("v0.0.0-pr146", "開発確認用"),
        ] {
            assert_eq!(
                release_name(tag, d).unwrap(),
                format!("Asteria {tag} — {stage}（2026-10-09）")
            );
        }
    }

    #[test]
    fn keeps_original_version_and_publication_date() {
        let d = SimpleDate {
            year: 2026,
            month: 1,
            day: 2,
        };
        assert_eq!(
            release_name("v12.34.56-rc.123", d).unwrap(),
            "Asteria v12.34.56-rc.123 — リリース候補123（2026-01-02）"
        );
    }

    #[test]
    fn rejects_ambiguous_tags() {
        let d = SimpleDate {
            year: 2026,
            month: 10,
            day: 9,
        };
        for tag in ["1.1.0", "v1.1", "v1.1.0-rc.0", "v1.1.0-rc.3x", "v1.1.0\n"] {
            assert!(release_name(tag, d).is_err(), "tag={tag:?}");
        }
    }
}
