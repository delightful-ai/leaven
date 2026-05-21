//! Replay saved `Trace2Skill` JSON patch merge artifacts.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs, io,
    path::{Path, PathBuf},
};

use leaven_agentic_skill::{
    SkillPatchApplication, SkillPatchMergeBatch, SkillPatchMergeInput, SkillPatchMergeLevel,
    SkillPatchMergeTree, SkillPatchMergeTreeError, SkillPatchPlanId, SkillPatchPlanRecord,
};
use leaven_artifact_skill::{SkillBank, SkillName};
use leaven_evidence::{
    AgentAnalystCallEvidence, AgentAnalystCallEvidenceInput, AgentAnalystCallStatus,
    AgentAnalystFanoutEvidence, AgentPatchMergeDecision, AgentPatchMergeNode,
    AgentPatchMergeNodeInput, AgentPatchMergeTreeEvidence, OutputRecord,
};
use leaven_kernel::BlobRef;

use crate::{
    lower_trace2skill_json_patch, Trace2SkillPatchError, Trace2SkillPatchLowering,
    Trace2SkillPatchLoweringInput,
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

/// Inputs for replaying an upstream `--save-intermediates` output directory.
#[derive(Clone, Copy, Debug)]
pub struct Trace2SkillSavedJsonPatchReplayInput<'a> {
    /// Parent skill bank all upstream patches were authored against.
    pub parent: &'a SkillBank,
    /// Skill folder targeted by the upstream runner.
    pub skill: &'a SkillName,
    /// Directory passed as upstream `--intermediates-dir`, or the default
    /// `<skill>_parallel_output` directory.
    pub intermediates_dir: &'a Path,
    /// Upstream `--merge-batch-size` used for the run.
    pub merge_batch_size: usize,
}

/// Inputs for importing an upstream `--save-intermediates` directory as merge evidence.
#[derive(Clone, Copy, Debug)]
pub struct Trace2SkillSavedJsonPatchMergeEvidenceInput<'a> {
    /// Parent skill bank all upstream patches were authored against.
    pub parent: &'a SkillBank,
    /// Skill folder targeted by the upstream runner.
    pub skill: &'a SkillName,
    /// Directory passed as upstream `--intermediates-dir`, or the default
    /// `<skill>_parallel_output` directory.
    pub intermediates_dir: &'a Path,
    /// Upstream `--merge-batch-size` used for the run.
    pub merge_batch_size: usize,
}

/// Inputs for importing upstream saved MAP patches into a pending fan-out.
#[derive(Clone, Copy, Debug)]
pub struct Trace2SkillSavedMapPatchFanoutInput<'a> {
    /// Parent skill bank the saved patches were authored against.
    pub parent: &'a SkillBank,
    /// Skill folder targeted by the upstream runner.
    pub skill: &'a SkillName,
    /// Pending analyst fan-out produced before model execution.
    pub fanout: &'a AgentAnalystFanoutEvidence,
    /// Directory passed as upstream `--intermediates-dir`, or the default
    /// `<skill>_parallel_output` directory.
    pub intermediates_dir: &'a Path,
    /// Directory passed as upstream `--parse-failure-dir`, when saved
    /// parse-failure artifacts are available.
    pub parse_failure_dir: Option<&'a Path>,
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
    /// Patch plan id used for application. This can differ from
    /// `merge_tree.final_plan_id()` when upstream saved a translated final
    /// patch after hierarchical merge.
    pub applied_plan_id: SkillPatchPlanId,
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
        applied_plan_id: final_id,
        final_reasoning: final_lowering.reasoning,
        final_changelog_entries: final_lowering.changelog_entries,
    })
}

/// Replays an upstream `--save-intermediates` directory through Leaven primitives.
///
/// The upstream JSON pipeline saves `map_patches/patch_*.json`, optional
/// `merge_level_N/merged_*.json` directories, `final_patch.json`, optional
/// `translated_final_patch.json`, and optional `applied_diffs.patch`. This
/// loader reconstructs the deterministic merge batches from the saved file
/// order and `merge_batch_size`, then applies `translated_final_patch.json`
/// when present because that is what upstream's programmatic apply phase uses
/// after translation.
pub fn replay_trace2skill_saved_json_patch_outputs(
    input: Trace2SkillSavedJsonPatchReplayInput<'_>,
) -> Result<Trace2SkillJsonPatchReplay, Trace2SkillPatchReplayError> {
    if input.merge_batch_size == 0 {
        return Err(Trace2SkillPatchReplayError::InvalidMergeBatchSize);
    }
    if !input.intermediates_dir.is_dir() {
        return Err(Trace2SkillPatchReplayError::MissingIntermediatesDir {
            path: input.intermediates_dir.to_path_buf(),
        });
    }

    let mut state = load_saved_map_patches(input)?;
    load_saved_merge_levels(input, &mut state)?;
    finish_saved_json_patch_replay(input, state)
}

