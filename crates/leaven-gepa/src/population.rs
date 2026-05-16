//! Population adapters used by the reusable GEPA loop.

use leaven_core::PartitionId;
use leaven_engine::PopulationEvent;
use leaven_evidence::{CasewiseEvidence, ScalarEvidence};
use leaven_kernel::{AssessmentId, CandidateId, PopulationId};
use leaven_population::{KeepBest, ParetoFrontier};
use serde::{Serialize, de::DeserializeOwned};

/// Population behavior the reusable GEPA loop needs.
pub trait GepaPopulation {
    /// Population identifier for events.
    fn id(&self) -> PopulationId;
    /// Current best candidate.
    fn best(&self) -> Option<CandidateId>;
    /// Observe casewise scalar evidence.
    fn observe_gepa(
        &mut self,
        partition: Option<&PartitionId>,
        candidate: CandidateId,
        assessments: &[AssessmentId],
        evidence: &CasewiseEvidence<ScalarEvidence>,
    ) -> Vec<PopulationEvent>;
}

/// Private population state that must survive GEPA checkpoint/restore.
pub trait CheckpointPopulation {
    /// Serializable state shape.
    type State: Serialize + DeserializeOwned;

    /// Capture population state.
    fn checkpoint_state(&self) -> Self::State;

    /// Restore population state.
    fn restore_state(&mut self, state: Self::State);
}

impl GepaPopulation for ParetoFrontier {
    fn id(&self) -> PopulationId {
        self.id()
    }

    fn best(&self) -> Option<CandidateId> {
        self.best()
    }

    fn observe_gepa(
        &mut self,
        partition: Option<&PartitionId>,
        candidate: CandidateId,
        assessments: &[AssessmentId],
        evidence: &CasewiseEvidence<ScalarEvidence>,
    ) -> Vec<PopulationEvent> {
        let Some(assessment) = assessments.first().copied() else {
            return vec![PopulationEvent::Ignored {
                population: self.id(),
                candidate,
                reason: "casewise evidence had no assessment source rows".to_owned(),
            }];
        };
        match partition {
            Some(partition) => {
                self.observe_partitioned_casewise_scalar(partition, candidate, assessment, evidence)
            }
            None => self.observe_casewise_scalar(candidate, assessment, evidence),
        }
    }
}

impl CheckpointPopulation for ParetoFrontier {
    type State = Self;

    fn checkpoint_state(&self) -> Self::State {
        self.clone()
    }

    fn restore_state(&mut self, state: Self::State) {
        *self = state;
    }
}

impl GepaPopulation for KeepBest {
    fn id(&self) -> PopulationId {
        self.id()
    }

    fn best(&self) -> Option<CandidateId> {
        self.best()
    }

    fn observe_gepa(
        &mut self,
        _partition: Option<&PartitionId>,
        candidate: CandidateId,
        assessments: &[AssessmentId],
        evidence: &CasewiseEvidence<ScalarEvidence>,
    ) -> Vec<PopulationEvent> {
        let Some(assessment) = assessments.first().copied() else {
            return vec![PopulationEvent::Ignored {
                population: self.id(),
                candidate,
                reason: "casewise evidence had no assessment source rows".to_owned(),
            }];
        };
        let Some(score) = average_scalar(evidence) else {
            return vec![PopulationEvent::Ignored {
                population: self.id(),
                candidate,
                reason: "casewise evidence had no comparable score".to_owned(),
            }];
        };
        self.observe(
            candidate,
            assessment,
            ScalarEvidence::new(score).expect("finite average"),
        )
    }
}

impl CheckpointPopulation for KeepBest {
    type State = Self;

    fn checkpoint_state(&self) -> Self::State {
        self.clone()
    }

    fn restore_state(&mut self, state: Self::State) {
        *self = state;
    }
}

fn average_scalar(evidence: &CasewiseEvidence<ScalarEvidence>) -> Option<f64> {
    if evidence.outcomes().is_empty() {
        return None;
    }
    let total: f64 = evidence
        .outcomes()
        .iter()
        .map(|outcome| outcome.evidence().score())
        .sum();
    let count = u32::try_from(evidence.outcomes().len()).expect("case count fits into u32");
    Some(total / f64::from(count))
}

#[cfg(test)]
mod tests {
    use leaven_evidence::{CaseOutcome, CasewiseEvidence, ScalarEvidence};
    use leaven_kernel::{AssessmentId, CandidateId, CaseId};
    use leaven_population::{KeepBest, ParetoFrontier};

    use super::GepaPopulation;

    #[test]
    fn pareto_frontier_gepa_population_reports_missing_assessment_rows() {
        let candidate = CandidateId::new();
        let evidence = CasewiseEvidence::new(vec![CaseOutcome::new(
            CaseId::new(0),
            ScalarEvidence::new(1.0).expect("finite score"),
        )]);
        let mut frontier = ParetoFrontier::default();

        let events = GepaPopulation::observe_gepa(&mut frontier, None, candidate, &[], &evidence);

        assert_eq!(events.len(), 1);
        let event = format!("{:?}", events[0]);
        assert!(event.contains(&candidate.to_string()));
        assert!(event.contains("no assessment"));
        assert_eq!(GepaPopulation::best(&frontier), None);
    }

    #[test]
    fn keep_best_gepa_population_reports_missing_assessment_rows() {
        let candidate = CandidateId::new();
        let evidence = CasewiseEvidence::new(vec![CaseOutcome::new(
            CaseId::new(0),
            ScalarEvidence::new(1.0).expect("finite score"),
        )]);
        let mut keep_best = KeepBest::new();

        let events = GepaPopulation::observe_gepa(&mut keep_best, None, candidate, &[], &evidence);

        assert_eq!(events.len(), 1);
        let event = format!("{:?}", events[0]);
        assert!(event.contains(&candidate.to_string()));
        assert!(event.contains("no assessment"));
        assert_eq!(GepaPopulation::best(&keep_best), None);
    }

    #[test]
    fn keep_best_gepa_population_reports_empty_casewise_evidence() {
        let candidate = CandidateId::new();
        let evidence = CasewiseEvidence::new(Vec::new());
        let mut keep_best = KeepBest::new();

        let events = GepaPopulation::observe_gepa(
            &mut keep_best,
            None,
            candidate,
            &[AssessmentId::new()],
            &evidence,
        );

        assert_eq!(events.len(), 1);
        let event = format!("{:?}", events[0]);
        assert!(event.contains(&candidate.to_string()));
        assert!(event.contains("no comparable score"));
        assert_eq!(GepaPopulation::best(&keep_best), None);
    }

    #[test]
    fn keep_best_gepa_population_restores_checkpoint_state() {
        let candidate = CandidateId::new();
        let assessment = AssessmentId::new();
        let evidence = CasewiseEvidence::new(vec![
            CaseOutcome::new(CaseId::new(0), ScalarEvidence::new(0.25).unwrap()),
            CaseOutcome::new(CaseId::new(1), ScalarEvidence::new(0.75).unwrap()),
        ]);
        let mut keep_best = KeepBest::new();
        assert!(
            !GepaPopulation::observe_gepa(
                &mut keep_best,
                None,
                candidate,
                &[assessment],
                &evidence,
            )
            .is_empty()
        );
        let state = <KeepBest as super::CheckpointPopulation>::checkpoint_state(&keep_best);

        let mut restored = KeepBest::new();
        <KeepBest as super::CheckpointPopulation>::restore_state(&mut restored, state);

        assert_eq!(GepaPopulation::best(&restored), Some(candidate));
    }
}
