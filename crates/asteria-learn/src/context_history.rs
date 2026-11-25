//! 文脈つき学習・文単位再発防止学習 (docs/adr/0075, 0082, `asteria_context_history.{h,cc}` 移植)。

use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use asteria_proto::ipc::InputScope;

pub const MAX_ENTRIES: usize = 2000;
pub const MAX_CUES: usize = 12;
pub const SENTENCE_READING: &str = "#文";
pub const SENTENCE_SEPARATOR: char = '\x1f';

const CONTEXT_DEFAULTS_TSV: &str = include_str!("../data/context_defaults.tsv");

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefaultContextEntry {
    pub reading: &'static str,
    pub chosen: &'static str,
    pub rejected: &'static str,
    pub cues: &'static str,
}

/// 同梱の同音異義語デフォルト手がかり一覧 (1,088 件) を返す。
pub fn default_context_entries() -> &'static [DefaultContextEntry] {
    static ENTRIES: OnceLock<Vec<DefaultContextEntry>> = OnceLock::new();
    ENTRIES.get_or_init(|| {
        CONTEXT_DEFAULTS_TSV
            .lines()
            .filter_map(|line| {
                let mut cols = line.split('\t');
                let reading = cols.next()?.trim();
                let chosen = cols.next()?.trim();
                let rejected = cols.next()?.trim();
                let cues = cols.next()?.trim();
                if reading.is_empty() || chosen.is_empty() || rejected.is_empty() || cues.is_empty()
                {
                    return None;
                }
                Some(DefaultContextEntry {
                    reading,
                    chosen,
                    rejected,
                    cues,
                })
            })
            .collect()
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextEntry {
    pub reading: String,
    pub chosen: String,
    pub rejected: String,
    pub cues: Vec<String>,
    pub count: i32,
    pub last: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LearnedSentence {
    pub keys: Vec<String>,
    pub values: Vec<String>,
}

#[derive(Debug, Clone, Copy)]
pub struct LearnSegmentRequest<'a> {
    pub scope: InputScope,
    pub incognito: bool,
    pub reading: &'a str,
    pub chosen: &'a str,
    pub rejected: &'a str,
    pub cues: &'a [String],
    pub now: i64,
}

#[derive(Debug)]
struct Inner {
    entries: Vec<ContextEntry>,
}

/// 学習可否ポリシー (Password 欄やシークレットモードでは学習しない)。
pub fn learning_allowed(scope: InputScope, incognito: bool) -> bool {
    !incognito && scope != InputScope::Password
}

fn is_kanji(c: char) -> bool {
    ('\u{4E00}'..='\u{9FFF}').contains(&c) || c == '\u{3005}'
}

fn is_katakana(c: char) -> bool {
    ('\u{30A1}'..='\u{30FA}').contains(&c) || c == '\u{30FC}'
}

fn has_kanji(s: &str) -> bool {
    s.chars().any(is_kanji)
}

fn is_hiragana_only(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| ('\u{3041}'..='\u{309F}').contains(&c) || c == '\u{30FC}')
}

fn unsafe_tsv(s: &str) -> bool {
    s.contains('\t') || s.contains('\n') || s.contains('\r')
}

/// `min_kanji` 文字以上の漢字列と 2 文字以上のカタカナ列を手がかり語として抽出する。
pub fn cues_of(text: &str, min_kanji: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut run = String::new();
    let mut kind = 0u8; // 0: none, 1: kanji, 2: katakana
    let min_k = min_kanji.max(1);

    let flush = |out: &mut Vec<String>, run: &mut String, kind: &mut u8| {
        let len = run.chars().count();
        let req = if *kind == 1 { min_k } else { 2 };
        if *kind != 0 && len >= req && !out.iter().any(|x| x == run) {
            out.push(run.clone());
        }
        run.clear();
        *kind = 0;
    };

    for c in text.chars() {
        let k = if is_kanji(c) {
            1
        } else if is_katakana(c) {
            2
        } else {
            0
        };
        if k != kind {
            flush(&mut out, &mut run, &mut kind);
        }
        if k != 0 {
            run.push(c);
            kind = k;
        }
    }
    flush(&mut out, &mut run, &mut kind);
    out
}

/// 文全体の完全再発防止キー (`=<16桁hex>`) を計算する。
pub fn exact_cue(input_reading: &str, context: &str) -> String {
    let mut h = DefaultHasher::new();
    input_reading.hash(&mut h);
    b'\n'.hash(&mut h);
    context.hash(&mut h);
    format!("={:016x}", h.finish())
}

#[derive(Debug)]
pub struct ContextHistory {
    path: Option<PathBuf>,
    weight: f64,
    inner: Mutex<Inner>,
}

impl ContextHistory {
    pub fn in_memory() -> Self {
        Self {
            path: None,
            weight: 2.0,
            inner: Mutex::new(Inner {
                entries: Vec::new(),
            }),
        }
    }

    pub fn open(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let mut hist = Self {
            path: Some(path),
            weight: 2.0,
            inner: Mutex::new(Inner {
                entries: Vec::new(),
            }),
        };
        hist.load();
        hist
    }

    pub fn set_weight(&mut self, weight: f64) {
        self.weight = weight;
    }

    pub fn add_default(&self, reading: &str, chosen: &str, rejected: &str, cues: &str) {
        let cue_vec: Vec<String> = cues
            .split('、')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToOwned::to_owned)
            .collect();
        if reading.is_empty() || chosen.is_empty() || rejected.is_empty() || cue_vec.is_empty() {
            return;
        }
        if let Ok(mut guard) = self.inner.lock() {
            guard.entries.push(ContextEntry {
                reading: reading.to_owned(),
                chosen: chosen.to_owned(),
                rejected: rejected.to_owned(),
                cues: cue_vec,
                count: 1,
                last: 0,
            });
        }
    }

    /// 既定の同音語手がかりをすべて読み込む。
    pub fn load_bundled_defaults(&self) {
        for e in default_context_entries() {
            self.add_default(e.reading, e.chosen, e.rejected, e.cues);
        }
    }

    /// プライバシーチェック付きで文節選択を学習する (パスワード欄/シークレットは拒否)。
    pub fn learn_checked(&self, req: LearnSegmentRequest<'_>) {
        if !learning_allowed(req.scope, req.incognito) {
            return;
        }
        self.learn(req.reading, req.chosen, req.rejected, req.cues, req.now);
    }

    pub fn learn(&self, reading: &str, chosen: &str, rejected: &str, cues: &[String], now: i64) {
        if reading.is_empty() || chosen.is_empty() || rejected.is_empty() || chosen == rejected {
            return;
        }
        if unsafe_tsv(reading) || unsafe_tsv(chosen) || unsafe_tsv(rejected) {
            return;
        }
        if is_hiragana_only(chosen) && has_kanji(rejected) {
            return;
        }
        let usable: Vec<String> = cues
            .iter()
            .filter(|c| {
                !c.is_empty()
                    && !unsafe_tsv(c)
                    && *c != chosen
                    && *c != rejected
                    && !chosen.contains(c.as_str())
            })
            .cloned()
            .collect();
        if usable.is_empty() {
            return;
        }
        let Ok(mut guard) = self.inner.lock() else {
            return;
        };
        for e in &mut guard.entries {
            if e.reading == reading && e.chosen == chosen && e.rejected == rejected {
                e.count += 1;
                e.last = now;
                for c in &usable {
                    if !e.cues.iter().any(|existing| existing == c) {
                        e.cues.push(c.clone());
                    }
                }
                if e.cues.len() > MAX_CUES {
                    let drop_n = e.cues.len() - MAX_CUES;
                    e.cues.drain(0..drop_n);
                }
                return;
            }
            if e.reading == reading && e.chosen == rejected && e.rejected == chosen {
                e.count = (e.count - 1).max(0);
            }
        }
        let mut entry = ContextEntry {
            reading: reading.to_owned(),
            chosen: chosen.to_owned(),
            rejected: rejected.to_owned(),
            cues: usable,
            count: 1,
            last: now,
        };
        entry.cues.truncate(MAX_CUES);
        guard.entries.push(entry);
        trim_to_max(&mut guard.entries);
    }

    /// プライバシーチェック付きで文全体の確定結果を記憶する。
    pub fn learn_sentence_checked(
        &self,
        scope: InputScope,
        incognito: bool,
        input_reading: &str,
        context: &str,
        sentence: &LearnedSentence,
        now: i64,
    ) {
        if !learning_allowed(scope, incognito) {
            return;
        }
        self.learn_sentence(input_reading, context, sentence, now);
    }

    pub fn learn_sentence(
        &self,
        input_reading: &str,
        context: &str,
        sentence: &LearnedSentence,
        now: i64,
    ) {
        if input_reading.is_empty()
            || sentence.keys.is_empty()
            || sentence.keys.len() != sentence.values.len()
        {
            return;
        }
        let sep = SENTENCE_SEPARATOR.to_string();
        let chosen = sentence.values.join(&sep);
        let keys = sentence.keys.join(&sep);
        if unsafe_tsv(&chosen) || unsafe_tsv(&keys) {
            return;
        }
        let exact = exact_cue(input_reading, context);
        let Ok(mut guard) = self.inner.lock() else {
            return;
        };
        for e in &mut guard.entries {
            if e.reading == SENTENCE_READING && e.cues.first().map(String::as_str) == Some(&exact) {
                e.chosen = chosen;
                e.rejected = keys;
                e.count = e.count.max(0) + 1;
                e.last = now;
                return;
            }
        }
        guard.entries.push(ContextEntry {
            reading: SENTENCE_READING.to_owned(),
            chosen,
            rejected: keys,
            cues: vec![exact],
            count: 1,
            last: now,
        });
        trim_to_max(&mut guard.entries);
    }

    pub fn exact_sentence(&self, input_reading: &str, context: &str) -> Option<LearnedSentence> {
        let guard = self.inner.lock().ok()?;
        let exact = exact_cue(input_reading, context);
        for e in &guard.entries {
            if e.reading != SENTENCE_READING
                || e.count <= 0
                || e.cues.first().map(String::as_str) != Some(&exact)
            {
                continue;
            }
            let values: Vec<String> = e
                .chosen
                .split(SENTENCE_SEPARATOR)
                .map(ToOwned::to_owned)
                .collect();
            let keys: Vec<String> = e
                .rejected
                .split(SENTENCE_SEPARATOR)
                .map(ToOwned::to_owned)
                .collect();
            if keys.len() != values.len() || keys.concat() != input_reading {
                return None;
            }
            return Some(LearnedSentence { keys, values });
        }
        None
    }

    pub fn score(&self, input_reading: &str, context: &str, text: &str) -> f64 {
        let Ok(guard) = self.inner.lock() else {
            return 0.0;
        };
        let mut s = 0.0;
        for e in &guard.entries {
            if !applies(e, input_reading, context, text) {
                continue;
            }
            let w = self.weight * f64::from(e.count.min(3));
            let has_chosen = text.contains(&e.chosen);
            let has_rejected = text.contains(&e.rejected) && !e.chosen.contains(&e.rejected);
            if has_chosen {
                s += w;
            } else if has_rejected {
                s -= w;
            }
        }
        s
    }

    pub fn variants(&self, input_reading: &str, context: &str, text: &str) -> Vec<String> {
        let Ok(guard) = self.inner.lock() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for e in &guard.entries {
            if !applies(e, input_reading, context, text) {
                continue;
            }
            if text.contains(&e.chosen) {
                continue;
            }
            if let Some(at) = text.find(&e.rejected) {
                let mut v = text.to_owned();
                v.replace_range(at..at + e.rejected.len(), &e.chosen);
                if !out.contains(&v) {
                    out.push(v);
                }
            }
        }
        out
    }

    pub fn forget(&self, reading: &str, chosen: &str, rejected: &str) {
        let Ok(mut guard) = self.inner.lock() else {
            return;
        };
        if let Some(pos) = guard
            .entries
            .iter()
            .position(|e| e.reading == reading && e.chosen == chosen && e.rejected == rejected)
        {
            guard.entries[pos].count -= 1;
            if guard.entries[pos].count <= 0 {
                guard.entries.remove(pos);
            }
        }
    }

    pub fn clear(&self) {
        if let Ok(mut guard) = self.inner.lock() {
            guard.entries.clear();
        }
        if let Some(p) = &self.path {
            let _ = fs::remove_file(p);
        }
    }

    pub fn len(&self) -> usize {
        self.inner.lock().map(|g| g.entries.len()).unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn save(&self) -> std::io::Result<()> {
        let Some(p) = &self.path else {
            return Ok(());
        };
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent)?;
        }
        let body = {
            let guard = self
                .inner
                .lock()
                .map_err(|_| std::io::Error::other("context history lock poisoned"))?;
            let mut out = String::new();
            for e in &guard.entries {
                out.push_str(&format!(
                    "{}\t{}\t{}\t{}\t{}\t{}\n",
                    e.reading,
                    e.chosen,
                    e.rejected,
                    e.count,
                    e.last,
                    e.cues.join("、")
                ));
            }
            out
        };
        let tmp = tmp_path(p);
        fs::write(&tmp, body)?;
        fs::rename(&tmp, p)?;
        Ok(())
    }

    fn load(&mut self) {
        let Some(p) = &self.path else {
            return;
        };
        let Ok(text) = fs::read_to_string(p) else {
            return;
        };
        let mut loaded = Vec::new();
        for line in text.lines() {
            let cols: Vec<&str> = line.split('\t').collect();
            if cols.len() != 6 {
                continue;
            }
            let Ok(count) = cols[3].parse::<i32>() else {
                continue;
            };
            let Ok(last) = cols[4].parse::<i64>() else {
                continue;
            };
            let cues: Vec<String> = cols[5]
                .split('、')
                .filter(|c| !c.is_empty())
                .map(ToOwned::to_owned)
                .collect();
            if cols[0].is_empty() || cols[1].is_empty() || cols[2].is_empty() || cues.is_empty() {
                continue;
            }
            loaded.push(ContextEntry {
                reading: cols[0].to_owned(),
                chosen: cols[1].to_owned(),
                rejected: cols[2].to_owned(),
                cues,
                count,
                last,
            });
            if loaded.len() >= MAX_ENTRIES {
                break;
            }
        }
        if let Ok(mut guard) = self.inner.lock() {
            guard.entries = loaded;
        }
    }
}

