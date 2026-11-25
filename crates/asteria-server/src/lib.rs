//! エンジン本体(docs/SPEC.md 4章)。
//!
//! セッションごとに状態機械([`asteria_session::Session`])を持ち、キーを変換して表示の内容を
//! 返す。変換の部品([`Engine`])がないとき(辞書を読めなかったときなど)は、キーをすべて
//! 未処理(`consumed = false`)として返し、フロントエンドにアプリへ渡させる。
//!
//! LM があれば、変換のたびに文全体の上位 K 件をリランクに出し、締め切り(25ms)までに
//! 終われば並べ替えて返す。間に合わなければラティスの順で返し、終わったら `PollUpdate` で
//! 返す(2段階応答、REQ-4-4、REQ-6-2、docs/adr/0008)。

mod instance;
pub mod state;

use std::collections::HashMap;
use std::io::{Read, Write};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use asteria_composer::RomajiTable;
use asteria_dict::{CollocationTable, Dictionary, EnglishLexicon};
use asteria_learn::{
    cues_of, rerank_with_histories, ContextHistory, LearnSegmentRequest, LearnedSentence,
    RecentConversion, RecentConversions, UserDictionary,
};
pub use asteria_lm::multi_model::RuntimeTelemetry as LmTelemetry;
use asteria_lm::multi_model::{RuntimeTelemetry, SharedTelemetry};
use asteria_lm::rerank::{
    GenerationResult, Outcome, Pending, RerankRequest, Reranker, DEFAULT_DEADLINE,
};
use asteria_lm::ScoreWeights;
use asteria_proto::ipc::{self, request, response, ErrorCode, Request, Response};
use asteria_proto::{protocol_version, read_message, write_message, FrameError, PROTOCOL_MAJOR};
use asteria_session::{
    Attribute, Command, Converter, Key, Keymap, LatticeConverter, Sentence, Session,
};
pub use instance::InstanceGuard;
pub use state::{write_ai_status_file, AiRuntimeStatus, ServerSettings};

/// リランクに出す文全体の候補の数(5.1 の K)。
pub const RERANK_K: usize = 16;

/// LM によるリランク(6.2 モード A)。
struct Rerank {
    reranker: Reranker,
    weights: ScoreWeights,
}

/// 文脈履歴・同音異字既定規則が AI 判定を上書きできる最大スコア差 (`ADR 0075`)。
pub const CONTEXT_HISTORY_MARGIN: f32 = 6.0;

/// 変換に使う部品。セッションをまたいで共有する。
#[derive(Clone)]
pub struct Engine {
    converter: Arc<dyn Converter + Send + Sync>,
    keymap: Arc<Keymap>,
    romaji: Arc<RomajiTable>,
    rerank: Option<Arc<Rerank>>,
}

impl Engine {
    /// システム辞書と MS-IME 互換のキーマップ・ローマ字表で作る。
    pub fn new(dict: Dictionary) -> Self {
        Self::with_converter(Arc::new(LatticeConverter::new(Arc::new(dict))))
    }

    /// システム辞書と補助辞書（連語・英語・共有ユーザー辞書・タイポ設定）で作る。
    pub fn from_bundle(
        dict: Dictionary,
        collocations: Option<CollocationTable>,
        english: Option<EnglishLexicon>,
        user_dict: Arc<RwLock<UserDictionary>>,
    ) -> Self {
        Self::from_bundle_arc(Arc::new(dict), collocations, english, user_dict)
    }

    /// 共有済み `Arc<Dictionary>` と補助辞書で作る。
    pub fn from_bundle_arc(
        dict: Arc<Dictionary>,
        collocations: Option<CollocationTable>,
        english: Option<EnglishLexicon>,
        user_dict: Arc<RwLock<UserDictionary>>,
    ) -> Self {
        let mut conv = LatticeConverter::new(dict).with_user_dictionary(user_dict);
        if let Some(col) = collocations {
            conv = conv.with_collocations(Arc::new(col));
        }
        if let Some(eng) = english {
            conv = conv.with_english_lexicon(Arc::new(eng));
        }
        Self::with_converter(Arc::new(conv))
    }

    /// 変換器を差し替えて作る(テスト用)。
    pub fn with_converter(converter: Arc<dyn Converter + Send + Sync>) -> Self {
        Self {
            converter,
            keymap: Arc::new(Keymap::ms_ime()),
            romaji: Arc::new(RomajiTable::ms_ime()),
            rerank: None,
        }
    }

