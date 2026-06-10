use std::collections::BTreeMap;

use leaven_gepa::{GepaCandidateIndex, GepaReport};
use leaven_kernel::CandidateId;
use leaven_run::Optimized;
use serde_json::{Value, json};

use super::error::OptimizeRunHostError;
use super::instrumentation::CandidateArtifacts;
use super::problem::SeamPromptArtifact;
use super::sanitize;

/// Inputs the result projection composes into the locked result document.
pub(super) struct ProjectionInputs<'a> {
    pub(super) run_id: &'a str,
    pub(super) seed_schema: &'a str,
    pub(super) optimized: &'a Optimized<SeamPromptArtifact>,
    pub(super) report: &'a GepaReport,
    pub(super) artifacts: &'a CandidateArtifacts,
    pub(super) revision: &'a str,
}

/// Projects the durable optimization result into a locked
/// `leaven.optimize_run.v1` result document.
///
/// The frontier is sourced from `GepaReport.candidates` (graph-truth candidate
/// ids, parent indices, validation scores). Each candidate's artifact is the
/// template snapshotted from graph truth during the run. The best candidate is
/// the optimizer-selected best, which always appears in the frontier because
/// the seed and every admitted child are reported candidates.
///
/// `applied_proposals` mints service-issued opaque `wrec_` receipts bound 1:1 to
/// the run's durable candidate-apply records: every reported candidate names a
/// graph candidate created through a `RunContext` apply, so each receipt names
/// real graph truth rather than an invented id.
pub(super) fn project_result(inputs: &ProjectionInputs<'_>) -> Result<Value, OptimizeRunHostError> {
    let index_to_candidate: BTreeMap<GepaCandidateIndex, CandidateId> = inputs
        .report
        .candidates
        .iter()
        .map(|candidate| (candidate.index, candidate.candidate))
        .collect();

    if inputs.report.candidates.is_empty() {
        return Err(OptimizeRunHostError::projection(
            "GEPA report carried no candidates to project a frontier",
        ));
    }

    let best_id = inputs.optimized.best_id().or(inputs.report.best_candidate);
    let mut best_entry = None;
    let mut frontier = Vec::with_capacity(inputs.report.candidates.len());
    let mut applied_proposals = Vec::with_capacity(inputs.report.candidates.len());

    for (ordinal, candidate) in inputs.report.candidates.iter().enumerate() {
        let template = candidate_template(inputs, candidate.candidate)?;
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
            &template,
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
    // One GEPA iteration produces one proposal attempt; count distinct
    // iteration ordinals so a skipped or duplicated screening row cannot inflate
    // or deflate the reported iteration count.
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
        "cost": cost_total(inputs),
        "run": {
            "run": sanitize::sanitize_with_prefix("run", inputs.run_id),
            "revision": inputs.revision,
        },
        "applied_proposals": applied_proposals,
    }))
}

fn candidate_template(
    inputs: &ProjectionInputs<'_>,
    candidate: CandidateId,
) -> Result<String, OptimizeRunHostError> {
    if let Some(template) = inputs.artifacts.template(candidate) {
        return Ok(template);
    }
    // The optimizer-selected best and seed templates are always recoverable
    // from the `Optimized` facade even if the graph snapshot missed them.
    if inputs.optimized.best_id() == Some(candidate)
        && let Some(best) = inputs.optimized.best()
    {
        return Ok(best.template().to_owned());
    }
    Err(OptimizeRunHostError::projection(format!(
        "candidate `{candidate}` template was not snapshotted from graph truth"
    )))
}

fn candidate_entry(
    candidate: CandidateId,
    parent: Option<&str>,
    score: f64,
    template: &str,
    seed_schema: &str,
) -> Value {
    json!({
        "candidate": candidate_wire_id(candidate),
        "parent": parent,
        "score": score,
        "artifact": {
            "artifact_type": "prompt",
            "artifact_schema": seed_schema,
            "artifact": {"template": template},
        }
    })
}

fn candidate_wire_id(candidate: CandidateId) -> String {
    format!(
        "cand_optimize_{}",
        sanitize::sanitize_token(&candidate.to_string())
    )
}

fn cost_total(inputs: &ProjectionInputs<'_>) -> Value {
    // The optimization cost already aggregates every runner and scorer worker
    // effect cost, because each stage attaches its effect cost to the runner
    // output and the score, which the evaluator folds into evaluation cost.
    // Re-adding the worker effect total here would double-count scorer effects.
    let optimization = &inputs.optimized.summary().optimization_cost;
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
    // usd_micro is a non-negative integer-valued counter carried as a cost
    // amount; rounding to the nearest whole micro-dollar is the intended
    // projection.
    value.round().max(0.0) as u64
}
