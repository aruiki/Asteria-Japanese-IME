//! 入力の状態機械(docs/SPEC.md 11.1、REQ-11-1)。
//!
//! キーを受けてキーマップでコマンドに変え、読みの組み立て・変換・候補の選択・確定を行う。
//! 表示に要るもの(プリエディット、確定文字列、候補ウィンドウ)は [`Output`] で返し、
//! フロントエンドは状態を持たない。

use std::sync::Arc;

use asteria_composer::{Composer, RomajiTable};

use crate::converter::{to_hiragana, Converter, SegmentCandidates, Sentence};
use crate::keymap::{Command, Key, Keymap, State};
use asteria_lattice::{to_halfwidth_katakana, RomajiEnglishLookup};
use asteria_predict::{
    apply_homophone_hints, generate_nuance_alternatives, promote_standard_notations,
    HintedCandidate,
};

/// 候補ウィンドウの1ページの件数(11.3)。
pub const PAGE_SIZE: usize = 9;

/// 持っておく左文脈の最大の文字数(4.2 の SetContext)。
pub const MAX_LEFT_CONTEXT: usize = 256;

/// プリエディットの区間の表示属性。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Attribute {
    /// 入力中の読み。
    Input,
    /// 変換済みの文節。
    Converted,
    /// 注目文節。
    Focused,
}

/// 候補ウィンドウの内容。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateWindow {
    /// 注目文節の候補すべて。
    pub candidates: Vec<String>,
    /// 選択中の候補の位置。
    pub focused: usize,
}

impl CandidateWindow {
    /// 選択中の候補を含むページの先頭の位置。
    pub fn page_start(&self) -> usize {
        self.focused / PAGE_SIZE * PAGE_SIZE
    }
}

/// キー1つを処理した結果。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Output {
    /// false ならフロントエンドはキーをアプリへ渡す。
    pub consumed: bool,
    pub preedit: Vec<(String, Attribute)>,
    /// プリエディット内のカーソル位置(文字数)。
    pub cursor: usize,
    /// 確定した文字列。
    pub committed: String,
    /// 開いている候補ウィンドウ。
    pub candidate_window: Option<CandidateWindow>,
    /// 候補ウィンドウの各候補に対応する注釈 (同音語ヒント等)。
    pub candidate_annotations: Vec<String>,
    /// ライブ変換または予測サジェスト文字列。
    pub suggestion_text: String,
    pub live_conversion_active: bool,
    pub callback_delay_ms: u32,
    pub refresh_suggestion: bool,
    pub show_candidate_window: bool,
}

/// 確定時に学習・直近履歴へ渡すメタデータ。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitRecord {
    pub reading: String,
    pub committed: String,
    pub ai_first: String,
    pub segment_readings: Vec<String>,
    pub segment_values: Vec<String>,
    pub segment_firsts: Vec<String>,
    pub candidates: Vec<String>,
}

#[derive(Debug, Clone)]
struct Segment {
    /// 読みの文字数。
    len: usize,
    candidates: Vec<String>,
    selected: usize,
}

impl From<SegmentCandidates> for Segment {
    fn from(s: SegmentCandidates) -> Self {
        Self {
            len: s.len,
            candidates: s.candidates,
            selected: 0,
        }
    }
}

impl Segment {
    fn text(&self) -> &str {
        self.candidates
            .get(self.selected)
            .map(String::as_str)
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone)]
struct Conversion {
    /// 変換した読み(カタカナ)。文節を伸縮するときに変換し直す。
    reading: String,
    segments: Vec<Segment>,
    focus: usize,
    window_open: bool,
    /// 利用者が候補・文節・表記を変えたか。変えたら LM のリランクの結果を当てない(REQ-6-2)。
    touched: bool,
}

impl Conversion {
    fn focused(&mut self) -> Option<&mut Segment> {
        self.segments.get_mut(self.focus)
    }

    fn text(&self) -> String {
        self.segments.iter().map(Segment::text).collect()
    }
}

/// 1つの入力セッション。
#[derive(Debug, Clone)]
pub struct Session {
    keymap: Arc<Keymap>,
    composer: Composer,
    conversion: Option<Conversion>,
    /// 確定済みの左文脈。LM のリランクの入力になる(6.2)。
    left_context: String,
    /// カーソル以降の文脈 (SetContext.following_text)。
    following_text: String,
    /// 今のキー(コマンド)で変換を始めたか。
    converted_now: bool,
    live_conversion: bool,
    homophone_hints: bool,
    prediction_enabled: bool,
    nuance_enabled: bool,
    standard_notation: bool,
    romaji_english: bool,
    suggest_window: bool,
    last_suggestion: String,
    last_commit: Option<CommitRecord>,
}

