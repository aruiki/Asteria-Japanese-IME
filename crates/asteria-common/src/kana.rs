//! かな・正規化・3-gram Jaccard 類似判定・評価読み抽出。

use std::collections::{HashMap, HashSet};

use serde_json::Value;
use unicode_normalization::UnicodeNormalization;

/// カタカナ（ァ〜ヶ）をひらがなへ変換する。
pub fn kata_to_hira(s: &str) -> String {
    s.chars()
        .map(|c| {
            if ('ァ'..='ヶ').contains(&c) {
                char::from_u32(c as u32 - 0x60).unwrap_or(c)
            } else {
                c
            }
        })
        .collect()
}

/// ひらがな（ぁ〜ゖ）をカタカナへ変換する。
pub fn hira_to_kata(s: &str) -> String {
    s.chars()
        .map(|c| {
            if ('ぁ'..='ゖ').contains(&c) {
                char::from_u32(c as u32 + 0x60).unwrap_or(c)
            } else {
                c
            }
        })
        .collect()
}

/// Unicode NFKC 正規化。
pub fn nfkc(s: &str) -> String {
    s.nfkc().collect()
}

/// 読みの比較・除外用の正規化キー（NFKC → 空白除去 → カタカナをひらがな化）。
pub fn reading_key(value: &str) -> String {
    value
        .nfkc()
        .filter(|c| !c.is_whitespace())
        .map(|c| {
            if ('ァ'..='ヶ').contains(&c) {
                char::from_u32(c as u32 - 0x60).unwrap_or(c)
            } else {
                c
            }
        })
        .collect()
}

/// 3 文字の Unicode スカラ値をパックした 64bit キー（重複排除・ソート済み）。
/// `char` は最大 21bit なので 3 文字を 1 つの `u64` に衝突なく格納できる。
pub fn grams3_packed(value: &str) -> Vec<u64> {
    let chars: Vec<char> = value.chars().collect();
    if chars.len() < 3 {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(chars.len() - 2);
    for w in chars.windows(3) {
        let k = ((w[0] as u64) << 42) | ((w[1] as u64) << 21) | (w[2] as u64);
        out.push(k);
    }
    out.sort_unstable();
    out.dedup();
    out
}

fn sorted_intersection_len(a: &[u64], b: &[u64]) -> usize {
    let (mut i, mut j, mut common) = (0, 0, 0);
    while i < a.len() && j < b.len() {
        match a[i].cmp(&b[j]) {
            std::cmp::Ordering::Less => i += 1,
            std::cmp::Ordering::Greater => j += 1,
            std::cmp::Ordering::Equal => {
                common += 1;
                i += 1;
                j += 1;
            }
        }
    }
    common
}

/// 2 つの文字列の 3-gram Jaccard 類似度が閾値以上（または完全一致）か判定する。
pub fn similar_jaccard(a: &str, b: &str, threshold: f64) -> bool {
    if a == b {
        return true;
    }
    let ga = grams3_packed(a);
    let gb = grams3_packed(b);
    if ga.is_empty() || gb.is_empty() {
        return false;
    }
    let min_len = ga.len().min(gb.len());
    let max_len = ga.len().max(gb.len());
    if (min_len as f64) / (max_len as f64) < threshold {
        return false;
    }
    let common = sorted_intersection_len(&ga, &gb);
    (common as f64) / ((ga.len() + gb.len() - common) as f64) >= threshold
}

/// 3-gram 集合の Jaccard >= threshold 判定をアンカー索引で高速化するフィルタ。
#[derive(Debug, Clone)]
pub struct Exclusions {
    exact: HashSet<String>,
    threshold: f64,
    values: Vec<Vec<u64>>,
    anchors: HashMap<u64, Vec<usize>>,
}

impl Exclusions {
    /// 除外集合と閾値（既定 0.8）からアンカー索引を構築する。
    pub fn new<'a>(excluded: impl IntoIterator<Item = &'a str>, threshold: f64) -> Self {
        let exact: HashSet<String> = excluded.into_iter().map(str::to_owned).collect();
        let mut sorted_exact: Vec<&str> = exact.iter().map(String::as_str).collect();
        sorted_exact.sort_unstable();

        let mut values = Vec::new();
        let mut freq: HashMap<u64, usize> = HashMap::new();
        for key in sorted_exact {
            let g = grams3_packed(key);
            if !g.is_empty() {
                for &gram in &g {
                    *freq.entry(gram).or_insert(0) += 1;
                }
                values.push(g);
            }
        }

        let mut anchors: HashMap<u64, Vec<usize>> = HashMap::new();
        for (idx, grams) in values.iter().enumerate() {
            let count = (((1.0 - threshold + 1e-9) * (grams.len() as f64)).floor() as usize) + 1;
            let mut ranked: Vec<u64> = grams.clone();
            ranked.sort_unstable_by_key(|&g| (freq.get(&g).copied().unwrap_or(0), g));
            for &gram in ranked.iter().take(count) {
                anchors.entry(gram).or_default().push(idx);
            }
        }

        Self {
            exact,
            threshold,
            values,
            anchors,
        }
    }

    /// 指定の正規化読みが除外集合と完全一致または Jaccard >= threshold で重複するか。
    pub fn overlaps(&self, key: &str) -> bool {
        if self.exact.contains(key) {
            return true;
        }
        let values = grams3_packed(key);
        if values.is_empty() {
            return false;
        }
        let mut seen_candidates = HashSet::new();
        for gram in &values {
            if let Some(indices) = self.anchors.get(gram) {
                for &idx in indices {
                    if !seen_candidates.insert(idx) {
                        continue;
                    }
                    let other = &self.values[idx];
                    let min_len = values.len().min(other.len());
                    let max_len = values.len().max(other.len());
                    if (min_len as f64) / (max_len as f64) < self.threshold {
                        continue;
                    }
                    let common = sorted_intersection_len(&values, other);
                    let denom = values.len() + other.len() - common;
                    if (common as f64) / (denom as f64) >= self.threshold {
                        return true;
                    }
                }
            }
        }
        false
    }
}