    /// LM のリランクを足す。モデルの読み込みは `reranker` のワーカーで行い、終わるまでと
    /// 読めなかったときはラティス単体で動く(REQ-6-4、REQ-6-5)。
    pub fn with_reranker(mut self, reranker: Reranker, weights: ScoreWeights) -> Self {
        self.rerank = Some(Arc::new(Rerank { reranker, weights }));
        self
    }

    fn apply_settings_to_session(s: &mut Session, settings: &ServerSettings) {
        s.set_live_conversion(settings.live_conversion);
        s.set_homophone_hints(settings.homophone_hints);
        s.set_prediction_enabled(settings.prediction_enabled);
        s.set_nuance_enabled(settings.nuance_enabled);
        s.set_standard_notation(settings.standard_notation);
        s.set_romaji_english(settings.romaji_english);
        s.set_suggest_window(settings.suggest_window);
    }

    fn session(&self, settings: &ServerSettings) -> Session {
        let mut s = Session::new(Arc::clone(&self.keymap), Arc::clone(&self.romaji));
        Self::apply_settings_to_session(&mut s, settings);
        s
    }
}

impl std::fmt::Debug for Engine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Engine").finish_non_exhaustive()
    }
}

#[derive(Debug)]
struct SessionEntry {
    scope: ipc::InputScope,
    session: Option<Session>,
    /// 状態機械を作る前に届いた左文脈。作るときに渡す。
    pending_context: Option<String>,
    pending_following: Option<String>,
    /// 締め切りに間に合わなかったリランク。`PollUpdate` で結果を当てる。
    pending_rerank: Option<PendingRerank>,
}

#[derive(Debug)]
struct PendingRerank {
    pending: Pending,
    reading: String,
    sentences: Vec<Sentence>,
}

/// サーバーの状態。接続をまたいで共有する。
pub struct Server {
    engine: Option<Engine>,
    sessions: HashMap<u64, SessionEntry>,
    next_session_id: u64,
    data_dir: Option<PathBuf>,
    settings: ServerSettings,
    ai_status: AiRuntimeStatus,
    bundled_defaults: Arc<ContextHistory>,
    history: Arc<ContextHistory>,
    recent: RecentConversions,
    user_dict: Arc<RwLock<UserDictionary>>,
    telemetry: SharedTelemetry,
}

impl std::fmt::Debug for Server {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Server")
            .field("next_session_id", &self.next_session_id)
            .field("data_dir", &self.data_dir)
            .finish_non_exhaustive()
    }
}

impl Default for Server {
    fn default() -> Self {
        Self::new()
    }
}

impl Server {
    /// 変換の部品なし・メモリ内状態で作る (テスト互換)。
    pub fn new() -> Self {
        let bundled = ContextHistory::in_memory();
        bundled.load_bundled_defaults();
        Self {
            engine: None,
            sessions: HashMap::new(),
            next_session_id: 0,
            data_dir: None,
            settings: ServerSettings::default(),
            ai_status: AiRuntimeStatus::default(),
            bundled_defaults: Arc::new(bundled),
            history: Arc::new(ContextHistory::in_memory()),
            recent: RecentConversions::in_memory(),
            user_dict: Arc::new(RwLock::new(UserDictionary::new())),
            telemetry: Arc::new(Mutex::new(RuntimeTelemetry::default())),
        }
    }

    /// 変換の部品を持たせて作る。
    pub fn with_engine(engine: Engine) -> Self {
        let mut srv = Self::new();
        srv.set_engine(engine);
        srv
    }

    /// 永続化フォルダ指定付きで構築する (`asteria-server` 本番・永続化テスト用)。
    pub fn with_data_dir(engine: Option<Engine>, dir: PathBuf) -> Self {
        let settings = ServerSettings::load_from(&dir.join("asteria_settings.json"));
        let bundled = ContextHistory::in_memory();
        bundled.load_bundled_defaults();
        let history = ContextHistory::open(dir.join("asteria_context.tsv"));
        let recent = RecentConversions::open(dir.join("asteria_recent_conversions.txt"));
        let mut ud = UserDictionary::new();
        let _ = ud.load_file(&dir.join("user_dictionary.tsv"));
        let mut srv = Self {
            engine: None,
            sessions: HashMap::new(),
            next_session_id: 0,
            data_dir: Some(dir),
            settings,
            ai_status: AiRuntimeStatus::default(),
            bundled_defaults: Arc::new(bundled),
            history: Arc::new(history),
            recent,
            user_dict: Arc::new(RwLock::new(ud)),
            telemetry: Arc::new(Mutex::new(RuntimeTelemetry::default())),
        };
        if let Some(eng) = engine {
            srv.set_engine(eng);
        }
        srv
    }

    /// 同梱 1,094 件同音語規則ハンドルを返す。
    pub fn bundled_defaults(&self) -> Arc<ContextHistory> {
        Arc::clone(&self.bundled_defaults)
    }