impl Session {
    pub fn new(keymap: Arc<Keymap>, romaji: Arc<RomajiTable>) -> Self {
        Self {
            keymap,
            composer: Composer::new(romaji),
            conversion: None,
            left_context: String::new(),
            following_text: String::new(),
            converted_now: false,
            live_conversion: false,
            homophone_hints: false,
            prediction_enabled: true,
            nuance_enabled: true,
            standard_notation: true,
            romaji_english: true,
            suggest_window: false,
            last_suggestion: String::new(),
            last_commit: None,
        }
    }

    pub fn set_live_conversion(&mut self, enabled: bool) {
        self.live_conversion = enabled;
    }

    pub fn set_homophone_hints(&mut self, enabled: bool) {
        self.homophone_hints = enabled;
    }

    pub fn set_prediction_enabled(&mut self, enabled: bool) {
        self.prediction_enabled = enabled;
    }

    pub fn set_nuance_enabled(&mut self, enabled: bool) {
        self.nuance_enabled = enabled;
    }

    pub fn set_standard_notation(&mut self, enabled: bool) {
        self.standard_notation = enabled;
    }

    pub fn set_romaji_english(&mut self, enabled: bool) {
        self.romaji_english = enabled;
    }

    pub fn set_suggest_window(&mut self, enabled: bool) {
        self.suggest_window = enabled;
    }

    pub fn set_following_text(&mut self, s: &str) {
        self.following_text.clear();
        self.following_text.push_str(s);
    }

    pub fn following_text(&self) -> &str {
        &self.following_text
    }

    pub fn take_last_commit(&mut self) -> Option<CommitRecord> {
        self.last_commit.take()
    }

    /// 今のキー(コマンド)で変換を始めたか。サーバーが LM のリランクを頼む合図(6.2)。
    pub fn just_converted(&self) -> bool {
        self.converted_now
    }

    /// 利用者がまだ変えていない変換の読み。変換していない・候補や文節を変えた・候補ウィンドウを
    /// 開いたなら `None`(リランクの結果で順位を変えない、REQ-6-2)。
    pub fn untouched_reading(&self) -> Option<&str> {
        self.conversion
            .as_ref()
            .filter(|c| !c.touched && !c.window_open)
            .map(|c| c.reading.as_str())
    }

    /// 変換前で入力中の読み(カタカナ)があれば返す (KV キャッシュ先読み用)。
    pub fn composing_reading(&self) -> Option<String> {
        if self.conversion.is_none() && !self.composer.is_empty() {
            Some(self.composer.reading())
        } else {
            None
        }
    }

    /// 文全体の候補 `sentence`(LM のリランクで選ばれたもの)の区切りと表記で変換し直す。
    /// 読みが `reading` と違う・利用者が変えたあとなら何もせず false。
    pub fn apply_sentence(
        &mut self,
        reading: &str,
        sentence: &Sentence,
        converter: &dyn Converter,
    ) -> bool {
        if self.untouched_reading() != Some(reading) {
            return false;
        }
        let mut ends = Vec::with_capacity(sentence.segments.len());
        let mut pos = 0;
        for (len, _) in &sentence.segments {
            pos += len;
            ends.push(pos);
        }
        if pos != reading.chars().count() || ends.is_empty() {
            return false;
        }
        // 区切りを固定して変換し、同じ区間の文節があればその候補を使う。なければ表記と
        // ひらがなだけの文節にする。
        let converted = converter.convert_with_boundaries(reading, &ends[..ends.len() - 1]);
        let mut spans = std::collections::HashMap::new();
        let mut at = 0;
        for s in converted {
            let len = s.len;
            spans.insert((at, len), s.candidates);
            at += len;
        }
        let chars: Vec<char> = reading.chars().collect();
        let mut start = 0;
        let segments = sentence
            .segments
            .iter()
            .map(|(len, text)| {
                let span: String = chars[start..start + len].iter().collect();
                let mut candidates = spans
                    .remove(&(start, *len))
                    .unwrap_or_else(|| vec![to_hiragana(&span)]);
                start += len;
                candidates.retain(|t| t != text);
                candidates.insert(0, text.clone());
                Segment {
                    len: *len,
                    candidates,
                    selected: 0,
                }
            })
            .collect();
        self.conversion = Some(Conversion {
            reading: reading.to_owned(),
            segments,
            focus: 0,
            window_open: false,
            touched: false,
        });
        true
    }

