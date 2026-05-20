//! Skill utility state for optimizer-owned retrieval bookkeeping.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use leaven_artifact_skill::SkillName;
use leaven_evidence::PairedRolloutEvidence;
use leaven_kernel::FiniteF64;
use serde::{Deserialize, Serialize};

/// Retrieval/use counters associated with one skill's utility state.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct SkillUseStats {
    /// Number of times a skill was retrieved for consideration.
    pub retrievals: u64,
    /// Number of times runtime evidence says the skill was triggered/used.
    pub triggers: u64,
    /// Number of utility observations folded into the EMA.
    pub utility_updates: u64,
}

impl SkillUseStats {
    fn record_retrieval(&mut self) {
        self.retrievals = self.retrievals.saturating_add(1);
    }

    fn record_trigger(&mut self) {
        self.triggers = self.triggers.saturating_add(1);
    }

    fn record_utility_update(&mut self) {
        self.utility_updates = self.utility_updates.saturating_add(1);
    }
}

/// EMA smoothing weight for skill utility updates.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "f64", into = "f64")]
pub struct SkillUtilitySmoothing(f64);

impl SkillUtilitySmoothing {
    /// No movement toward the new observation.
    pub const ZERO: Self = Self(0.0);
    /// Replace utility with the new observation.
    pub const ONE: Self = Self(1.0);

    /// Constructs a smoothing weight in the inclusive range `[0.0, 1.0]`.
    ///
    /// # Errors
    ///
    /// Returns [`SkillUtilitySmoothingError`] when the value is not finite or
    /// falls outside the EMA weight range.
    pub fn new(value: f64) -> Result<Self, SkillUtilitySmoothingError> {
        if !value.is_finite() {
            return Err(SkillUtilitySmoothingError::NonFinite { value });
        }
        if !(0.0..=1.0).contains(&value) {
            return Err(SkillUtilitySmoothingError::OutOfRange { value });
        }
        Ok(Self(value))
    }

    /// Returns zero smoothing.
    #[must_use]
    pub const fn zero() -> Self {
        Self::ZERO
    }

    /// Returns full replacement smoothing.
    #[must_use]
    pub const fn one() -> Self {
        Self::ONE
    }

    /// Returns the numeric smoothing weight.
    #[must_use]
    pub const fn as_f64(self) -> f64 {
        self.0
    }
}

impl TryFrom<f64> for SkillUtilitySmoothing {
    type Error = SkillUtilitySmoothingError;

    fn try_from(value: f64) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<SkillUtilitySmoothing> for f64 {
    fn from(value: SkillUtilitySmoothing) -> Self {
        value.0
    }
}

/// Error returned when constructing [`SkillUtilitySmoothing`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SkillUtilitySmoothingError {
    /// Value was NaN or infinite.
    NonFinite {
        /// Rejected value.
        value: f64,
    },
    /// Value was finite but outside `[0.0, 1.0]`.
    OutOfRange {
        /// Rejected value.
        value: f64,
    },
}

impl std::fmt::Display for SkillUtilitySmoothingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NonFinite { value } => {
                write!(f, "skill utility smoothing must be finite, got {value}")
            }
            Self::OutOfRange { value } => {
                write!(
                    f,
                    "skill utility smoothing must be between 0.0 and 1.0, got {value}"
                )
            }
        }
    }
}

impl std::error::Error for SkillUtilitySmoothingError {}

/// Result of applying one skill utility observation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SkillUtilityUpdate {
    skill: SkillName,
    utility_before: FiniteF64,
    utility_after: FiniteF64,
    stats_after: SkillUseStats,
}

impl SkillUtilityUpdate {
    /// Skill whose utility changed.
    #[must_use]
    pub const fn skill(&self) -> &SkillName {
        &self.skill
    }

    /// Utility before the observation.
    #[must_use]
    pub const fn utility_before(&self) -> FiniteF64 {
        self.utility_before
    }

    /// Utility after the observation.
    #[must_use]
    pub const fn utility_after(&self) -> FiniteF64 {
        self.utility_after
    }

    /// Counters after the observation.
    #[must_use]
    pub const fn stats_after(&self) -> SkillUseStats {
        self.stats_after
    }
}

