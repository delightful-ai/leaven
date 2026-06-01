//! Evaluation cache vocabulary.

use std::collections::HashMap;

use leaven_core::{AssessmentGranularity, CacheIdentity, CaseSetVersion, EvaluationPurpose};
use leaven_kernel::{AssessmentId, CandidateId, CaseId, Fingerprint};
use serde::{Deserialize, Serialize};

/// Cache behavior requested for an evaluation.
#[derive(Clone, Debug, Default, Eq, PartialEq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
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
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum CacheStatus {
    /// Existing cache entry supplied the assessment IDs.
    Hit,
    /// Cache was checked and no entry existed.
    Miss,
    /// Cache was not consulted.
    Bypassed(CacheBypassReason),
}

/// Why an evaluation did not consult the cache.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum CacheBypassReason {
    /// The evaluator declared [`CachePolicy::Never`].
    DisabledByPolicy,
    /// The run context has no attached evaluation cache.
    CacheUnavailable,
    /// Deterministic caching was requested, but a candidate did not provide
    /// a cache-safe identity.
    MissingCandidateIdentity { candidate: CandidateId },
}

/// Evaluator-visible request shape that participates in evaluation-cache identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
pub enum EvaluationCacheRequestKind {
    /// Independent scoring of each candidate.
    Independent,
    /// Pairwise comparison where left/right order can affect evidence.
    OrderedPairwise,
    /// Pairwise comparison where the evaluator declares symmetric evidence.
    UnorderedPairwise,
    /// Listwise ranking of a candidate group.
    Listwise,
}

/// Key used for engine-owned evaluation caching.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
pub struct EvaluationCacheKey {
    /// Fingerprint of evaluator behavior.
    pub evaluator: Fingerprint,
    /// Cache policy that participates in key identity.
    pub policy: CachePolicy,
    /// Concrete case-set version used to resolve the request.
    pub case_set_version: CaseSetVersion,
    /// Resolved case IDs.
    pub case_ids: Vec<CaseId>,
    /// Evaluator-visible request kind.
    pub request_kind: EvaluationCacheRequestKind,
    /// Aggregate vs per-case evidence shape requested from the evaluator.
    pub granularity: AssessmentGranularity,
    /// Why the evaluation is being run.
    pub purpose: EvaluationPurpose,
    /// Candidate cache identities in request order.
    pub candidates: Vec<CacheIdentity>,
}

/// In-memory evaluation cache.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EvaluationCacheEntry {
    pub key: EvaluationCacheKey,
    pub assessment_ids: Vec<AssessmentId>,
}

/// Serializable cache index captured at a clean checkpoint boundary.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct EvaluationCacheSnapshot {
    pub entries: Vec<EvaluationCacheEntry>,
}

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

    /// Number of cache entries currently retained.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns true when the cache contains no entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Captures the cache index in deterministic key order.
    #[must_use]
    pub fn snapshot(&self) -> EvaluationCacheSnapshot {
        let mut entries = self
            .entries
            .iter()
            .map(|(key, assessment_ids)| EvaluationCacheEntry {
                key: key.clone(),
                assessment_ids: assessment_ids.clone(),
            })
            .collect::<Vec<_>>();
        entries.sort_by(|left, right| left.key.cmp(&right.key));
        EvaluationCacheSnapshot { entries }
    }

    /// Rebuilds a cache index from a checkpoint snapshot.
    #[must_use]
    pub fn from_snapshot(snapshot: EvaluationCacheSnapshot) -> Self {
        let entries = snapshot
            .entries
            .into_iter()
            .map(|entry| (entry.key, entry.assessment_ids))
            .collect();
        Self { entries }
    }
}

#[cfg(test)]
mod tests {
    use leaven_core::{AssessmentGranularity, CacheIdentity, CaseSetVersion, EvaluationPurpose};
    use leaven_kernel::{AssessmentId, CaseId, ContentId, Fingerprint};

    use crate::{CachePolicy, EvaluationCache, EvaluationCacheKey, EvaluationCacheRequestKind};

    #[test]
    fn cache_snapshot_round_trips_entries_for_resume() {
        let key = EvaluationCacheKey {
            evaluator: Fingerprint::from_bytes([1; 32]),
            policy: CachePolicy::Deterministic,
            case_set_version: CaseSetVersion("cases-v1".to_owned()),
            case_ids: vec![CaseId::new(1)],
            request_kind: EvaluationCacheRequestKind::Independent,
            granularity: AssessmentGranularity::Aggregate,
            purpose: EvaluationPurpose::Search,
            candidates: vec![CacheIdentity::Content(ContentId::from_bytes([2; 32]))],
        };
        let assessments = vec![AssessmentId::new(), AssessmentId::new()];
        let mut cache = EvaluationCache::default();
        cache.insert(key.clone(), assessments.clone());

        let snapshot = cache.snapshot();
        let restored = EvaluationCache::from_snapshot(snapshot);

        assert_eq!(restored.len(), 1);
        assert_eq!(restored.get(&key), Some(&assessments));
    }

    #[test]
    fn cache_snapshot_has_deterministic_key_order() {
        let first = EvaluationCacheKey {
            evaluator: Fingerprint::from_bytes([1; 32]),
            policy: CachePolicy::Deterministic,
            case_set_version: CaseSetVersion("cases-v1".to_owned()),
            case_ids: vec![CaseId::new(1)],
            request_kind: EvaluationCacheRequestKind::Independent,
            granularity: AssessmentGranularity::Aggregate,
            purpose: EvaluationPurpose::Search,
            candidates: vec![CacheIdentity::Content(ContentId::from_bytes([1; 32]))],
        };
        let second = EvaluationCacheKey {
            evaluator: Fingerprint::from_bytes([2; 32]),
            policy: CachePolicy::DeterministicWithSeed(3),
            case_set_version: CaseSetVersion("cases-v1".to_owned()),
            case_ids: vec![CaseId::new(2)],
            request_kind: EvaluationCacheRequestKind::Listwise,
            granularity: AssessmentGranularity::PerCase,
            purpose: EvaluationPurpose::Validation,
            candidates: vec![CacheIdentity::Content(ContentId::from_bytes([2; 32]))],
        };
        let mut cache = EvaluationCache::default();
        cache.insert(second.clone(), vec![AssessmentId::new()]);
        cache.insert(first.clone(), vec![AssessmentId::new()]);

        let keys = cache
            .snapshot()
            .entries
            .into_iter()
            .map(|entry| entry.key)
            .collect::<Vec<_>>();

        assert_eq!(keys, vec![first, second]);
    }
}
