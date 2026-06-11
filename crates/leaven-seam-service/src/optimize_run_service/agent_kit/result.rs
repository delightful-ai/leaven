//! Projects the durable kit optimization result into the locked
//! `leaven.optimize_run.v1` result document.
//!
//! Each frontier candidate's artifact is its `GitProgramArtifact` revision read
//! back into flat `agent_kit` wire parts (`system_prompt` plus path-relative
//! skill files). This mirrors the prompt-path projection but resolves the
//! candidate artifact by reading the candidate's Git revision out of the durable
//! store rather than reading a template string.

use std::collections::BTreeMap;

use leaven_agentic_git::{GitProgramSeed, read_revision_files};
use leaven_artifact_git::GitProgramArtifact;
use leaven_gepa::{GepaCandidateIndex, GepaReport};
use leaven_kernel::CandidateId;
use leaven_run::Optimized;
use serde_json::{Value, json};

use super::super::error::OptimizeRunHostError;
use super::super::sanitize;
use super::instrumentation::KitArtifacts;
use super::projection::{kit_parts_from_files, kit_wire_artifact};

/// Inputs the kit result projection composes into the locked result document.
pub(in crate::optimize_run_service) struct KitProjectionInputs<'a> {
    pub(in crate::optimize_run_service) run_id: &'a str,
    pub(in crate::optimize_run_service) seed_schema: &'a str,
    pub(in crate::optimize_run_service) optimized: &'a Optimized<GitProgramArtifact>,
    pub(in crate::optimize_run_service) report: &'a GepaReport,
    pub(in crate::optimize_run_service) artifacts: &'a KitArtifacts,
    pub(in crate::optimize_run_service) seed: &'a GitProgramSeed,
    pub(in crate::optimize_run_service) revision: &'a str,
}

/// Projects the durable kit optimization result into a locked
/// `leaven.optimize_run.v1` result document.
pub(in crate::optimize_run_service) fn project_kit_result(
    inputs: &KitProjectionInputs<'_>,
) -> Result<Value, OptimizeRunHostError> {
    let index_to_candidate: BTreeMap<GepaCandidateIndex, CandidateId> = inputs
        .report
        .candidates
        .iter()
        .map(|candidate| (candidate.index, candidate.candidate))
        .collect();

    if inputs.report.candidates.is_empty() {
        return Err(OptimizeRunHostError::projection(
            "GEPA kit report carried no candidates to project a frontier",
        ));
    }

    let best_id = inputs.optimized.best_id().or(inputs.report.best_candidate);
    let mut best_entry = None;
    let mut frontier = Vec::with_capacity(inputs.report.candidates.len());
    let mut applied_proposals = Vec::with_capacity(inputs.report.candidates.len());

    for (ordinal, candidate) in inputs.report.candidates.iter().enumerate() {
        let kit_artifact = candidate_kit_artifact(inputs, candidate.candidate)?;
        let parent = candidate
            .parents
            .first()
            .and_then(|index| index_to_candidate.get(index))
            .map(|parent| candidate_wire_id(*parent));
        let score = candidate.validation_score.unwrap_or(0.0);
        let entry = candidate_entry(
            candidate.candidate,
            parent.as_deref(),
            score,
            &kit_artifact,
            inputs.seed_schema,
        );
        if Some(candidate.candidate) == best_id {
            best_entry = Some(entry.clone());
        }
        frontier.push(entry);
        applied_proposals.push(format!("wrec_optimize_apply_{ordinal}"));
    }

    let best = best_entry.unwrap_or_else(|| frontier[0].clone());
    let metric_calls_used = inputs.optimized.summary().optimization_cost.metric_calls;
    let iterations = u64::try_from(
        inputs
            .report
            .proposal_attempts
            .iter()
            .map(|attempt| attempt.iteration)
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
    )
    .unwrap_or(u64::MAX);

    Ok(json!({
        "schema_version": "leaven.optimize_run.v1",
        "message": "optimize_run_result",
        "best": best,
        "frontier": frontier,
        "iterations": iterations,
        "metric_calls_used": metric_calls_used,
        "cost": cost_total(inputs.optimized),
        "run": {
            "run": sanitize::sanitize_with_prefix("run", inputs.run_id),
            "revision": inputs.revision,
        },
        "applied_proposals": applied_proposals,
    }))
}

/// Reads a candidate's kit revision back into the flat `agent_kit` wire
/// artifact.
fn candidate_kit_artifact(
    inputs: &KitProjectionInputs<'_>,
    candidate: CandidateId,
) -> Result<Value, OptimizeRunHostError> {
    let artifact = inputs.artifacts.artifact(candidate).or_else(|| {
        (inputs.optimized.best_id() == Some(candidate))
            .then(|| inputs.optimized.best().cloned())
            .flatten()
    });
    let artifact = artifact.ok_or_else(|| {
        OptimizeRunHostError::projection(format!(
            "kit candidate `{candidate}` artifact was not snapshotted from graph truth"
        ))
    })?;
    let repo = inputs.seed.repo();
    let revision = artifact
        .repo(repo)
        .ok_or_else(|| {
            OptimizeRunHostError::projection(format!(
                "kit candidate `{candidate}` is missing repo `{repo}`"
            ))
        })?
        .revision()
        .clone();
    let files = read_revision_files(inputs.seed.stores(), repo, &revision)
        .map_err(|error| OptimizeRunHostError::projection(error.to_string()))?;
    let parts = kit_parts_from_files(&files)?;
    Ok(kit_wire_artifact(&parts))
}

fn candidate_entry(
    candidate: CandidateId,
    parent: Option<&str>,
    score: f64,
    kit_artifact: &Value,
    seed_schema: &str,
) -> Value {
    json!({
        "candidate": candidate_wire_id(candidate),
        "parent": parent,
        "score": score,
        "artifact": {
            "artifact_type": "agent_kit",
            "artifact_schema": seed_schema,
            "artifact": kit_artifact,
        }
    })
}

fn candidate_wire_id(candidate: CandidateId) -> String {
    format!(
        "cand_optimize_{}",
        sanitize::sanitize_token(&candidate.to_string())
    )
}

fn cost_total(optimized: &Optimized<GitProgramArtifact>) -> Value {
    let optimization = &optimized.summary().optimization_cost;
    let usd_micro = optimization
        .other
        .get("usd_micro")
        .map_or(0, |amount| f64_to_u64(amount.as_f64()));
    json!({
        "usd_micro": usd_micro,
        "lm_calls": optimization.llm_calls,
        "input_tokens": optimization.prompt_tokens,
        "output_tokens": optimization.completion_tokens,
        "metric_calls": optimization.metric_calls,
    })
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn f64_to_u64(value: f64) -> u64 {
    value.round().max(0.0) as u64
}