/// 評価 JSON から読みフィールド（`input`, `reading`, `yomi`, `kana`, `typed`）だけを再帰抽出する。
/// 正解表記（`expected_output` / `truth` 等）は学習へ渡さないため抽出しない。
pub fn evaluation_readings(value: &Value) -> Vec<String> {
    let mut out = Vec::new();
    collect_evaluation_readings(value, &mut out);
    out
}

fn collect_evaluation_readings(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::Array(items) => {
            for item in items {
                collect_evaluation_readings(item, out);
            }
        }
        Value::Object(map) => {
            for (k, v) in map {
                match v {
                    Value::String(s)
                        if matches!(
                            k.as_str(),
                            "input" | "reading" | "yomi" | "kana" | "typed"
                        ) =>
                    {
                        out.push(reading_key(s));
                    }
                    Value::Array(_) | Value::Object(_) => collect_evaluation_readings(v, out),
                    _ => {}
                }
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reading_key_normalizes_katakana_and_widths() {
        assert_eq!(reading_key("ｶﾀｶﾅ ヨミ"), reading_key("かたかなよみ"));
    }

    #[test]
    fn evaluation_readings_extracts_only_reading_fields() {
        let v = json!([
            {"typed": "ハシヲツカ", "conv": "橋を使", "truth": "箸を使う"},
            {"cases": [{"reading": "ハシ", "expected_output": ["箸"]}]}
        ]);
        assert_eq!(evaluation_readings(&v), vec!["はしをつか", "はし"]);
    }

    #[test]
    fn exclusions_detects_exact_and_near_duplicates() {
        let excl = Exclusions::new(["きょうはあめがふっています。"], 0.8);
        assert!(excl.overlaps("きょうはあめがふっています。"));
        assert!(excl.overlaps("きょうはあめがふっています"));
        assert!(!excl.overlaps("あすのかいぎはごごです"));
    }
}