/// One finite utility credit assigned to a validated skill identity.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SkillUtilityCredit {
    skill: SkillName,
    credit: FiniteF64,
}

impl SkillUtilityCredit {
    /// Build a utility credit for one skill.
    #[must_use]
    pub const fn new(skill: SkillName, credit: FiniteF64) -> Self {
        Self { skill, credit }
    }

    /// Credited skill.
    #[must_use]
    pub const fn skill(&self) -> &SkillName {
        &self.skill
    }

    /// Signed utility credit.
    #[must_use]
    pub const fn credit(&self) -> FiniteF64 {
        self.credit
    }
}

/// One skill-injected trajectory's reward and retrieved step skills.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SkillStepTrajectoryOutcome {
    trajectory_id: String,
    reward: FiniteF64,
    retrieved_step_skills: Vec<SkillName>,
}

impl SkillStepTrajectoryOutcome {
    /// Build a step-skill trajectory outcome.
    ///
    /// # Errors
    ///
    /// Returns [`SkillStepTrajectoryOutcomeError`] when the trajectory
    /// identity is blank after trimming.
    pub fn new(
        trajectory_id: impl Into<String>,
        reward: FiniteF64,
        retrieved_step_skills: Vec<SkillName>,
    ) -> Result<Self, SkillStepTrajectoryOutcomeError> {
        let trajectory_id = trajectory_id.into();
        if trajectory_id.trim().is_empty() {
            return Err(SkillStepTrajectoryOutcomeError::EmptyTrajectoryId);
        }

        Ok(Self {
            trajectory_id,
            reward,
            retrieved_step_skills,
        })
    }

    /// Runner-provided trajectory identity.
    #[must_use]
    pub fn trajectory_id(&self) -> &str {
        &self.trajectory_id
    }

    /// Terminal reward or success indicator for this skill-injected trajectory.
    #[must_use]
    pub const fn reward(&self) -> FiniteF64 {
        self.reward
    }

    /// Step skills retrieved by this trajectory, in runner observation order.
    #[must_use]
    pub fn retrieved_step_skills(&self) -> &[SkillName] {
        &self.retrieved_step_skills
    }
}

/// Refusal reasons for step-trajectory outcome construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SkillStepTrajectoryOutcomeError {
    /// The trajectory identity was blank.
    EmptyTrajectoryId,
}

impl std::fmt::Display for SkillStepTrajectoryOutcomeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyTrajectoryId => {
                f.write_str("skill step trajectory outcome requires a non-empty trajectory id")
            }
        }
    }
}

impl std::error::Error for SkillStepTrajectoryOutcomeError {}

/// D2Skill-style utility input derived from paired rollout evidence.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SkillPairedRolloutUtilityInput {
    paired_rollout: PairedRolloutEvidence,
    task_skills: Vec<SkillName>,
    step_skill_credits: Vec<SkillUtilityCredit>,
}

impl SkillPairedRolloutUtilityInput {
    /// Build utility-update input from a paired rollout and retrieved skills.
    ///
    /// # Errors
    ///
    /// Returns [`SkillPairedRolloutUtilityInputError`] when a task skill is
    /// repeated, which would make the group-level task delta ambiguous.
    pub fn new(
        paired_rollout: PairedRolloutEvidence,
        task_skills: Vec<SkillName>,
        step_skill_credits: Vec<SkillUtilityCredit>,
    ) -> Result<Self, SkillPairedRolloutUtilityInputError> {
        let mut seen = BTreeSet::new();
        for skill in &task_skills {
            if !seen.insert(skill.clone()) {
                return Err(SkillPairedRolloutUtilityInputError::DuplicateTaskSkill {
                    skill: skill.clone(),
                });
            }
        }

        Ok(Self {
            paired_rollout,
            task_skills,
            step_skill_credits,
        })
    }

