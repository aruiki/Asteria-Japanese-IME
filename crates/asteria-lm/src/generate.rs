//! zenz / LLM ビームサーチ生成および同音判定再ランキングロジック。

use std::cmp::Ordering;

use crate::rerank::CancelToken;

/// ビーム 1 本の状態。
#[derive(Debug, Clone)]
pub struct Beam {
    pub tokens: Vec<i32>,
    pub text: String,
    pub log_prob: f32,
    pub finished: bool,
}

/// 次トークン上位枝を広げる抽象ビーム探索（任意の言語モデルでテスト可能）。
pub fn beam_search<F>(
    initial_text: &str,
    beams_width: usize,
    max_steps: usize,
    eos_token: i32,
    cancel: Option<&CancelToken>,
    mut step_fn: F,
) -> Vec<Beam>
where
    F: FnMut(&[i32]) -> Vec<(i32, String, f32)>,
{
    if beams_width == 0 || max_steps == 0 {
        return Vec::new();
    }
    let mut active = vec![Beam {
        tokens: Vec::new(),
        text: initial_text.to_owned(),
        log_prob: 0.0,
        finished: false,
    }];
    let mut completed = Vec::new();

    for _ in 0..max_steps {
        if cancel.is_some_and(CancelToken::is_cancelled) {
            return Vec::new();
        }
        if active.is_empty() {
            break;
        }
        let mut next_beams = Vec::new();
        for b in &active {
            if b.finished {
                completed.push(b.clone());
                continue;
            }
            for (tok, piece, lp) in step_fn(&b.tokens) {
                let mut nt = b.tokens.clone();
                nt.push(tok);
                let finished = tok == eos_token;
                let mut text = b.text.clone();
                if !finished {
                    text.push_str(&piece);
                }
                let nb = Beam {
                    tokens: nt,
                    text,
                    log_prob: b.log_prob + lp,
                    finished,
                };
                if finished {
                    completed.push(nb);
                } else {
                    next_beams.push(nb);
                }
            }
        }
        next_beams.sort_by(|a, b| {
            b.log_prob
                .partial_cmp(&a.log_prob)
                .unwrap_or(Ordering::Equal)
        });
        next_beams.truncate(beams_width);
        active = next_beams;
        if completed.len() >= beams_width {
            break;
        }
    }
    completed.extend(active);
    completed.sort_by(|a, b| {
        b.log_prob
            .partial_cmp(&a.log_prob)
            .unwrap_or(Ordering::Equal)
    });
    completed.truncate(beams_width);
    completed
}

/// 僅差候補のみ大型判定役モデル (`High` / `Unreal`) で再採点するか判定する (カード 90)。
pub fn should_invoke_judge(scores_sorted_desc: &[f32], margin: f32) -> bool {
    if scores_sorted_desc.len() < 2 {
        return false;
    }
    (scores_sorted_desc[0] - scores_sorted_desc[1]).abs() <= margin
}

/// 候補位置を保ちつつ候補テキスト重複を取り除く。
pub fn dedup_generated_texts(beams: &[Beam]) -> Vec<String> {
    let mut out = Vec::new();
    for b in beams {
        if !b.text.is_empty() && !out.contains(&b.text) {
            out.push(b.text.clone());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn beam_search_picks_highest_sequence_and_honors_cancel() {
        let cancel_gen = crate::rerank::Generation::new();
        let g0 = cancel_gen.advance();
        let tok = cancel_gen.token(g0);
        let res = beam_search("", 2, 4, 99, Some(&tok), |prefix| match prefix {
            [] => vec![(1, "今".into(), -0.5), (2, "京".into(), -0.8)],
            [1] => vec![(99, String::new(), -0.1), (3, "日".into(), -1.5)],
            [2] => vec![(99, String::new(), -0.2)],
            _ => vec![],
        });
        assert_eq!(dedup_generated_texts(&res), vec!["今", "京"]);

        cancel_gen.advance();
        let cancelled = beam_search("", 2, 4, 99, Some(&tok), |_| vec![(1, "x".into(), -0.1)]);
        assert!(cancelled.is_empty());
        assert!(should_invoke_judge(&[-1.0, -1.2], 0.5));
        assert!(!should_invoke_judge(&[-1.0, -3.0], 0.5));
    }
}
