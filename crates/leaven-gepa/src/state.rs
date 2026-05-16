//! GEPA reference candidate and validation-frontier state.

use std::collections::{BTreeMap, BTreeSet};

use leaven_evidence::{CasewiseEvidence, ScalarEvidence};
use leaven_kernel::{AssessmentId, CandidateId, CaseId};
use serde::{Deserialize, Serialize};

/// Stable GEPA candidate index in discovery order.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
pub struct GepaCandidateIndex(u32);

impl GepaCandidateIndex {
    /// Build a GEPA candidate index from a discovery-order value.
    #[must_use]
    pub const fn new(index: u32) -> Self {
        Self(index)
    }

    /// Raw discovery-order index. The seed candidate is always `0`.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// One accepted candidate in GEPA reference state.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GepaCandidateRecord {
    index: GepaCandidateIndex,
    candidate: CandidateId,
    parents: Vec<GepaCandidateIndex>,
    discovery_metric_calls: u64,
    validation_score: Option<f64>,
    validation_rows: Vec<AssessmentId>,
}

impl GepaCandidateRecord {
    /// GEPA discovery-order index.
    #[must_use]
    pub const fn index(&self) -> GepaCandidateIndex {
        self.index
    }

    /// Candidate id in graph truth.
    #[must_use]
    pub const fn candidate(&self) -> CandidateId {
        self.candidate
    }

    /// Parent GEPA indices.
    #[must_use]
    pub fn parents(&self) -> &[GepaCandidateIndex] {
        &self.parents
    }

    /// Metric calls spent when this candidate was admitted.
    #[must_use]
    pub const fn discovery_metric_calls(&self) -> u64 {
        self.discovery_metric_calls
    }

    /// Aggregate validation score.
    #[must_use]
    pub const fn validation_score(&self) -> Option<f64> {
        self.validation_score
    }

    /// Validation assessment rows backing this candidate.
    #[must_use]
    pub fn validation_rows(&self) -> &[AssessmentId] {
        &self.validation_rows
    }
}

/// GEPA-owned reference state used for validation-frontier selection and reports.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct GepaReferenceState {
    records: Vec<GepaCandidateRecord>,
    candidate_to_index: BTreeMap<CandidateId, GepaCandidateIndex>,
    validation_subscores: Vec<BTreeMap<CaseId, ScalarEvidence>>,
    validation_frontier_scores: BTreeMap<CaseId, ScalarEvidence>,
    validation_frontier_candidates: BTreeMap<CaseId, BTreeSet<GepaCandidateIndex>>,
    total_metric_calls: u64,
    full_validation_evals: u64,
}

impl GepaReferenceState {
    /// Accepted candidate records in GEPA discovery order.
    #[must_use]
    pub fn records(&self) -> &[GepaCandidateRecord] {
        &self.records
    }

    /// Per-validation-case frontier membership.
    #[must_use]
    pub const fn validation_frontier(&self) -> &BTreeMap<CaseId, BTreeSet<GepaCandidateIndex>> {
        &self.validation_frontier_candidates
    }

    /// Total new evaluator metric calls charged to GEPA search.
    #[must_use]
    pub const fn total_metric_calls(&self) -> u64 {
        self.total_metric_calls
    }

    /// Number of full validation evaluations GEPA has run.
    #[must_use]
    pub const fn full_validation_evals(&self) -> u64 {
        self.full_validation_evals
    }

    pub(crate) fn index_of(&self, candidate: CandidateId) -> Option<GepaCandidateIndex> {
        self.candidate_to_index.get(&candidate).copied()
    }

    pub(crate) fn best_candidate(&self) -> Option<CandidateId> {
        self.records
            .iter()
            .filter_map(|record| {
                record
                    .validation_score
                    .map(|score| (score, record.candidate))
            })
            .max_by(|left, right| left.0.total_cmp(&right.0))
            .map(|(_, candidate)| candidate)
    }

