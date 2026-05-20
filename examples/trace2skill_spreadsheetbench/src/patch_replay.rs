//! Replay saved `Trace2Skill` JSON patch merge artifacts.

use std::collections::BTreeMap;

use leaven_agentic_skill::{
    SkillPatchApplication, SkillPatchMergeBatch, SkillPatchMergeInput, SkillPatchMergeLevel,
    SkillPatchMergeTree, SkillPatchMergeTreeError, SkillPatchPlanId, SkillPatchPlanRecord,
};
use leaven_artifact_skill::{SkillBank, SkillName};

use crate::{
    Trace2SkillPatchError, Trace2SkillPatchLowering, Trace2SkillPatchLoweringInput,
    lower_trace2skill_json_patch,
};

/// Inputs for replaying saved or live upstream JSON patch merge artifacts.
#[derive(Debug)]
pub struct Trace2SkillJsonPatchReplayInput<'a> {
    /// Parent skill bank all upstream patches were authored against.
    pub parent: &'a SkillBank,
    /// Skill folder targeted by the upstream runner.
    pub skill: &'a SkillName,
    /// Leaf map-stage analyst patches.
    pub leaf_patches: Vec<Trace2SkillJsonPatchArtifact<'a>>,
    /// Hierarchical merge-stage patches in execution order.
    pub merge_levels: Vec<Trace2SkillJsonPatchMergeLevel<'a>>,
    /// Upstream id for the selected final patch plan.
    pub final_plan_id: &'a str,
}

/// One upstream JSON patch artifact with its merge-tree id.
#[derive(Clone, Copy, Debug)]
pub struct Trace2SkillJsonPatchArtifact<'a> {
    /// Stable upstream patch id, for example `map/error-13-1`.
    pub plan_id: &'a str,
    /// Upstream JSON patch object or an LLM response containing one fenced JSON patch.
    pub payload: &'a str,
    /// Number of independent analyst patches supporting this artifact.
    pub support_count: u32,
}

/// One hierarchical merge level from the upstream consolidation run.
#[derive(Debug)]
pub struct Trace2SkillJsonPatchMergeLevel<'a> {
    /// Merge batches in this level. Batches may consume only earlier plans.
    pub batches: Vec<Trace2SkillJsonPatchMergeBatch<'a>>,
}

/// One upstream merge batch output and its input decisions.
#[derive(Debug)]
pub struct Trace2SkillJsonPatchMergeBatch<'a> {
    /// Stable upstream id for the merged output patch.
    pub output_plan_id: &'a str,
    /// Upstream JSON patch emitted by the merge batch.
    pub output_payload: &'a str,
    /// Number of independent observations supporting the merged output.
    pub support_count: u32,
    /// Accepted/discarded inputs recorded by the upstream merge.
    pub inputs: Vec<Trace2SkillJsonPatchMergeInput<'a>>,
}

/// Upstream merge decision for one input patch.
#[derive(Clone, Copy, Debug)]
pub enum Trace2SkillJsonPatchMergeInput<'a> {
    /// Input patch influenced the merge output.
    Accepted {
        /// Upstream input patch id.
        plan_id: &'a str,
    },
    /// Input patch was considered and rejected.
    Discarded {
        /// Upstream input patch id.
        plan_id: &'a str,
        /// Upstream reason or local parse of the merge rationale.
        reason: &'a str,
    },
}

/// Result of replaying one saved/live JSON patch merge run.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Trace2SkillJsonPatchReplay {
    /// Paper-neutral provenance for the replayed merge graph.
    pub merge_tree: SkillPatchMergeTree,
    /// Atomic application of the selected final patch to the parent bank.
    pub application: SkillPatchApplication,
    /// Reasoning text carried by the selected final upstream patch.
    pub final_reasoning: String,
    /// Changelog entries carried by the selected final upstream patch.
    pub final_changelog_entries: Vec<String>,
}

