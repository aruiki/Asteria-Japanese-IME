//! Mozc ローマ字テーブル逆引きおよび imebench 用ローマ字変換。

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use anyhow::{bail, Context, Result};
use unicode_normalization::UnicodeNormalization;

use crate::kana::kata_to_hira;

/// Mozc の `romanji-hiragana.tsv` から構築した逆引きローマ字テーブル。
#[derive(Debug, Clone)]
pub struct RomanTable {
    entries: Vec<(String, String)>,
}

impl RomanTable {
    /// TSV ファイルを読み込んでテーブルを構築する。
    pub fn from_tsv_file(path: &Path) -> Result<Self> {
        let text = fs::read_to_string(path)
            .with_context(|| format!("ローマ字テーブルを読めません: {}", path.display()))?;
        Ok(Self::from_tsv_str(&text))
    }

    /// TSV 文字列からテーブルを構築する。
    pub fn from_tsv_str(text: &str) -> Self {
        let mut inverse: HashMap<String, String> = HashMap::new();
        for line in text.lines() {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() < 2 || (parts.len() > 2 && !parts[2].is_empty()) || parts[1].is_empty() {
                continue;
            }
            let key = parts[0];
            let val = parts[1];
            if !key.is_ascii() || key.chars().any(char::is_whitespace) {
                continue;
            }
            let replace = match inverse.get(val) {
                None => true,
                Some(prev) => (key.len(), key) < (prev.len(), prev.as_str()),
            };
            if replace {
                inverse.insert(val.to_owned(), key.to_owned());
            }
        }
        for (k, v) in [
            ("ん", "n'"),
            ("っ", "xtu"),
            ("ー", "-"),
            ("？", "?"),
            ("！", "!"),
            ("（", "("),
            ("）", ")"),
        ] {
            inverse.insert(k.to_owned(), v.to_owned());
        }
        let mut entries: Vec<(String, String)> = inverse.into_iter().collect();
        // 最長一致・辞書順で並べる（文字数降順）。
        entries.sort_by(|a, b| {
            b.0.chars()
                .count()
                .cmp(&a.0.chars().count())
                .then_with(|| a.0.cmp(&b.0))
        });
        Self { entries }
    }

    /// 読み文字列をローマ字キー列へ変換する。`allow_ascii_digits` が true なら ASCII 数字をそのまま通す。
    pub fn romanize(&self, reading: &str, allow_ascii_digits: bool) -> Result<String> {
        let mut out = String::new();
        let mut rest = reading;
        while !rest.is_empty() {
            if let Some((surface, key)) = self
                .entries
                .iter()
                .find(|(surface, _)| rest.starts_with(surface.as_str()))
            {
                out.push_str(key);
                rest = &rest[surface.len()..];
            } else if allow_ascii_digits {
                let Some(ch) = rest.chars().next() else {
                    break;
                };
                if ch.is_ascii_digit() {
                    out.push(ch);
                    rest = &rest[ch.len_utf8()..];
                } else {
                    let preview: String = rest.chars().take(8).collect();
                    bail!("対応しない読み: {preview:?}");
                }
            } else {
                let preview: String = rest.chars().take(8).collect();
                bail!("対応しない読み: {preview:?}");
            }
        }
        Ok(out)
    }
}

/// `eval/imebench/imebench.py` の `to_romaji` 互換変換（固定ローマ字表）。
/// 打てない文字が含まれる場合は `None` を返す。
pub fn imebench_to_romaji(reading: &str) -> Option<String> {
    let normalized = kata_to_hira(reading).nfkc().collect::<String>();
    let mut out = String::new();
    for ch in normalized.chars() {
        let h = kata_to_hira(&ch.to_string());
        let Some(c) = h.chars().next() else {
            continue;
        };
        if let Some(rom) = imebench_base_char(c) {
            out.push_str(rom);
        } else if c.is_ascii_digit() || matches!(c, ',' | '.' | '!' | '?' | '(' | ')') {
            out.push(c);
        } else {
            return None;
        }
    }
    Some(out)
}

fn imebench_base_char(c: char) -> Option<&'static str> {
    Some(match c {
        'あ' => "a",
        'い' => "i",
        'う' => "u",
        'え' => "e",
        'お' => "o",
        'か' => "ka",
        'き' => "ki",
        'く' => "ku",
        'け' => "ke",
        'こ' => "ko",
        'さ' => "sa",
        'し' => "si",
        'す' => "su",
        'せ' => "se",
        'そ' => "so",
        'た' => "ta",
        'ち' => "ti",
        'つ' => "tu",
        'て' => "te",
        'と' => "to",
        'な' => "na",
        'に' => "ni",
        'ぬ' => "nu",
        'ね' => "ne",
        'の' => "no",
        'は' => "ha",
        'ひ' => "hi",
        'ふ' => "hu",
        'へ' => "he",
        'ほ' => "ho",
        'ま' => "ma",
        'み' => "mi",
        'む' => "mu",
        'め' => "me",
        'も' => "mo",
        'や' => "ya",
        'ゆ' => "yu",
        'よ' => "yo",
        'ら' => "ra",
        'り' => "ri",
        'る' => "ru",
        'れ' => "re",
        'ろ' => "ro",
        'わ' => "wa",
        'を' => "wo",
        'ん' => "nn",
        'ゐ' => "wyi",
        'ゑ' => "wye",
        'が' => "ga",
        'ぎ' => "gi",
        'ぐ' => "gu",
        'げ' => "ge",
        'ご' => "go",
        'ざ' => "za",
        'じ' => "zi",
        'ず' => "zu",
        'ぜ' => "ze",
        'ぞ' => "zo",
        'だ' => "da",
        'ぢ' => "di",
        'づ' => "du",
        'で' => "de",
        'ど' => "do",
        'ば' => "ba",
        'び' => "bi",
        'ぶ' => "bu",
        'べ' => "be",
        'ぼ' => "bo",
        'ぱ' => "pa",
        'ぴ' => "pi",
        'ぷ' => "pu",
        'ぺ' => "pe",
        'ぽ' => "po",
        'ぁ' => "xa",
        'ぃ' => "xi",
        'ぅ' => "xu",
        'ぇ' => "xe",
        'ぉ' => "xo",
        'ゃ' => "xya",
        'ゅ' => "xyu",
        'ょ' => "xyo",
        'ゎ' => "xwa",
        'っ' => "xtu",
        'ゔ' => "vu",
        'ー' => "-",
        '、' => ",",
        '。' => ".",
        '・' => "/",
        '「' => "[",
        '」' => "]",
        '！' => "!",
        '？' => "?",
        '〜' | '～' => "~",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roman_table_handles_n_small_tsu_and_rejects_control_chars() {
        let tsv = "a\tあ\nn\tん\nnn\tん\nkka\tっ\tk\n.\t。\n";
        let table = RomanTable::from_tsv_str(tsv);
        assert_eq!(table.romanize("あんっ。", true).unwrap(), "an'xtu.");
        assert!(table.romanize("あ\nSEND_KEY", true).is_err());
    }

    #[test]
    fn imebench_romaji_matches_python() {
        assert_eq!(imebench_to_romaji("にほんご").as_deref(), Some("nihonngo"));
        assert_eq!(imebench_to_romaji("カタカナ").as_deref(), Some("katakana"));
        assert_eq!(imebench_to_romaji("abc"), None);
    }
}
