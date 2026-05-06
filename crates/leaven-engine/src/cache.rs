//! Evaluation cache vocabulary.

use std::collections::HashMap;

use leaven_kernel::{AssessmentId, CandidateId, CaseId, Fingerprint};

#[derive(Clone, Debug, Default, Eq, PartialEq, Hash)]
pub enum CachePolicy {
    #[default]
    Never,
    Deterministic,
    DeterministicWithSeed(u64),
    UserKey(Fingerprint),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum CacheStatus {
    Hit,
    Miss,
    Bypassed,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct EvaluationCacheKey {
    pub evaluator: Fingerprint,
    pub policy: CachePolicy,
    pub case_set_version: String,
    pub case_ids: Vec<CaseId>,
    pub candidates: Vec<CandidateId>,
}

#[derive(Default)]
pub struct EvaluationCache {
    entries: HashMap<EvaluationCacheKey, Vec<AssessmentId>>,
}

impl EvaluationCache {
    #[must_use]
    pub fn get(&self, key: &EvaluationCacheKey) -> Option<&Vec<AssessmentId>> {
        self.entries.get(key)
    }

    pub fn insert(&mut self, key: EvaluationCacheKey, value: Vec<AssessmentId>) {
        self.entries.insert(key, value);
    }
}