/// Imports saved upstream MAP-stage outputs into pending analyst-call evidence.
///
/// The upstream JSON pipeline saves parsed MAP responses under
/// `map_patches/patch_*.json` and records the originating one-based
/// `batch_index` in each patch. With the paper's `--batch-size 1` setup, that
/// batch index is the durable bridge back to the caller-declared fan-out order.
/// Upstream also saves parse-failed MAP prompt/response artifacts under
/// `{parse_failure_dir}/map/*_batch_000N_*_parse_failed.md`. Those saved
/// failures become terminal `ParseFailed` calls. Calls with neither saved
/// parsed patches nor saved parse-failure artifacts stay pending.
pub fn import_trace2skill_saved_map_patches_into_fanout(
    input: Trace2SkillSavedMapPatchFanoutInput<'_>,
) -> Result<AgentAnalystFanoutEvidence, Trace2SkillPatchReplayError> {
    if !input.intermediates_dir.is_dir() {
        return Err(Trace2SkillPatchReplayError::MissingIntermediatesDir {
            path: input.intermediates_dir.to_path_buf(),
        });
    }

    let map_dir = input.intermediates_dir.join("map_patches");
    let files = patch_files(&map_dir, "patch_")?;
    let parse_failures = input
        .parse_failure_dir
        .map(|dir| parse_failure_files(&dir.join("map")))
        .transpose()?
        .unwrap_or_default();
    if files.is_empty() && parse_failures.is_empty() {
        return Err(Trace2SkillPatchReplayError::MissingMapPatches { path: map_dir });
    }

    let mut imported = input.fanout.clone();
    let mut seen_batches = BTreeSet::new();
    for file in files {
        import_saved_map_patch(input, &mut imported, &mut seen_batches, file)?;
    }

    for file in parse_failures {
        import_saved_map_parse_failure(input, &mut imported, &mut seen_batches, file)?;
    }

    Ok(imported)
}

fn import_saved_map_patch(
    input: Trace2SkillSavedMapPatchFanoutInput<'_>,
    imported: &mut AgentAnalystFanoutEvidence,
    seen_batches: &mut BTreeSet<usize>,
    file: PatchFile,
) -> Result<(), Trace2SkillPatchReplayError> {
    let payload =
        fs::read_to_string(&file.path).map_err(|source| Trace2SkillPatchReplayError::Io {
            path: file.path.clone(),
            source,
        })?;
    let batch_index = map_patch_batch_index(&file.path, &payload)?;
    if !seen_batches.insert(batch_index) {
        return Err(Trace2SkillPatchReplayError::DuplicateMapPatchBatchIndex {
            batch_index,
            path: file.path,
        });
    }
    let call_index = batch_index - 1;
    let call_id = input
        .fanout
        .expected_call_ids()
        .get(call_index)
        .ok_or_else(|| Trace2SkillPatchReplayError::InvalidMapPatchBatchIndex {
            batch_index,
            expected_calls: input.fanout.expected_call_ids().len(),
            path: file.path.clone(),
        })?;
    let pending_call = input.fanout.by_call(call_id).ok_or_else(|| {
        Trace2SkillPatchReplayError::MissingAnalystCallForMapPatch {
            call_id: call_id.clone(),
            path: file.path.clone(),
        }
    })?;

    lower_trace2skill_json_patch(Trace2SkillPatchLoweringInput {
        parent: input.parent,
        skill: input.skill,
        payload: &payload,
        support_count: pending_call.support_count(),
    })
    .map_err(|source| Trace2SkillPatchReplayError::PatchFile {
        path: file.path.clone(),
        source,
    })?;

    imported.push(AgentAnalystCallEvidence::new(
        AgentAnalystCallEvidenceInput {
            call_id: call_id.clone(),
            role: pending_call.role(),
            source_task_ids: pending_call.source_task_ids().to_vec(),
            prompt: pending_call.prompt().clone(),
            response: Some(OutputRecord::blob(BlobRef {
                store: "trace2skill-stage2".to_owned(),
                key: file.path.display().to_string(),
            })),
            status: AgentAnalystCallStatus::Succeeded,
            retry_count: pending_call.retry_count(),
            support_count: pending_call.support_count(),
        },
    )?)?;
    Ok(())
}