    /// 今の表示(プリエディット、カーソル、候補ウィンドウ)。確定文字列は空。
    pub fn view(&self) -> Output {
        self.output(true, String::new())
    }

    /// フロントエンドから届いた左文脈に置き換える。長ければ末尾の
    /// [`MAX_LEFT_CONTEXT`] 文字だけ持つ。
    pub fn set_left_context(&mut self, s: &str) {
        self.left_context.clear();
        self.push_left_context(s);
    }

    /// 確定済みの左文脈。
    pub fn left_context(&self) -> &str {
        &self.left_context
    }

    fn push_left_context(&mut self, s: &str) {
        self.left_context.push_str(s);
        let n = self.left_context.chars().count();
        if n > MAX_LEFT_CONTEXT {
            let start = self
                .left_context
                .char_indices()
                .nth(n - MAX_LEFT_CONTEXT)
                .map_or(0, |(i, _)| i);
            self.left_context.drain(..start);
        }
    }

    /// 今の状態(11.1)。
    pub fn state(&self) -> State {
        match &self.conversion {
            Some(c) if c.window_open => State::Selecting,
            Some(_) => State::Converting,
            None if self.composer.is_empty() => State::Idle,
            None => State::Composing,
        }
    }

    /// キーを1つ処理する。`text` はキーが生む文字(なければ空)。
    pub fn key(&mut self, key: Key, text: &str, converter: &dyn Converter) -> Output {
        self.converted_now = false;
        let state = self.state();
        let mut committed = String::new();
        // 確定アンドゥ(REQ-9-5)は確定の直後だけ効く。取り消せる確定がないときは割り当てが
        // ないのと同じに扱い、待機中ならアプリへ渡す(Ctrl+Backspace の単語削除など)。
        // 確定アンドゥそのものはまだないので、今は常にアプリへ渡す。
        let command = self
            .keymap
            .command(state, key)
            .filter(|&c| c != Command::UndoCommit);
        let consumed = if let Some(command) = command {
            self.run(command, converter, &mut committed);
            true
        } else if !text.is_empty() && !key.ctrl && !key.alt && !text.chars().any(char::is_control) {
            // 変換中の文字入力は、変換結果を確定してから新しい入力を始める(11.1)。
            if let Some(c) = self.conversion.take() {
                self.record_commit(&c);
                committed.push_str(&c.text());
                self.composer.clear();
            }
            text.chars().for_each(|c| self.composer.push(c));
            true
        } else {
            // 待機中の割り当てのないキーはアプリへ渡す。入力中は飲み込む。
            state != State::Idle
        };
        if self.conversion.is_none() && !self.composer.is_empty() {
            let mut preview = self.composer.clone();
            preview.flush();
            let reading = preview.reading();
            if self.live_conversion && reading.chars().count() >= 2 {
                let segs = converter.convert(&reading);
                self.last_suggestion = segs
                    .iter()
                    .filter_map(|s| s.candidates.first())
                    .cloned()
                    .collect();
            } else if !self.live_conversion
                && self.prediction_enabled
                && reading.chars().count() >= 2
            {
                self.last_suggestion = converter
                    .predict(&reading, &self.left_context, 1)
                    .into_iter()
                    .next()
                    .unwrap_or_default();
            } else {
                self.last_suggestion.clear();
            }
        } else if self.conversion.is_some() || self.composer.is_empty() {
            self.last_suggestion.clear();
        }
        self.push_left_context(&committed);
        self.output(consumed, committed)
    }

    /// フロントエンドから届いたコマンド(4.2 の SendCommand の確定・取消など)を実行する。
    pub fn command(&mut self, command: Command, converter: &dyn Converter) -> Output {
        self.converted_now = false;
        let mut committed = String::new();
        self.run(command, converter, &mut committed);
        // 確定した文字列は次の変換の左文脈になる。
        self.push_left_context(&committed);
        self.output(true, committed)
    }

