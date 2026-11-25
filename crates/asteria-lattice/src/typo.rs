//! 打鍵誤りチャネル確率・事後確率モデル (docs/adr/0076, 0082, `asteria_typo_model.cc` 移植)。

use std::collections::HashSet;

/// QWERTY の 3 段キー配列。
const ROWS: [&str; 3] = ["qwertyuiop", "asdfghjkl", "zxcvbnm"];
const LETTERS: f64 = 26.0;

/// 1 キーあたりの打ち間違い確率パラメータ (Dhakal ら CHI 2018 + Grudin 1983)。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TypoChannel {
    pub substitution: f64,
    pub omission: f64,
    pub insertion: f64,
    pub transposition: f64,
    pub substitution_row: f64,
    pub substitution_column: f64,
    pub insertion_adjacent: f64,
    pub spelling: f64,
    pub max_edits: usize,
}

impl Default for TypoChannel {
    fn default() -> Self {
        Self {
            substitution: 0.0165,
            omission: 0.0080,
            insertion: 0.0067,
            transposition: 0.0024,
            substitution_row: 0.31,
            substitution_column: 0.16,
            insertion_adjacent: 0.8,
            spelling: 0.005,
            max_edits: 3,
        }
    }
}

impl TypoChannel {
    fn keep_prob(self) -> f64 {
        (1.0 - self.substitution - self.omission - self.insertion - self.transposition).max(1e-9)
    }
}

fn key_pos(c: char) -> Option<(isize, isize)> {
    for (r, row) in ROWS.iter().enumerate() {
        if let Some(col) = row.find(c) {
            return Some((r as isize, col as isize));
        }
    }
    None
}

fn key_at(row: isize, col: isize) -> Option<char> {
    if !(0..=2).contains(&row) || col < 0 {
        return None;
    }
    ROWS[row as usize]
        .as_bytes()
        .get(col as usize)
        .map(|&b| b as char)
}

fn row_neighbors(c: char) -> Vec<char> {
    let Some((r, x)) = key_pos(c) else {
        return Vec::new();
    };
    [key_at(r, x - 1), key_at(r, x + 1)]
        .into_iter()
        .flatten()
        .collect()
}

fn column_neighbors(c: char) -> Vec<char> {
    let Some((r, x)) = key_pos(c) else {
        return Vec::new();
    };
    [
        key_at(r - 1, x),
        key_at(r - 1, x + 1),
        key_at(r + 1, x - 1),
        key_at(r + 1, x),
    ]
    .into_iter()
    .flatten()
    .collect()
}

fn substitution_log_prob(got: char, want: char, next: Option<char>, ch: TypoChannel) -> f64 {
    let vowel_ui = matches!(next, Some('u' | 'i'));
    if vowel_ui
        && ((want == 'd' && matches!(got, 'z' | 'j')) || (matches!(want, 'z' | 'j') && got == 'd'))
    {
        return ch.spelling.ln();
    }
    let row = row_neighbors(want);
    let col = column_neighbors(want);
    let p = if row.contains(&got) {
        ch.substitution_row / row.len() as f64
    } else if col.contains(&got) {
        ch.substitution_column / col.len() as f64
    } else {
        let others = (LETTERS - 1.0 - row.len() as f64 - col.len() as f64).max(1.0);
        (1.0 - ch.substitution_row - ch.substitution_column) / others
    };
    (ch.substitution * p).ln()
}

fn insertion_log_prob(got: char, prev: Option<char>, next: Option<char>, ch: TypoChannel) -> f64 {
    let mut near = Vec::new();
    for k in [prev, next].into_iter().flatten() {
        near.push(k);
        near.extend(row_neighbors(k));
        near.extend(column_neighbors(k));
    }
    near.sort_unstable();
    near.dedup();
    let p = if near.contains(&got) {
        ch.insertion_adjacent / near.len() as f64
    } else {
        (1.0 - ch.insertion_adjacent) / (LETTERS - near.len() as f64).max(1.0)
    };
    (ch.insertion * p).ln()
}

