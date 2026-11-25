//! 補助辞書読み込み (`short_lexicon.tsv`, `name_lexicon.tsv`, `fast_collocations.txt`, `AsteriaEnglish.tsv`).

use std::collections::{HashMap, HashSet};

/// 補助語彙 TSV の 1 エントリ。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuxLexiconEntry {
    pub reading: String,
    pub surface: String,
    pub lid: u16,
    pub rid: u16,
    pub cost: i16,
    pub flags: u16,
}

/// カタカナ語彙のデフォルト文脈 ID (`名詞,一般` 相当)。
const DEFAULT_NOUN_ID: u16 = 1851;
const DEFAULT_AUX_COST: i16 = 4500;

/// ひらがな (`ぁ`..`ゖ`) をカタカナへ正規化する。
fn hiragana_to_katakana(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '\u{3041}'..='\u{3096}' => char::from_u32(c as u32 + 0x60).unwrap_or(c),
            _ => c,
        })
        .collect()
}

/// `short_lexicon.tsv` や `name_lexicon.tsv` を読み込む。
/// 5 列以上 (`reading\tlid\trid\tcost\tsurface[\tflags]`) と
/// 2-3 列 (`reading\tsurface[\tcost]`) の両方に対応する。
pub fn parse_aux_lexicon_tsv(
    text: &str,
    default_flags: u16,
) -> Result<Vec<AuxLexiconEntry>, String> {
    let mut entries = Vec::new();
    for (idx, raw_line) in text.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let cols: Vec<&str> = line.split('\t').collect();
        let entry = match cols.as_slice() {
            [reading, lid, rid, cost, surface] => AuxLexiconEntry {
                reading: hiragana_to_katakana(reading),
                surface: (*surface).to_owned(),
                lid: lid
                    .parse()
                    .map_err(|e| format!("line {}: invalid lid: {e}", idx + 1))?,
                rid: rid
                    .parse()
                    .map_err(|e| format!("line {}: invalid rid: {e}", idx + 1))?,
                cost: cost
                    .parse()
                    .map_err(|e| format!("line {}: invalid cost: {e}", idx + 1))?,
                flags: default_flags,
            },
            [reading, lid, rid, cost, surface, flags_col] => {
                let flags = flags_col.parse::<u16>().unwrap_or(default_flags);
                AuxLexiconEntry {
                    reading: hiragana_to_katakana(reading),
                    surface: (*surface).to_owned(),
                    lid: lid
                        .parse()
                        .map_err(|e| format!("line {}: invalid lid: {e}", idx + 1))?,
                    rid: rid
                        .parse()
                        .map_err(|e| format!("line {}: invalid rid: {e}", idx + 1))?,
                    cost: cost
                        .parse()
                        .map_err(|e| format!("line {}: invalid cost: {e}", idx + 1))?,
                    flags: flags & 0x0F,
                }
            }
            [reading, surface] => AuxLexiconEntry {
                reading: hiragana_to_katakana(reading),
                surface: (*surface).to_owned(),
                lid: DEFAULT_NOUN_ID,
                rid: DEFAULT_NOUN_ID,
                cost: DEFAULT_AUX_COST,
                flags: default_flags,
            },
            [reading, surface, cost] => AuxLexiconEntry {
                reading: hiragana_to_katakana(reading),
                surface: (*surface).to_owned(),
                lid: DEFAULT_NOUN_ID,
                rid: DEFAULT_NOUN_ID,
                cost: cost
                    .parse()
                    .map_err(|e| format!("line {}: invalid cost: {e}", idx + 1))?,
                flags: default_flags,
            },
            _ => {
                return Err(format!(
                    "line {}: unexpected column count {}",
                    idx + 1,
                    cols.len()
                ))
            }
        };
        if !entry.reading.is_empty() && !entry.surface.is_empty() {
            entries.push(entry);
        }
    }
    Ok(entries)
}

/// 高速連語テーブル (`fast_collocations.txt`)。文全体の候補に連語が含まれれば加点・保持する。
#[derive(Debug, Clone, Default)]
pub struct CollocationTable {
    phrases: HashSet<String>,
    weights: HashMap<String, f32>,
}

