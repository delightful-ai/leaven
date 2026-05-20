//! Provenance for hierarchical skill patch-plan merges.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use crate::SkillPatchPlan;

/// Stable identifier for one patch plan in a merge tree.
#[derive(
    Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, serde::Deserialize, serde::Serialize,
)]
#[serde(transparent)]
pub struct SkillPatchPlanId(String);

impl SkillPatchPlanId {
    /// Builds a non-empty patch plan id.
    ///
    /// # Errors
    ///
    /// Returns [`SkillPatchMergeTreeError::EmptyPlanId`] when the id is empty
    /// or whitespace only.
    pub fn new(id: impl Into<String>) -> Result<Self, SkillPatchMergeTreeError> {
        let id = id.into();
        if id.trim().is_empty() {
            return Err(SkillPatchMergeTreeError::EmptyPlanId);
        }
        Ok(Self(id))
    }

    /// Returns the plan id as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SkillPatchPlanId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// One named patch plan produced before or during merge consolidation.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct SkillPatchPlanRecord {
    id: SkillPatchPlanId,
    plan: SkillPatchPlan,
}

impl SkillPatchPlanRecord {
    /// Builds a named patch plan record.
    #[must_use]
    pub const fn new(id: SkillPatchPlanId, plan: SkillPatchPlan) -> Self {
        Self { id, plan }
    }

    /// Returns the patch plan id.
    #[must_use]
    pub const fn id(&self) -> &SkillPatchPlanId {
        &self.id
    }

    /// Returns the validated patch plan.
    #[must_use]
    pub const fn plan(&self) -> &SkillPatchPlan {
        &self.plan
    }
}

/// Whether a merge batch retained or discarded one input patch plan.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum SkillPatchMergeDecision {
    /// Input patch influenced the merge output.
    Accepted,
    /// Input patch was considered and rejected.
    Discarded {
        /// Reason the input was rejected.
        reason: String,
    },
}

/// One input edge into a merge batch.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct SkillPatchMergeInput {
    plan_id: SkillPatchPlanId,
    decision: SkillPatchMergeDecision,
}

impl SkillPatchMergeInput {
    /// Records an accepted input patch plan.
    #[must_use]
    pub const fn accepted(plan_id: SkillPatchPlanId) -> Self {
        Self {
            plan_id,
            decision: SkillPatchMergeDecision::Accepted,
        }
    }

    /// Records a discarded input patch plan.
    #[must_use]
    pub fn discarded(plan_id: SkillPatchPlanId, reason: impl Into<String>) -> Self {
        Self {
            plan_id,
            decision: SkillPatchMergeDecision::Discarded {
                reason: reason.into(),
            },
        }
    }

    /// Returns the input patch plan id.
    #[must_use]
    pub const fn plan_id(&self) -> &SkillPatchPlanId {
        &self.plan_id
    }

    /// Returns the merge decision for this input.
    #[must_use]
    pub const fn decision(&self) -> &SkillPatchMergeDecision {
        &self.decision
    }
}

/// One merge operation that consolidates input plans into an output plan.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct SkillPatchMergeBatch {
    output_id: SkillPatchPlanId,
    inputs: Vec<SkillPatchMergeInput>,
    output_plan: SkillPatchPlan,
}

impl SkillPatchMergeBatch {
    /// Builds a merge batch. Graph validation happens in
    /// [`SkillPatchMergeTree::validate`].
    #[must_use]
    pub fn new(
        output_id: SkillPatchPlanId,
        inputs: impl Into<Vec<SkillPatchMergeInput>>,
        output_plan: SkillPatchPlan,
    ) -> Self {
        Self {
            output_id,
            inputs: inputs.into(),
            output_plan,
        }
    }

    /// Returns the output patch plan id.
    #[must_use]
    pub const fn output_id(&self) -> &SkillPatchPlanId {
        &self.output_id
    }

    /// Returns input edges into this batch.
    #[must_use]
    pub fn inputs(&self) -> &[SkillPatchMergeInput] {
        &self.inputs
    }

    /// Returns the batch output patch plan.
    #[must_use]
    pub const fn output_plan(&self) -> &SkillPatchPlan {
        &self.output_plan
    }

    /// Accepted input patch plan ids.
    #[must_use]
    pub fn accepted_input_ids(&self) -> Vec<&SkillPatchPlanId> {
        self.inputs
            .iter()
            .filter_map(|input| {
                matches!(input.decision, SkillPatchMergeDecision::Accepted)
                    .then_some(&input.plan_id)
            })
            .collect()
    }