    /// ユーザー文脈学習履歴ハンドルを返す。
    pub fn user_history(&self) -> Arc<ContextHistory> {
        Arc::clone(&self.history)
    }

    /// LM ワーカーへ渡す共有テレメトリハンドル。
    pub fn telemetry_handle(&self) -> SharedTelemetry {
        Arc::clone(&self.telemetry)
    }

    /// 最新 AI テレメトリを `AiRuntimeStatus` へ同期する。
    fn sync_ai_status(&mut self) {
        if let Ok(t) = self.telemetry.lock() {
            if !t.state.is_empty() {
                self.ai_status.state = t.state.clone();
            }
            if !t.active_models.is_empty() {
                self.ai_status.model = t.active_models.clone();
            }
            if !t.device.is_empty() {
                self.ai_status.device = t.device.clone();
            }
            if !t.gpu_name.is_empty() {
                self.ai_status.gpu = t.gpu_name.clone();
            }
            self.ai_status.vram_gb = t.vram_gb;
            self.ai_status.infer_calls = t.infer_calls;
            self.ai_status.infer_ms = t.infer_ms;
        }
    }

    /// 共有ユーザー辞書ハンドル (設定再読込・変換器の共有に使う)。
    pub fn user_dictionary(&self) -> Arc<RwLock<UserDictionary>> {
        Arc::clone(&self.user_dict)
    }

    /// 現在の設定参照。
    pub fn settings(&self) -> &ServerSettings {
        &self.settings
    }

    fn reload_disk_state(&mut self) {
        if let Some(dir) = &self.data_dir {
            let settings_path = dir.join("asteria_settings.json");
            if settings_path.is_file() {
                self.settings = ServerSettings::load_from(&settings_path);
            }
            let ud_path = dir.join("user_dictionary.tsv");
            if let Ok(mut guard) = self.user_dict.write() {
                let mut fresh = UserDictionary::new();
                if ud_path.is_file() {
                    let _ = fresh.load_file(&ud_path);
                }
                *guard = fresh;
            }
        }
        for entry in self.sessions.values_mut() {
            if let Some(s) = &mut entry.session {
                Engine::apply_settings_to_session(s, &self.settings);
            }
        }
        self.flush_ai_status_file();
    }

    /// 変換の部品を後から持たせる(辞書をバックグラウンドで読み込んだあと)。
    /// 既存のセッションは次のキーから変換を始める。
    pub fn set_engine(&mut self, engine: Engine) {
        if engine.rerank.is_some() {
            self.ai_status.state = "ready".to_owned();
        }
        self.engine = Some(engine);
        self.flush_ai_status_file();
    }

    fn flush_ai_status_file(&mut self) {
        self.sync_ai_status();
        if let Some(dir) = &self.data_dir {
            let _ = write_ai_status_file(dir, &self.ai_status);
        }
    }

    fn settings_path(&self) -> Option<PathBuf> {
        self.data_dir
            .as_ref()
            .map(|d| d.join("asteria_settings.json"))
    }

    /// 1つの要求を処理して応答本体を返す。バージョン確認は [`serve`] が済ませている前提。
    pub fn handle(&mut self, body: Option<request::Body>) -> response::Body {
        match body {
            Some(request::Body::CreateSession(req)) => {
                self.next_session_id += 1;
                let id = self.next_session_id;
                let entry = SessionEntry {
                    scope: req.input_scope(),
                    session: None,
                    pending_context: None,
                    pending_following: None,
                    pending_rerank: None,
                };
                self.sessions.insert(id, entry);
                response::Body::SessionCreated(ipc::SessionCreated { session_id: id })
            }
            Some(request::Body::DeleteSession(req)) => {
                match self.sessions.remove(&req.session_id) {
                    Some(_) => response::Body::Ack(ipc::Ack {}),
                    None => unknown_session(req.session_id),
                }
            }
            Some(request::Body::SendKey(req)) => self.send_key(&req),
            Some(request::Body::SendCommand(req)) => self.send_command(&req),
            Some(request::Body::SetContext(req)) => self.set_context(&req),
            Some(request::Body::PollUpdate(req)) => self.poll_update(&req),
            Some(request::Body::GetConfig(_)) => response::Body::Config(self.settings.to_proto()),
            Some(request::Body::SetConfig(req)) => {
                self.settings.apply_toml_or_json(&req.toml);
                if let Some(path) = self.settings_path() {
                    let _ = self.settings.save_to(&path);
                }
                for entry in self.sessions.values_mut() {
                    if let Some(s) = &mut entry.session {
                        Engine::apply_settings_to_session(s, &self.settings);
                    }
                }
                response::Body::Ack(ipc::Ack {})
            }
            Some(request::Body::Reload(_)) => {
                self.reload_disk_state();
                response::Body::Ack(ipc::Ack {})
            }
            Some(request::Body::GetStatus(_)) => {
                self.sync_ai_status();
                response::Body::Status(self.ai_status.to_proto())
            }
            Some(request::Body::GetRecentConversions(_)) => {
                let entries = self
                    .recent
                    .recent()
                    .into_iter()
                    .map(|e| ipc::RecentConversionEntry {
                        time: e.time,
                        first_kept: e.first_kept,
                        reading: e.reading,
                        committed: e.committed,
                        preceding: e.preceding,
                        candidates: e.candidates,
                    })
                    .collect();
                response::Body::RecentConversions(ipc::RecentConversionsResponse { entries })
            }
            Some(request::Body::ClearRecentConversions(_)) => {
                self.recent.clear();
                response::Body::Ack(ipc::Ack {})
            }
            None => error(ErrorCode::InvalidRequest, "要求の本体がない"),
        }
    }