fn import_saved_map_parse_failure(
    input: Trace2SkillSavedMapPatchFanoutInput<'_>,
    imported: &mut AgentAnalystFanoutEvidence,
    seen_batches: &mut BTreeSet<usize>,
    file: ParseFailureFile,
) -> Result<(), Trace2SkillPatchReplayError> {
    let payload =
        fs::read_to_string(&file.path).map_err(|source| Trace2SkillPatchReplayError::Io {
            path: file.path.clone(),
            source,
        })?;
    let metadata = parse_failure_metadata(&file.path, &payload)?;
    if !seen_batches.insert(metadata.batch_index) {
        return Err(
            Trace2SkillPatchReplayError::DuplicateMapParseFailureBatchIndex {
                batch_index: metadata.batch_index,
                path: file.path,
            },
        );
    }
    let call_index = metadata.batch_index - 1;
    let call_id = input
        .fanout
        .expected_call_ids()
        .get(call_index)
        .ok_or_else(
            || Trace2SkillPatchReplayError::InvalidMapParseFailureBatchIndex {
                batch_index: metadata.batch_index,
                expected_calls: input.fanout.expected_call_ids().len(),
                path: file.path.clone(),
            },
        )?;
    let pending_call = input.fanout.by_call(call_id).ok_or_else(|| {
        Trace2SkillPatchReplayError::MissingAnalystCallForMapParseFailure {
            call_id: call_id.clone(),
            path: file.path.clone(),
        }
    })?;
    let artifact = OutputRecord::blob(BlobRef {
        store: "trace2skill-stage2".to_owned(),
        key: file.path.display().to_string(),
    });

    imported.push(AgentAnalystCallEvidence::new(
        AgentAnalystCallEvidenceInput {
            call_id: call_id.clone(),
            role: pending_call.role(),
            source_task_ids: pending_call.source_task_ids().to_vec(),
            prompt: pending_call.prompt().clone(),
            response: Some(artifact.clone()),
            status: AgentAnalystCallStatus::ParseFailed {
                reason: format!(
                    "upstream Trace2Skill {} {} failed {} parsing",
                    metadata.phase, metadata.label, metadata.expected_format
                ),
                artifact: Some(artifact),
            },
            retry_count: pending_call.retry_count(),
            support_count: pending_call.support_count(),
        },
    )?)?;
    Ok(())
}

fn load_saved_map_patches(
    input: Trace2SkillSavedJsonPatchReplayInput<'_>,
) -> Result<SavedJsonPatchReplayState, Trace2SkillPatchReplayError> {
    let mut state = SavedJsonPatchReplayState::default();
    for file in patch_files(&input.intermediates_dir.join("map_patches"), "patch_")? {
        let id = SkillPatchPlanId::new(format!("map/{}", file.stem))?;
        let support_count = 1;
        let lowering = lower_patch_file(input.parent, input.skill, &file.path, support_count)?;
        state
            .leaf_records
            .push(SkillPatchPlanRecord::new(id.clone(), lowering.plan.clone()));
        insert_lowering(&mut state.lowerings, id.clone(), lowering)?;
        state.current.push(LoadedPlan { id, support_count });
    }
    if state.current.is_empty() {
        return Err(Trace2SkillPatchReplayError::MissingMapPatches {
            path: input.intermediates_dir.join("map_patches"),
        });
    }
    Ok(state)
}

fn map_patch_batch_index(path: &Path, payload: &str) -> Result<usize, Trace2SkillPatchReplayError> {
    let value: serde_json::Value = serde_json::from_str(payload).map_err(|source| {
        Trace2SkillPatchReplayError::PatchMetadata {
            path: path.to_path_buf(),
            source,
        }
    })?;
    let batch_index = value
        .get("batch_index")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| Trace2SkillPatchReplayError::MissingMapPatchBatchIndex {
            path: path.to_path_buf(),
        })?;
    if batch_index == 0 {
        return Err(
            Trace2SkillPatchReplayError::InvalidMapPatchBatchIndexValue {
                batch_index,
                path: path.to_path_buf(),
            },
        );
    }
    usize::try_from(batch_index).map_err(|_| {
        Trace2SkillPatchReplayError::InvalidMapPatchBatchIndexValue {
            batch_index,
            path: path.to_path_buf(),
        }
    })
}

