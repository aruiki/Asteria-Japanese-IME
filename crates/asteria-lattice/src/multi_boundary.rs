//! 多境界・複数区切りの文全体仮説生成 (`Segments::asteria_sentences()` 相当, ADR0015/0086)。

use std::collections::HashMap;

use asteria_dict::Dictionary;

use crate::{segment, segment_with_boundaries, Lattice};

/// 文節ごとの `(読み文字数, 表記)` とコストからなる文全体の仮説。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SentenceHypothesis {
    pub segments: Vec<(usize, String)>,
    pub cost: i64,
    pub alt_boundary: bool,
}

impl SentenceHypothesis {
    pub fn surface(&self) -> String {
        self.segments.iter().map(|(_, s)| s.as_str()).collect()
    }
}

/// N-best 経路に加え、異なる境界での最適経路を生成して重複なくコスト昇順で最大 `k` 件返す。
pub fn generate_multi_boundary_sentences(
    lattice: &Lattice,
    dict: &Dictionary,
    k: usize,
) -> Vec<SentenceHypothesis> {
    if k == 0 {
        return Vec::new();
    }
    let len = lattice.reading().chars().count();
    if len == 0 {
        return Vec::new();
    }

    let mut by_surface: HashMap<String, SentenceHypothesis> = HashMap::new();
    let mut insert = |segments: Vec<(usize, String)>, cost: i64, alt_boundary: bool| {
        if segments.is_empty() {
            return;
        }
        let hyp = SentenceHypothesis {
            segments,
            cost,
            alt_boundary,
        };
        let key = hyp.surface();
        by_surface
            .entry(key)
            .and_modify(|existing| {
                if hyp.cost < existing.cost {
                    *existing = hyp.clone();
                }
            })
            .or_insert(hyp);
    };

    for cand in lattice.n_best(dict, k) {
        let segs = segment(dict, &cand.nodes)
            .into_iter()
            .map(|s| (s.end - s.start, s.surface()))
            .collect();
        insert(segs, cand.cost, false);
    }

    // 異なる文節区切りの仮説を生成 (長い読みでも O(1) 件に抑えるため最大 12 境界)。
    let step = (len / 12).max(1);
    for boundary in (1..len).step_by(step) {
        if let Some((path, cost)) = lattice.best_path_with_boundaries(dict, &[boundary]) {
            let segs = segment_with_boundaries(dict, &path, &[boundary])
                .into_iter()
                .map(|s| (s.end - s.start, s.surface()))
                .collect();
            insert(segs, cost, true);
        }
    }

    let mut out: Vec<SentenceHypothesis> = by_surface.into_values().collect();
    out.sort_by(|a, b| {
        a.cost
            .cmp(&b.cost)
            .then_with(|| a.surface().cmp(&b.surface()))
    });
    out.truncate(k);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Config;
    use asteria_dict::DictBuilder;

    #[test]
    fn generates_alternative_boundary_sentences() {
        let mut b = DictBuilder::new();
        b.add("キョウ", "今日", 1, 1, 2000, 0).unwrap();
        b.add("ハ", "は", 2, 2, 500, 0).unwrap();
        b.add("キョウハ", "京葉", 1, 1, 2100, 0).unwrap();
        b.set_connection(3, 3, vec![0; 9]).unwrap();
        let dict = Dictionary::from_bytes(b.build().unwrap()).unwrap();

        let mut lat = Lattice::new(Config::default());
        lat.set_reading(&dict, "キョウハ");
        let hyps = generate_multi_boundary_sentences(&lat, &dict, 8);
        let texts: Vec<String> = hyps.iter().map(SentenceHypothesis::surface).collect();
        assert!(texts.contains(&"今日は".to_string()));
        assert!(texts.contains(&"京葉".to_string()));
    }
}
