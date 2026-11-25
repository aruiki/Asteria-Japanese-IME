//! 予測・表記正規化・同音語ヒント・ニュアンス補完 (9章, ADR0079)。

pub mod homophone_hint;
pub mod notation;
pub mod nuance;
pub mod prediction;

pub use homophone_hint::{apply_homophone_hints, HintedCandidate};
pub use notation::{
    normalize_notation, notation_rule_count, promote_standard_notations, NotationRule,
};
pub use nuance::{generate_nuance_alternatives, NuanceAlternative};
pub use prediction::{predict_candidates, PredictionItem};
