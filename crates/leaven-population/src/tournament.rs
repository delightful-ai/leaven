//! Pairwise tournament population with fitted Bradley-Terry state.

use std::{cmp::Ordering, collections::BTreeMap};

use leaven_engine::PopulationEvent;
use leaven_evidence::{PairwiseJudgment, PairwiseJudgmentEvidence};
use leaven_kernel::{AssessmentId, CandidateId, FiniteF64, PopulationId};

/// Online Bradley-Terry-style ability fit for pairwise judgments.
#[derive(Clone, Debug)]
pub struct BradleyTerryFit {
    learning_rate: FiniteF64,
    abilities: BTreeMap<CandidateId, FiniteF64>,
}

impl BradleyTerryFit {
    /// Create a fit with the supplied finite learning rate.
    #[must_use]
    pub fn new(learning_rate: FiniteF64) -> Self {
        Self {
            learning_rate,
            abilities: BTreeMap::new(),
        }
    }

    /// Return a candidate's current ability. Unseen candidates start at zero.
    #[must_use]
    pub fn ability(&self, candidate: CandidateId) -> FiniteF64 {
        self.abilities
            .get(&candidate)
            .copied()
            .unwrap_or(FiniteF64::ZERO)
    }

    /// Observe one pairwise judgment and update both candidates.
    pub fn observe_pairwise(
        &mut self,
        left: CandidateId,
        right: CandidateId,
        judgment: PairwiseJudgment,
    ) {
        let left_ability = self.ability(left).as_f64();
        let right_ability = self.ability(right).as_f64();
        let expected_left = logistic(left_ability - right_ability);
        let observed_left = match judgment {
            PairwiseJudgment::Left => 1.0,
            PairwiseJudgment::Right => 0.0,
            PairwiseJudgment::Tie => 0.5,
        };
        let delta = self.learning_rate.as_f64() * (observed_left - expected_left);
        self.set_ability(left, left_ability + delta);
        self.set_ability(right, right_ability - delta);
    }

    /// Return the highest-ability candidate, if any candidate has been observed.
    #[must_use]
    pub fn best(&self) -> Option<CandidateId> {
        self.abilities
            .iter()
            .max_by(|(left_id, left), (right_id, right)| {
                ability_order(**left, **right).then_with(|| left_id.cmp(right_id).reverse())
            })
            .map(|(candidate, _ability)| *candidate)
    }

    fn set_ability(&mut self, candidate: CandidateId, ability: f64) {
        let finite = FiniteF64::new(ability).expect("Bradley-Terry update remains finite");
        self.abilities.insert(candidate, finite);
    }
}

impl Default for BradleyTerryFit {
    fn default() -> Self {
        Self::new(FiniteF64::new(0.1).expect("default learning rate is finite"))
    }
}

/// Population state for pairwise tournament optimizers.
#[derive(Clone, Debug)]
pub struct TournamentPopulation {
    id: PopulationId,
    fit: BradleyTerryFit,
    observations: usize,
}

impl TournamentPopulation {
    /// Create a tournament population around fitted pairwise state.
    #[must_use]
    pub fn new(fit: BradleyTerryFit) -> Self {
        Self {
            id: PopulationId::new(),
            fit,
            observations: 0,
        }
    }

    /// Population identifier for graph events.
    #[must_use]
    pub const fn id(&self) -> PopulationId {
        self.id
    }

    /// Number of pairwise observations consumed by this population.
    #[must_use]
    pub const fn observations(&self) -> usize {
        self.observations
    }

    /// Current fitted ability for a candidate.
    #[must_use]
    pub fn ability(&self, candidate: CandidateId) -> FiniteF64 {
        self.fit.ability(candidate)
    }

    /// Observe one pairwise assessment and update fitted state.
    pub fn observe_pairwise(
        &mut self,
        left: CandidateId,
        right: CandidateId,
        _assessment: AssessmentId,
        evidence: &PairwiseJudgmentEvidence,
    ) -> Vec<PopulationEvent> {
        self.fit.observe_pairwise(left, right, evidence.judgment());
        self.observations += 1;
        vec![
            PopulationEvent::Reweighted {
                population: self.id,
                candidate: left,
                weight: self.fit.ability(left),
                reason: "pairwise tournament observation".to_owned(),
            },
            PopulationEvent::Reweighted {
                population: self.id,
                candidate: right,
                weight: self.fit.ability(right),
                reason: "pairwise tournament observation".to_owned(),
            },
        ]
    }

    /// Return the highest-ability candidate, if any.
    #[must_use]
    pub fn best(&self) -> Option<CandidateId> {
        self.fit.best()
    }
}

impl Default for TournamentPopulation {
    fn default() -> Self {
        Self::new(BradleyTerryFit::default())
    }
}

fn logistic(x: f64) -> f64 {
    if x >= 0.0 {
        1.0 / (1.0 + (-x).exp())
    } else {
        let exp = x.exp();
        exp / (1.0 + exp)
    }
}

fn ability_order(left: FiniteF64, right: FiniteF64) -> Ordering {
    left.partial_cmp(&right)
        .expect("FiniteF64 values are comparable")
}
