//! `Evaluator` — runs candidates against the world and returns
//! evidence.
//!
//! Stub: the full async signature, fingerprint plumbing, and cache
//! policy land with the engine.

use serde::{Deserialize, Serialize};

use crate::ids::EvaluatorId;

/// Stable fingerprint over an evaluator's behaviour. Part of cache
/// keys: changing the fingerprint invalidates cached assessments.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct Fingerprint(pub String);

/// When evaluation results may be reused.
#[derive(Clone, Debug, Default, Eq, PartialEq, Hash)]
pub enum CachePolicy {
    #[default]
    Never,
    Deterministic,
    DeterministicWithSeed(u64),
    UserKey(Fingerprint),
}

/// Marker trait until the full async surface lands.
pub trait Evaluator: Send + Sync {
    fn id(&self) -> EvaluatorId;
    fn fingerprint(&self) -> Fingerprint;
    fn cache_policy(&self) -> CachePolicy {
        CachePolicy::Never
    }
}