    /// 左文脈・後続文脈をセッションに持たせる。パスワード欄の文脈は受け取らない(REQ-10-3)。
    fn set_context(&mut self, req: &ipc::SetContext) -> response::Body {
        let Some(entry) = self.sessions.get_mut(&req.session_id) else {
            return unknown_session(req.session_id);
        };
        if entry.scope != ipc::InputScope::Password {
            match &mut entry.session {
                Some(session) => {
                    session.set_left_context(&req.left_context);
                    session.set_following_text(&req.following_text);
                }
                None => {
                    entry.pending_context = Some(req.left_context.clone());
                    entry.pending_following = Some(req.following_text.clone());
                }
            }
        }
        response::Body::Ack(ipc::Ack {})
    }

    /// 遅れて終わったリランクで表示が変わったかを返す(docs/adr/0008)。変わっていなければ
    /// consumed = false の空の Output。
    fn poll_update(&mut self, req: &ipc::PollUpdate) -> response::Body {
        let Some(entry) = self.sessions.get_mut(&req.session_id) else {
            return unknown_session(req.session_id);
        };
        let no_change = response::Body::Output(ipc::Output::default());
        let (Some(engine), Some(session)) = (&self.engine, entry.session.as_mut()) else {
            return no_change;
        };
        let (Some(rerank), Some(p)) = (&engine.rerank, entry.pending_rerank.take()) else {
            return no_change;
        };
        let (bundled_defaults, user_history) = (
            Arc::clone(&self.bundled_defaults),
            Arc::clone(&self.history),
        );
        let outcome = match p.pending.wait(Duration::ZERO) {
            Ok(o) => o,
            Err(pending) => {
                entry.pending_rerank = Some(PendingRerank { pending, ..p });
                return no_change;
            }
        };
        let converter = &*engine.converter;
        let hist_refs = [bundled_defaults.as_ref(), user_history.as_ref()];
        let applied = catch_unwind(AssertUnwindSafe(|| match outcome {
            Outcome::Ready(scores) => apply_rerank(
                rerank,
                session,
                converter,
                &p.reading,
                &p.sentences,
                &scores,
                &hist_refs,
            ),
            Outcome::ReadyGenerated(gen_res) => apply_rerank_gen(
                rerank,
                session,
                converter,
                &p.reading,
                &p.sentences,
                &gen_res,
                &hist_refs,
            ),
            Outcome::Cancelled => false,
        }));
        match applied {
            Ok(true) => response::Body::Output(to_proto(session.view())),
            _ => no_change,
        }
    }

    /// セッションの左文脈(テストと診断用)。セッションがなければ `None`。
    pub fn left_context(&self, session_id: u64) -> Option<&str> {
        let entry = self.sessions.get(&session_id)?;
        Some(match &entry.session {
            Some(session) => session.left_context(),
            None => entry.pending_context.as_deref().unwrap_or_default(),
        })
    }

    fn send_key(&mut self, req: &ipc::SendKey) -> response::Body {
        let m = req.modifiers.unwrap_or_default();
        let key = Key {
            vk: req.virtual_key,
            shift: m.shift,
            ctrl: m.ctrl,
            alt: m.alt,
        };
        if req.key_up {
            return self.pass_through(req.session_id);
        }
        self.with_session(req.session_id, |session, converter| {
            session.key(key, &req.text, converter)
        })
    }

