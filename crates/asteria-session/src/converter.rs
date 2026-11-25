//! 読みを文節と候補にする(docs/SPEC.md 5.4、5.5)。

use std::sync::{Arc, RwLock};

use asteria_dict::{CollocationTable, Dictionary, EnglishLexicon};
use asteria_lattice::{
    generate_multi_boundary_sentences, kana_slip_hypotheses, segment, segment_candidates,
    segment_with_boundaries, special_candidates, Config, DateTime, Lattice, TypoChannel,
    DEFAULT_N_BEST,
};
use asteria_learn::UserDictionary;
use asteria_predict::predict_candidates;

/// 1文節の読みの長さと候補。候補の先頭が第1候補。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SegmentCandidates {
    /// 読み(カタカナ)の文字数。
    pub len: usize,
    pub candidates: Vec<String>,
}

/// 文全体の候補(LM のリランクの単位、6.2 モード A)。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sentence {
    /// 文節ごとの読みの文字数と表記。
    pub segments: Vec<(usize, String)>,
    /// ラティスの経路のコスト。
    pub cost: i64,
}

impl Sentence {
    /// 文全体の表記。
    pub fn surface(&self) -> String {
        self.segments.iter().map(|(_, s)| s.as_str()).collect()
    }
}

/// 読みを文節に分けて候補を出すもの。
pub trait Converter {
    /// 読み(カタカナ)を変換する。文節の読みの長さの和は読みの文字数と等しい。
    fn convert(&self, reading: &str) -> Vec<SegmentCandidates>;

    /// `boundaries`(読みの文字位置)で必ず文節を分けて変換する(文節の伸縮、REQ-5-9)。
    /// 既定の実装は境界を無視して [`Converter::convert`] を呼ぶ。
    fn convert_with_boundaries(
        &self,
        reading: &str,
        boundaries: &[usize],
    ) -> Vec<SegmentCandidates> {
        let _ = boundaries;
        self.convert(reading)
    }

    /// 文全体の上位 `k` 件(コストの昇順)。LM のリランクに使う。既定の実装は空を返す
    /// (リランクしない)。
    fn sentences(&self, reading: &str, k: usize) -> Vec<Sentence> {
        let _ = (reading, k);
        Vec::new()
    }

    /// 入力途中の読み(カタカナ)と左文脈から予測候補を上位 `limit` 件返す。
    fn predict(&self, reading: &str, left_context: &str, limit: usize) -> Vec<String> {
        let _ = (reading, left_context, limit);
        Vec::new()
    }
}

/// ひらがなをカタカナにする。
fn to_katakana(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '\u{3041}'..='\u{3096}' => char::from_u32(c as u32 + 0x60).unwrap_or(c),
            _ => c,
        })
        .collect()
}

/// カタカナをひらがなにする。
pub fn to_hiragana(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '\u{30A1}'..='\u{30F6}' => char::from_u32(c as u32 - 0x60).unwrap_or(c),
            _ => c,
        })
        .collect()
}

/// 現在の日時(UTC に日本標準時の 9 時間を足す)。
pub fn system_now() -> DateTime {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64);
    DateTime::from_unix(secs + 9 * 3600)
}

/// システム辞書とラティスによる変換(5.4)。
#[derive(Debug, Clone)]
pub struct LatticeConverter {
    dict: Arc<Dictionary>,
    collocations: Option<Arc<CollocationTable>>,
    english: Option<Arc<EnglishLexicon>>,
    user_dict: Option<Arc<RwLock<UserDictionary>>>,
    typo_correction: bool,
    now: fn() -> DateTime,
}

impl LatticeConverter {
    pub fn new(dict: Arc<Dictionary>) -> Self {
        Self {
            dict,
            collocations: None,
            english: None,
            user_dict: None,
            typo_correction: true,
            now: system_now,
        }
    }

    pub fn with_collocations(mut self, collocations: Arc<CollocationTable>) -> Self {
        self.collocations = Some(collocations);
        self
    }

