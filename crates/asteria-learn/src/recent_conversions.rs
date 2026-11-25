//! 直近の変換記録 (最大 20 件、誤変換報告ウィザード用: `asteria_recent_conversions.{h,cc}` 移植)。

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use asteria_proto::ipc::InputScope;

pub const MAX_RECENT_ENTRIES: usize = 20;
pub const MAX_RECENT_CANDIDATES: usize = 40;
pub const MAX_RECENT_CHARS: usize = 96;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecentConversion {
    pub time: String,
    pub first_kept: bool,
    pub reading: String,
    pub committed: String,
    pub preceding: String,
    pub candidates: Vec<String>,
}

fn truncate_tail_chars(s: &str, max_chars: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max_chars {
        s.to_owned()
    } else {
        chars[chars.len() - max_chars..].iter().collect()
    }
}

/// 最小の RFC 4648 base64 エンコーダ。
fn base64_encode(input: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b0 = u32::from(chunk[0]);
        let b1 = chunk.get(1).copied().map(u32::from).unwrap_or(0);
        let b2 = chunk.get(2).copied().map(u32::from).unwrap_or(0);
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[((n >> 18) & 0x3F) as usize] as char);
        out.push(TABLE[((n >> 12) & 0x3F) as usize] as char);
        out.push(if chunk.len() > 1 {
            TABLE[((n >> 6) & 0x3F) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[(n & 0x3F) as usize] as char
        } else {
            '='
        });
    }
    out
}