    /// Discarded input patch plan ids.
    #[must_use]
    pub fn discarded_input_ids(&self) -> Vec<&SkillPatchPlanId> {
        self.inputs
            .iter()
            .filter_map(|input| {
                matches!(input.decision, SkillPatchMergeDecision::Discarded { .. })
                    .then_some(&input.plan_id)
            })
            .collect()
    }
}

/// One hierarchical merge level. Batches in a level may only consume plans
/// available before that level starts.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct SkillPatchMergeLevel {
    batches: Vec<SkillPatchMergeBatch>,
}

impl SkillPatchMergeLevel {
    /// Builds a merge level. Graph validation happens in
    /// [`SkillPatchMergeTree::validate`].
    #[must_use]
    pub fn new(batches: impl Into<Vec<SkillPatchMergeBatch>>) -> Self {
        Self {
            batches: batches.into(),
        }
    }

    /// Returns merge batches in this level.
    #[must_use]
    pub fn batches(&self) -> &[SkillPatchMergeBatch] {
        &self.batches
    }
}

/// Paper-neutral provenance for hierarchical skill patch-plan consolidation.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct SkillPatchMergeTree {
    leaf_plans: Vec<SkillPatchPlanRecord>,
    levels: Vec<SkillPatchMergeLevel>,
    final_plan_id: SkillPatchPlanId,
}

impl SkillPatchMergeTree {
    /// Validates and records a hierarchical patch-plan merge tree.
    ///
    /// The tree is intentionally policy-light: callers own merge prompts,
    /// prevalence thresholds, batch sizes, and result selection. This type
    /// proves only reusable provenance invariants: every input was known before
    /// the level that consumed it, outputs are uniquely named, each batch has at
    /// least one accepted input, and the final plan id resolves.
    ///
    /// # Errors
    ///
    /// Returns [`SkillPatchMergeTreeError`] when the graph is empty, malformed,
    /// has duplicate plan ids, consumes unknown inputs, or names an unknown
    /// final plan.
    pub fn validate(
        leaf_plans: impl Into<Vec<SkillPatchPlanRecord>>,
        levels: impl Into<Vec<SkillPatchMergeLevel>>,
        final_plan_id: SkillPatchPlanId,
    ) -> Result<Self, SkillPatchMergeTreeError> {
        let leaf_plans = leaf_plans.into();
        if leaf_plans.is_empty() {
            return Err(SkillPatchMergeTreeError::EmptyTree);
        }
        let levels = levels.into();
        let mut available = BTreeSet::new();
        for record in &leaf_plans {
            insert_plan_id(&mut available, record.id().clone())?;
        }

        for (level_index, level) in levels.iter().enumerate() {
            if level.batches.is_empty() {
                return Err(SkillPatchMergeTreeError::EmptyMergeLevel { level_index });
            }
            let available_before_level = available.clone();
            let mut outputs_this_level = BTreeSet::new();
            for (batch_index, batch) in level.batches.iter().enumerate() {
                validate_batch_inputs(level_index, batch_index, batch, &available_before_level)?;
                insert_plan_id(&mut outputs_this_level, batch.output_id().clone())?;
            }
            for id in outputs_this_level {
                insert_plan_id(&mut available, id)?;
            }
        }

        if !available.contains(&final_plan_id) {
            return Err(SkillPatchMergeTreeError::UnknownFinalPlan { final_plan_id });
        }

        Ok(Self {
            leaf_plans,
            levels,
            final_plan_id,
        })
    }

    /// Returns leaf patch plans produced before hierarchical merge.
    #[must_use]
    pub fn leaf_plans(&self) -> &[SkillPatchPlanRecord] {
        &self.leaf_plans
    }

    /// Returns merge levels in execution order.
    #[must_use]
    pub fn levels(&self) -> &[SkillPatchMergeLevel] {
        &self.levels
    }

    /// Returns the selected final plan id.
    #[must_use]
    pub const fn final_plan_id(&self) -> &SkillPatchPlanId {
        &self.final_plan_id
    }

    /// Returns the selected final plan.
    #[must_use]
    pub fn final_plan(&self) -> &SkillPatchPlan {
        self.plan(&self.final_plan_id)
            .expect("validated merge tree final plan id resolves")
    }

    /// Finds a patch plan by id.
    #[must_use]
    pub fn plan(&self, id: &SkillPatchPlanId) -> Option<&SkillPatchPlan> {
        self.leaf_plans
            .iter()
            .find_map(|record| (record.id() == id).then_some(record.plan()))
            .or_else(|| {
                self.levels
                    .iter()
                    .flat_map(SkillPatchMergeLevel::batches)
                    .find_map(|batch| (batch.output_id() == id).then_some(batch.output_plan()))
            })
    }
}