    fn send_command(&mut self, req: &ipc::SendCommand) -> response::Body {
        if req.kind() == ipc::CommandKind::ClearUserHistory {
            self.history.clear();
            return response::Body::Ack(ipc::Ack {});
        }
        let command = match req.kind() {
            ipc::CommandKind::SelectCandidate => Command::SelectIndex(req.argument as usize),
            ipc::CommandKind::Commit => Command::Commit,
            ipc::CommandKind::Cancel => Command::CancelInput,
            ipc::CommandKind::FocusNextSegment => Command::FocusNext,
            ipc::CommandKind::FocusPrevSegment => Command::FocusPrev,
            ipc::CommandKind::ResizeSegment => {
                if (req.argument as i32) < 0 {
                    Command::ShrinkSegment
                } else {
                    Command::ExpandSegment
                }
            }
            ipc::CommandKind::CommitPrediction => Command::CommitPrediction,
            _ => return error(ErrorCode::Unimplemented, "このコマンドは未実装"),
        };
        self.with_session(req.session_id, |session, converter| {
            session.command(command, converter)
        })
    }

    /// セッションの状態機械で処理する。パスワード欄(REQ-10-3)と、変換の部品がないときは
    /// キーをアプリへ渡す。処理中のパニックはそのセッションだけを作り直して閉じ込める(REQ-4-5)。
    fn with_session(
        &mut self,
        session_id: u64,
        f: impl FnOnce(&mut Session, &dyn Converter) -> asteria_session::Output,
    ) -> response::Body {
        let Some(entry) = self.sessions.get_mut(&session_id) else {
            return unknown_session(session_id);
        };
        let Some(engine) = &self.engine else {
            return pass_through();
        };
        if entry.scope == ipc::InputScope::Password {
            return pass_through();
        }
        let settings = &self.settings;
        let pending_ctx = &mut entry.pending_context;
        let pending_follow = &mut entry.pending_following;
        let session = entry.session.get_or_insert_with(|| {
            let mut session = engine.session(settings);
            if let Some(context) = pending_ctx.take() {
                session.set_left_context(&context);
            }
            if let Some(follow) = pending_follow.take() {
                session.set_following_text(&follow);
            }
            session
        });
        let scope = entry.scope;
        let converter = &*engine.converter;
        // 新しいキーでは、待っているリランクを捨てて進行中の推論を打ち切る(REQ-4-4)。
        if entry.pending_rerank.take().is_some() {
            if let Some(rerank) = &engine.rerank {
                rerank.reranker.generation().advance();
            }
        }
        let pending_rerank = &mut entry.pending_rerank;
        let pre_left = session.left_context().to_owned();
        let bundled_defaults = Arc::clone(&self.bundled_defaults);
        let history = Arc::clone(&self.history);
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            let out = f(session, converter);
            if session.just_converted() {
                if let Some(reading) = session.untouched_reading().map(str::to_owned) {
                    if let Some(learned) = history.exact_sentence(&reading, &pre_left) {
                        let sent = Sentence {
                            segments: learned
                                .keys
                                .iter()
                                .zip(&learned.values)
                                .map(|(k, v)| (k.chars().count(), v.clone()))
                                .collect(),
                            cost: 0,
                        };
                        if session.apply_sentence(&reading, &sent, converter) {
                            return asteria_session::Output {
                                committed: out.committed,
                                ..session.view()
                            };
                        }
                    }
                }
            }
            match &engine.rerank {
                Some(rerank) if self.settings.ai_enabled && session.just_converted() => {
                    let hist_refs = [bundled_defaults.as_ref(), history.as_ref()];
                    rerank_now(rerank, session, converter, out, pending_rerank, &hist_refs)
                }
                Some(rerank) if self.settings.ai_enabled => {
                    if let Some(comp_reading) = session.composing_reading() {
                        rerank
                            .reranker
                            .prewarm(session.left_context().to_owned(), comp_reading);
                    }
                    out
                }
                _ => out,
            }
        }));
        match outcome {
            Ok(out) => {
                if let Some(commit) = session.take_last_commit() {
                    let now = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .map_or(0, |d| d.as_secs() as i64);
                    if !self.settings.incognito {
                        // ADR0082: 文脈・文全体の選択学習 (同じ間違いを二度しない)
                        let mut sentence_cues = cues_of(&pre_left, 2);
                        for v in &commit.segment_values {
                            for cue in cues_of(v, 2) {
                                if !sentence_cues.contains(&cue) {
                                    sentence_cues.push(cue);
                                }
                            }
                        }
                        let mut changed_any = false;
                        for ((seg_r, seg_v), seg_first) in commit
                            .segment_readings
                            .iter()
                            .zip(&commit.segment_values)
                            .zip(&commit.segment_firsts)
                        {
                            if seg_v != seg_first {
                                changed_any = true;
                                history.learn_checked(LearnSegmentRequest {
                                    scope,
                                    incognito: self.settings.incognito,
                                    reading: seg_r,
                                    chosen: seg_v,
                                    rejected: seg_first,
                                    cues: &sentence_cues,
                                    now,
                                });
                            }
                        }
                        if changed_any {
                            let sent = LearnedSentence {
                                keys: commit.segment_readings.clone(),
                                values: commit.segment_values.clone(),
                            };
                            history.learn_sentence_checked(
                                scope,
                                self.settings.incognito,
                                &commit.reading,
                                &pre_left,
                                &sent,
                                now,
                            );
                            let _ = history.save();
                        }
                    }
                    if self.settings.record_recent_conversions {
                        self.recent.record_checked(
                            scope,
                            self.settings.incognito,
                            RecentConversion {
                                time: now.to_string(),
                                first_kept: commit.committed == commit.ai_first,
                                reading: commit.reading,
                                committed: commit.committed,
                                preceding: pre_left,
                                candidates: commit.candidates,
                            },
                        );
                    }
                }
                self.flush_ai_status_file();
                response::Body::Output(to_proto(out))
            }
            Err(_) => {
                *session = engine.session(settings);
                pass_through()
            }
        }
    }

    fn pass_through(&self, session_id: u64) -> response::Body {
        if self.sessions.contains_key(&session_id) {
            pass_through()
        } else {
            unknown_session(session_id)
        }
    }
}