    /// Build utility-update input from skill-injected trajectory outcomes.
    ///
    /// Each retrieved step skill receives the `D2Skill` trajectory-level credit
    /// `trajectory_reward - baseline_group_mean`.
    ///
    /// # Errors
    ///
    /// Returns [`SkillPairedRolloutUtilityInputError`] when a task skill is
    /// repeated.
    pub fn from_step_trajectories(
        paired_rollout: PairedRolloutEvidence,
        task_skills: Vec<SkillName>,
        step_trajectories: Vec<SkillStepTrajectoryOutcome>,
    ) -> Result<Self, SkillPairedRolloutUtilityInputError> {
        let baseline_mean = paired_rollout.baseline().mean_reward();
        let step_skill_credits = step_trajectories
            .into_iter()
            .flat_map(|trajectory| {
                let credit = FiniteF64::new(trajectory.reward.as_f64() - baseline_mean.as_f64())
                    .expect("difference between finite rewards remains finite");
                trajectory
                    .retrieved_step_skills
                    .into_iter()
                    .map(move |skill| SkillUtilityCredit::new(skill, credit))
            })
            .collect();

        Self::new(paired_rollout, task_skills, step_skill_credits)
    }

    /// Paired rollout evidence supplying the task-level reward gap.
    #[must_use]
    pub const fn paired_rollout(&self) -> &PairedRolloutEvidence {
        &self.paired_rollout
    }

    /// Treatment-minus-baseline task-level utility signal.
    #[must_use]
    pub fn task_delta(&self) -> FiniteF64 {
        self.paired_rollout.treatment_minus_baseline()
    }

    /// Retrieved task skills with the shared task-level utility signal.
    #[must_use]
    pub fn task_skill_credits(&self) -> Vec<SkillUtilityCredit> {
        let task_delta = self.task_delta();
        self.task_skills
            .iter()
            .cloned()
            .map(|skill| SkillUtilityCredit::new(skill, task_delta))
            .collect()
    }

    /// Step-skill utility credits supplied by trajectory-level credit assignment.
    #[must_use]
    pub fn step_skill_credits(&self) -> &[SkillUtilityCredit] {
        &self.step_skill_credits
    }

    /// Task and step utility credits in application order.
    #[must_use]
    pub fn all_utility_credits(&self) -> Vec<SkillUtilityCredit> {
        let mut credits = self.task_skill_credits();
        credits.extend(self.step_skill_credits.iter().cloned());
        credits
    }

    /// Apply task and step utility credits to optimizer-owned utility state.
    pub fn apply_to_state(
        &self,
        state: &mut SkillUtilityState,
        task_smoothing: SkillUtilitySmoothing,
        step_smoothing: SkillUtilitySmoothing,
    ) -> SkillPairedRolloutUtilityUpdates {
        let task_updates = self
            .task_skill_credits()
            .into_iter()
            .map(|credit| state.observe_delta(credit.skill, credit.credit, task_smoothing))
            .collect();
        let step_updates = self
            .step_skill_credits
            .iter()
            .cloned()
            .map(|credit| state.observe_delta(credit.skill, credit.credit, step_smoothing))
            .collect();

        SkillPairedRolloutUtilityUpdates {
            task_updates,
            step_updates,
        }
    }
}

/// Utility updates produced by applying paired rollout skill credits.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SkillPairedRolloutUtilityUpdates {
    task_updates: Vec<SkillUtilityUpdate>,
    step_updates: Vec<SkillUtilityUpdate>,
}

impl SkillPairedRolloutUtilityUpdates {
    /// Updates applied to retrieved task skills.
    #[must_use]
    pub fn task_updates(&self) -> &[SkillUtilityUpdate] {
        &self.task_updates
    }

    /// Updates applied to retrieved step skills.
    #[must_use]
    pub fn step_updates(&self) -> &[SkillUtilityUpdate] {
        &self.step_updates
    }
}

/// Refusal reasons for paired rollout utility inputs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SkillPairedRolloutUtilityInputError {
    /// A task skill appeared more than once.
    DuplicateTaskSkill {
        /// Repeated skill identity.
        skill: SkillName,
    },
}

impl std::fmt::Display for SkillPairedRolloutUtilityInputError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateTaskSkill { skill } => {
                write!(
                    f,
                    "paired rollout task skill `{skill}` was credited more than once"
                )
            }
        }
    }
}