fn base64_val(b: u8) -> Option<u32> {
    match b {
        b'A'..=b'Z' => Some(u32::from(b - b'A')),
        b'a'..=b'z' => Some(u32::from(b - b'a' + 26)),
        b'0'..=b'9' => Some(u32::from(b - b'0' + 52)),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

fn base64_decode(s: &str) -> Option<Vec<u8>> {
    let bytes = s.as_bytes();
    if bytes.len() % 4 != 0 {
        return None;
    }
    let mut out = Vec::with_capacity(bytes.len() / 4 * 3);
    for chunk in bytes.chunks_exact(4) {
        let a = base64_val(chunk[0])?;
        let b = base64_val(chunk[1])?;
        let pad1 = chunk[2] == b'=';
        let pad2 = chunk[3] == b'=';
        let c = if pad1 { 0 } else { base64_val(chunk[2])? };
        let d = if pad2 { 0 } else { base64_val(chunk[3])? };
        let n = (a << 18) | (b << 12) | (c << 6) | d;
        out.push(((n >> 16) & 0xFF) as u8);
        if !pad1 {
            out.push(((n >> 8) & 0xFF) as u8);
        }
        if !pad2 {
            out.push((n & 0xFF) as u8);
        }
    }
    Some(out)
}

#[derive(Debug)]
pub struct RecentConversions {
    path: Option<PathBuf>,
    entries: Mutex<Vec<RecentConversion>>,
}

impl RecentConversions {
    pub fn in_memory() -> Self {
        Self {
            path: None,
            entries: Mutex::new(Vec::new()),
        }
    }

    pub fn open(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let initial = fs::read_to_string(&path)
            .map(|s| Self::parse(&s))
            .unwrap_or_default();
        Self {
            path: Some(path),
            entries: Mutex::new(initial),
        }
    }

    /// プライバシー境界付きで記録 (Password/Incognito では何もしない)。
    pub fn record_checked(&self, scope: InputScope, incognito: bool, entry: RecentConversion) {
        if incognito || scope == InputScope::Password {
            return;
        }
        self.record(entry);
    }

    pub fn record(&self, mut entry: RecentConversion) {
        entry.preceding = truncate_tail_chars(&entry.preceding, MAX_RECENT_CHARS);
        entry.candidates.truncate(MAX_RECENT_CANDIDATES);
        let Ok(mut guard) = self.entries.lock() else {
            return;
        };
        guard.insert(0, entry);
        guard.truncate(MAX_RECENT_ENTRIES);
        let body = Self::serialize(&guard);
        drop(guard);
        if let Some(p) = &self.path {
            let _ = write_atomic(p, &body);
        }
    }

    pub fn recent(&self) -> Vec<RecentConversion> {
        self.entries.lock().map(|g| g.clone()).unwrap_or_default()
    }

    pub fn clear(&self) {
        if let Ok(mut guard) = self.entries.lock() {
            guard.clear();
        }
        if let Some(p) = &self.path {
            let _ = fs::remove_file(p);
        }
    }

    pub fn serialize(entries: &[RecentConversion]) -> String {
        let mut out = String::new();
        for e in entries {
            let joined_cands = e.candidates.join("\x1f");
            let line = format!(
                "{}\t{}\t{}\t{}\t{}\t{}\n",
                e.time,
                if e.first_kept { '1' } else { '0' },
                base64_encode(e.reading.as_bytes()),
                base64_encode(e.committed.as_bytes()),
                base64_encode(e.preceding.as_bytes()),
                base64_encode(joined_cands.as_bytes()),
            );
            out.push_str(&line);
        }
        out
    }

    pub fn parse(text: &str) -> Vec<RecentConversion> {
        let mut out = Vec::new();
        for line in text.lines() {
            if line.is_empty() {
                continue;
            }
            let cols: Vec<&str> = line.split('\t').collect();
            if cols.len() != 6 {
                continue;
            }
            let Some(reading) = base64_decode(cols[2]).and_then(|b| String::from_utf8(b).ok())
            else {
                continue;
            };
            let Some(committed) = base64_decode(cols[3]).and_then(|b| String::from_utf8(b).ok())
            else {
                continue;
            };
            let Some(preceding) = base64_decode(cols[4]).and_then(|b| String::from_utf8(b).ok())
            else {
                continue;
            };
            let Some(cands_raw) = base64_decode(cols[5]).and_then(|b| String::from_utf8(b).ok())
            else {
                continue;
            };
            let candidates = if cands_raw.is_empty() {
                Vec::new()
            } else {
                cands_raw.split('\x1f').map(ToOwned::to_owned).collect()
            };
            out.push(RecentConversion {
                time: cols[0].to_owned(),
                first_kept: cols[1] == "1",
                reading,
                committed,
                preceding,
                candidates,
            });
            if out.len() >= MAX_RECENT_ENTRIES {
                break;
            }
        }
        out
    }
}

fn write_atomic(p: &Path, body: &str) -> std::io::Result<()> {
    if let Some(parent) = p.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut tmp = p.as_os_str().to_os_string();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    fs::write(&tmp, body)?;
    fs::rename(tmp, p)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_limits_and_password_guard() {
        let rc = RecentConversions::in_memory();
        rc.record_checked(
            InputScope::Password,
            false,
            RecentConversion {
                time: "2026-11-25T21:00:00+09:00".into(),
                first_kept: true,
                reading: "ひみつ".into(),
                committed: "秘密".into(),
                preceding: "".into(),
                candidates: vec!["秘密".into()],
            },
        );
        assert!(rc.recent().is_empty());

        for i in 0..25 {
            rc.record_checked(
                InputScope::Default,
                false,
                RecentConversion {
                    time: format!("2026-11-25T21:{i:02}:00+09:00"),
                    first_kept: i % 2 == 0,
                    reading: format!("よみ{i}"),
                    committed: format!("確定\t{i}\n改行"),
                    preceding: "あ".repeat(120),
                    candidates: vec!["A".into(), "B".into()],
                },
            );
        }
        let entries = rc.recent();
        assert_eq!(entries.len(), MAX_RECENT_ENTRIES);
        assert_eq!(entries[0].reading, "よみ24");
        assert_eq!(entries[0].preceding.chars().count(), MAX_RECENT_CHARS);

        let serialized = RecentConversions::serialize(&entries);
        let parsed = RecentConversions::parse(&serialized);
        assert_eq!(parsed, entries);
    }
}