impl CollocationTable {
    /// `phrase` または `phrase\tweight` の行を読み込む。
    pub fn parse_txt(text: &str) -> Self {
        let mut out = Self::default();
        for raw_line in text.lines() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((phrase, w_str)) = line.split_once('\t') {
                let phrase = phrase.trim();
                if phrase.is_empty() {
                    continue;
                }
                let weight = w_str.trim().parse::<f32>().unwrap_or(1.0);
                out.phrases.insert(phrase.to_owned());
                out.weights.insert(phrase.to_owned(), weight);
            } else {
                out.phrases.insert(line.to_owned());
                out.weights.insert(line.to_owned(), 1.0);
            }
        }
        out
    }

    pub fn len(&self) -> usize {
        self.phrases.len()
    }

    pub fn is_empty(&self) -> bool {
        self.phrases.is_empty()
    }

    pub fn contains(&self, phrase: &str) -> bool {
        self.phrases.contains(phrase)
    }

    /// 候補文 `sentence` に含まれる連語重みの合計を返す。
    pub fn score_sentence(&self, sentence: &str) -> f32 {
        self.weights
            .iter()
            .filter(|(phrase, _)| sentence.contains(phrase.as_str()))
            .map(|(_, &w)| w)
            .sum()
    }
}

/// カタカナ英語辞書 (`AsteriaEnglish.tsv`)。
#[derive(Debug, Clone, Default)]
pub struct EnglishLexicon {
    /// カタカナ読み -> 英単語リスト
    by_katakana: HashMap<String, Vec<String>>,
    /// 英小文字 -> 代表英語表記 (大文字混在を優先)
    by_lower_english: HashMap<String, String>,
}

fn is_ascii_letters(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphabetic())
}

fn has_ascii_upper(s: &str) -> bool {
    s.bytes().any(|b| b.is_ascii_uppercase())
}

impl EnglishLexicon {
    /// `katakana\tenglish` の TSV を読み込む。
    pub fn parse_tsv(text: &str) -> Self {
        let mut out = Self::default();
        for raw_line in text.lines() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((kana, eng)) = line.split_once('\t') else {
                continue;
            };
            let kana = hiragana_to_katakana(kana.trim());
            let eng = eng.trim();
            if kana.is_empty() || eng.is_empty() {
                continue;
            }
            let list = out.by_katakana.entry(kana).or_default();
            if !list.iter().any(|x| x == eng) {
                list.push(eng.to_owned());
            }
            if eng.len() >= 5 && eng.len() <= 24 && is_ascii_letters(eng) {
                let lower = eng.to_ascii_lowercase();
                out.by_lower_english
                    .entry(lower)
                    .and_modify(|existing| {
                        if !has_ascii_upper(existing) && has_ascii_upper(eng) {
                            *existing = eng.to_owned();
                        }
                    })
                    .or_insert_with(|| eng.to_owned());
            }
        }
        out
    }

    /// カタカナ語に対応する英単語一覧を返す。
    pub fn english_for_katakana(&self, katakana: &str) -> &[String] {
        self.by_katakana
            .get(&hiragana_to_katakana(katakana))
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    /// 英小文字に対する正規英語表記を返す (ADR0080 用)。
    pub fn canonical_english(&self, lower_ascii: &str) -> Option<&str> {
        self.by_lower_english
            .get(&lower_ascii.to_ascii_lowercase())
            .map(String::as_str)
    }

    pub fn len_katakana(&self) -> usize {
        self.by_katakana.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_short_lexicon_and_collocations_and_english() {
        let tsv = "# comment\nきゅうとう\t1851\t1851\t3200\t急騰\nさくら\t桜\t4000\n";
        let items = parse_aux_lexicon_tsv(tsv, crate::flags::PROPER_NOUN).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].reading, "キュウトウ");
        assert_eq!(items[0].surface, "急騰");
        assert_eq!(items[1].cost, 4000);
        assert_eq!(items[1].flags, crate::flags::PROPER_NOUN);

        let col = CollocationTable::parse_txt("電車に乗る\t1.5\n朝ご飯\n");
        assert_eq!(col.len(), 2);
        assert!(col.contains("電車に乗る"));
        assert!((col.score_sentence("朝電車に乗る。") - 1.5).abs() < 1e-6);

        let eng = EnglishLexicon::parse_tsv("インテル\tintel\nインテル\tIntel\nバズ\tbuzz\n");
        assert_eq!(eng.english_for_katakana("インテル"), ["intel", "Intel"]);
        assert_eq!(eng.canonical_english("intel"), Some("Intel"));
        assert_eq!(
            eng.canonical_english("buzz"),
            None,
            "5文字未満は一般辞書から除外"
        );
    }
}