/// 変換したばかりの文をリランクに出し、締め切りまで待つ。間に合えば並べ替えた表示を返し、
/// 間に合わなければ `pending` に置いてラティスの順の表示を返す。
fn rerank_now(
    rerank: &Rerank,
    session: &mut Session,
    converter: &dyn Converter,
    out: asteria_session::Output,
    pending: &mut Option<PendingRerank>,
    histories: &[&ContextHistory],
) -> asteria_session::Output {
    let Some(reading) = session.untouched_reading().map(str::to_owned) else {
        return out;
    };
    let sentences = converter.sentences(&reading, RERANK_K);
    if sentences.len() < 2 {
        return out;
    }
    let request = RerankRequest {
        left_context: session.left_context().to_owned(),
        reading: reading.clone(),
        candidates: sentences.iter().map(Sentence::surface).collect(),
    };
    match rerank
        .reranker
        .submit_with_gen(request)
        .wait(DEFAULT_DEADLINE)
    {
        Ok(Outcome::Ready(scores)) => {
            if apply_rerank(
                rerank, session, converter, &reading, &sentences, &scores, histories,
            ) {
                // 変換のキーでは確定文字列はないので、表示だけを差し替える。
                return asteria_session::Output {
                    committed: out.committed,
                    ..session.view()
                };
            }
            out
        }
        Ok(Outcome::ReadyGenerated(gen_res)) => {
            if apply_rerank_gen(
                rerank, session, converter, &reading, &sentences, &gen_res, histories,
            ) {
                return asteria_session::Output {
                    committed: out.committed,
                    ..session.view()
                };
            }
            out
        }
        Ok(Outcome::Cancelled) => out,
        Err(p) => {
            *pending = Some(PendingRerank {
                pending: p,
                reading,
                sentences,
            });
            asteria_session::Output {
                callback_delay_ms: 50,
                ..out
            }
        }
    }
}

/// LM + 生成候補のスコアで文を選び、ラティスの第1候補と違えば状態機械に当てる。
fn apply_rerank_gen(
    rerank: &Rerank,
    session: &mut Session,
    converter: &dyn Converter,
    reading: &str,
    sentences: &[Sentence],
    gen_res: &GenerationResult,
    histories: &[&ContextHistory],
) -> bool {
    if gen_res.scores.is_empty() {
        return false;
    }
    let combined: Vec<f32> = if gen_res.includes_llm || gen_res.pure_lm_ranking {
        gen_res.scores.clone()
    } else {
        let base_cost = sentences.first().map_or(0, |s| s.cost);
        (0..gen_res.candidates.len())
            .map(|i| {
                let cost = sentences.get(i).map_or(base_cost, |s| s.cost);
                rerank.weights.combine(gen_res.scores[i], cost, 0.0)
            })
            .collect()
    };
    let left = session.left_context().to_owned();
    let best = rerank_with_histories(
        histories,
        reading,
        &left,
        &gen_res.candidates,
        &combined,
        CONTEXT_HISTORY_MARGIN,
    );
    match best {
        Some(idx) if idx != 0 => {
            if let Some(sent) = sentences.get(idx) {
                session.apply_sentence(reading, sent, converter)
            } else if let Some(surf) = gen_res.candidates.get(idx) {
                let synth = Sentence {
                    segments: vec![(reading.chars().count(), surf.clone())],
                    cost: 0,
                };
                session.apply_sentence(reading, &synth, converter)
            } else {
                false
            }
        }
        _ => false,
    }
}

