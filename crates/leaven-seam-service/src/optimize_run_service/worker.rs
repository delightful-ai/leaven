use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use leaven_kernel::{CaseId, Cost, Fingerprint};
use leaven_public_seam::{LockedMethod, PublicSeamError};
use leaven_run::{RunCase, RunError, RunOutput, Score, ScoreContext, ScoreError};
use serde_json::{Value, json};

use super::lowering::LoweredCase;
use super::sanitize;
use super::worker_runtime_fingerprint;
use crate::stage::command_runner_result;

/// Services a worker's nested `leaven/lm.complete` plan-IR callback.
pub(super) type LmHandler = Arc<dyn Fn(&Value) -> Result<Value, PublicSeamError> + Send + Sync>;

/// Worker stage payload role being dispatched. Drives both the stage kind and
/// the nested-callback authority scope, so target reads are refused during
/// runner dispatch and permitted (with receipts) during scorer dispatch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StageRole {
    Runner,
    Scorer,
}

/// Shared host state the runner/scorer closures dispatch through.
///
/// Cloned cheaply into each closure: the configured worker argv, the case set
/// (for input, target, and target custody), the request's capability
/// fingerprint, and the LM effect handler that services nested
/// `leaven/lm.complete` callbacks. Worker effect costs are attached to the
/// runner output and the score, so the evaluator folds them into the durable
/// optimization cost; this struct does not separately accumulate them.
#[derive(Clone)]
pub(super) struct WorkerDispatch {
    argv: Arc<Vec<String>>,
    run_id: String,
    capability_fingerprint: String,
    cases_by_id: Arc<BTreeMap<CaseId, LoweredCase>>,
    lm: LmHandler,
    stage_counter: Arc<Mutex<u64>>,
}

impl WorkerDispatch {
    pub(super) fn new(
        argv: Vec<String>,
        run_id: &str,
        capability_fingerprint: String,
        cases_by_id: BTreeMap<CaseId, LoweredCase>,
        lm: LmHandler,
    ) -> Self {
        Self {
            argv: Arc::new(argv),
            run_id: sanitize::sanitize_with_prefix("run", run_id),
            capability_fingerprint,
            cases_by_id: Arc::new(cases_by_id),
            lm,
            stage_counter: Arc::new(Mutex::new(0)),
        }
    }

    /// Declared durable fingerprint for a worker-backed runner/scorer role.
    ///
    /// Hashes this dispatch's configured CommandRunner argv and request
    /// capability fingerprint so resume/eval-cache refuse when the effective
    /// worker binary or capability grant changes.
    pub(super) fn role_fingerprint(&self, role: &str) -> Fingerprint {
        worker_runtime_fingerprint(role, self.argv.as_slice(), &self.capability_fingerprint)
    }

    fn next_stage_call_id(&self, role: StageRole) -> String {
        let mut counter = self
            .stage_counter
            .lock()
            .expect("worker stage counter lock poisoned");
        *counter += 1;
        let label = match role {
            StageRole::Runner => "runner",
            StageRole::Scorer => "scorer",
        };
        format!("sc_optimize_{label}_{counter}")
    }

    fn lowered_case(&self, case_id: CaseId) -> Result<&LoweredCase, PublicSeamError> {
        self.cases_by_id
            .get(&case_id)
            .ok_or_else(|| PublicSeamError::InvalidStageRun {
                message: format!("optimize.run dispatch referenced unknown case `{case_id}`"),
            })
    }
}

/// Runs one runner-stage dispatch for the GEPA loop's runner seam.
///
/// The dispatched payload carries the candidate material (`candidate_payload`,
/// keyed `candidate_template` for prompts or `candidate_agent_kit` for
/// Git-backed kits) and the target-free case input; it never carries target
/// material. Nested `leaven/lm.complete` callbacks are serviced;
/// `leaven/case.target` is refused because runner stages are structurally
/// target-free.
///
/// The candidate material is projected by the caller because the projection is
/// artifact-specific (a prompt template versus a flat kit-revision file map),
/// while the dispatch transport, target isolation, and effect accounting here
/// are shared across artifact types.
pub(super) async fn run_runner_stage(
    dispatch: WorkerDispatch,
    candidate_payload: Value,
    case: RunCase<Value>,
) -> Result<RunOutput<Value>, RunError> {
    let case_id = case.id();
    let lowered = dispatch
        .lowered_case(case_id)
        .map_err(|error| RunError::new(error.to_string()))?
        .clone();
    let stage_call_id = dispatch.next_stage_call_id(StageRole::Runner);
    let params = runner_stage_params(
        &dispatch.run_id,
        &stage_call_id,
        &dispatch.capability_fingerprint,
        candidate_payload,
        &lowered,
        case.input(),
    );

    let result = dispatch_stage(&dispatch, StageRole::Runner, &params)
        .map_err(|error| RunError::new(error.to_string()))?;
    let output_text =
        stage_output_text(&result).map_err(|error| RunError::new(error.to_string()))?;
    let cost = stage_effect_cost(&result);
    Ok(RunOutput::typed(Value::String(output_text.clone()))
        .with_reportable_text(output_text)
        .with_cost(cost))
}

