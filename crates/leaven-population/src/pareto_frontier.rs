//! Casewise Pareto frontier population state.

use std::collections::{BTreeMap, BTreeSet};

use leaven_core::PartitionId;
use leaven_engine::PopulationEvent;
use leaven_evidence::{CasewiseEvidence, ScalarEvidence};
use leaven_kernel::{AssessmentId, CandidateId, CaseId, PopulationId};

/// Case partition filter applied before frontier updates.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum PartitionFilter {
    /// Accept every observation.
    #[default]
    All,
    /// Accept observations from these partitions.
    Only(BTreeSet<PartitionId>),
}

impl From<BTreeSet<PartitionId>> for PartitionFilter {
    fn from(value: BTreeSet<PartitionId>) -> Self {
        Self::Only(value)
    }
}

/// Builder for a casewise Pareto frontier.
#[derive(Clone, Debug, Default)]
pub struct ParetoFrontierBuilder {
    partition_filter: PartitionFilter,
}

impl ParetoFrontierBuilder {
    /// Set the partition filter applied before frontier updates.
    #[must_use]
    pub fn partition_filter(mut self, filter: impl Into<PartitionFilter>) -> Self {
        self.partition_filter = filter.into();
        self
    }

    /// Build frontier state.
    #[must_use]
    pub fn build(self) -> ParetoFrontier {
        ParetoFrontier {
            id: PopulationId::new(),
            partition_filter: self.partition_filter,
            scores: BTreeMap::new(),
            frontier: BTreeSet::new(),
        }
    }
}

/// Pareto frontier over sparse casewise scalar evidence.
#[derive(Clone, Debug)]
pub struct ParetoFrontier {
    id: PopulationId,
    partition_filter: PartitionFilter,
    scores: BTreeMap<CandidateId, BTreeMap<CaseId, ScalarEvidence>>,
    frontier: BTreeSet<CandidateId>,
}

impl ParetoFrontier {
    /// Build a frontier that treats case ids as Pareto axes.
    #[must_use]
    pub fn by_case() -> ParetoFrontierBuilder {
        ParetoFrontierBuilder::default()
    }

    /// Population identifier for graph events.
    #[must_use]
    pub const fn id(&self) -> PopulationId {
        self.id
    }

    /// Observe sparse casewise scalar evidence and recompute the frontier.
    pub fn observe_casewise_scalar(
        &mut self,
        candidate: CandidateId,
        _assessment: AssessmentId,
        evidence: &CasewiseEvidence<ScalarEvidence>,
    ) -> Vec<PopulationEvent> {
        self.observe_casewise_scalar_inner(None, candidate, evidence)
    }

    /// Observe sparse casewise scalar evidence from a named partition.
    ///
    /// If this frontier was built with a partition filter, non-matching
    /// observations are ignored before candidate scores or frontier membership
    /// can change.
    pub fn observe_partitioned_casewise_scalar(
        &mut self,
        partition: &PartitionId,
        candidate: CandidateId,
        _assessment: AssessmentId,
        evidence: &CasewiseEvidence<ScalarEvidence>,
    ) -> Vec<PopulationEvent> {
        self.observe_casewise_scalar_inner(Some(partition), candidate, evidence)
    }