/// LM のスコアで文を選び、ラティスの第1候補と違えば状態機械に当てる。当てたら true。
fn apply_rerank(
    rerank: &Rerank,
    session: &mut Session,
    converter: &dyn Converter,
    reading: &str,
    sentences: &[Sentence],
    scores: &[f32],
    histories: &[&ContextHistory],
) -> bool {
    if scores.len() != sentences.len() || scores.is_empty() {
        return false;
    }
    let combined: Vec<f32> = scores
        .iter()
        .zip(sentences)
        .map(|(&s, sent)| rerank.weights.combine(s, sent.cost, 0.0))
        .collect();
    let surfaces: Vec<String> = sentences.iter().map(Sentence::surface).collect();
    let left = session.left_context().to_owned();
    match rerank_with_histories(
        histories,
        reading,
        &left,
        &surfaces,
        &combined,
        CONTEXT_HISTORY_MARGIN,
    ) {
        Some(best) if best != 0 => session.apply_sentence(reading, &sentences[best], converter),
        _ => false,
    }
}

/// 同梱のシステム辞書の既定の置き場所(12.1)。
pub fn default_dict_path() -> Option<std::path::PathBuf> {
    const FILE: &str = "system.dict";
    if cfg!(windows) {
        std::env::var_os("ProgramFiles").map(|p| {
            std::path::PathBuf::from(p)
                .join("Asteria")
                .join("data")
                .join(FILE)
        })
    } else if cfg!(target_os = "macos") {
        // アプリバンドルに入れるまでの仮の置き場所。
        Some(std::path::PathBuf::from("/Library/Application Support/Asteria").join(FILE))
    } else {
        Some(std::path::PathBuf::from("/usr/share/asteria").join(FILE))
    }
}

/// `--dict` がないときに探すシステム辞書の場所を、探す順に返す(12.1)。
///
/// 先頭は実行ファイル `exe` の隣の `data/system.dict`。32 bit のアプリから起動されても
/// 環境変数に左右されず、配置した場所の辞書を読む。次に [`default_dict_path`] を見る。
pub fn dict_candidates(exe: Option<&std::path::Path>) -> Vec<std::path::PathBuf> {
    exe.and_then(std::path::Path::parent)
        .map(|dir| dir.join("data").join("system.dict"))
        .into_iter()
        .chain(default_dict_path())
        .collect()
}

/// LM のモデル(`just zenz` で作る GGUF)のファイル名。
pub const MODEL_FILE: &str = "zenz-v2.5-small-f16.gguf";

/// `--model` がないときに探す LM のモデルの場所を、探す順に返す(辞書と同じ `data` フォルダ)。
pub fn model_candidates(exe: Option<&std::path::Path>) -> Vec<std::path::PathBuf> {
    dict_candidates(exe)
        .into_iter()
        .filter_map(|p| p.parent().map(|dir| dir.join(MODEL_FILE)))
        .collect()
}

/// コマンドラインの引数。
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Args {
    pub dict: Option<std::path::PathBuf>,
    pub model: Option<std::path::PathBuf>,
}

impl Args {
    /// `--dict <パス>` と `--model <パス>` を読む(順は問わない)。
    pub fn parse(args: &[String]) -> Result<Self, String> {
        let mut out = Self::default();
        let mut it = args.iter();
        while let Some(flag) = it.next() {
            let slot = match flag.as_str() {
                "--dict" => &mut out.dict,
                "--model" => &mut out.model,
                other => return Err(format!("知らない引数: {other}")),
            };
            let value = it.next().ok_or_else(|| format!("{flag} の値がない"))?;
            *slot = Some(std::path::PathBuf::from(value));
        }
        Ok(out)
    }
}

/// キーを処理せず、アプリへ渡させる応答。
fn pass_through() -> response::Body {
    response::Body::Output(ipc::Output {
        consumed: false,
        input_mode: ipc::InputMode::Direct.into(),
        ..Default::default()
    })
}