/// Validation failure for a skill patch merge tree.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SkillPatchMergeTreeError {
    /// Plan ids must be non-empty.
    EmptyPlanId,
    /// Merge tree contained no leaf patch plans.
    EmptyTree,
    /// A plan id was reused.
    DuplicatePlanId {
        /// Duplicate plan id.
        plan_id: SkillPatchPlanId,
    },
    /// A merge level contained no batches.
    EmptyMergeLevel {
        /// Zero-based level index.
        level_index: usize,
    },
    /// A merge batch contained no inputs.
    EmptyMergeBatch {
        /// Zero-based level index.
        level_index: usize,
        /// Zero-based batch index inside the level.
        batch_index: usize,
    },
    /// A merge batch listed one input more than once.
    DuplicateMergeInput {
        /// Zero-based level index.
        level_index: usize,
        /// Zero-based batch index inside the level.
        batch_index: usize,
        /// Duplicate input plan id.
        plan_id: SkillPatchPlanId,
    },
    /// A merge batch consumed a plan not available before its level.
    UnknownInputPlan {
        /// Zero-based level index.
        level_index: usize,
        /// Zero-based batch index inside the level.
        batch_index: usize,
        /// Unknown input plan id.
        plan_id: SkillPatchPlanId,
    },
    /// A merge batch discarded every input.
    AcceptedInputRequired {
        /// Zero-based level index.
        level_index: usize,
        /// Zero-based batch index inside the level.
        batch_index: usize,
    },
    /// The selected final plan id did not resolve.
    UnknownFinalPlan {
        /// Unknown final plan id.
        final_plan_id: SkillPatchPlanId,
    },
}

impl fmt::Display for SkillPatchMergeTreeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyPlanId => f.write_str("skill patch plan id must be non-empty"),
            Self::EmptyTree => {
                f.write_str("skill patch merge tree must contain at least one leaf plan")
            }
            Self::DuplicatePlanId { plan_id } => {
                write!(f, "skill patch merge tree reuses plan id {plan_id}")
            }
            Self::EmptyMergeLevel { level_index } => {
                write!(f, "skill patch merge level {level_index} has no batches")
            }
            Self::EmptyMergeBatch {
                level_index,
                batch_index,
            } => write!(
                f,
                "skill patch merge batch {level_index}.{batch_index} has no inputs"
            ),
            Self::DuplicateMergeInput {
                level_index,
                batch_index,
                plan_id,
            } => write!(
                f,
                "skill patch merge batch {level_index}.{batch_index} repeats input {plan_id}"
            ),
            Self::UnknownInputPlan {
                level_index,
                batch_index,
                plan_id,
            } => write!(
                f,
                "skill patch merge batch {level_index}.{batch_index} consumes unknown input {plan_id}"
            ),
            Self::AcceptedInputRequired {
                level_index,
                batch_index,
            } => write!(
                f,
                "skill patch merge batch {level_index}.{batch_index} must accept at least one input"
            ),
            Self::UnknownFinalPlan { final_plan_id } => {
                write!(
                    f,
                    "skill patch merge tree final plan id {final_plan_id} is unknown"
                )
            }
        }
    }
}

impl Error for SkillPatchMergeTreeError {}

fn insert_plan_id(
    plans: &mut BTreeSet<SkillPatchPlanId>,
    id: SkillPatchPlanId,
) -> Result<(), SkillPatchMergeTreeError> {
    if !plans.insert(id.clone()) {
        return Err(SkillPatchMergeTreeError::DuplicatePlanId { plan_id: id });
    }
    Ok(())
}

fn validate_batch_inputs(
    level_index: usize,
    batch_index: usize,
    batch: &SkillPatchMergeBatch,
    available_before_level: &BTreeSet<SkillPatchPlanId>,
) -> Result<(), SkillPatchMergeTreeError> {
    if batch.inputs.is_empty() {
        return Err(SkillPatchMergeTreeError::EmptyMergeBatch {
            level_index,
            batch_index,
        });
    }
    let mut seen = BTreeSet::new();
    let mut accepted = false;
    for input in &batch.inputs {
        if !seen.insert(input.plan_id.clone()) {
            return Err(SkillPatchMergeTreeError::DuplicateMergeInput {
                level_index,
                batch_index,
                plan_id: input.plan_id.clone(),
            });
        }
        if !available_before_level.contains(&input.plan_id) {
            return Err(SkillPatchMergeTreeError::UnknownInputPlan {
                level_index,
                batch_index,
                plan_id: input.plan_id.clone(),
            });
        }
        accepted |= matches!(input.decision, SkillPatchMergeDecision::Accepted);
    }
    if !accepted {
        return Err(SkillPatchMergeTreeError::AcceptedInputRequired {
            level_index,
            batch_index,
        });
    }
    Ok(())
}