    pub fn with_english_lexicon(mut self, english: Arc<EnglishLexicon>) -> Self {
        self.english = Some(english);
        self
    }

    pub fn with_user_dictionary(mut self, user_dict: Arc<RwLock<UserDictionary>>) -> Self {
        self.user_dict = Some(user_dict);
        self
    }

    pub fn with_typo_correction(mut self, enabled: bool) -> Self {
        self.typo_correction = enabled;
        self
    }

    /// 日付の特殊変換(5.3)に使う時計を差し替える(テスト用)。
    pub fn with_clock(mut self, now: fn() -> DateTime) -> Self {
        self.now = now;
        self
    }

    fn user_surfaces(&self, reading: &str) -> Vec<String> {
        let Some(ud) = &self.user_dict else {
            return Vec::new();
        };
        ud.read()
            .map(|guard| {
                guard
                    .lookup(reading)
                    .into_iter()
                    .map(|e| e.surface.clone())
                    .collect()
            })
            .unwrap_or_default()
    }
}

impl Converter for LatticeConverter {
    fn convert(&self, reading: &str) -> Vec<SegmentCandidates> {
        self.convert_with_boundaries(reading, &[])
    }

    fn predict(&self, reading: &str, left_context: &str, limit: usize) -> Vec<String> {
        let ud_guard = self.user_dict.as_ref().and_then(|ud| ud.read().ok());
        predict_candidates(
            &self.dict,
            left_context,
            reading,
            self.collocations.as_deref(),
            ud_guard.as_deref(),
            limit,
        )
        .into_iter()
        .map(|item| item.surface)
        .collect()
    }

