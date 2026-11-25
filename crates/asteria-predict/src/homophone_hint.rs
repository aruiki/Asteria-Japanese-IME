//! 同音語ヒント付与 (docs/adr/0079, `asteria_homophone_hint_rewriter.{h,cc}` 移植)。

use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

use asteria_learn::default_context_entries;

const MAX_HINT_CUES: usize = 2;

#[derive(Debug, Clone)]
struct HintRule {
    rejected: &'static str,
    text: String,
}

fn hint_map() -> &'static HashMap<(&'static str, &'static str), Vec<HintRule>> {
    static MAP: OnceLock<HashMap<(&'static str, &'static str), Vec<HintRule>>> = OnceLock::new();
    MAP.get_or_init(|| {
        let mut map: HashMap<(&'static str, &'static str), Vec<HintRule>> = HashMap::new();
        for entry in default_context_entries() {
            let cues: Vec<&str> = entry
                .cues
                .split('、')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .take(MAX_HINT_CUES)
                .collect();
            if cues.is_empty() {
                continue;
            }
            map.entry((entry.reading, entry.chosen))
                .or_default()
                .push(HintRule {
                    rejected: entry.rejected,
                    text: cues.join("・"),
                });
        }
        map
    })
}

/// 候補1件と説明文の組。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HintedCandidate {
    pub reading: String,
    pub surface: String,
    pub description: Option<String>,
}

/// 同じ文節の候補集合内に競合する同音語がある場合のみ、意味識別用の手がかり語を説明に付与する。
/// 候補の並び順は一切変更しない。
pub fn apply_homophone_hints(candidates: &mut [HintedCandidate], enabled: bool) -> bool {
    if !enabled || candidates.is_empty() {
        return false;
    }
    let map = hint_map();
    let present: HashSet<String> = candidates.iter().map(|c| c.surface.clone()).collect();
    let mut changed = false;

    for cand in candidates {
        let reading_hira = katakana_to_hiragana(&cand.reading);
        let Some(rules) = map.get(&(reading_hira.as_str(), cand.surface.as_str())) else {
            continue;
        };
        for rule in rules {
            if !present.contains(rule.rejected) {
                continue;
            }
            let already = cand
                .description
                .as_deref()
                .is_some_and(|d| d.contains(&rule.text));
            if already {
                break;
            }
            cand.description = Some(match &cand.description {
                None => rule.text.clone(),
                Some(prev) if prev.is_empty() => rule.text.clone(),
                Some(prev) => format!("{prev} {}", rule.text),
            });
            changed = true;
            break;
        }
    }
    changed
}

fn katakana_to_hiragana(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '\u{30A1}'..='\u{30F6}' => char::from_u32(c as u32 - 0x60).unwrap_or(c),
            _ => c,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attaches_cues_only_when_competitor_exists() {
        let mut cands = vec![
            HintedCandidate {
                reading: "イシ".into(),
                surface: "意志".into(),
                description: None,
            },
            HintedCandidate {
                reading: "イシ".into(),
                surface: "医師".into(),
                description: None,
            },
        ];
        assert!(apply_homophone_hints(&mut cands, true));
        assert!(cands[0].description.as_deref().unwrap().contains('・'));
        assert!(cands[1].description.as_deref().unwrap().contains('・'));

        let mut single = vec![HintedCandidate {
            reading: "イシ".into(),
            surface: "意志".into(),
            description: None,
        }];
        assert!(!apply_homophone_hints(&mut single, true));
    }
}