    fn run(&mut self, command: Command, converter: &dyn Converter, committed: &mut String) {
        match command {
            Command::Convert | Command::ConvertPrev => {
                self.composer.flush();
                let reading = self.composer.reading();
                if reading.is_empty() {
                    return;
                }
                let raw_keys: String = self
                    .composer
                    .units()
                    .iter()
                    .map(|u| u.keys.as_str())
                    .collect();
                let mut segments: Vec<Segment> = converter
                    .convert(&reading)
                    .into_iter()
                    .map(Segment::from)
                    .collect();
                if self.romaji_english {
                    if let Some((eng, is_name)) = RomajiEnglishLookup::default().find(&raw_keys) {
                        let single_segment = segments.len() == 1;
                        if let Some(first_seg) = segments.first_mut() {
                            first_seg.candidates.retain(|c| c != &eng);
                            if is_name && single_segment {
                                first_seg.candidates.insert(0, eng);
                            } else {
                                let idx = usize::from(!first_seg.candidates.is_empty());
                                first_seg.candidates.insert(idx, eng);
                            }
                        }
                    }
                }
                for seg in &mut segments {
                    if self.standard_notation {
                        promote_standard_notations(&mut seg.candidates);
                    }
                    if self.nuance_enabled {
                        if let Some(first) = seg.candidates.first().cloned() {
                            let alts = generate_nuance_alternatives(&first, &seg.candidates);
                            for alt in alts.into_iter().rev() {
                                let pos = usize::from(!seg.candidates.is_empty());
                                seg.candidates.insert(pos, alt.text);
                            }
                        }
                    }
                }
                self.conversion = Some(Conversion {
                    reading,
                    segments,
                    focus: 0,
                    window_open: false,
                    touched: false,
                });
                self.converted_now = true;
                if command == Command::ConvertPrev {
                    self.move_candidate(-1);
                }
            }
            Command::NextCandidate => self.move_candidate(1),
            Command::PrevCandidate => self.move_candidate(-1),
            Command::NextPage => self.move_candidate(PAGE_SIZE as isize),
            Command::Select(n) => {
                // 表示中のページの n 番目の候補で確定する。
                let chosen = self
                    .conversion
                    .as_mut()
                    .and_then(Conversion::focused)
                    .is_some_and(|seg| {
                        let at = seg.selected / PAGE_SIZE * PAGE_SIZE + usize::from(n) - 1;
                        let ok = at < seg.candidates.len();
                        if ok {
                            seg.selected = at;
                        }
                        ok
                    });
                if chosen {
                    self.commit(committed);
                }
            }
            Command::SelectIndex(i) => {
                if let Some(c) = &mut self.conversion {
                    if let Some(seg) = c.focused().filter(|s| i < s.candidates.len()) {
                        seg.selected = i;
                        c.window_open = true;
                        c.touched = true;
                    }
                }
            }
            Command::Commit => self.commit(committed),
            Command::CancelInput => self.clear(),
            Command::DeleteBack => self.composer.backspace(),
            Command::BackToComposing => self.conversion = None,
            Command::BackToConverting => {
                if let Some(c) = &mut self.conversion {
                    c.window_open = false;
                }
            }
            Command::FocusPrev | Command::FocusNext => {
                if let Some(c) = &mut self.conversion {
                    let last = c.segments.len().saturating_sub(1);
                    c.focus = if command == Command::FocusPrev {
                        c.focus.saturating_sub(1)
                    } else {
                        (c.focus + 1).min(last)
                    };
                    c.window_open = false;
                    c.touched = true;
                }
            }
            Command::ToHiragana
            | Command::ToFullKatakana
            | Command::ToHalfKatakana
            | Command::ToFullAscii
            | Command::ToHalfAscii => self.change_script(command),
            Command::CursorLeft => self.composer.move_left(),
            Command::CursorRight => self.composer.move_right(),
            Command::ShrinkSegment => self.resize_focused(-1, converter),
            Command::ExpandSegment => self.resize_focused(1, converter),
            Command::InsertSpace => committed.push('\u{3000}'),
            Command::InsertHalfSpace => committed.push(' '),
            Command::CommitPrediction if !self.last_suggestion.is_empty() => {
                committed.push_str(&self.last_suggestion);
                self.last_suggestion.clear();
                self.composer.clear();
                self.conversion = None;
            }
            // 確定アンドゥは後続の変更で入れる。キーは飲み込む。
            _ => {}
        }
    }