    fn sentences(&self, reading: &str, k: usize) -> Vec<Sentence> {
        let dict = &*self.dict;
        let char_len = reading.chars().count();
        let mut lattice = Lattice::new(Config::default());
        lattice.set_reading(dict, reading);
        let mut hyps: Vec<Sentence> = generate_multi_boundary_sentences(&lattice, dict, k)
            .into_iter()
            .map(|h| {
                let mut cost = h.cost;
                if let Some(col) = &self.collocations {
                    let text: String = h.segments.iter().map(|(_, s)| s.as_str()).collect();
                    let bonus = col.score_sentence(&text);
                    if bonus > 0.0 {
                        cost -= (bonus * 1000.0) as i64;
                    }
                }
                Sentence {
                    segments: h.segments,
                    cost,
                }
            })
            .collect();

        for surf in self.user_surfaces(reading) {
            if !hyps.iter().any(|h| h.surface() == surf) {
                hyps.push(Sentence {
                    segments: vec![(char_len, surf)],
                    cost: -20_000,
                });
            }
        }
        if let Some(eng) = &self.english {
            for word in eng.english_for_katakana(reading) {
                if !hyps.iter().any(|h| h.surface() == *word) {
                    hyps.push(Sentence {
                        segments: vec![(char_len, word.clone())],
                        cost: 4_000,
                    });
                }
            }
        }
        if self.typo_correction && (2..=16).contains(&char_len) {
            let hira = to_hiragana(reading);
            let mut slips = kana_slip_hypotheses(&hira, TypoChannel::default());
            slips.sort_by(|(a_hira, a_ratio), (b_hira, b_ratio)| {
                let a_known = dict.lookup(&to_katakana(a_hira)).is_some();
                let b_known = dict.lookup(&to_katakana(b_hira)).is_some();
                b_known.cmp(&a_known).then_with(|| {
                    b_ratio
                        .partial_cmp(a_ratio)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
            });
            for (alt_hira, ratio) in slips.into_iter().take(3) {
                let alt_kata = to_katakana(&alt_hira);
                let mut alt_lat = Lattice::new(Config::default());
                alt_lat.set_reading(dict, &alt_kata);
                if let Some((path, base_cost)) = alt_lat.best_path(dict) {
                    let surf = segment(dict, &path)
                        .into_iter()
                        .map(|s| s.surface())
                        .collect::<String>();
                    if !surf.is_empty() && !hyps.iter().any(|h| h.surface() == surf) {
                        let penalty = (-ratio * 600.0) as i64;
                        hyps.push(Sentence {
                            segments: vec![(char_len, surf)],
                            cost: base_cost + penalty,
                        });
                    }
                }
            }
        }

        hyps.sort_by_key(|s| s.cost);
        if k > 0 {
            hyps.truncate(k);
        }
        hyps
    }

    fn convert_with_boundaries(
        &self,
        reading: &str,
        boundaries: &[usize],
    ) -> Vec<SegmentCandidates> {
        let dict = &*self.dict;
        let chars: Vec<char> = reading.chars().collect();
        let mut lattice = Lattice::new(Config::default());
        lattice.set_reading(dict, reading);
        let best = if boundaries.is_empty() {
            lattice.best_path(dict)
        } else {
            lattice.best_path_with_boundaries(dict, boundaries)
        };
        let Some((path, _)) = best else {
            // 未知語ノードがあるので経路は必ずあるが、念のため読みのまま返す。
            return vec![SegmentCandidates {
                len: chars.len(),
                candidates: vec![to_hiragana(reading)],
            }];
        };
        let nbest = lattice.n_best(dict, DEFAULT_N_BEST);
        let now = (self.now)();
        let segments = if boundaries.is_empty() {
            segment(dict, &path)
        } else {
            segment_with_boundaries(dict, &path, boundaries)
        };
        let single_seg = segments.len() == 1;
        segments
            .iter()
            .map(|s| {
                let span: String = chars[s.start..s.end].iter().collect();
                let extra = special_candidates(&span, &now);
                let mut candidates = self.user_surfaces(&span);
                let surf = s.surface();
                if !candidates.contains(&surf) {
                    candidates.push(surf);
                }
                for c in segment_candidates(
                    dict,
                    Config::default(),
                    reading,
                    &nbest,
                    s.start,
                    s.end,
                    &extra,
                ) {
                    if !candidates.contains(&c) {
                        candidates.push(c);
                    }
                }
                if let Some(eng) = &self.english {
                    for word in eng.english_for_katakana(&span) {
                        if !candidates.contains(word) {
                            candidates.push(word.clone());
                        }
                    }
                }
                if single_seg
                    && boundaries.is_empty()
                    && self.typo_correction
                    && (2..=16).contains(&chars.len())
                    && dict.lookup(&span).is_none()
                {
                    let mut slips =
                        kana_slip_hypotheses(&to_hiragana(&span), TypoChannel::default());
                    slips.sort_by(|(a_hira, a_ratio), (b_hira, b_ratio)| {
                        let a_known = dict.lookup(&to_katakana(a_hira)).is_some();
                        let b_known = dict.lookup(&to_katakana(b_hira)).is_some();
                        b_known.cmp(&a_known).then_with(|| {
                            b_ratio
                                .partial_cmp(a_ratio)
                                .unwrap_or(std::cmp::Ordering::Equal)
                        })
                    });
                    for (alt_hira, _) in slips
                        .into_iter()
                        .filter(|(h, _)| dict.lookup(&to_katakana(h)).is_some())
                        .take(3)
                    {
                        let alt_kata = to_katakana(&alt_hira);
                        let mut alt_lat = Lattice::new(Config::default());
                        alt_lat.set_reading(dict, &alt_kata);
                        if let Some((alt_path, _)) = alt_lat.best_path(dict) {
                            let alt_surf: String = segment(dict, &alt_path)
                                .into_iter()
                                .map(|sg| sg.surface())
                                .collect();
                            if !alt_surf.is_empty() && !candidates.contains(&alt_surf) {
                                candidates.push(alt_surf);
                            }
                        }
                    }
                }
                SegmentCandidates {
                    len: s.end - s.start,
                    candidates,
                }
            })
            .collect()
    }
}