fn load_saved_merge_levels(
    input: Trace2SkillSavedJsonPatchReplayInput<'_>,
    state: &mut SavedJsonPatchReplayState,
) -> Result<(), Trace2SkillPatchReplayError> {
    for merge_dir in merge_level_dirs(input.intermediates_dir)? {
        let chunks = chunk_loaded_plans(&state.current, input.merge_batch_size);
        let outputs = patch_files(&merge_dir.path, "merged_")?;
        if outputs.len() != chunks.len() {
            return Err(Trace2SkillPatchReplayError::MergeOutputCountMismatch {
                level: merge_dir.level,
                expected: chunks.len(),
                actual: outputs.len(),
                path: merge_dir.path,
            });
        }

        let mut batches = Vec::new();
        let mut next = Vec::new();
        for (output, inputs) in outputs.iter().zip(chunks.iter()) {
            let output_id =
                SkillPatchPlanId::new(format!("merge_level_{}/{}", merge_dir.level, output.stem))?;
            let support_count = inputs.iter().map(|input| input.support_count).sum();
            let lowering =
                lower_patch_file(input.parent, input.skill, &output.path, support_count)?;
            let plan = lowering.plan.clone();
            insert_lowering(&mut state.lowerings, output_id.clone(), lowering)?;
            batches.push(SkillPatchMergeBatch::new(
                output_id.clone(),
                inputs
                    .iter()
                    .map(|input| SkillPatchMergeInput::accepted(input.id.clone()))
                    .collect::<Vec<_>>(),
                plan,
            ));
            next.push(LoadedPlan {
                id: output_id,
                support_count,
            });
        }
        state.tree_levels.push(SkillPatchMergeLevel::new(batches));
        state.current = next;
    }
    Ok(())
}