fn to_proto(out: asteria_session::Output) -> ipc::Output {
    let annotations = out.candidate_annotations;
    ipc::Output {
        consumed: out.consumed,
        preedit: out
            .preedit
            .into_iter()
            .map(|(text, attr)| ipc::PreeditSegment {
                text,
                attribute: match attr {
                    Attribute::Input => ipc::SegmentAttribute::Input,
                    Attribute::Converted => ipc::SegmentAttribute::Converted,
                    Attribute::Focused => ipc::SegmentAttribute::Focused,
                }
                .into(),
            })
            .collect(),
        cursor: u32::try_from(out.cursor).unwrap_or(u32::MAX),
        committed_text: out.committed,
        candidate_window: out.candidate_window.map(|w| ipc::CandidateWindow {
            candidates: w
                .candidates
                .into_iter()
                .enumerate()
                .map(|(i, text)| ipc::Candidate {
                    text,
                    annotation: annotations.get(i).cloned().unwrap_or_default(),
                })
                .collect(),
            focused_index: u32::try_from(w.focused).unwrap_or(0),
            visible: true,
        }),
        input_mode: ipc::InputMode::Hiragana.into(),
        callback_delay_ms: out.callback_delay_ms,
        refresh_suggestion: out.refresh_suggestion,
        live_conversion_active: out.live_conversion_active,
        suggestion_text: out.suggestion_text,
        show_candidate_window: out.show_candidate_window,
    }
}

fn error(code: ErrorCode, message: &str) -> response::Body {
    response::Body::Error(ipc::Error {
        code: code.into(),
        message: message.into(),
    })
}

fn unknown_session(session_id: u64) -> response::Body {
    error(
        ErrorCode::UnknownSession,
        &format!("セッション {session_id} は存在しない"),
    )
}

/// 1本の接続を、相手が閉じるまで処理する。
///
/// メジャーバージョンが一致しない要求にはエラーを返して接続を閉じる(4.2)。
pub fn serve<S: Read + Write>(server: &Mutex<Server>, stream: &mut S) -> Result<(), FrameError> {
    while let Some(req) = read_message::<_, Request>(stream)? {
        let major = req.protocol_version.map(|v| v.major);
        if major != Some(PROTOCOL_MAJOR) {
            let body = error(
                ErrorCode::VersionMismatch,
                &format!("プロトコルのメジャーバージョン不一致: {major:?} != {PROTOCOL_MAJOR}"),
            );
            write_message(stream, &reply(req.request_id, body))?;
            return Ok(());
        }
        let body = match server.lock() {
            Ok(mut s) => s.handle(req.body),
            Err(_) => error(ErrorCode::Unspecified, "サーバー状態が壊れている"),
        };
        write_message(stream, &reply(req.request_id, body))?;
    }
    Ok(())
}

fn reply(request_id: u64, body: response::Body) -> Response {
    Response {
        protocol_version: Some(protocol_version()),
        request_id,
        body: Some(body),
    }
}

/// UNIX ドメインソケットで待ち受け、接続ごとにスレッドを立てて処理する。
///
/// ソケットのパーミッションは 0600 にする(4.2)。既存のソケットファイルは置き換える。
/// 呼び出し元は `path` の親ディレクトリを専用にすること(0700 に変更する)。
/// 既存のソケットを置き換えるので、先に [`InstanceGuard`] を取っておくこと(REQ-4-1)。
#[cfg(unix)]
pub fn listen_unix(
    path: &std::path::Path,
    server: std::sync::Arc<Mutex<Server>>,
) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::net::UnixListener;

    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
    }
    match std::fs::remove_file(path) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e),
        _ => {}
    }
    // 0600 にしてから本来の名前へ移し、緩いパーミッションで見える瞬間をなくす。
    let staging = path.with_extension("sock.new");
    let _ = std::fs::remove_file(&staging);
    let listener = UnixListener::bind(&staging)?;
    std::fs::set_permissions(&staging, std::fs::Permissions::from_mode(0o600))?;
    std::fs::rename(&staging, path)?;
    for stream in listener.incoming() {
        let mut stream = stream?;
        let server = std::sync::Arc::clone(&server);
        std::thread::spawn(move || {
            if let Err(e) = serve(&server, &mut stream) {
                eprintln!("asteria-server: 接続エラー: {e}");
            }
        });
    }
    Ok(())
}

/// 名前付きパイプで待ち受け、接続ごとにスレッドを立てて処理する(4.2)。
///
/// パイプの DACL は現在のユーザーだけに接続を許す。同名のパイプが既にあれば失敗する。
#[cfg(windows)]
pub fn listen_pipe(name: &str, server: std::sync::Arc<Mutex<Server>>) -> std::io::Result<()> {
    let mut listener = asteria_client::PipeListener::bind(name)?;
    loop {
        let mut stream = listener.accept()?;
        let server = std::sync::Arc::clone(&server);
        std::thread::spawn(move || {
            if let Err(e) = serve(&server, &mut stream) {
                eprintln!("asteria-server: 接続エラー: {e}");
            }
        });
    }
}
