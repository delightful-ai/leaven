//! leaven-evidence crate skeleton.

pub mod attribution {
    use leaven_core::Evidence;
    pub trait AttributableEvidence<K>: Evidence {
        fn attributions(&self) -> Vec<Attribution<K>>;
        fn evidence_for(&self, key: &K) -> Option<String>;
    }
    #[derive(Clone, Debug)]
    pub struct Attribution<K> {
        pub key: K,
        pub weight: Option<f64>,
        pub note: Option<String>,
    }
    pub trait AttributionKey: Eq + std::hash::Hash + Clone + Send + Sync + 'static {}
    impl<T> AttributionKey for T where T: Eq + std::hash::Hash + Clone + Send + Sync + 'static {}
}
pub mod command {
    pub struct CommandEvidence;
    pub struct CommandRecord;
}
pub mod diff {
    pub struct DiffEvidence;
    pub struct RenderedDiff;
}
pub mod json {
    pub struct JsonEvidence;
}
pub mod listwise {
    pub struct ListwiseRankingEvidence;
    pub struct RankingItem;
}
pub mod mixed {
    pub struct MixedEvidence;
}
pub mod pairwise {
    pub struct PairwiseJudgment;
    pub struct PairwiseJudgmentEvidence;
}
pub mod scalar;
pub mod score_vector {
    pub enum Direction {
        Higher,
        Lower,
    }
    pub struct RawScoreValue;
    pub struct ScoreAxis;
    pub struct ScorePoint;
    pub struct ScoreVectorEvidence;
}
pub mod string {
    pub struct StringEvidence;
}
pub use attribution::{AttributableEvidence, Attribution, AttributionKey};
pub use command::{CommandEvidence, CommandRecord};
pub use diff::{DiffEvidence, RenderedDiff};
pub use json::JsonEvidence;
pub use listwise::{ListwiseRankingEvidence, RankingItem};
pub use mixed::MixedEvidence;
pub use pairwise::{PairwiseJudgment, PairwiseJudgmentEvidence};
pub use scalar::{ScalarEvidence, ScalarEvidenceError};
pub use score_vector::{Direction, RawScoreValue, ScoreAxis, ScorePoint, ScoreVectorEvidence};
pub use string::StringEvidence;
pub mod prelude {
    pub use crate::{
        AttributableEvidence, CommandEvidence, DiffEvidence, Direction, JsonEvidence,
        ListwiseRankingEvidence, MixedEvidence, PairwiseJudgmentEvidence, ScalarEvidence,
        ScalarEvidenceError, ScoreVectorEvidence, StringEvidence,
    };
}