fn finish_saved_json_patch_replay(
    input: Trace2SkillSavedJsonPatchReplayInput<'_>,
    mut state: SavedJsonPatchReplayState,
) -> Result<Trace2SkillJsonPatchReplay, Trace2SkillPatchReplayError> {
    let final_patch = input.intermediates_dir.join("final_patch.json");
    if !final_patch.is_file() {
        return Err(Trace2SkillPatchReplayError::MissingFinalPatch { path: final_patch });
    }
    let final_support_count = state.current.iter().map(|plan| plan.support_count).sum();
    let final_id = SkillPatchPlanId::new("final/final_patch")?;
    let final_lowering =
        lower_patch_file(input.parent, input.skill, &final_patch, final_support_count)?;
    let final_plan = final_lowering.plan.clone();
    insert_lowering(&mut state.lowerings, final_id.clone(), final_lowering)?;
    state
        .tree_levels
        .push(SkillPatchMergeLevel::new([SkillPatchMergeBatch::new(
            final_id.clone(),
            state
                .current
                .iter()
                .map(|input| SkillPatchMergeInput::accepted(input.id.clone()))
                .collect::<Vec<_>>(),
            final_plan,
        )]));

    let merge_tree =
        SkillPatchMergeTree::validate(state.leaf_records, state.tree_levels, final_id.clone())?;
    let translated_patch = input.intermediates_dir.join("translated_final_patch.json");
    let (applied_plan_id, final_lowering) = if translated_patch.is_file() {
        let applied_plan_id = SkillPatchPlanId::new("final/translated_final_patch")?;
        (
            applied_plan_id,
            lower_patch_file(
                input.parent,
                input.skill,
                &translated_patch,
                final_support_count,
            )?,
        )
    } else {
        (
            final_id.clone(),
            state.lowerings.remove(&final_id).ok_or_else(|| {
                Trace2SkillPatchReplayError::UnknownFinalLowering {
                    final_plan_id: final_id.to_string(),
                }
            })?,
        )
    };
    let application = SkillPatchApplication::apply(
        input.parent,
        final_lowering.plan.clone(),
        final_lowering.changes.clone(),
    )
    .map_err(Trace2SkillPatchError::Application)?;

    Ok(Trace2SkillJsonPatchReplay {
        merge_tree,
        application,
        applied_plan_id,
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

fn lower_patch_file(
    parent: &SkillBank,
    skill: &SkillName,
    path: &Path,
    support_count: u32,
) -> Result<Trace2SkillPatchLowering, Trace2SkillPatchReplayError> {
    let payload = fs::read_to_string(path).map_err(|source| Trace2SkillPatchReplayError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    lower_trace2skill_json_patch(Trace2SkillPatchLoweringInput {
        parent,
        skill,
        payload: &payload,
        support_count,
    })
    .map_err(|source| Trace2SkillPatchReplayError::PatchFile {
        path: path.to_path_buf(),
        source,
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

#[derive(Debug, Default)]
struct SavedJsonPatchReplayState {
    lowerings: BTreeMap<SkillPatchPlanId, Trace2SkillPatchLowering>,
    leaf_records: Vec<SkillPatchPlanRecord>,
    tree_levels: Vec<SkillPatchMergeLevel>,
    current: Vec<LoadedPlan>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LoadedPlan {
    id: SkillPatchPlanId,
    support_count: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PatchFile {
    stem: String,
    path: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ParseFailureFile {
    path: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ParseFailureMetadata {
    phase: String,
    label: String,
    expected_format: String,
    batch_index: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct MergeLevelDir {
    level: usize,
    path: PathBuf,
}

fn patch_files(dir: &Path, prefix: &str) -> Result<Vec<PatchFile>, Trace2SkillPatchReplayError> {
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut files = Vec::new();
    for entry in fs::read_dir(dir).map_err(|source| Trace2SkillPatchReplayError::Io {
        path: dir.to_path_buf(),
        source,
    })? {
        let entry = entry.map_err(|source| Trace2SkillPatchReplayError::Io {
            path: dir.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("json") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
            return Err(Trace2SkillPatchReplayError::NonUnicodePath { path });
        };
        if !stem.starts_with(prefix) {
            continue;
        }
        files.push(PatchFile {
            stem: stem.to_owned(),
            path,
        });
    }
    files.sort_by(|left, right| left.stem.cmp(&right.stem));
    Ok(files)
}

fn parse_failure_files(dir: &Path) -> Result<Vec<ParseFailureFile>, Trace2SkillPatchReplayError> {
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut files = Vec::new();
    for entry in fs::read_dir(dir).map_err(|source| Trace2SkillPatchReplayError::Io {
        path: dir.to_path_buf(),
        source,
    })? {
        let entry = entry.map_err(|source| Trace2SkillPatchReplayError::Io {
            path: dir.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("md") {
            continue;
        }
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            return Err(Trace2SkillPatchReplayError::NonUnicodePath { path });
        };
        if !name.ends_with("_parse_failed.md") {
            continue;
        }
        files.push(ParseFailureFile { path });
    }
    files.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(files)
}

fn parse_failure_metadata(
    path: &Path,
    payload: &str,
) -> Result<ParseFailureMetadata, Trace2SkillPatchReplayError> {
    let phase = parse_failure_field(path, payload, "PHASE")?;
    let label = parse_failure_field(path, payload, "LABEL")?;
    let expected_format = parse_failure_field(path, payload, "EXPECTED FORMAT")?;
    let batch_index = parse_failure_batch_index(path, &label)?;
    Ok(ParseFailureMetadata {
        phase,
        label,
        expected_format,
        batch_index,
    })
}

fn parse_failure_field(
    path: &Path,
    payload: &str,
    field: &'static str,
) -> Result<String, Trace2SkillPatchReplayError> {
    let prefix = format!("{field}:");
    payload
        .lines()
        .find_map(|line| line.strip_prefix(&prefix).map(str::trim))
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .ok_or_else(
            || Trace2SkillPatchReplayError::MissingMapParseFailureField {
                path: path.to_path_buf(),
                field,
            },
        )
}

fn parse_failure_batch_index(
    path: &Path,
    label: &str,
) -> Result<usize, Trace2SkillPatchReplayError> {
    let Some(raw) = label.strip_prefix("batch_") else {
        return Err(
            Trace2SkillPatchReplayError::InvalidMapParseFailureBatchLabel {
                path: path.to_path_buf(),
                label: label.to_owned(),
            },
        );
    };
    let Ok(batch_index) = raw.parse::<usize>() else {
        return Err(
            Trace2SkillPatchReplayError::InvalidMapParseFailureBatchLabel {
                path: path.to_path_buf(),
                label: label.to_owned(),
            },
        );
    };
    if batch_index == 0 {
        return Err(
            Trace2SkillPatchReplayError::InvalidMapParseFailureBatchLabel {
                path: path.to_path_buf(),
                label: label.to_owned(),
            },
        );
    }
    Ok(batch_index)
}

fn merge_level_dirs(root: &Path) -> Result<Vec<MergeLevelDir>, Trace2SkillPatchReplayError> {
    let mut levels = Vec::new();
    for entry in fs::read_dir(root).map_err(|source| Trace2SkillPatchReplayError::Io {
        path: root.to_path_buf(),
        source,
    })? {
        let entry = entry.map_err(|source| Trace2SkillPatchReplayError::Io {
            path: root.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            return Err(Trace2SkillPatchReplayError::NonUnicodePath { path });
        };
        let Some(level) = name
            .strip_prefix("merge_level_")
            .and_then(|suffix| suffix.parse::<usize>().ok())
        else {
            continue;
        };
        levels.push(MergeLevelDir { level, path });
    }
    levels.sort_by_key(|level| level.level);
    Ok(levels)
}

fn chunk_loaded_plans(plans: &[LoadedPlan], chunk_size: usize) -> Vec<Vec<LoadedPlan>> {
    plans
        .chunks(chunk_size)
        .map(<[LoadedPlan]>::to_vec)
        .collect()
}

/// Error while replaying upstream `Trace2Skill` patch merge artifacts.
#[derive(Debug, thiserror::Error)]
pub enum Trace2SkillPatchReplayError {
    /// Filesystem access failed while reading upstream artifacts.
    #[error("failed to read Trace2Skill replay artifact {path}: {source}")]
    Io {
        /// Artifact path.
        path: PathBuf,
        /// Underlying IO error.
        source: io::Error,
    },
    /// A patch artifact failed JSON lowering, validation, or application.
    #[error(transparent)]
    Patch(#[from] Trace2SkillPatchError),
    /// A named patch file failed JSON lowering, validation, or application.
    #[error("Trace2Skill patch artifact {path} failed replay lowering: {source}")]
    PatchFile {
        /// Artifact path.
        path: PathBuf,
        /// Patch lowering failure.
        source: Trace2SkillPatchError,
    },
    /// Merge-tree provenance was malformed.
    #[error(transparent)]
    MergeTree(#[from] SkillPatchMergeTreeError),
    /// Upstream merge batch size must be nonzero.
    #[error("Trace2Skill replay merge_batch_size must be nonzero")]
    InvalidMergeBatchSize,
    /// The supplied upstream intermediate output directory does not exist.
    #[error("Trace2Skill replay intermediates directory is missing: {path}")]
    MissingIntermediatesDir {
        /// Missing directory.
        path: PathBuf,
    },
    /// The upstream output directory contained no saved map patches.
    #[error("Trace2Skill replay found no map patches in {path}")]
    MissingMapPatches {
        /// Missing or empty map patch directory.
        path: PathBuf,
    },
    /// The upstream output directory did not contain `final_patch.json`.
    #[error("Trace2Skill replay final_patch.json is missing: {path}")]
    MissingFinalPatch {
        /// Expected final patch path.
        path: PathBuf,
    },
    /// A path from the filesystem was not valid UTF-8.
    #[error("Trace2Skill replay artifact path is not UTF-8: {path}")]
    NonUnicodePath {
        /// Non-UTF-8 path.
        path: PathBuf,
    },
    /// Saved merge outputs did not match the deterministic upstream batch shape.
    #[error(
        "Trace2Skill replay merge level {level} at {path} has {actual} outputs, expected {expected}"
    )]
    MergeOutputCountMismatch {
        /// One-based upstream merge level.
        level: usize,
        /// Expected output count from chunking previous plans by merge batch size.
        expected: usize,
        /// Actual saved output count.
        actual: usize,
        /// Merge level directory.
        path: PathBuf,
    },
    /// The same plan id appeared in more than one lowered patch artifact.
    #[error("Trace2Skill replay lowered duplicate patch plan id {plan_id}")]
    DuplicateLoweredPlan {
        /// Duplicate plan id.
        plan_id: String,
    },
    /// A saved map patch's metadata JSON could not be parsed.
    #[error("Trace2Skill map patch {path} has invalid metadata: {source}")]
    PatchMetadata {
        /// Saved map patch path.
        path: PathBuf,
        /// Metadata parse failure.
        source: serde_json::Error,
    },
    /// A saved map patch did not carry upstream `batch_index` metadata.
    #[error("Trace2Skill map patch {path} is missing batch_index")]
    MissingMapPatchBatchIndex {
        /// Saved map patch path.
        path: PathBuf,
    },
    /// A saved map patch's `batch_index` was not a positive platform-sized integer.
    #[error("Trace2Skill map patch {path} has invalid batch_index {batch_index}")]
    InvalidMapPatchBatchIndexValue {
        /// Saved map patch path.
        path: PathBuf,
        /// Raw one-based upstream batch index.
        batch_index: u64,
    },
    /// A saved map patch's `batch_index` does not map to a declared fan-out call.
    #[error(
        "Trace2Skill map patch {path} has batch_index {batch_index}, but fan-out has {expected_calls} calls"
    )]
    InvalidMapPatchBatchIndex {
        /// Saved map patch path.
        path: PathBuf,
        /// One-based upstream batch index.
        batch_index: usize,
        /// Caller-declared fan-out call count.
        expected_calls: usize,
    },
    /// Two saved map patches claimed the same upstream batch.
    #[error("Trace2Skill map patch {path} repeats batch_index {batch_index}")]
    DuplicateMapPatchBatchIndex {
        /// One-based upstream batch index.
        batch_index: usize,
        /// Saved map patch path.
        path: PathBuf,
    },
    /// A saved map patch mapped to a call id that has no prompt/source metadata.
    #[error("Trace2Skill map patch {path} mapped to missing analyst call {call_id}")]
    MissingAnalystCallForMapPatch {
        /// Expected call id.
        call_id: String,
        /// Saved map patch path.
        path: PathBuf,
    },
    /// A saved MAP parse-failure artifact is missing an upstream metadata line.
    #[error("Trace2Skill MAP parse failure {path} is missing {field}")]
    MissingMapParseFailureField {
        /// Saved parse-failure path.
        path: PathBuf,
        /// Missing upstream metadata field.
        field: &'static str,
    },
    /// A saved MAP parse-failure artifact did not carry a `batch_000N` label.
    #[error("Trace2Skill MAP parse failure {path} has invalid batch label {label}")]
    InvalidMapParseFailureBatchLabel {
        /// Saved parse-failure path.
        path: PathBuf,
        /// Raw upstream label.
        label: String,
    },
    /// A saved MAP parse failure's batch does not map to a declared fan-out call.
    #[error(
        "Trace2Skill MAP parse failure {path} has batch_index {batch_index}, but fan-out has {expected_calls} calls"
    )]
    InvalidMapParseFailureBatchIndex {
        /// One-based upstream batch index.
        batch_index: usize,
        /// Caller-declared fan-out call count.
        expected_calls: usize,
        /// Saved parse-failure path.
        path: PathBuf,
    },
    /// Two saved MAP outputs claimed the same upstream batch.
    #[error("Trace2Skill MAP parse failure {path} repeats batch_index {batch_index}")]
    DuplicateMapParseFailureBatchIndex {
        /// One-based upstream batch index.
        batch_index: usize,
        /// Saved parse-failure path.
        path: PathBuf,
    },
    /// A saved MAP parse failure mapped to a call id that has no prompt/source metadata.
    #[error("Trace2Skill MAP parse failure {path} mapped to missing analyst call {call_id}")]
    MissingAnalystCallForMapParseFailure {
        /// Expected call id.
        call_id: String,
        /// Saved parse-failure path.
        path: PathBuf,
    },
    /// Analyst-call evidence construction refused imported data.
    #[error(transparent)]
    AnalystCall(#[from] leaven_evidence::AgentAnalystCallError),
    /// Analyst fan-out evidence refused imported data.
    #[error(transparent)]
    AnalystFanout(#[from] leaven_evidence::AgentAnalystFanoutError),
    /// The validated final id did not have corresponding lowered patch changes.
    #[error("Trace2Skill replay final plan {final_plan_id} has no lowered changes")]
    UnknownFinalLowering {
        /// Missing lowered final plan id.
        final_plan_id: String,
    },
}
