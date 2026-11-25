//! ユーザー学習・文脈記憶・直近変換履歴・ユーザー辞書 (8章, ADR0075/0082, #212)。

pub mod context_history;
pub mod recent_conversions;
pub mod user_dict;

pub use context_history::{
    cues_of, default_context_entries, exact_cue, learning_allowed, rerank_with_histories,
    ContextEntry, ContextHistory, DefaultContextEntry, LearnSegmentRequest, LearnedSentence,
    MAX_CUES, MAX_ENTRIES, SENTENCE_READING,
};
pub use recent_conversions::{
    RecentConversion, RecentConversions, MAX_RECENT_CANDIDATES, MAX_RECENT_CHARS,
    MAX_RECENT_ENTRIES,
};
pub use user_dict::{UserDictEntry, UserDictionary};
