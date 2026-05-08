//! Evaluation cache vocabulary.

use std::collections::HashMap;

use leaven_core::{CacheIdentity, CaseSetVersion};
use leaven_kernel::{AssessmentId, CaseId, Fingerprint};

/// Cache behavior requested for an evaluation.
#[derive(Clone, Debug, Default, Eq, PartialEq, Hash)]
pub enum CachePolicy {
    /// Do not read or write the evaluation cache.
    #[default]
    Never,
    /// Cache deterministic results using the evaluator fingerprint and request.
    Deterministic,
    /// Cache deterministic results with an explicit seed in the policy.
    DeterministicWithSeed(u64),
    /// Cache under a caller-provided fingerprint.
    UserKey(Fingerprint),
}

/// Whether an evaluation request used the cache.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum CacheStatus {
    /// Existing cache entry supplied the assessment IDs.
    Hit,
    /// Cache was checked and no entry existed.
    Miss,
    /// Cache was not consulted.
    Bypassed,
}

/// Key used for engine-owned evaluation caching.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct EvaluationCacheKey {
    /// Fingerprint of evaluator behavior.
    pub evaluator: Fingerprint,
    /// Cache policy that participates in key identity.
    pub policy: CachePolicy,
    /// Concrete case-set version used to resolve the request.
    pub case_set_version: CaseSetVersion,
    /// Resolved case IDs.
    pub case_ids: Vec<CaseId>,
    /// Candidate cache identities in request order.
    pub candidates: Vec<CacheIdentity>,
}

/// In-memory evaluation cache.
#[derive(Default)]
pub struct EvaluationCache {
    entries: HashMap<EvaluationCacheKey, Vec<AssessmentId>>,
}

impl EvaluationCache {
    /// Returns cached assessment IDs for a key.
    #[must_use]
    pub fn get(&self, key: &EvaluationCacheKey) -> Option<&Vec<AssessmentId>> {
        self.entries.get(key)
    }

    /// Inserts cached assessment IDs for a key.
    pub fn insert(&mut self, key: EvaluationCacheKey, value: Vec<AssessmentId>) {
        self.entries.insert(key, value);
    }
}