fn to_hiragana(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '\u{30A1}'..='\u{30F6}' => char::from_u32(c as u32 - 0x60).unwrap_or(c),
            _ => c,
        })
        .collect()
}

fn applies(e: &ContextEntry, input_reading: &str, context: &str, text: &str) -> bool {
    if e.count <= 0 || e.reading == SENTENCE_READING {
        return false;
    }
    // 既定辞書 (last == 0) は左文脈に同語が既出の場合の引きずりコピーを後押ししない。
    if e.last == 0 && e.chosen.chars().count() >= 2 && context.contains(&e.chosen) {
        return false;
    }
    let reading_hira = to_hiragana(input_reading);
    let entry_hira = to_hiragana(&e.reading);
    if !reading_hira.contains(&entry_hira) {
        return false;
    }
    e.cues
        .iter()
        .any(|c| context.contains(c) || text.contains(c))
}

/// 複数の `ContextHistory`（既定同音語辞書＋ユーザー学習）を安全マージン付きで候補スコアへ反映する。
pub fn rerank_with_histories(
    histories: &[&ContextHistory],
    input_reading: &str,
    context: &str,
    candidates: &[String],
    base_scores: &[f32],
    margin: f32,
) -> Option<usize> {
    if candidates.is_empty() || base_scores.len() != candidates.len() {
        return None;
    }
    let (top_idx, &top_score) = base_scores
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.total_cmp(b.1))?;
    if !top_score.is_finite() {
        return Some(top_idx);
    }
    let mut learned_best = top_idx;
    let mut learned_max = f32::NEG_INFINITY;
    for (i, cand) in candidates.iter().enumerate() {
        if !base_scores[i].is_finite() {
            continue;
        }
        let bonus: f64 = histories
            .iter()
            .map(|h| h.score(input_reading, context, cand))
            .sum();
        let combined = base_scores[i] + bonus as f32;
        if combined > learned_max {
            learned_max = combined;
            learned_best = i;
        }
    }
    if learned_best != top_idx
        && base_scores[learned_best].is_finite()
        && top_score - base_scores[learned_best] <= margin
    {
        Some(learned_best)
    } else {
        Some(top_idx)
    }
}