    /// 注目文節の候補を `delta` だけ動かし(端では反対側へ回る)、候補ウィンドウを開く。
    fn move_candidate(&mut self, delta: isize) {
        let Some(c) = &mut self.conversion else {
            return;
        };
        c.window_open = true;
        c.touched = true;
        if let Some(seg) = c.focused() {
            let n = seg.candidates.len() as isize;
            if n > 0 {
                seg.selected = (seg.selected as isize + delta).rem_euclid(n) as usize;
            }
        }
    }

    /// 文字種変換(F6〜F10、11.2)。入力中なら読み全体を1文節として変換中にし、
    /// 変換中なら注目文節の表記を置き換える(候補の先頭に置いて選ぶ)。
    fn change_script(&mut self, command: Command) {
        if self.conversion.is_none() {
            self.composer.flush();
            let reading = self.composer.reading();
            if reading.is_empty() {
                return;
            }
            self.conversion = Some(Conversion {
                segments: vec![Segment {
                    len: reading.chars().count(),
                    candidates: Vec::new(),
                    selected: 0,
                }],
                reading,
                focus: 0,
                window_open: false,
                touched: true,
            });
        }
        let units = self.composer.units();
        let Some(c) = &mut self.conversion else {
            return;
        };
        let start: usize = c.segments[..c.focus].iter().map(|s| s.len).sum();
        let Some(seg) = c.segments.get_mut(c.focus) else {
            return;
        };
        let end = start + seg.len;
        let kana: String = c.reading.chars().skip(start).take(seg.len).collect();
        // 文節の生キーは、始まりが文節の中にあるかなの生キーをつなげたもの。
        let mut keys = String::new();
        let mut pos = 0;
        for u in units {
            if (start..end).contains(&pos) {
                keys.push_str(&u.keys);
            }
            pos += u.kana.chars().count();
        }
        let text = match command {
            Command::ToHiragana => to_hiragana(&kana),
            Command::ToFullKatakana => kana,
            Command::ToHalfKatakana => to_halfwidth_katakana(&kana),
            Command::ToFullAscii => to_fullwidth_ascii(&keys),
            _ => keys,
        };
        seg.candidates.retain(|t| *t != text);
        seg.candidates.insert(0, text);
        seg.selected = 0;
        c.window_open = false;
        c.touched = true;
    }

    /// 注目文節の読みを `delta` 文字だけ伸縮し、読み全体を変換し直す(11.2、REQ-5-9)。
    ///
    /// 注目文節より前の文節の境界と、注目文節の新しい終わりを固定の境界にする。
    /// 前の文節は長さが変わらなければ選んだ候補を保つ。注目文節の位置は保ち、
    /// 候補ウィンドウは閉じる。1文字の文節は縮めず、最後の文節は伸ばさない。
    fn resize_focused(&mut self, delta: isize, converter: &dyn Converter) {
        let Some(c) = &mut self.conversion else {
            return;
        };
        let Some(focused) = c.segments.get(c.focus) else {
            return;
        };
        let start: usize = c.segments[..c.focus].iter().map(|s| s.len).sum();
        let total = c.reading.chars().count();
        let end = start + focused.len;
        let new_end = end.saturating_add_signed(delta);
        let is_last = c.focus + 1 == c.segments.len();
        if new_end <= start || new_end > total || (is_last && delta > 0) {
            return;
        }
        let mut boundaries: Vec<usize> = c.segments[..c.focus]
            .iter()
            .scan(0, |pos, s| {
                *pos += s.len;
                Some(*pos)
            })
            .collect();
        boundaries.push(new_end);
        let mut segments: Vec<Segment> = converter
            .convert_with_boundaries(&c.reading, &boundaries)
            .into_iter()
            .map(Segment::from)
            .collect();
        for (new, old) in segments.iter_mut().zip(&c.segments[..c.focus]) {
            if new.len == old.len {
                *new = old.clone();
            }
        }
        // 注目文節の始まりから始まる文節に注目を置く。
        let mut pos = 0;
        c.focus = segments
            .iter()
            .position(|s| {
                let at = pos;
                pos += s.len;
                at == start
            })
            .unwrap_or(0);
        c.segments = segments;
        c.window_open = false;
        c.touched = true;
    }

