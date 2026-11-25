//! 表記揺れ正規化テーブル (605 件, `asteria_notation_table.inc` 移植)。

use std::collections::HashMap;
use std::sync::OnceLock;

const NOTATION_TSV: &str = include_str!("../data/notation_table.tsv");

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NotationRule {
    pub nonstandard: &'static str,
    pub standard: &'static str,
    pub majority_ratio: f32,
}

fn notation_map() -> &'static HashMap<&'static str, NotationRule> {
    static MAP: OnceLock<HashMap<&'static str, NotationRule>> = OnceLock::new();
    MAP.get_or_init(|| {
        let mut m = HashMap::new();
        for line in NOTATION_TSV.lines() {
            let mut cols = line.split('\t');
            let (Some(nonstd), Some(std_form), Some(ratio_s)) =
                (cols.next(), cols.next(), cols.next())
            else {
                continue;
            };
            if let Ok(ratio) = ratio_s.trim().parse::<f32>() {
                let key = nonstd.trim();
                let rule = NotationRule {
                    nonstandard: key,
                    standard: std_form.trim(),
                    majority_ratio: ratio,
                };
                m.entry(key)
                    .and_modify(|existing: &mut NotationRule| {
                        if rule.majority_ratio > existing.majority_ratio {
                            *existing = rule;
                        }
                    })
                    .or_insert(rule);
            }
        }
        m
    })
}

pub fn notation_rule_count() -> usize {
    notation_map().len()
}

/// 候補文に非標準・揺れ表記が含まれる場合、標準表記に置き換えた文と多数派割合を返す。
pub fn normalize_notation(text: &str) -> Option<(String, f32)> {
    let map = notation_map();
    let mut best: Option<(String, f32)> = None;
    for (&nonstd, rule) in map {
        if let Some(pos) = text.find(nonstd) {
            let mut replaced = text.to_owned();
            replaced.replace_range(pos..pos + nonstd.len(), rule.standard);
            if best
                .as_ref()
                .map_or(true, |(_, prev_ratio)| rule.majority_ratio > *prev_ratio)
            {
                best = Some((replaced, rule.majority_ratio));
            }
        }
    }
    best
}

/// 候補リストに揺れの標準表記を上位補完または付与する。
pub fn promote_standard_notations(candidates: &mut Vec<String>) -> bool {
    let mut insertions = Vec::new();
    for (idx, cand) in candidates.iter().enumerate() {
        if let Some((std_text, ratio)) = normalize_notation(cand) {
            if ratio >= 0.80
                && !candidates.contains(&std_text)
                && !insertions.iter().any(|(_, s)| s == &std_text)
            {
                insertions.push((idx + 1, std_text));
            }
        }
    }
    let changed = !insertions.is_empty();
    for (offset, (idx, s)) in insertions.into_iter().enumerate() {
        let pos = (idx + offset).min(candidates.len());
        candidates.insert(pos, s);
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_rules_and_normalizes_variants() {
        assert!(notation_rule_count() >= 600);
        let (norm, ratio) = normalize_notation("取り敢えず行く").unwrap();
        assert_eq!(norm, "とりあえず行く");
        assert!(ratio > 0.9);
    }
}
