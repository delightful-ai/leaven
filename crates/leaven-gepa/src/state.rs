//! GEPA reference candidate and validation-frontier state.

use std::collections::{BTreeMap, BTreeSet};

use leaven_evidence::{CasewiseEvidence, ScalarEvidence};
use leaven_kernel::{AssessmentId, CandidateId, CaseId};
use serde::{Deserialize, Serialize};

use crate::validation::GepaRandom;

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
    selector_rng: GepaRandom,
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

    /// Per-candidate validation subscores in discovery order.
    #[must_use]
    pub fn validation_subscores(&self) -> &[BTreeMap<CaseId, ScalarEvidence>] {
        &self.validation_subscores
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
            .fold(
                None,
                |best: Option<(f64, CandidateId)>, current| match best {
                    Some(best) if best.0 >= current.0 => Some(best),
                    _ => Some(current),
                },
            )
            .map(|(_, candidate)| candidate)
    }

    #[cfg(test)]
    pub(crate) fn select_by_validation_frontier_frequency(
        &mut self,
    ) -> Option<(GepaCandidateIndex, CandidateId)> {
        let mut rng = std::mem::take(&mut self.selector_rng);
        let selected = self.select_by_validation_frontier_frequency_with_rng(&mut rng);
        self.selector_rng = rng;
        selected
    }

    pub(crate) fn select_by_validation_frontier_frequency_with_rng(
        &self,
        rng: &mut GepaRandom,
    ) -> Option<(GepaCandidateIndex, CandidateId)> {
        let fronts = self.non_dominated_validation_frontier();
        let mut frequencies = BTreeMap::<GepaCandidateIndex, usize>::new();
        for candidates in fronts.values() {
            for candidate in candidates {
                *frequencies.entry(*candidate).or_default() += 1;
            }
        }
        let mut sampling_list = Vec::new();
        for (candidate, frequency) in frequencies {
            sampling_list.extend(std::iter::repeat_n(candidate, frequency));
        }
        if sampling_list.is_empty() {
            return None;
        }
        let selected = rng.randbelow(sampling_list.len());
        let index = sampling_list[selected];
        Some((index, self.record(index)?.candidate()))
    }

    fn non_dominated_validation_frontier(&self) -> BTreeMap<CaseId, BTreeSet<GepaCandidateIndex>> {
        let mut frequencies = BTreeMap::<GepaCandidateIndex, usize>::new();
        for candidates in self.validation_frontier_candidates.values() {
            for candidate in candidates {
                *frequencies.entry(*candidate).or_default() += 1;
            }
        }
        let mut programs = frequencies.keys().copied().collect::<Vec<_>>();
        programs.sort_by(|left, right| {
            let left_score = self
                .record(*left)
                .and_then(GepaCandidateRecord::validation_score)
                .unwrap_or(1.0);
            let right_score = self
                .record(*right)
                .and_then(GepaCandidateRecord::validation_score)
                .unwrap_or(1.0);
            left_score
                .total_cmp(&right_score)
                .then_with(|| left.cmp(right))
        });
        let mut dominated = BTreeSet::<GepaCandidateIndex>::new();
        loop {
            let mut removed = false;
            for candidate in &programs {
                if dominated.contains(candidate) {
                    continue;
                }
                let remaining = programs
                    .iter()
                    .copied()
                    .filter(|program| program != candidate && !dominated.contains(program))
                    .collect::<BTreeSet<_>>();
                if is_dominated(*candidate, &remaining, &self.validation_frontier_candidates) {
                    dominated.insert(*candidate);
                    removed = true;
                    break;
                }
            }
            if !removed {
                break;
            }
        }
        self.validation_frontier_candidates
            .iter()
            .map(|(case, candidates)| {
                (
                    *case,
                    candidates
                        .iter()
                        .copied()
                        .filter(|candidate| !dominated.contains(candidate))
                        .collect(),
                )
            })
            .collect()
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

fn is_dominated(
    candidate: GepaCandidateIndex,
    programs: &BTreeSet<GepaCandidateIndex>,
    fronts: &BTreeMap<CaseId, BTreeSet<GepaCandidateIndex>>,
) -> bool {
    for front in fronts.values().filter(|front| front.contains(&candidate)) {
        if !front.iter().any(|other| programs.contains(other)) {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use leaven_evidence::{CaseOutcome, CasewiseEvidence, ScalarEvidence};
    use leaven_kernel::{AssessmentId, CandidateId, CaseId};

    use super::{GepaCandidateIndex, GepaReferenceState};

    fn scalar_rows(rows: &[(u64, f64)]) -> CasewiseEvidence<ScalarEvidence> {
        CasewiseEvidence::new(
            rows.iter()
                .map(|(case, score)| {
                    CaseOutcome::new(
                        CaseId::new(*case),
                        ScalarEvidence::new(*score).expect("finite score"),
                    )
                })
                .collect(),
        )
    }

    #[test]
    fn reference_state_preserves_records_frontier_ties_and_duplicate_admission() {
        let mut state = GepaReferenceState::default();
        assert!(state.records().is_empty());
        assert_eq!(state.validation_frontier().len(), 0);
        assert_eq!(state.total_metric_calls(), 0);
        assert_eq!(state.full_validation_evals(), 0);
        assert_eq!(state.best_candidate(), None);
        assert_eq!(state.select_by_validation_frontier_frequency(), None);

        state.add_metric_calls(4);
        state.note_full_validation();
        let seed = CandidateId::new();
        let seed_rows = vec![AssessmentId::new(), AssessmentId::new()];
        let seed_index = state.add_validated_candidate(
            seed,
            Vec::new(),
            state.total_metric_calls(),
            0.6,
            seed_rows.clone(),
            &scalar_rows(&[(0, 0.5), (1, 0.7)]),
        );
        assert_eq!(seed_index, GepaCandidateIndex::new(0));

        let child = CandidateId::new();
        let child_index = state.add_validated_candidate(
            child,
            vec![seed_index],
            9,
            0.55,
            vec![AssessmentId::new()],
            &scalar_rows(&[(0, 0.5), (1, 0.2), (2, 0.9)]),
        );
        assert_eq!(child_index, GepaCandidateIndex::new(1));

        assert_eq!(
            state.add_validated_candidate(
                child,
                Vec::new(),
                99,
                1.0,
                Vec::new(),
                &scalar_rows(&[(0, 1.0)]),
            ),
            child_index
        );
        assert_eq!(state.index_of(seed), Some(seed_index));
        assert_eq!(state.best_candidate(), Some(seed));
        let tied_child = CandidateId::new();
        let tied_child_index = state.add_validated_candidate(
            tied_child,
            vec![seed_index],
            11,
            0.6,
            vec![AssessmentId::new()],
            &scalar_rows(&[(0, 0.5), (1, 0.7)]),
        );
        assert_eq!(
            state.best_candidate(),
            Some(seed),
            "validation-score ties keep the earliest candidate like upstream full-eval best selection"
        );
        assert_eq!(state.full_validation_evals(), 1);
        let selected = state.select_by_validation_frontier_frequency();
        assert!(matches!(
            selected,
            Some((index, candidate))
                if (index, candidate) == (seed_index, seed)
                    || (index, candidate) == (child_index, child)
                    || (index, candidate) == (tied_child_index, tied_child)
        ));

        let seed_record = &state.records()[0];
        assert_eq!(seed_record.index(), seed_index);
        assert_eq!(seed_record.candidate(), seed);
        assert_eq!(seed_record.parents(), &[]);
        assert_eq!(seed_record.discovery_metric_calls(), 4);
        assert_eq!(seed_record.validation_score(), Some(0.6));
        assert_eq!(seed_record.validation_rows(), seed_rows.as_slice());
        assert!(
            state
                .validation_frontier()
                .get(&CaseId::new(0))
                .expect("case 0 frontier")
                .contains(&child_index)
        );

        let unvalidated = CandidateId::new();
        let unvalidated_index = state.add_unvalidated_candidate(unvalidated, vec![child_index]);
        assert_eq!(unvalidated_index, GepaCandidateIndex::new(3));
        assert_eq!(
            state.add_unvalidated_candidate(unvalidated, Vec::new()),
            unvalidated_index
        );
        let unvalidated_record = &state.records()[3];
        assert_eq!(unvalidated_record.parents(), &[child_index]);
        assert_eq!(unvalidated_record.validation_score(), None);
        assert_eq!(unvalidated_record.validation_rows(), &[]);
        assert_eq!(
            state
                .record(GepaCandidateIndex::new(u32::MAX))
                .map(super::GepaCandidateRecord::candidate),
            None
        );
    }

    #[test]
    fn reference_state_selector_repeats_candidates_by_frontier_frequency() {
        let mut state = GepaReferenceState::default();
        let candidate_zero = CandidateId::new();
        let candidate_one = CandidateId::new();
        state.add_validated_candidate(
            candidate_zero,
            Vec::new(),
            3,
            1.0,
            vec![AssessmentId::new()],
            &scalar_rows(&[(0, 1.0), (1, 0.0), (2, 0.0)]),
        );
        state.add_validated_candidate(
            candidate_one,
            Vec::new(),
            6,
            2.0,
            vec![AssessmentId::new()],
            &scalar_rows(&[(0, 0.0), (1, 1.0), (2, 1.0)]),
        );

        let fronts = state.non_dominated_validation_frontier();
        assert_eq!(
            fronts.get(&CaseId::new(0)).cloned().unwrap_or_default(),
            std::collections::BTreeSet::from([GepaCandidateIndex::new(0)])
        );
        assert_eq!(
            fronts.get(&CaseId::new(1)).cloned().unwrap_or_default(),
            std::collections::BTreeSet::from([GepaCandidateIndex::new(1)])
        );
        assert_eq!(
            fronts.get(&CaseId::new(2)).cloned().unwrap_or_default(),
            std::collections::BTreeSet::from([GepaCandidateIndex::new(1)])
        );

        assert_eq!(
            state.select_by_validation_frontier_frequency(),
            Some((GepaCandidateIndex::new(1), candidate_one))
        );
    }

    #[test]
    fn reference_state_selector_removes_dominated_candidates() {
        let mut state = GepaReferenceState::default();
        let dominated = CandidateId::new();
        let dominator = CandidateId::new();
        state.add_validated_candidate(
            dominated,
            Vec::new(),
            2,
            0.5,
            vec![AssessmentId::new()],
            &scalar_rows(&[(0, 1.0)]),
        );
        state.add_validated_candidate(
            dominator,
            Vec::new(),
            4,
            1.0,
            vec![AssessmentId::new()],
            &scalar_rows(&[(0, 1.0), (1, 1.0)]),
        );

        let fronts = state.non_dominated_validation_frontier();
        assert_eq!(
            fronts.get(&CaseId::new(0)).cloned().unwrap_or_default(),
            std::collections::BTreeSet::from([GepaCandidateIndex::new(1)])
        );
        assert_eq!(
            fronts.get(&CaseId::new(1)).cloned().unwrap_or_default(),
            std::collections::BTreeSet::from([GepaCandidateIndex::new(1)])
        );
        assert_eq!(
            state.select_by_validation_frontier_frequency(),
            Some((GepaCandidateIndex::new(1), dominator))
        );
    }

    #[test]
    fn reference_state_selector_rng_state_round_trips() {
        let mut state = GepaReferenceState::default();
        let candidate_zero = CandidateId::new();
        let candidate_one = CandidateId::new();
        state.add_validated_candidate(
            candidate_zero,
            Vec::new(),
            2,
            1.0,
            vec![AssessmentId::new()],
            &scalar_rows(&[(0, 1.0)]),
        );
        state.add_validated_candidate(
            candidate_one,
            Vec::new(),
            4,
            1.0,
            vec![AssessmentId::new()],
            &scalar_rows(&[(1, 1.0)]),
        );

        let mut restored = serde_json::from_value::<GepaReferenceState>(
            serde_json::to_value(&state).expect("reference state serializes"),
        )
        .expect("reference state restores");

        assert_eq!(
            restored.select_by_validation_frontier_frequency(),
            state.select_by_validation_frontier_frequency()
        );
        assert_eq!(
            restored.select_by_validation_frontier_frequency(),
            state.select_by_validation_frontier_frequency()
        );
    }
}
