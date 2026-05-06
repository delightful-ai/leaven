//! Single-objective population that keeps the best scalar-scored candidate.

use leaven_evidence::ScalarEvidence;
use leaven_kernel::{AssessmentId, CandidateId, PopulationId};

#[derive(Clone, Debug)]
pub struct KeepBest {
    id: PopulationId,
    best: Option<ScoredCandidate>,
}

impl KeepBest {
    #[must_use]
    pub fn new() -> Self {
        Self {
            id: PopulationId::new(),
            best: None,
        }
    }

    #[must_use]
    pub const fn id(&self) -> PopulationId {
        self.id
    }

    #[must_use]
    pub const fn best(&self) -> Option<CandidateId> {
        match &self.best {
            Some(best) => Some(best.candidate),
            None => None,
        }
    }

    #[must_use]
    pub fn best_score(&self) -> Option<f64> {
        self.best.as_ref().map(|best| best.score.score())
    }

    #[must_use]
    pub const fn best_assessment(&self) -> Option<AssessmentId> {
        match &self.best {
            Some(best) => Some(best.assessment),
            None => None,
        }
    }

    pub fn observe(
        &mut self,
        candidate: CandidateId,
        assessment: AssessmentId,
        score: ScalarEvidence,
    ) -> Vec<leaven_engine::PopulationEvent> {
        let next = ScoredCandidate {
            candidate,
            assessment,
            score,
        };
        match &self.best {
            None => {
                self.best = Some(next);
                vec![leaven_engine::PopulationEvent::Inserted {
                    population: self.id,
                    candidate,
                    reason: "first scalar assessment".to_owned(),
                }]
            }
            Some(current) if score.score() > current.score.score() => {
                let old = current.candidate;
                self.best = Some(next);
                vec![leaven_engine::PopulationEvent::Replaced {
                    population: self.id,
                    old,
                    new: candidate,
                    reason: "higher scalar score".to_owned(),
                }]
            }
            Some(_) => vec![leaven_engine::PopulationEvent::Ignored {
                population: self.id,
                candidate,
                reason: "score did not improve current best".to_owned(),
            }],
        }
    }
}

impl Default for KeepBest {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug)]
struct ScoredCandidate {
    candidate: CandidateId,
    assessment: AssessmentId,
    score: ScalarEvidence,
}