/// Runs one scorer-stage dispatch for the GEPA loop's scorer seam.
///
/// The dispatched payload carries the runner output and a `target_handle` bound
/// to the scored case. Nested `leaven/lm.complete` callbacks are serviced;
/// `leaven/case.target` (and case.input/metadata) reads are served from the
/// request case set with read receipts. The typed reward vector is lowered into
/// the engine [`Score`], preserving per-reward values as metrics and feedback
/// text into the channel GEPA's reflective dataset reads.
///
/// The scorer is artifact-agnostic: it never inspects the candidate artifact,
/// only the runner output and the scored case, so one implementation serves both
/// the prompt and Git-backed kit paths.
pub(super) async fn run_scorer_stage<A>(
    dispatch: WorkerDispatch,
    ctx: ScoreContext<A, Value, Value, Value>,
) -> Result<Score, ScoreError>
where
    A: leaven_core::Artifact,
{
    let case_id = ctx.case.id();
    let lowered = dispatch
        .lowered_case(case_id)
        .map_err(|error| ScoreError::new(error.to_string()))?
        .clone();
    let runner_text = match &ctx.output.output {
        Value::String(text) => text.clone(),
        other => other.to_string(),
    };
    let stage_call_id = dispatch.next_stage_call_id(StageRole::Scorer);
    let params = scorer_stage_params(
        &dispatch.run_id,
        &stage_call_id,
        &dispatch.capability_fingerprint,
        &lowered,
        &runner_text,
    );

    let result = dispatch_stage(&dispatch, StageRole::Scorer, &params)
        .map_err(|error| ScoreError::new(error.to_string()))?;
    let score = lower_score(&ctx, &result)?;
    Ok(score)
}

fn dispatch_stage(
    dispatch: &WorkerDispatch,
    role: StageRole,
    params: &Value,
) -> Result<Value, PublicSeamError> {
    let mut effects = stage_effects(dispatch, role);
    let result = command_runner_result(&dispatch.argv, params, &mut effects)?;
    Ok(result)
}

/// Builds the nested-callback handler scoped to the dispatched stage role.
///
/// `leaven/lm.complete` is always serviced through the configured LM. Case
/// reads are served from the request case set: `case.target` is refused during
/// runner-stage dispatch (target isolation at the execution layer) and served
/// with a read receipt during scorer-stage dispatch; `case.input` and
/// `case.metadata` are served for both roles.
fn stage_effects(
    dispatch: &WorkerDispatch,
    role: StageRole,
) -> impl FnMut(LockedMethod, &Value) -> Result<Value, PublicSeamError> + '_ {
    move |method, params| {
        match method {
        LockedMethod::LmComplete => (dispatch.lm)(params),
        LockedMethod::CaseTarget => match role {
            StageRole::Runner => Err(PublicSeamError::InvalidPlan {
                message: "leaven/case.target is refused during runner-stage dispatch; runner stages are target-free"
                    .to_owned(),
            }),
            StageRole::Scorer => serve_case_read(dispatch, params, CaseReadField::Target),
        },
        LockedMethod::CaseInput => serve_case_read(dispatch, params, CaseReadField::Input),
        LockedMethod::CaseMetadata => serve_case_read(dispatch, params, CaseReadField::Metadata),
        other => Err(PublicSeamError::InvalidPlan {
            message: format!(
                "optimize.run worker requested unsupported callback method `{}`",
                other.as_str()
            ),
        }),
    }
    }
}

#[derive(Clone, Copy)]
enum CaseReadField {
    Input,
    Target,
    Metadata,
}

impl CaseReadField {
    const fn field(self) -> &'static str {
        match self {
            Self::Input => "input",
            Self::Target => "target",
            Self::Metadata => "metadata",
        }
    }

    const fn data_class(self) -> &'static str {
        match self {
            Self::Input => "case.input",
            Self::Target => "case.target",
            Self::Metadata => "case.metadata",
        }
    }

    const fn receipt_suffix(self) -> &'static str {
        match self {
            Self::Input => "input",
            Self::Target => "target",
            Self::Metadata => "metadata",
        }
    }
}