impl std::error::Error for SkillPairedRolloutUtilityInputError {}

/// Outcome of transferring utility state across a skill rename.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum SkillUtilityTransfer {
    /// Source state moved to the target skill name.
    Transferred,
    /// No utility or use stats existed for the source skill.
    SourceMissing,
    /// The target already had utility or use stats, so no state moved.
    TargetExists,
}

/// Optimizer-owned skill utility and use counters keyed by validated skill name.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SkillUtilityState {
    utilities: BTreeMap<SkillName, FiniteF64>,
    stats: BTreeMap<SkillName, SkillUseStats>,
}

impl SkillUtilityState {
    /// Returns all tracked utilities.
    #[must_use]
    pub const fn utilities(&self) -> &BTreeMap<SkillName, FiniteF64> {
        &self.utilities
    }

    /// Returns all tracked use counters.
    #[must_use]
    pub const fn stats_by_skill(&self) -> &BTreeMap<SkillName, SkillUseStats> {
        &self.stats
    }

    /// Returns a skill's utility. Unseen skills start at zero.
    #[must_use]
    pub fn utility(&self, skill: &SkillName) -> FiniteF64 {
        self.utilities
            .get(skill)
            .copied()
            .unwrap_or(FiniteF64::ZERO)
    }

    /// Returns a skill's counters. Unseen skills have zero counters.
    #[must_use]
    pub fn stats(&self, skill: &SkillName) -> SkillUseStats {
        self.stats.get(skill).copied().unwrap_or_default()
    }

    /// Records that a skill was retrieved for possible use.
    pub fn record_retrieval(&mut self, skill: SkillName) {
        self.stats.entry(skill).or_default().record_retrieval();
    }

    /// Records that runtime evidence says a skill triggered.
    pub fn record_trigger(&mut self, skill: SkillName) {
        self.stats.entry(skill).or_default().record_trigger();
    }

    /// Folds one signed utility delta into the skill's EMA utility.
    pub fn observe_delta(
        &mut self,
        skill: SkillName,
        delta: FiniteF64,
        smoothing: SkillUtilitySmoothing,
    ) -> SkillUtilityUpdate {
        let utility_before = self.utility(&skill);
        let utility_after = ema(utility_before, delta, smoothing);
        let stats_after = {
            let stats = self.stats.entry(skill.clone()).or_default();
            stats.record_utility_update();
            *stats
        };
        self.utilities.insert(skill.clone(), utility_after);
        SkillUtilityUpdate {
            skill,
            utility_before,
            utility_after,
            stats_after,
        }
    }

    /// Transfers utility and use counters across a skill rename.
    pub fn transfer_skill(&mut self, from: &SkillName, to: SkillName) -> SkillUtilityTransfer {
        if from == &to {
            return if self.has_skill_state(from) {
                SkillUtilityTransfer::Transferred
            } else {
                SkillUtilityTransfer::SourceMissing
            };
        }
        if !self.has_skill_state(from) {
            return SkillUtilityTransfer::SourceMissing;
        }
        if self.has_skill_state(&to) {
            return SkillUtilityTransfer::TargetExists;
        }

        if let Some(utility) = self.utilities.remove(from) {
            self.utilities.insert(to.clone(), utility);
        }
        if let Some(stats) = self.stats.remove(from) {
            self.stats.insert(to, stats);
        }
        SkillUtilityTransfer::Transferred
    }

    /// Removes all utility and use counters for a skill.
    pub fn remove_skill(&mut self, skill: &SkillName) -> bool {
        let removed_utility = self.utilities.remove(skill).is_some();
        let removed_stats = self.stats.remove(skill).is_some();
        removed_utility || removed_stats
    }

    fn has_skill_state(&self, skill: &SkillName) -> bool {
        self.utilities.contains_key(skill) || self.stats.contains_key(skill)
    }

    fn total_retrievals(&self) -> u64 {
        self.stats
            .values()
            .map(|stats| stats.retrievals)
            .fold(0_u64, u64::saturating_add)
    }
}

