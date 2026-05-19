//! Fixed-capacity scalar frontier.

use std::num::NonZeroUsize;

use leaven_evidence::ScalarEvidence;
use leaven_kernel::{AssessmentId, CandidateId, FiniteF64, PopulationId};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TopKFrontier {
    id: PopulationId,
    capacity: NonZeroUsize,
    members: Vec<ScoredMember>,
    ordered: Vec<CandidateId>,
}

impl TopKFrontier {
    #[must_use]
    pub fn new(capacity: NonZeroUsize) -> Self {
        Self {
            id: PopulationId::new(),
            capacity,
            members: Vec::new(),
            ordered: Vec::new(),
        }
    }

    #[must_use]
    pub const fn id(&self) -> PopulationId {
        self.id
    }

    #[must_use]
    pub const fn capacity(&self) -> NonZeroUsize {
        self.capacity
    }

    #[must_use]
    pub fn members(&self) -> &[CandidateId] {
        &self.ordered
    }

    #[must_use]
    pub fn contains(&self, candidate: CandidateId) -> bool {
        self.members
            .iter()
            .any(|member| member.candidate == candidate)
    }

    #[must_use]
    pub fn best(&self) -> Option<CandidateId> {
        self.ordered.first().copied()
    }

    #[must_use]
    pub fn weakest(&self) -> Option<CandidateId> {
        self.ordered.last().copied()
    }

    #[must_use]
    pub fn best_score(&self) -> Option<f64> {
        self.best()
            .and_then(|candidate| self.member_score(candidate))
    }

    #[must_use]
    pub fn member_score(&self, candidate: CandidateId) -> Option<f64> {
        self.members
            .iter()
            .find(|member| member.candidate == candidate)
            .map(|member| member.score.score())
    }

    pub fn observe(
        &mut self,
        candidate: CandidateId,
        assessment: AssessmentId,
        score: ScalarEvidence,
    ) -> Vec<leaven_engine::PopulationEvent> {
        if let Some(index) = self
            .members
            .iter()
            .position(|member| member.candidate == candidate)
        {
            self.members[index] = ScoredMember {
                candidate,
                assessment,
                score,
            };
            self.reorder();
            return vec![leaven_engine::PopulationEvent::Reweighted {
                population: self.id,
                candidate,
                weight: FiniteF64::new(score.score()).expect("ScalarEvidence is finite"),
                reason: "existing top-k frontier member score updated".to_owned(),
            }];
        }

        if self.members.len() < self.capacity.get() {
            self.members.push(ScoredMember {
                candidate,
                assessment,
                score,
            });
            self.reorder();
            return vec![leaven_engine::PopulationEvent::Inserted {
                population: self.id,
                candidate,
                reason: "top-k frontier had free capacity".to_owned(),
            }];
        }

        let Some((weakest_index, weakest)) = self.weakest_member() else {
            return Vec::new();
        };
        if score.score() <= weakest.score.score() {
            return vec![leaven_engine::PopulationEvent::Ignored {
                population: self.id,
                candidate,
                reason: "score did not beat weakest top-k frontier member".to_owned(),
            }];
        }

        let old = weakest.candidate;
        self.members[weakest_index] = ScoredMember {
            candidate,
            assessment,
            score,
        };
        self.reorder();
        vec![leaven_engine::PopulationEvent::Replaced {
            population: self.id,
            old,
            new: candidate,
            reason: "score beat weakest top-k frontier member".to_owned(),
        }]
    }

    fn weakest_member(&self) -> Option<(usize, &ScoredMember)> {
        self.members
            .iter()
            .enumerate()
            .min_by(|(_, left), (_, right)| left.score.cmp(&right.score))
    }

    fn reorder(&mut self) {
        let mut members = self.members.clone();
        members.sort_by(|left, right| {
            right
                .score
                .cmp(&left.score)
                .then_with(|| left.candidate.cmp(&right.candidate))
        });
        self.ordered = members.into_iter().map(|member| member.candidate).collect();
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct ScoredMember {
    candidate: CandidateId,
    assessment: AssessmentId,
    score: ScalarEvidence,
}