fn serve_case_read(
    dispatch: &WorkerDispatch,
    params: &Value,
    field: CaseReadField,
) -> Result<Value, PublicSeamError> {
    let wire_case = case_read_wire_id(params)?;
    let lowered = dispatch
        .cases_by_id
        .values()
        .find(|case| case.wire_case == wire_case)
        .ok_or_else(|| PublicSeamError::InvalidPlan {
            message: format!("optimize.run worker requested unknown case `{wire_case}`"),
        })?;
    // Input and target are required; metadata is optional, so an absent
    // metadata bag reads as an empty object rather than refusing the scorer's
    // read. The worker can then build a complete `ScoringCaseView` without
    // pre-knowing which cases carry metadata.
    let empty_metadata = Value::Object(serde_json::Map::new());
    let value = match field {
        CaseReadField::Input => &lowered.input,
        CaseReadField::Target => &lowered.target,
        CaseReadField::Metadata => lowered.metadata.as_ref().unwrap_or(&empty_metadata),
    };
    Ok(json!({
        "primary": {
            "kind": "case_record",
            "case": wire_case,
            field.field(): value,
            "data_classes": [field.data_class()]
        },
        "receipts": [{
            "kind": "query",
            "receipt": format!("qrec_{}_{}", sanitize::sanitize_token(&wire_case), field.receipt_suffix())
        }]
    }))
}

/// Extracts the wire case id from a worker case-read callback.
///
/// Worker callbacks are Plan IR: `case_query.load` carries the case ref under
/// `ops[*].expr.query.case`. A direct `case` field is also accepted for simpler
/// callbacks.
fn case_read_wire_id(params: &Value) -> Result<String, PublicSeamError> {
    if let Some(id) = params
        .get("ops")
        .and_then(Value::as_array)
        .and_then(|ops| {
            ops.iter()
                .find_map(|op| op.pointer("/expr/query/case").map(case_ref_id))
        })
        .flatten()
    {
        return Ok(id);
    }
    case_ref_id(
        params
            .get("case")
            .ok_or_else(|| PublicSeamError::InvalidPlan {
                message: "case read callback must carry a case id".to_owned(),
            })?,
    )
    .ok_or_else(|| PublicSeamError::InvalidPlan {
        message: "case read callback must carry a case id".to_owned(),
    })
}

fn case_ref_id(case_ref: &Value) -> Option<String> {
    match case_ref {
        Value::String(id) => Some(id.clone()),
        Value::Object(object) => object
            .get("id")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        _ => None,
    }
}

fn runner_stage_params(
    run_id: &str,
    stage_call_id: &str,
    capability_fingerprint: &str,
    candidate_payload: Value,
    lowered: &LoweredCase,
    case_input: &Value,
) -> Value {
    // `case_input` carries the target-free material the worker needs: the
    // projected candidate material (a `candidate_template` for prompts or a
    // `candidate_agent_kit` flat file map for Git-backed kits) plus the case
    // input. Target material is never included.
    let mut payload_case_input = match candidate_payload {
        Value::Object(map) => map,
        other => {
            let mut map = serde_json::Map::new();
            map.insert("candidate".to_owned(), other);
            map
        }
    };
    payload_case_input.insert("case_input".to_owned(), case_input.clone());
    let payload_case_input = Value::Object(payload_case_input);
    json!({
        "schema_version": "leaven.stage_run.v1",
        "message": "stage_run_request",
        "stage": "runner",
        "payload": {
            "schema_version": "leaven.stage_payloads.v1",
            "role": "runner",
            "run": run_id,
            "stage_call_id": stage_call_id,
            "candidate": candidate_ref(lowered),
            "case": lowered.wire_case,
            "case_input": payload_case_input,
            "target_forbidden": true,
            "capability_fingerprint": capability_fingerprint
        }
    })
}

fn scorer_stage_params(
    run_id: &str,
    stage_call_id: &str,
    capability_fingerprint: &str,
    lowered: &LoweredCase,
    runner_text: &str,
) -> Value {
    json!({
        "schema_version": "leaven.stage_run.v1",
        "message": "stage_run_request",
        "stage": "scorer",
        "payload": {
            "schema_version": "leaven.stage_payloads.v1",
            "role": "scorer",
            "run": run_id,
            "stage_call_id": stage_call_id,
            "evaluation_request_id": format!("evalreq_optimize_{}", sanitize::sanitize_token(&lowered.wire_case)),
            "candidate": candidate_ref(lowered),
            "case": lowered.wire_case,
            "output": {
                "kind": "text",
                "summary": runner_text,
                "value": runner_text,
                "visibility": "optimizer_visible",
                "data_classes": ["candidate.output"]
            },
            "target_handle": lowered.wire_case,
            "capability_fingerprint": capability_fingerprint
        }
    })
}

