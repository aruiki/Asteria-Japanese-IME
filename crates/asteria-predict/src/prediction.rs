//! リアルタイム予測・前方一致検索 (9章)。

use std::collections::HashSet;

use asteria_dict::{CollocationTable, Dictionary};
use asteria_learn::UserDictionary;

#[derive(Debug, Clone, PartialEq)]
pub struct PredictionItem {
    pub reading: String,
    pub surface: String,
    pub score: i32,
}

/// 入力中の読み (カタカナ) と直前文脈から予測候補を上位 `limit` 件返す。
/// ユーザー辞書 → 連語・システム辞書前方一致の順に優先度を統合する。
pub fn predict_candidates(
    dict: &Dictionary,
    left_context: &str,
    reading_katakana: &str,
    collocations: Option<&CollocationTable>,
    user_dict: Option<&UserDictionary>,
    limit: usize,
) -> Vec<PredictionItem> {
    if reading_katakana.is_empty() || limit == 0 {
        return Vec::new();
    }
    let mut seen = HashSet::new();
    let mut items = Vec::new();

    if let Some(ud) = user_dict {
        for entry in ud.prefix_lookup(reading_katakana) {
            if seen.insert(entry.surface.clone()) {
                items.push(PredictionItem {
                    reading: entry.reading.clone(),
                    surface: entry.surface.clone(),
                    score: -10_000, // ユーザー辞書を最優先
                });
            }
        }
    }

    for m in dict.predict(reading_katakana).take(limit * 4) {
        let r = m.reading();
        for e in m.entries().take(4) {
            let s = e.surface.into_owned();
            if seen.insert(s.clone()) {
                let mut score = i32::from(e.cost);
                if let Some(col) = collocations {
                    let tail: String = left_context
                        .chars()
                        .rev()
                        .take(12)
                        .collect::<Vec<_>>()
                        .into_iter()
                        .rev()
                        .collect();
                    let combined = format!("{tail}{s}");
                    let bonus = col.score_sentence(&combined);
                    if bonus > 0.0 {
                        score -= (bonus * 1000.0) as i32;
                    }
                }
                items.push(PredictionItem {
                    reading: r.clone(),
                    surface: s,
                    score,
                });
            }
        }
    }

    items.sort_by(|a, b| {
        a.score
            .cmp(&b.score)
            .then_with(|| a.surface.cmp(&b.surface))
    });
    items.truncate(limit);
    items
}

#[cfg(test)]
mod tests {
    use super::*;
    use asteria_dict::DictBuilder;

    #[test]
    fn predicts_from_user_and_system_dictionary() {
        let mut b = DictBuilder::new();
        b.add("キョウ", "今日", 1, 1, 2000, 0).unwrap();
        b.add("キョウト", "京都", 1, 1, 1500, 0).unwrap();
        b.add("キョウカイ", "教会", 1, 1, 3000, 0).unwrap();
        b.set_connection(2, 2, vec![0; 4]).unwrap();
        let dict = Dictionary::from_bytes(b.build().unwrap()).unwrap();

        let mut ud = UserDictionary::new();
        ud.add("きょうとタワー", "京都タワー", "名詞", "");

        let col = CollocationTable::parse_txt("日本の京都\t2.0");
        let res = predict_candidates(&dict, "日本の", "キョウ", Some(&col), Some(&ud), 5);
        assert_eq!(res[0].surface, "京都タワー");
        assert!(res.iter().any(|p| p.surface == "京都"));
        assert!(res.iter().any(|p| p.surface == "今日"));
    }
}