    pub(crate) fn select_by_validation_frontier_frequency(
        &self,
    ) -> Option<(GepaCandidateIndex, CandidateId)> {
        let mut frequencies = BTreeMap::<GepaCandidateIndex, usize>::new();
        for candidates in self.validation_frontier_candidates.values() {
            for candidate in candidates {
                *frequencies.entry(*candidate).or_default() += 1;
            }
        }
        let (index, _) = frequencies.into_iter().max_by(|left, right| {
            let left_score = self
                .record(left.0)
                .and_then(GepaCandidateRecord::validation_score)
                .unwrap_or(f64::NEG_INFINITY);
            let right_score = self
                .record(right.0)
                .and_then(GepaCandidateRecord::validation_score)
                .unwrap_or(f64::NEG_INFINITY);
            left.1
                .cmp(&right.1)
                .then_with(|| left_score.total_cmp(&right_score))
                .then_with(|| right.0.cmp(&left.0))
        })?;
        Some((index, self.record(index)?.candidate()))
    }

    pub(crate) fn add_validated_candidate(
        &mut self,
        candidate: CandidateId,
        parents: Vec<GepaCandidateIndex>,
        discovery_metric_calls: u64,
        validation_score: f64,
        validation_rows: Vec<AssessmentId>,
        scalar_evidence: &CasewiseEvidence<ScalarEvidence>,
    ) -> GepaCandidateIndex {
        if let Some(index) = self.index_of(candidate) {
            return index;
        }
        let index = GepaCandidateIndex::new(
            u32::try_from(self.records.len()).expect("GEPA candidate count fits u32"),
        );
        let mut subscores = BTreeMap::new();
        for outcome in scalar_evidence.outcomes() {
            let case = outcome.case();
            let score = *outcome.evidence();
            subscores.insert(case, score);
            match self.validation_frontier_scores.get(&case).copied() {
                None => {
                    self.validation_frontier_scores.insert(case, score);
                    self.validation_frontier_candidates
                        .insert(case, BTreeSet::from([index]));
                }
                Some(best) if score.score() > best.score() => {
                    self.validation_frontier_scores.insert(case, score);
                    self.validation_frontier_candidates
                        .insert(case, BTreeSet::from([index]));
                }
                Some(best) if (score.score() - best.score()).abs() < f64::EPSILON => {
                    self.validation_frontier_candidates
                        .entry(case)
                        .or_default()
                        .insert(index);
                }
                Some(_) => {}
            }
        }
        self.candidate_to_index.insert(candidate, index);
        self.validation_subscores.push(subscores);
        self.records.push(GepaCandidateRecord {
            index,
            candidate,
            parents,
            discovery_metric_calls,
            validation_score: Some(validation_score),
            validation_rows,
        });
        index
    }

    pub(crate) fn add_unvalidated_candidate(
        &mut self,
        candidate: CandidateId,
        parents: Vec<GepaCandidateIndex>,
    ) -> GepaCandidateIndex {
        if let Some(index) = self.index_of(candidate) {
            return index;
        }
        let index = GepaCandidateIndex::new(
            u32::try_from(self.records.len()).expect("GEPA candidate count fits u32"),
        );
        self.candidate_to_index.insert(candidate, index);
        self.validation_subscores.push(BTreeMap::new());
        self.records.push(GepaCandidateRecord {
            index,
            candidate,
            parents,
            discovery_metric_calls: self.total_metric_calls,
            validation_score: None,
            validation_rows: Vec::new(),
        });
        index
    }

    pub(crate) fn add_metric_calls(&mut self, calls: u64) {
        self.total_metric_calls = self.total_metric_calls.saturating_add(calls);
    }

    pub(crate) fn note_full_validation(&mut self) {
        self.full_validation_evals = self.full_validation_evals.saturating_add(1);
    }

    fn record(&self, index: GepaCandidateIndex) -> Option<&GepaCandidateRecord> {
        self.records.get(usize::try_from(index.get()).ok()?)
    }
}
