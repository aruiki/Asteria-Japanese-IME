//! TSF TIP 文字列・表示属性・入力スコープの純粋ロジック (`frontends/windows/src/text.cpp` 移植)。

use crate::guids::{GUID_ATTR_CONVERTED, GUID_ATTR_FOCUSED, GUID_ATTR_INPUT};

pub const MAX_LEFT_CONTEXT: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreeditSegment {
    pub text: String,
    pub attribute: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompositionRange {
    pub begin: i32,
    pub end: i32,
    pub attribute: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Composition {
    pub text_utf16: Vec<u16>,
    pub ranges: Vec<CompositionRange>,
    pub cursor_utf16: i32,
}

/// IPC `SegmentAttribute` (0=入力中, 1=変換済み, 2=注目文節) に対応する GUID バイト列を返す。
pub const fn attribute_guid_bytes(attribute: u32) -> [u8; 16] {
    match attribute {
        1 => GUID_ATTR_CONVERTED,
        2 => GUID_ATTR_FOCUSED,
        _ => GUID_ATTR_INPUT,
    }
}

/// Windows `InputScope` がパスワード・PIN・非公開欄を示すか判定する (REQ-10-3)。
/// `IS_PASSWORD=31`, `IS_PRIVATE=61`, `IS_NUMERIC_PASSWORD=63`, `IS_NUMERIC_PIN=64`, `IS_ALPHANUMERIC_PIN=65`.
pub fn is_private_input_scope(scopes: &[i32]) -> bool {
    scopes.iter().any(|s| matches!(*s, 31 | 61 | 63 | 64 | 65))
}

/// UTF-16 スライスから末尾の最大 `max_chars` 文字 (サロゲートペア境界を保持) を返す。
pub fn last_chars_utf16(units: &[u16], max_chars: usize) -> Vec<u16> {
    let mut begin = units.len();
    let mut count = 0;
    while count < max_chars && begin > 0 {
        begin -= 1;
        if begin > 0
            && (0xDC00..=0xDFFF).contains(&units[begin])
            && (0xD800..=0xDBFF).contains(&units[begin - 1])
        {
            begin -= 1;
        }
        count += 1;
    }
    if begin < units.len() && (0xDC00..=0xDFFF).contains(&units[begin]) {
        begin += 1;
    }
    units[begin..].to_vec()
}

pub fn build_composition(segments: &[PreeditSegment], cursor_chars: usize) -> Composition {
    let mut text_utf16 = Vec::new();
    let mut ranges = Vec::with_capacity(segments.len());
    for seg in segments {
        let begin = text_utf16.len() as i32;
        text_utf16.extend(seg.text.encode_utf16());
        let end = text_utf16.len() as i32;
        ranges.push(CompositionRange {
            begin,
            end,
            attribute: seg.attribute,
        });
    }
    let full = segments.iter().map(|s| s.text.as_str()).collect::<String>();
    let cursor_utf16: usize = full.chars().take(cursor_chars).map(char::len_utf16).sum();
    Composition {
        text_utf16,
        ranges,
        cursor_utf16: cursor_utf16 as i32,
    }
}

pub fn focused_range(comp: &Composition) -> (i32, i32) {
    for r in &comp.ranges {
        if r.attribute == 2 {
            return (r.begin, r.end);
        }
    }
    (0, comp.text_utf16.len() as i32)
}

pub fn is_converting(preedit: &[PreeditSegment]) -> bool {
    preedit.iter().any(|s| s.attribute != 0)
}

/// IME オン/オフ切替キー (半角/全角 `0xF3`/`0xF4`, `VK_KANJI` `0x19`)。
pub const fn is_open_close_key(vk: u32) -> bool {
    matches!(vk, 0xF3 | 0xF4 | 0x19)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn composition_and_private_scope_and_surrogate_trimming() {
        assert!(is_private_input_scope(&[0, 31]));
        assert!(is_private_input_scope(&[64]));
        assert!(!is_private_input_scope(&[0, 1, 5]));

        let segs = vec![
            PreeditSegment {
                text: "今日".into(),
                attribute: 2,
            },
            PreeditSegment {
                text: "は🌸".into(),
                attribute: 1,
            },
        ];
        let comp = build_composition(&segs, 3);
        assert_eq!(focused_range(&comp), (0, 2));
        assert!(is_converting(&segs));

        let trimmed = last_chars_utf16(&comp.text_utf16, 2);
        let s = String::from_utf16(&trimmed).unwrap();
        assert_eq!(s, "は🌸");
        assert!(is_open_close_key(0xF3));
        assert!(!is_open_close_key(0x20));
    }
}