fn candidate_ref(lowered: &LoweredCase) -> String {
    // The candidate ref names the candidate under evaluation. The host owns
    // candidate identity through the run graph; the worker only needs a
    // schema-valid opaque ref, so a stable per-case candidate label is enough
    // for stateless rollout dispatch.
    format!(
        "cand_optimize_{}",
        sanitize::sanitize_token(&lowered.wire_case)
    )
}

fn stage_output_text(result: &Value) -> Result<String, PublicSeamError> {
    result
        .pointer("/output/value")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| PublicSeamError::InvalidStageRun {
            message: "stage result output must carry a text value".to_owned(),
        })
}

fn stage_effect_cost(result: &Value) -> Cost {
    let mut cost = Cost::zero();
    if let Some(receipts) = result.get("effect_receipts").and_then(Value::as_array) {
        for receipt in receipts {
            cost = cost.combine(&effect_receipt_cost(receipt));
        }
    }
    cost
}

fn lower_score<A>(
    ctx: &ScoreContext<A, Value, Value, Value>,
    result: &Value,
) -> Result<Score, ScoreError>
where
    A: leaven_core::Artifact,
{
    let score_value = result
        .pointer("/score/value")
        .and_then(Value::as_f64)
        .ok_or_else(|| ScoreError::new("scorer stage result must carry a numeric score value"))?;
    if !score_value.is_finite() {
        return Err(ScoreError::new("scorer stage score value must be finite"));
    }
    let feedback = collect_reward_feedback(result);
    // The reportable output must match the runner's declared assessed output
    // (the candidate answer text), not the scalar score.
    let assessed_output = match &ctx.output.output {
        Value::String(text) => text.clone(),
        other => other.to_string(),
    };
    let mut score =
        Score::new(score_value, feedback).with_output(ctx.report_text_output(assessed_output));
    // The collapsed reward is the comparable score GEPA selects on; the per-
    // reward vector is preserved as durable score trace lines (not as cost,
    // which would corrupt budget accounting) so reward breakdowns stay visible
    // without being charged.
    for line in reward_trace_lines(result) {
        score = score.with_trace(line);
    }
    // Stage effect cost is the real metered scorer cost (LM callbacks, etc.).
    // The evaluator folds this into the durable optimization cost.
    score = score.with_cost(stage_effect_cost(result));
    Ok(score)
}

fn reward_trace_lines(result: &Value) -> Vec<String> {
    let mut lines = Vec::new();
    if let Some(rewards) = result.pointer("/score/rewards").and_then(Value::as_array) {
        for reward in rewards {
            if let (Some(id), Some(value)) = (
                reward.get("id").and_then(Value::as_str),
                reward.get("value").and_then(Value::as_f64),
            ) {
                let weight = reward.get("weight").and_then(Value::as_f64).unwrap_or(1.0);
                lines.push(format!("reward {id}: value={value} weight={weight}"));
            }
        }
    }
    lines
}

fn collect_reward_feedback(result: &Value) -> String {
    let mut lines = Vec::new();
    if let Some(rewards) = result.pointer("/score/rewards").and_then(Value::as_array) {
        for reward in rewards {
            if let Some(feedback) = reward.get("feedback").and_then(Value::as_str)
                && !feedback.trim().is_empty()
            {
                let id = reward.get("id").and_then(Value::as_str).unwrap_or("reward");
                lines.push(format!("{id}: {feedback}"));
            }
        }
    }
    if lines.is_empty() {
        "scorer reward vector carried no feedback".to_owned()
    } else {
        lines.join("\n")
    }
}

fn effect_receipt_cost(receipt: &Value) -> Cost {
    let mut cost = Cost::zero();
    let Some(fact) = receipt.get("cost") else {
        return cost;
    };
    if let Some(lm_calls) = fact.get("lm_calls").and_then(Value::as_u64) {
        cost = cost.combine(&Cost::llm_calls(lm_calls));
    }
    if let Some(input_tokens) = fact.get("input_tokens").and_then(Value::as_u64) {
        cost = cost.combine(&Cost {
            prompt_tokens: input_tokens,
            ..Cost::zero()
        });
    }
    if let Some(output_tokens) = fact.get("output_tokens").and_then(Value::as_u64) {
        cost = cost.combine(&Cost {
            completion_tokens: output_tokens,
            ..Cost::zero()
        });
    }
    if let Some(usd_micro) = fact.get("usd_micro").and_then(Value::as_u64)
        && let Ok(axis) = Cost::custom("usd_micro", u64_to_f64(usd_micro))
    {
        cost = cost.combine(&axis);
    }
    cost
}

#[allow(clippy::cast_precision_loss)]
fn u64_to_f64(value: u64) -> f64 {
    // Cost amounts tolerate the f64 rounding for values beyond 2^53; usd_micro
    // counters in practice stay far below that bound.
    value as f64
}