    fn record_commit(&mut self, c: &Conversion) {
        let chars: Vec<char> = c.reading.chars().collect();
        let mut pos = 0usize;
        let mut segment_readings = Vec::with_capacity(c.segments.len());
        let mut segment_values = Vec::with_capacity(c.segments.len());
        let mut segment_firsts = Vec::with_capacity(c.segments.len());
        let mut flat_cands = Vec::new();
        for seg in &c.segments {
            let end = (pos + seg.len).min(chars.len());
            let r: String = chars[pos..end].iter().collect();
            pos = end;
            segment_readings.push(r);
            segment_values.push(seg.text().to_owned());
            segment_firsts.push(seg.candidates.first().cloned().unwrap_or_default());
            for cand in seg.candidates.iter().take(6) {
                if flat_cands.len() < 40 && !flat_cands.contains(cand) {
                    flat_cands.push(cand.clone());
                }
            }
        }
        let ai_first: String = segment_firsts.concat();
        self.last_commit = Some(CommitRecord {
            reading: c.reading.clone(),
            committed: c.text(),
            ai_first,
            segment_readings,
            segment_values,
            segment_firsts,
            candidates: flat_cands,
        });
    }

    fn commit(&mut self, committed: &mut String) {
        match self.conversion.take() {
            Some(c) => {
                self.record_commit(&c);
                committed.push_str(&c.text());
            }
            None => {
                self.composer.flush();
                committed.push_str(&to_hiragana(&self.composer.reading()));
            }
        }
        self.last_suggestion.clear();
        self.composer.clear();
    }

    fn clear(&mut self) {
        self.conversion = None;
        self.last_suggestion.clear();
        self.composer.clear();
    }

    fn output(&self, consumed: bool, committed: String) -> Output {
        let mut out = Output {
            consumed,
            committed,
            ..Output::default()
        };
        match &self.conversion {
            Some(c) => {
                for (i, seg) in c.segments.iter().enumerate() {
                    let attr = if i == c.focus {
                        Attribute::Focused
                    } else {
                        Attribute::Converted
                    };
                    out.preedit.push((seg.text().to_owned(), attr));
                }
                out.cursor = c.text().chars().count();
                if c.window_open {
                    if let Some(s) = c.segments.get(c.focus) {
                        let start_ch: usize = c.segments[..c.focus].iter().map(|seg| seg.len).sum();
                        let seg_reading: String =
                            c.reading.chars().skip(start_ch).take(s.len).collect();
                        let mut hinted: Vec<HintedCandidate> = s
                            .candidates
                            .iter()
                            .map(|surf| HintedCandidate {
                                reading: seg_reading.clone(),
                                surface: surf.clone(),
                                description: None,
                            })
                            .collect();
                        apply_homophone_hints(&mut hinted, self.homophone_hints);
                        let nuance_map: std::collections::HashMap<String, String> =
                            if self.nuance_enabled {
                                s.candidates
                                    .first()
                                    .map(|first| {
                                        generate_nuance_alternatives(first, &[])
                                            .into_iter()
                                            .map(|alt| (alt.text, alt.description))
                                            .collect()
                                    })
                                    .unwrap_or_default()
                            } else {
                                std::collections::HashMap::new()
                            };
                        out.candidate_annotations = hinted
                            .into_iter()
                            .map(|h| {
                                let desc = h.description.unwrap_or_default();
                                if desc.is_empty() {
                                    nuance_map.get(&h.surface).cloned().unwrap_or_default()
                                } else {
                                    desc
                                }
                            })
                            .collect();
                        out.candidate_window = Some(CandidateWindow {
                            candidates: s.candidates.clone(),
                            focused: s.selected,
                        });
                        out.show_candidate_window = true;
                    }
                }
            }
            None if !self.composer.is_empty() => {
                let text = to_hiragana(&self.composer.reading());
                out.cursor = self.composer.cursor();
                out.preedit.push((text, Attribute::Input));
                if !self.last_suggestion.is_empty() {
                    out.suggestion_text = self.last_suggestion.clone();
                    if self.live_conversion {
                        out.live_conversion_active = true;
                    } else if self.suggest_window {
                        out.show_candidate_window = true;
                    }
                }
            }
            None => {}
        }
        out
    }
}

/// ASCII の印字可能文字を全角に写す(空白は全角の空白)。
fn to_fullwidth_ascii(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            ' ' => '\u{3000}',
            '!'..='~' => char::from_u32(c as u32 + 0xFEE0).unwrap_or(c),
            _ => c,
        })
        .collect()
}