/// Replays saved or live `Trace2Skill` JSON patch artifacts through Leaven primitives.
///
/// The caller owns paper policy: analyst scheduling, merge prompts, batch size,
/// prevalence thresholds, and final-patch selection. This function proves the
/// saved artifacts can be lowered, represented as a validated merge tree, and
/// applied atomically to the parent `SkillBank`.
pub fn replay_trace2skill_json_patch_merge(
    input: Trace2SkillJsonPatchReplayInput<'_>,
) -> Result<Trace2SkillJsonPatchReplay, Trace2SkillPatchReplayError> {
    let Trace2SkillJsonPatchReplayInput {
        parent,
        skill,
        leaf_patches,
        merge_levels,
        final_plan_id,
    } = input;
    let mut lowerings = BTreeMap::new();
    let mut leaf_records = Vec::new();

    for artifact in &leaf_patches {
        let id = plan_id(artifact.plan_id)?;
        let lowering = lower_patch(parent, skill, artifact)?;
        let record = SkillPatchPlanRecord::new(id.clone(), lowering.plan.clone());
        insert_lowering(&mut lowerings, id, lowering)?;
        leaf_records.push(record);
    }

    let mut tree_levels = Vec::new();
    for level in &merge_levels {
        let mut batches = Vec::new();
        for batch in &level.batches {
            let output_id = plan_id(batch.output_plan_id)?;
            let artifact = Trace2SkillJsonPatchArtifact {
                plan_id: batch.output_plan_id,
                payload: batch.output_payload,
                support_count: batch.support_count,
            };
            let lowering = lower_patch(parent, skill, &artifact)?;
            let output_plan = lowering.plan.clone();
            insert_lowering(&mut lowerings, output_id.clone(), lowering)?;
            let inputs = batch
                .inputs
                .iter()
                .map(merge_input)
                .collect::<Result<Vec<_>, _>>()?;
            batches.push(SkillPatchMergeBatch::new(output_id, inputs, output_plan));
        }
        tree_levels.push(SkillPatchMergeLevel::new(batches));
    }

    let final_id = plan_id(final_plan_id)?;
    let merge_tree = SkillPatchMergeTree::validate(leaf_records, tree_levels, final_id.clone())?;
    let final_lowering = lowerings.remove(&final_id).ok_or_else(|| {
        Trace2SkillPatchReplayError::UnknownFinalLowering {
            final_plan_id: final_id.to_string(),
        }
    })?;
    let application = SkillPatchApplication::apply(
        parent,
        final_lowering.plan.clone(),
        final_lowering.changes.clone(),
    )
    .map_err(Trace2SkillPatchError::Application)?;

    Ok(Trace2SkillJsonPatchReplay {
        merge_tree,
        application,
        final_reasoning: final_lowering.reasoning,
        final_changelog_entries: final_lowering.changelog_entries,
    })
}

fn lower_patch(
    parent: &SkillBank,
    skill: &SkillName,
    artifact: &Trace2SkillJsonPatchArtifact<'_>,
) -> Result<Trace2SkillPatchLowering, Trace2SkillPatchError> {
    lower_trace2skill_json_patch(Trace2SkillPatchLoweringInput {
        parent,
        skill,
        payload: artifact.payload,
        support_count: artifact.support_count,
    })
}

fn merge_input(
    input: &Trace2SkillJsonPatchMergeInput<'_>,
) -> Result<SkillPatchMergeInput, Trace2SkillPatchReplayError> {
    match input {
        Trace2SkillJsonPatchMergeInput::Accepted { plan_id: raw } => {
            Ok(SkillPatchMergeInput::accepted(plan_id(raw)?))
        }
        Trace2SkillJsonPatchMergeInput::Discarded {
            plan_id: raw,
            reason,
        } => Ok(SkillPatchMergeInput::discarded(plan_id(raw)?, *reason)),
    }
}

fn plan_id(raw: &str) -> Result<SkillPatchPlanId, SkillPatchMergeTreeError> {
    SkillPatchPlanId::new(raw)
}

fn insert_lowering(
    lowerings: &mut BTreeMap<SkillPatchPlanId, Trace2SkillPatchLowering>,
    id: SkillPatchPlanId,
    lowering: Trace2SkillPatchLowering,
) -> Result<(), Trace2SkillPatchReplayError> {
    if lowerings.contains_key(&id) {
        return Err(Trace2SkillPatchReplayError::DuplicateLoweredPlan {
            plan_id: id.to_string(),
        });
    }
    lowerings.insert(id, lowering);
    Ok(())
}

/// Error while replaying upstream `Trace2Skill` patch merge artifacts.
#[derive(Debug, thiserror::Error)]
pub enum Trace2SkillPatchReplayError {
    /// A patch artifact failed JSON lowering, validation, or application.
    #[error(transparent)]
    Patch(#[from] Trace2SkillPatchError),
    /// Merge-tree provenance was malformed.
    #[error(transparent)]
    MergeTree(#[from] SkillPatchMergeTreeError),
    /// The same plan id appeared in more than one lowered patch artifact.
    #[error("Trace2Skill replay lowered duplicate patch plan id {plan_id}")]
    DuplicateLoweredPlan {
        /// Duplicate plan id.
        plan_id: String,
    },
    /// The validated final id did not have corresponding lowered patch changes.
    #[error("Trace2Skill replay final plan {final_plan_id} has no lowered changes")]
    UnknownFinalLowering {
        /// Missing lowered final plan id.
        final_plan_id: String,
    },
}