fn trim_to_max(entries: &mut Vec<ContextEntry>) {
    if entries.len() > MAX_ENTRIES {
        entries.sort_by_key(|a| std::cmp::Reverse(a.last));
        entries.truncate(MAX_ENTRIES);
    }
}

fn tmp_path(p: &Path) -> PathBuf {
    let mut os = p.as_os_str().to_os_string();
    os.push(".tmp");
    PathBuf::from(os)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_learning_variants_exact_sentence_and_privacy_guard() {
        assert!(default_context_entries().len() >= 1088);

        let hist = ContextHistory::in_memory();
        let cues = cues_of("カメラのピント", 2);
        assert_eq!(cues, vec!["カメラ", "ピント"]);

        // パスワード欄では学習されない
        hist.learn_checked(LearnSegmentRequest {
            scope: InputScope::Password,
            incognito: false,
            reading: "しょうてん",
            chosen: "焦点",
            rejected: "商店",
            cues: &cues,
            now: 100,
        });
        assert!(hist.is_empty());

        // 通常欄では学習される
        hist.learn_checked(LearnSegmentRequest {
            scope: InputScope::Default,
            incognito: false,
            reading: "しょうてん",
            chosen: "焦点",
            rejected: "商店",
            cues: &cues,
            now: 100,
        });
        assert!(
            hist.score("しょうてんをあわせる", "カメラの", "焦点を合わせる")
                > hist.score("しょうてんをあわせる", "カメラの", "商店を合わせる")
        );
        assert_eq!(
            hist.variants("しょうてんをあわせる", "カメラの", "商店を合わせる"),
            vec!["焦点を合わせる"]
        );

        // 文全体の記憶 (#文)
        let sent = LearnedSentence {
            keys: vec!["きょう".into(), "は".into()],
            values: vec!["今日".into(), "は".into()],
        };
        hist.learn_sentence_checked(InputScope::Default, false, "きょうは", "さて", &sent, 200);
        assert_eq!(hist.exact_sentence("きょうは", "さて"), Some(sent));
        assert_eq!(hist.exact_sentence("きょうは", "別の文"), None);

        // 既定辞書は左文脈の既存語への引きずりコピーを上書きせず、裸の「自分」で自信を自身へ誤誘導しない
        let bundled = ContextHistory::in_memory();
        bundled.load_bundled_defaults();
        assert_eq!(
            bundled.score("にほんごをかいせつ", "口座を開設しました。", "日本語を開設"),
            0.0
        );
        assert!(
            bundled.score("じぶんのうでにじしんがある", "", "自分の腕に自信がある")
                > bundled.score("じぶんのうでにじしんがある", "", "自分の腕に自身がある")
        );
    }
}
