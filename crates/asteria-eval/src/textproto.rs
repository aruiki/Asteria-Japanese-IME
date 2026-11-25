//! Mozc `SessionHandler` の `SHOW_OUTPUT` テキストプロトコル高速パーサ（`google.protobuf.text_format` 置換）。

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SegmentInfo {
    pub key: String,
    pub value: String,
    pub annotation: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct CandidateWindowEntry {
    pub value: String,
    pub id: i64,
    pub index: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ParsedOutput {
    pub key: String,
    pub value: String,
    pub segments: Vec<SegmentInfo>,
    pub result: String,
    pub candidates: Vec<String>,
    pub callback_ms: Option<u64>,
    pub callback_has_refresh_suggestion: bool,
    pub consumed: Option<bool>,
    pub candidate_window_size: usize,
    pub candidate_window: Vec<CandidateWindowEntry>,
    pub error_code: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Block {
    Root,
    Preedit,
    PreeditSegment,
    CandidateWindow,
    CandidateWindowCandidate,
    AllCandidateWords,
    AllCandidateWordsCandidate,
    Result,
    Callback,
    Other(usize),
}

/// `SHOW_OUTPUT` テキスト（オプションの `goo.gle/debugproto` ヘッダ付き）をゼロ正規表現で高速パースする。
pub fn parse_output(raw: &str) -> Result<ParsedOutput> {
    let mut text = raw.trim();
    if let Some((first, rest)) = text.split_once('\n') {
        if matches!(
            first.trim(),
            "goo.gle/debugproto" | "goo.gle/debugonly" | "goo.gle/debugstr"
        ) {
            text = rest;
        }
    }

    let mut out = ParsedOutput::default();
    let mut stack: Vec<Block> = vec![Block::Root];
    let mut cur_seg = SegmentInfo::default();
    let mut cur_cw = CandidateWindowEntry::default();
    let mut cur_all_cand = String::new();
    let mut all_cands: Vec<String> = Vec::new();

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let top = *stack.last().unwrap_or(&Block::Root);
        if trimmed == "}" {
            match top {
                Block::PreeditSegment => out.segments.push(std::mem::take(&mut cur_seg)),
                Block::CandidateWindowCandidate => {
                    out.candidate_window.push(std::mem::take(&mut cur_cw))
                }
                Block::AllCandidateWordsCandidate
                    if !cur_all_cand.is_empty() || !all_cands.is_empty() =>
                {
                    all_cands.push(std::mem::take(&mut cur_all_cand));
                }
                _ => {}
            }
            if stack.len() > 1 {
                stack.pop();
            }
            continue;
        }
        if let Some(name) = trimmed.strip_suffix('{').map(str::trim) {
            let next = match (top, name) {
                (Block::Root, "preedit") => Block::Preedit,
                (Block::Preedit, "segment") => {
                    cur_seg = SegmentInfo::default();
                    Block::PreeditSegment
                }
                (Block::Root, "candidate_window") => Block::CandidateWindow,
                (Block::CandidateWindow, "candidate") => {
                    cur_cw = CandidateWindowEntry::default();
                    Block::CandidateWindowCandidate
                }
                (Block::Root, "all_candidate_words") => Block::AllCandidateWords,
                (Block::AllCandidateWords, "candidates") => {
                    cur_all_cand.clear();
                    Block::AllCandidateWordsCandidate
                }
                (Block::Root, "result") => Block::Result,
                (Block::Root, "callback") => Block::Callback,
                (Block::Other(d), _) => Block::Other(d + 1),
                _ => Block::Other(1),
            };
            stack.push(next);
            continue;
        }

        if let Some((field, val_raw)) = trimmed.split_once(':') {
            let field = field.trim();
            let val = val_raw.trim();
            match top {
                Block::Root => match field {
                    "error_code" => {
                        out.error_code = val.parse().unwrap_or(1);
                    }
                    "consumed" => {
                        out.consumed = match val {
                            "true" => Some(true),
                            "false" => Some(false),
                            _ => None,
                        };
                    }
                    _ => {}
                },
                Block::PreeditSegment => match field {
                    "key" => cur_seg.key = unquote_proto_str(val),
                    "value" => cur_seg.value = unquote_proto_str(val),
                    "annotation" => cur_seg.annotation = val.to_owned(),
                    _ => {}
                },
                Block::CandidateWindow => {
                    if field == "size" {
                        out.candidate_window_size = val.parse().unwrap_or(0);
                    }
                }
                Block::CandidateWindowCandidate => match field {
                    "value" => cur_cw.value = unquote_proto_str(val),
                    "id" => cur_cw.id = val.parse().unwrap_or(0),
                    "index" => cur_cw.index = val.parse().unwrap_or(0),
                    _ => {}
                },
                Block::AllCandidateWordsCandidate => {
                    if field == "value" {
                        cur_all_cand = unquote_proto_str(val);
                    }
                }
                Block::Result => {
                    if field == "value" {
                        out.result = unquote_proto_str(val);
                    }
                }
                Block::Callback => match field {
                    "delay_millisec" => {
                        out.callback_ms = val.parse().ok();
                    }
                    "type" if val == "ASTERIA_REFRESH_SUGGESTION" => {
                        out.callback_has_refresh_suggestion = true;
                    }
                    _ => {}
                },
                _ => {}
            }
        }
    }

    if out.error_code != 0 {
        bail!("native session error_code={}", out.error_code);
    }
    out.key = out.segments.iter().map(|s| s.key.as_str()).collect();
    out.value = out.segments.iter().map(|s| s.value.as_str()).collect();
    out.candidates = if !all_cands.is_empty() {
        all_cands
    } else {
        out.candidate_window
            .iter()
            .map(|c| c.value.clone())
            .collect()
    };
    Ok(out)
}

/// text_format のエスケープ（`\n`, `\t`, `\"`, `\\`, 8進 `\NNN` UTF-8 バイト）を復号する。
pub fn unquote_proto_str(raw: &str) -> String {
    let s = raw.trim();
    let inner = s
        .strip_prefix('"')
        .and_then(|x| x.strip_suffix('"'))
        .unwrap_or(s);
    if !inner.contains('\\') {
        return inner.to_owned();
    }
    let bytes = inner.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' && i + 1 < bytes.len() {
            match bytes[i + 1] {
                b'n' => {
                    out.push(b'\n');
                    i += 2;
                }
                b'r' => {
                    out.push(b'\r');
                    i += 2;
                }
                b't' => {
                    out.push(b'\t');
                    i += 2;
                }
                b'\\' => {
                    out.push(b'\\');
                    i += 2;
                }
                b'"' => {
                    out.push(b'"');
                    i += 2;
                }
                b'0'..=b'7' => {
                    let mut val = (bytes[i + 1] - b'0') as u16;
                    let mut len = 2;
                    while len < 4
                        && i + len < bytes.len()
                        && (b'0'..=b'7').contains(&bytes[i + len])
                    {
                        val = val * 8 + (bytes[i + len] - b'0') as u16;
                        len += 1;
                    }
                    out.push((val & 0xff) as u8);
                    i += len;
                }
                other => {
                    out.push(other);
                    i += 2;
                }
            }
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_textproto_output_and_octal_escapes() {
        let raw = r#"goo.gle/debugproto
consumed: true
preedit {
  segment {
    annotation: HIGHLIGHT
    value: "\346\227\245\346\234\254\350\252\236"
    key: "にほんご"
  }
}
candidate_window {
  size: 1
  candidate {
    index: 0
    value: "日本語"
    id: 0
  }
}
callback {
  delay_millisec: 200
  type: ASTERIA_REFRESH_SUGGESTION
}
"#;
        let parsed = parse_output(raw).unwrap();
        assert_eq!(parsed.key, "にほんご");
        assert_eq!(parsed.value, "日本語");
        assert_eq!(parsed.consumed, Some(true));
        assert_eq!(parsed.callback_ms, Some(200));
        assert!(parsed.callback_has_refresh_suggestion);
        assert_eq!(parsed.candidates, vec!["日本語"]);
    }
}