fn ema(before: FiniteF64, observation: FiniteF64, smoothing: SkillUtilitySmoothing) -> FiniteF64 {
    let weight = smoothing.as_f64();
    let next = before.as_f64() + (weight * (observation.as_f64() - before.as_f64()));
    FiniteF64::new(next).expect("EMA over finite utility values remains finite")
}

/// Candidate relevance emitted by a paper/router-specific retrieval layer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SkillRetrievalCandidate {
    skill: SkillName,
    relevance: FiniteF64,
}

impl SkillRetrievalCandidate {
    /// Constructs a candidate with a caller-provided finite relevance score.
    pub fn new(skill: SkillName, relevance: FiniteF64) -> Self {
        Self { skill, relevance }
    }

    /// Candidate skill.
    #[must_use]
    pub const fn skill(&self) -> &SkillName {
        &self.skill
    }

    /// Caller-provided relevance score.
    #[must_use]
    pub const fn relevance(&self) -> FiniteF64 {
        self.relevance
    }
}

/// Non-negative weights for utility-aware skill ranking.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SkillUtilityRankingWeights {
    relevance: FiniteF64,
    utility: FiniteF64,
    exploration: FiniteF64,
}

impl SkillUtilityRankingWeights {
    /// Constructs ranking weights.
    ///
    /// # Errors
    ///
    /// Returns [`SkillUtilityRankingWeightsError`] when relevance or
    /// exploration weights are negative. Utility may be negative so callers can
    /// deliberately penalize high-utility skills in ablations.
    pub fn new(
        relevance: FiniteF64,
        utility: FiniteF64,
        exploration: FiniteF64,
    ) -> Result<Self, SkillUtilityRankingWeightsError> {
        if relevance.as_f64() < 0.0 {
            return Err(SkillUtilityRankingWeightsError::NegativeRelevanceWeight {
                value: relevance,
            });
        }
        if exploration.as_f64() < 0.0 {
            return Err(SkillUtilityRankingWeightsError::NegativeExplorationWeight {
                value: exploration,
            });
        }
        Ok(Self {
            relevance,
            utility,
            exploration,
        })
    }

    /// Weight applied to caller-provided relevance.
    #[must_use]
    pub const fn relevance(&self) -> FiniteF64 {
        self.relevance
    }

    /// Weight applied to stored utility.
    #[must_use]
    pub const fn utility(&self) -> FiniteF64 {
        self.utility
    }

    /// Weight applied to UCB-style exploration bonus.
    #[must_use]
    pub const fn exploration(&self) -> FiniteF64 {
        self.exploration
    }
}

impl Default for SkillUtilityRankingWeights {
    fn default() -> Self {
        Self::new(
            FiniteF64::new(1.0).expect("default relevance weight is finite"),
            FiniteF64::ZERO,
            FiniteF64::ZERO,
        )
        .expect("default weights are valid")
    }
}

/// Error returned when constructing [`SkillUtilityRankingWeights`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SkillUtilityRankingWeightsError {
    /// Relevance weight was negative.
    NegativeRelevanceWeight {
        /// Rejected value.
        value: FiniteF64,
    },
    /// Exploration weight was negative.
    NegativeExplorationWeight {
        /// Rejected value.
        value: FiniteF64,
    },
}

impl std::fmt::Display for SkillUtilityRankingWeightsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NegativeRelevanceWeight { value } => {
                write!(
                    f,
                    "skill relevance weight must be non-negative, got {}",
                    value.as_f64()
                )
            }
            Self::NegativeExplorationWeight { value } => {
                write!(
                    f,
                    "skill exploration weight must be non-negative, got {}",
                    value.as_f64()
                )
            }
        }
    }
}

impl std::error::Error for SkillUtilityRankingWeightsError {}

/// Ranked skill retrieval candidate.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SkillUtilityRank {
    skill: SkillName,
    relevance: FiniteF64,
    utility: FiniteF64,
    exploration_bonus: FiniteF64,
    score: FiniteF64,
    stats: SkillUseStats,
}

impl SkillUtilityRank {
    /// Ranked skill.
    #[must_use]
    pub const fn skill(&self) -> &SkillName {
        &self.skill
    }

    /// Caller-provided relevance component.
    #[must_use]
    pub const fn relevance(&self) -> FiniteF64 {
        self.relevance
    }