    fn observe_casewise_scalar_inner(
        &mut self,
        partition: Option<&PartitionId>,
        candidate: CandidateId,
        evidence: &CasewiseEvidence<ScalarEvidence>,
    ) -> Vec<PopulationEvent> {
        if !self.accepts_partition(partition) {
            return vec![PopulationEvent::Ignored {
                population: self.id,
                candidate,
                reason: "observation excluded by pareto frontier partition filter".to_owned(),
            }];
        }
        let before = self.frontier.clone();
        let candidate_scores = self.scores.entry(candidate).or_default();
        for outcome in evidence.outcomes() {
            candidate_scores.insert(outcome.case(), *outcome.evidence());
        }
        self.recompute_frontier();

        let mut events = Vec::new();
        if !before.contains(&candidate) && self.frontier.contains(&candidate) {
            events.push(PopulationEvent::Inserted {
                population: self.id,
                candidate,
                reason: "candidate entered casewise pareto frontier".to_owned(),
            });
        } else if before.contains(&candidate) && self.frontier.contains(&candidate) {
            events.push(PopulationEvent::Reweighted {
                population: self.id,
                candidate,
                weight: self
                    .average_score(candidate)
                    .expect("frontier score exists"),
                reason: "candidate frontier score changed".to_owned(),
            });
        } else {
            events.push(PopulationEvent::Ignored {
                population: self.id,
                candidate,
                reason: "candidate is dominated on observed cases".to_owned(),
            });
        }

        for removed in before.difference(&self.frontier) {
            events.push(PopulationEvent::Removed {
                population: self.id,
                candidate: *removed,
                reason: "candidate left casewise pareto frontier".to_owned(),
            });
        }
        events
    }

    /// Return the deterministic best candidate among the frontier.
    #[must_use]
    pub fn best(&self) -> Option<CandidateId> {
        self.frontier
            .iter()
            .filter_map(|candidate| {
                self.average_score(*candidate)
                    .map(|score| (*candidate, score))
            })
            .max_by(|(left_id, left), (right_id, right)| {
                left.partial_cmp(right)
                    .expect("scalar evidence scores are finite")
                    .then_with(|| left_id.cmp(right_id).reverse())
            })
            .map(|(candidate, _score)| candidate)
    }

    /// Whether the candidate is currently on the frontier.
    #[must_use]
    pub fn contains(&self, candidate: CandidateId) -> bool {
        self.frontier.contains(&candidate)
    }

    /// Current partition filter.
    #[must_use]
    pub const fn partition_filter(&self) -> &PartitionFilter {
        &self.partition_filter
    }

    fn accepts_partition(&self, partition: Option<&PartitionId>) -> bool {
        match &self.partition_filter {
            PartitionFilter::All => true,
            PartitionFilter::Only(allowed) => partition.is_some_and(|id| allowed.contains(id)),
        }
    }

    fn recompute_frontier(&mut self) {
        let mut next = BTreeSet::new();
        for candidate in self.scores.keys() {
            let dominated = self
                .scores
                .keys()
                .any(|other| other != candidate && self.dominates(*other, *candidate));
            if !dominated {
                next.insert(*candidate);
            }
        }
        self.frontier = next;
    }

    fn dominates(&self, left: CandidateId, right: CandidateId) -> bool {
        let Some(left_scores) = self.scores.get(&left) else {
            return false;
        };
        let Some(right_scores) = self.scores.get(&right) else {
            return false;
        };
        if right_scores.is_empty() {
            return false;
        }

        let mut strictly_better = false;
        for (case, right_score) in right_scores {
            let Some(left_score) = left_scores.get(case) else {
                return false;
            };
            if left_score.score() < right_score.score() {
                return false;
            }
            if left_score.score() > right_score.score() {
                strictly_better = true;
            }
        }
        strictly_better
    }

    fn average_score(&self, candidate: CandidateId) -> Option<leaven_kernel::FiniteF64> {
        let scores = self.scores.get(&candidate)?;
        if scores.is_empty() {
            return None;
        }
        let total: f64 = scores.values().map(ScalarEvidence::score).sum();
        let count =
            u32::try_from(scores.len()).expect("frontier case count fits into u32 for averaging");
        leaven_kernel::FiniteF64::new(total / f64::from(count)).ok()
    }
}

impl Default for ParetoFrontier {
    fn default() -> Self {
        Self::by_case().build()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dominance_refuses_candidates_with_missing_score_records() {
        let mut frontier = ParetoFrontier::default();
        let left = CandidateId::new();
        let right = CandidateId::new();

        assert!(!frontier.dominates(left, right));

        frontier.scores.insert(left, BTreeMap::new());
        assert!(!frontier.dominates(left, right));
    }
}