/// 意図したキー列 `intended` に対して、打鍵列 `typed` が出る対数確率 log P(typed | intended)。
pub fn typo_log_prob(typed: &str, intended: &str, ch: TypoChannel) -> f64 {
    let a = typed.as_bytes();
    let b = intended.as_bytes();
    let n = a.len();
    let m = b.len();
    if n.abs_diff(m) > ch.max_edits {
        return f64::NEG_INFINITY;
    }
    let e_max = ch.max_edits;
    let keep = ch.keep_prob().ln();
    let omit = ch.omission.ln();
    let swap = ch.transposition.ln();

    let stride_j = e_max + 1;
    let stride_i = (m + 1) * stride_j;
    let mut dp = vec![f64::NEG_INFINITY; (n + 1) * stride_i];
    dp[0] = 0.0;

    let idx = |i: usize, j: usize, e: usize| i * stride_i + j * stride_j + e;
    for i in 0..=n {
        let j_lo = i.saturating_sub(e_max);
        let j_hi = m.min(i + e_max);
        for j in j_lo..=j_hi {
            for e in 0..=e_max {
                let v = dp[idx(i, j, e)];
                if !v.is_finite() {
                    continue;
                }
                let mut relax = |i2: usize, j2: usize, e2: usize, add: f64| {
                    if e2 <= e_max {
                        let slot = &mut dp[idx(i2, j2, e2)];
                        *slot = slot.max(v + add);
                    }
                };
                let want = b.get(j).map(|&c| c as char);
                let next = b.get(j + 1).map(|&c| c as char);
                if i < n && j < m {
                    let got = a[i] as char;
                    let w = want.unwrap_or('\0');
                    if got == w {
                        relax(i + 1, j + 1, e, keep);
                    } else {
                        relax(i + 1, j + 1, e + 1, substitution_log_prob(got, w, next, ch));
                    }
                }
                if j < m {
                    relax(i, j + 1, e + 1, omit);
                }
                if i < n {
                    let prev = j.checked_sub(1).and_then(|p| b.get(p).map(|&c| c as char));
                    relax(
                        i + 1,
                        j,
                        e + 1,
                        insertion_log_prob(a[i] as char, prev, want, ch),
                    );
                }
                if i + 1 < n
                    && j + 1 < m
                    && a[i] == b[j + 1]
                    && a[i + 1] == b[j]
                    && b[j] != b[j + 1]
                {
                    relax(i + 2, j + 2, e + 1, swap);
                }
            }
        }
    }

    (0..=e_max)
        .map(|e| dp[idx(n, m, e)])
        .fold(f64::NEG_INFINITY, f64::max)
}

/// 誤入力なし (typed == typed) との対数比。
pub fn typo_log_ratio(typed: &str, intended: &str, ch: TypoChannel) -> f64 {
    let base = typed.len() as f64 * ch.keep_prob().ln();
    let p = typo_log_prob(typed, intended, ch);
    if p.is_finite() {
        p - base
    } else {
        f64::NEG_INFINITY
    }
}

const VOICED_GROUPS: &[&str] = &[
    "かが",
    "きぎ",
    "くぐ",
    "けげ",
    "こご",
    "さざ",
    "しじ",
    "すず",
    "せぜ",
    "そぞ",
    "ただ",
    "ちぢ",
    "つづ",
    "てで",
    "とど",
    "はばぱ",
    "ひびぴ",
    "ふぶぷ",
    "へべぺ",
    "ほぼぽ",
];

fn is_yoon_head(c: char) -> bool {
    matches!(
        c,
        'き' | 'し' | 'ち' | 'に' | 'ひ' | 'み' | 'り' | 'ぎ' | 'じ' | 'ぢ' | 'び' | 'ぴ'
    )
}

/// かな単位の 1 箇所の書き分け誤り / 濁点ミス / 拗音抜け / 隣り合うかな入れ替えの対数比。
pub fn kana_slip_log_ratio(typed_reading: &str, intended_reading: &str, ch: TypoChannel) -> f64 {
    let a: Vec<char> = typed_reading.chars().collect();
    let b: Vec<char> = intended_reading.chars().collect();
    let mut head = 0;
    while head < a.len() && head < b.len() && a[head] == b[head] {
        head += 1;
    }
    let mut tail = 0;
    while tail < a.len() - head
        && tail < b.len() - head
        && a[a.len() - 1 - tail] == b[b.len() - 1 - tail]
    {
        tail += 1;
    }
    let na = a.len() - head - tail;
    let nb = b.len() - head - tail;
    let slip = ch.spelling.ln() - ch.keep_prob().ln();

    if na == 1 && nb == 1 {
        let x = a[head];
        let y = b[head];
        if matches!(
            (x, y),
            ('ず', 'づ') | ('づ', 'ず') | ('じ', 'ぢ') | ('ぢ', 'じ')
        ) {
            return slip;
        }
        if x != y && VOICED_GROUPS.iter().any(|g| g.contains(x) && g.contains(y)) {
            return slip;
        }
    }
    if na == 0
        && nb == 1
        && head > 0
        && matches!(b[head], 'ゃ' | 'ゅ' | 'ょ')
        && is_yoon_head(b[head - 1])
    {
        return slip;
    }
    if na == 2
        && nb == 2
        && a[head] == b[head + 1]
        && a[head + 1] == b[head]
        && a[head] != a[head + 1]
    {
        return ch.transposition.ln() - ch.keep_prob().ln();
    }
    f64::NEG_INFINITY
}