    /// Stored utility component.
    #[must_use]
    pub const fn utility(&self) -> FiniteF64 {
        self.utility
    }

    /// UCB-style exploration bonus before weighting.
    #[must_use]
    pub const fn exploration_bonus(&self) -> FiniteF64 {
        self.exploration_bonus
    }

    /// Final weighted ranking score.
    #[must_use]
    pub const fn score(&self) -> FiniteF64 {
        self.score
    }

    /// Skill utility/use counters used for ranking.
    #[must_use]
    pub const fn stats(&self) -> SkillUseStats {
        self.stats
    }
}

/// Utility-aware ranker over caller-provided skill relevance scores.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SkillUtilityRanker {
    weights: SkillUtilityRankingWeights,
}

impl SkillUtilityRanker {
    /// Constructs a ranker with explicit weights.
    pub const fn new(weights: SkillUtilityRankingWeights) -> Self {
        Self { weights }
    }

    /// Ranking weights.
    #[must_use]
    pub const fn weights(&self) -> SkillUtilityRankingWeights {
        self.weights
    }

    /// Ranks candidates by weighted relevance, utility, and exploration bonus.
    pub fn rank(
        &self,
        state: &SkillUtilityState,
        candidates: impl IntoIterator<Item = SkillRetrievalCandidate>,
    ) -> Vec<SkillUtilityRank> {
        let total_retrievals = state.total_retrievals();
        let mut ranked = candidates
            .into_iter()
            .map(|candidate| self.score_candidate(state, total_retrievals, candidate))
            .collect::<Vec<_>>();
        ranked.sort_by(rank_order);
        ranked
    }

    /// Returns the first `k` ranked candidates.
    pub fn top_k(
        &self,
        state: &SkillUtilityState,
        candidates: impl IntoIterator<Item = SkillRetrievalCandidate>,
        k: std::num::NonZeroUsize,
    ) -> Vec<SkillUtilityRank> {
        let mut ranked = self.rank(state, candidates);
        ranked.truncate(k.get());
        ranked
    }

    fn score_candidate(
        &self,
        state: &SkillUtilityState,
        total_retrievals: u64,
        candidate: SkillRetrievalCandidate,
    ) -> SkillUtilityRank {
        let utility = state.utility(&candidate.skill);
        let stats = state.stats(&candidate.skill);
        let exploration_bonus = exploration_bonus(total_retrievals, stats.retrievals);
        let score = weighted_score(
            self.weights,
            candidate.relevance,
            utility,
            exploration_bonus,
        );
        SkillUtilityRank {
            skill: candidate.skill,
            relevance: candidate.relevance,
            utility,
            exploration_bonus,
            score,
            stats,
        }
    }
}

impl Default for SkillUtilityRanker {
    fn default() -> Self {
        Self::new(SkillUtilityRankingWeights::default())
    }
}

fn weighted_score(
    weights: SkillUtilityRankingWeights,
    relevance: FiniteF64,
    utility: FiniteF64,
    exploration_bonus: FiniteF64,
) -> FiniteF64 {
    let score = (weights.relevance().as_f64() * relevance.as_f64())
        + (weights.utility().as_f64() * utility.as_f64())
        + (weights.exploration().as_f64() * exploration_bonus.as_f64());
    FiniteF64::new(score).expect("weighted finite score remains finite")
}

fn exploration_bonus(total_retrievals: u64, skill_retrievals: u64) -> FiniteF64 {
    let total = retrieval_count_scale(total_retrievals);
    let seen = retrieval_count_scale(skill_retrievals);
    FiniteF64::new((total.ln() / seen).sqrt()).expect("UCB exploration bonus is finite")
}

fn retrieval_count_scale(count: u64) -> f64 {
    let capped = u32::try_from(count.saturating_add(1)).unwrap_or(u32::MAX);
    f64::from(capped)
}

fn rank_order(left: &SkillUtilityRank, right: &SkillUtilityRank) -> Ordering {
    right
        .score()
        .partial_cmp(&left.score())
        .expect("skill ranking scores are finite")
        .then_with(|| left.skill().cmp(right.skill()))
}