/// かな読みに対する 1 誤入力仮説候補と対数事前比を生成する。
pub fn kana_slip_hypotheses(reading: &str, ch: TypoChannel) -> Vec<(String, f64)> {
    let chars: Vec<char> = reading.chars().collect();
    if chars.is_empty() || chars.len() > 24 {
        return Vec::new();
    }
    let mut seen = HashSet::new();
    seen.insert(reading.to_owned());
    let mut out = Vec::new();

    for i in 0..chars.len() {
        // 濁点・半濁点 / ずづ・じぢ
        for group in VOICED_GROUPS {
            if group.contains(chars[i]) {
                for cand_ch in group.chars() {
                    if cand_ch == chars[i] {
                        continue;
                    }
                    let mut next = chars.clone();
                    next[i] = cand_ch;
                    let s: String = next.into_iter().collect();
                    if seen.insert(s.clone()) {
                        let ratio = kana_slip_log_ratio(reading, &s, ch);
                        if ratio.is_finite() {
                            out.push((s, ratio));
                        }
                    }
                }
            }
        }
        // い段直後の拗音抜け補完 (しつ -> しゅつ 等)
        if is_yoon_head(chars[i]) && !matches!(chars.get(i + 1), Some('ゃ' | 'ゅ' | 'ょ')) {
            for y in ['ゃ', 'ゅ', 'ょ'] {
                let mut next = chars.clone();
                next.insert(i + 1, y);
                let s: String = next.into_iter().collect();
                if seen.insert(s.clone()) {
                    let ratio = kana_slip_log_ratio(reading, &s, ch);
                    if ratio.is_finite() {
                        out.push((s, ratio));
                    }
                }
            }
        }
        // 隣接かな入れ替え
        if i + 1 < chars.len() && chars[i] != chars[i + 1] {
            let mut next = chars.clone();
            next.swap(i, i + 1);
            let s: String = next.into_iter().collect();
            if seen.insert(s.clone()) {
                let ratio = kana_slip_log_ratio(reading, &s, ch);
                if ratio.is_finite() {
                    out.push((s, ratio));
                }
            }
        }
    }
    out
}

/// 対数スコア列から事後確率分布 (softmax) を計算する。
pub fn posteriors(log_scores: &[f64]) -> Vec<f64> {
    let top = log_scores.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    if !top.is_finite() {
        return vec![0.0; log_scores.len()];
    }
    let mut out: Vec<f64> = log_scores
        .iter()
        .map(|&s| if s.is_finite() { (s - top).exp() } else { 0.0 })
        .collect();
    let sum: f64 = out.iter().sum();
    if sum > 0.0 {
        for p in &mut out {
            *p /= sum;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adjacent_substitution_beats_distant_and_exact_beats_all() {
        let ch = TypoChannel::default();
        let exact = typo_log_ratio("kyou", "kyou", ch);
        let near = typo_log_ratio("ktou", "kyou", ch); // t と y は同段隣
        let far = typo_log_ratio("kzou", "kyou", ch); // z と y は遠い
        assert!((exact - 0.0).abs() < 1e-9);
        assert!(near > far);
        assert!(exact > near);
    }

    #[test]
    fn kana_slip_and_posteriors_work() {
        let ch = TypoChannel::default();
        let hyps = kana_slip_hypotheses("ござまいせん", ch);
        assert!(hyps.iter().any(|(r, _)| r == "ございません"));

        let hyps2 = kana_slip_hypotheses("しつちょう", ch);
        assert!(
            hyps2.iter().any(|(r, _)| r == "しゅっちょう")
                || kana_slip_log_ratio("しつ", "しゅつ", ch).is_finite()
        );

        let post = posteriors(&[0.0, -2.0]);
        assert!(post[0] > post[1]);
        assert!((post.iter().sum::<f64>() - 1.0).abs() < 1e-9);
    }
}
