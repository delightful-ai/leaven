use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value, json};

use crate::{PlanResultDocument, PublicSeamError, SchemaFingerprint};

mod effects;
mod evaluate;
mod outcomes;
mod queries;
mod receipts;
mod result_value;

use effects::LiveWorkspaceHandle;
pub use effects::{
    AgentCommandOutputRefs, PlanAgentRunOutcome, PlanAgentRunRequest, PlanEmitRunEventOutcome,
    PlanEmitRunEventRequest, PlanLmCompleteOutcome, PlanLmCompleteRequest, PlanSandboxExecOutcome,
    PlanSandboxExecRequest, PlanWorkspaceMaterializeOutcome, PlanWorkspaceMaterializeRequest,
    PlanWorkspaceReleaseOutcome, PlanWorkspaceReleaseRequest,
};
use evaluate::{EvaluatedExpr, ResolvedDependencies, evaluate_expr, resolved_dependency_values};
pub use queries::{
    PlanCaseQueryOutcome, PlanCaseQueryRequest, PlanGraphQueryOutcome, PlanGraphQueryRequest,
    PlanGraphReadScope, PlanWorkspaceQueryOutcome, PlanWorkspaceQueryRequest,
};
use queries::{
    case_query_include, case_query_projection, plan_contains_case_query,
    require_included_case_fields, require_requested_case_field, validate_case_query_authority,
    validate_workspace_query_value_shape, workspace_query_expected_value_kind,
    workspace_query_projection, workspace_query_request, workspace_query_request_from_values,
};
pub use receipts::validate_plan_result_receipts;
pub use receipts::{validate_agent_session_value, validate_sandbox_exec_value};

/// Execution metadata for the advanced public-seam Plan IR harness.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanExecutionContext {
    capability_fingerprint: String,
    policy_fingerprint: String,
    base_revision: String,
    started_at: String,
    completed_at: String,
    evaluation_run: Option<String>,
    evaluation_request_id: Option<String>,
    case_partition: Option<String>,
}

impl PlanExecutionContext {
    /// Creates context used to project a representative Plan IR execution into a Plan Result.
    pub fn new(
        capability_fingerprint: impl Into<String>,
        policy_fingerprint: impl Into<String>,
        base_revision: impl Into<String>,
        started_at: impl Into<String>,
        completed_at: impl Into<String>,
    ) -> Self {
        Self {
            capability_fingerprint: capability_fingerprint.into(),
            policy_fingerprint: policy_fingerprint.into(),
            base_revision: base_revision.into(),
            started_at: started_at.into(),
            completed_at: completed_at.into(),
            evaluation_run: None,
            evaluation_request_id: None,
            case_partition: None,
        }
    }

    /// Adds evaluator request identity used for capability-authorized `case_query.load` reads.
    #[must_use]
    pub fn with_evaluation_request(
        mut self,
        run: impl Into<String>,
        evaluation_request_id: impl Into<String>,
    ) -> Self {
        self.evaluation_run = Some(run.into());
        self.evaluation_request_id = Some(evaluation_request_id.into());
        self
    }

    /// Adds the resolved case partition used for capability-authorized case reads.
    #[must_use]
    pub fn with_case_partition(mut self, partition: impl Into<String>) -> Self {
        self.case_partition = Some(partition.into());
        self
    }
}

/// Result of executing a representative Plan IR document through the public-seam harness.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanExecutionReport {
    value: Value,
    document: PlanResultDocument,
}

impl PlanExecutionReport {
    pub(crate) const fn new(value: Value, document: PlanResultDocument) -> Self {
        Self { value, document }
    }

    /// Schema-valid Plan Result wire value produced by the execution.
    pub fn value(&self) -> &Value {
        &self.value
    }

    /// Semantic Plan Result document validated by the active public-seam owner.
    pub const fn document(&self) -> &PlanResultDocument {
        &self.document
    }
}

/// Host for effectful Plan IR operations in the representative public-seam harness.
///
/// The harness owns Plan validation, dependency lowering, receipt production, and
/// result validation. The host owns the actual effect for each typed operation.
pub trait PlanExecutionHost {
    /// Executes a pure typed `graph_query` read.
    fn graph_query(
        &mut self,
        request: PlanGraphQueryRequest<'_>,
    ) -> Result<PlanGraphQueryOutcome, PublicSeamError> {
        let _ = request;
        Err(invalid_plan(
            "Plan execution host does not provide graph_query reads",
        ))
    }

    /// Executes a typed `case_query.load` read.
    fn case_query_load(
        &mut self,
        request: PlanCaseQueryRequest<'_>,
    ) -> Result<PlanCaseQueryOutcome, PublicSeamError> {
        let _ = request;
        Err(invalid_plan(
            "Plan execution host does not provide case_query.load reads",
        ))
    }

    /// Executes a typed `workspace_query` read.
    fn workspace_query(
        &mut self,
        request: PlanWorkspaceQueryRequest<'_>,
    ) -> Result<PlanWorkspaceQueryOutcome, PublicSeamError> {
        let _ = request;
        Err(invalid_plan(
            "Plan execution host does not provide workspace_query reads",
        ))
    }

    /// Executes a typed `lm_complete` call.
    fn lm_complete(
        &mut self,
        request: PlanLmCompleteRequest<'_>,
    ) -> Result<PlanLmCompleteOutcome, PublicSeamError>;

    /// Resolves a cached typed `lm_complete` call without live provider work.
    fn cached_lm_complete(
        &mut self,
        request: PlanLmCompleteRequest<'_>,
    ) -> Result<Option<PlanLmCompleteOutcome>, PublicSeamError> {
        let _ = request;
        Ok(None)
    }

    /// Executes a typed `agent_run` call.
    fn agent_run(
        &mut self,
        request: PlanAgentRunRequest<'_>,
    ) -> Result<PlanAgentRunOutcome, PublicSeamError> {
        let _ = request;
        Err(invalid_plan(
            "Plan execution host does not provide agent_run calls",
        ))
    }

    /// Executes a typed `sandbox_exec` call.
    fn sandbox_exec(
        &mut self,
        request: PlanSandboxExecRequest<'_>,
    ) -> Result<PlanSandboxExecOutcome, PublicSeamError> {
        let _ = request;
        Err(invalid_plan(
            "Plan execution host does not provide sandbox_exec calls",
        ))
    }

    /// Executes a typed `workspace_materialize` call.
    fn workspace_materialize(
        &mut self,
        request: PlanWorkspaceMaterializeRequest<'_>,
    ) -> Result<PlanWorkspaceMaterializeOutcome, PublicSeamError> {
        let _ = request;
        Err(invalid_plan(
            "Plan execution host does not provide workspace_materialize calls",
        ))
    }

    /// Executes a typed `workspace_release` call.
    fn workspace_release(
        &mut self,
        request: PlanWorkspaceReleaseRequest<'_>,
    ) -> Result<PlanWorkspaceReleaseOutcome, PublicSeamError> {
        let _ = request;
        Err(invalid_plan(
            "Plan execution host does not provide workspace_release calls",
        ))
    }

    /// Executes a typed `emit_run_event` write.
    fn emit_run_event(
        &mut self,
        request: PlanEmitRunEventRequest<'_>,
    ) -> Result<PlanEmitRunEventOutcome, PublicSeamError>;

    /// Loads a prior operation receipt for replay mode.
    fn replay_receipt(&mut self, receipt: &str) -> Result<Value, PublicSeamError> {
        Err(invalid_plan(format!(
            "replay mode could not load receipt `{receipt}`"
        )))
    }
}

pub fn execute_plan<H: PlanExecutionHost>(
    plan: &Value,
    plan_document: &crate::PlanDocument,
    context: &PlanExecutionContext,
    host: &mut H,
) -> Result<Value, PublicSeamError> {
    if plan_contains_case_query(plan)? {
        return Err(invalid_plan(
            "case_query.load execution requires capability-authorized Plan execution",
        ));
    }
    execute_authorized_plan(plan, plan_document, context, host, None)
}

pub fn execute_plan_with_capability<H: PlanExecutionHost>(
    plan: &Value,
    plan_document: &crate::PlanDocument,
    context: &PlanExecutionContext,
    capability: &crate::CapabilityDocument,
    host: &mut H,
) -> Result<Value, PublicSeamError> {
    validate_case_query_authority(plan, context, capability)?;
    execute_authorized_plan(plan, plan_document, context, host, Some(capability))
}

fn execute_authorized_plan<H: PlanExecutionHost>(
    plan: &Value,
    plan_document: &crate::PlanDocument,
    context: &PlanExecutionContext,
    host: &mut H,
    capability: Option<&crate::CapabilityDocument>,
) -> Result<Value, PublicSeamError> {
    match plan_document.mode_kind() {
        "execute" => execute_effects(
            plan,
            plan_document,
            context,
            host,
            EffectMode::Live,
            capability,
        ),
        "dry_run" => dry_run_result(plan, context),
        "require_cached" => execute_effects(
            plan,
            plan_document,
            context,
            host,
            EffectMode::RequireCached,
            capability,
        ),
        "replay" => replay_result(plan, context, host),
        other => Err(invalid_plan(format!(
            "unknown Plan execution mode `{other}`"
        ))),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EffectMode {
    Live,
    RequireCached,
}

#[derive(Clone, Copy, Debug)]
struct EffectContext<'a> {
    mode: EffectMode,
    capability: Option<&'a crate::CapabilityDocument>,
}

fn execute_effects<H: PlanExecutionHost>(
    plan: &Value,
    plan_document: &crate::PlanDocument,
    context: &PlanExecutionContext,
    host: &mut H,
    mode: EffectMode,
    capability: Option<&crate::CapabilityDocument>,
) -> Result<Value, PublicSeamError> {
    if plan_document.commit_kind() == "no_graph_writes"
        && plan_document
            .operation_kinds()
            .contains(&crate::PlanOperationKind::Write)
    {
        return Err(invalid_plan(
            "Plan execution harness cannot execute write ops under no_graph_writes commit",
        ));
    }
    let plan_object = object(plan, "plan")?;
    let ops = plan_object
        .get("ops")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid_plan("plan ops must be an array"))?;
    let plan_id = required_string(plan_object.get("plan_id"), "plan_id")?;
    let mut state = ExecutionState::new(&context.base_revision);
    let effect_context = EffectContext { mode, capability };

    for op in ops {
        execute_op(op, plan_document, context, host, &mut state, effect_context)?;
    }

    Ok(plan_result_value(&PlanResultValue {
        plan_id,
        context,
        final_revision: &state.final_revision,
        replayability_summary: result_value::replayability_summary(&state.values, &state.receipts),
        values: &state.values,
        receipts: &state.receipts,
        charges: &state.charges,
        errors: &state.errors,
    }))
}

fn dry_run_result(plan: &Value, context: &PlanExecutionContext) -> Result<Value, PublicSeamError> {
    let plan_id = required_string(object(plan, "plan")?.get("plan_id"), "plan_id")?;
    Ok(plan_result_value(&PlanResultValue {
        plan_id,
        context,
        final_revision: &context.base_revision,
        replayability_summary: "pure_read",
        values: &Map::new(),
        receipts: &[],
        charges: &[],
        errors: &[],
    }))
}

fn replay_result<H: PlanExecutionHost>(
    plan: &Value,
    context: &PlanExecutionContext,
    host: &mut H,
) -> Result<Value, PublicSeamError> {
    let plan_object = object(plan, "plan")?;
    let plan_id = required_string(plan_object.get("plan_id"), "plan_id")?;
    let mut receipts = Vec::new();
    let mut final_revision = context.base_revision.clone();
    for receipt in replay_receipt_refs(plan_object)? {
        let receipt = host.replay_receipt(receipt)?;
        if let Some(committed) = receipt
            .as_object()
            .and_then(|receipt| receipt.get("committed_revision"))
            .and_then(Value::as_str)
        {
            committed.clone_into(&mut final_revision);
        }
        receipts.push(receipt);
    }
    Ok(plan_result_value(&PlanResultValue {
        plan_id,
        context,
        final_revision: &final_revision,
        replayability_summary: "fully_managed",
        values: &Map::new(),
        receipts: &receipts,
        charges: &[],
        errors: &[],
    }))
}

fn replay_receipt_refs(plan: &Map<String, Value>) -> Result<Vec<&str>, PublicSeamError> {
    let mode = plan
        .get("mode")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid_plan("replay mode must be an object"))?;
    let receipts = mode
        .get("receipts")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid_plan("replay mode must carry receipts"))?;
    receipts
        .iter()
        .map(|receipt| {
            receipt
                .as_str()
                .filter(|receipt| !receipt.trim().is_empty())
                .ok_or_else(|| invalid_plan("replay receipt refs must be strings"))
        })
        .collect()
}

struct PlanResultValue<'a> {
    plan_id: &'a str,
    context: &'a PlanExecutionContext,
    final_revision: &'a str,
    replayability_summary: &'a str,
    values: &'a Map<String, Value>,
    receipts: &'a [Value],
    charges: &'a [Value],
    errors: &'a [Value],
}

fn plan_result_value(parts: &PlanResultValue<'_>) -> Value {
    json!({
        "schema_version": "leaven.plan_result.v1",
        "plan_id": parts.plan_id,
        "capability_fingerprint": parts.context.capability_fingerprint,
        "policy_fingerprint": parts.context.policy_fingerprint,
        "base_revision": parts.context.base_revision,
        "final_revision": parts.final_revision,
        "replayability_summary": parts.replayability_summary,
        "values": parts.values,
        "receipts": parts.receipts,
        "redactions": [],
        "charges": parts.charges,
        "errors": parts.errors
    })
}

struct ExecutionState {
    bindings: BTreeMap<String, Value>,
    binding_data_classes: BTreeMap<String, BTreeSet<String>>,
    live_workspaces: BTreeMap<String, LiveWorkspaceHandle>,
    values: Map<String, Value>,
    receipts: Vec<Value>,
    charges: Vec<Value>,
    errors: Vec<Value>,
    final_revision: String,
}

impl ExecutionState {
    fn new(base_revision: &str) -> Self {
        Self {
            bindings: BTreeMap::new(),
            binding_data_classes: BTreeMap::new(),
            live_workspaces: BTreeMap::new(),
            values: Map::new(),
            receipts: Vec::new(),
            charges: Vec::new(),
            errors: Vec::new(),
            final_revision: base_revision.to_owned(),
        }
    }
}

fn execute_op<H: PlanExecutionHost>(
    op: &Value,
    plan_document: &crate::PlanDocument,
    context: &PlanExecutionContext,
    host: &mut H,
    state: &mut ExecutionState,
    effect_context: EffectContext<'_>,
) -> Result<(), PublicSeamError> {
    let op_object = object(op, "plan op")?;
    let name = required_string(op_object.get("name"), "op.name")?.to_owned();
    let deps = resolved_dependency_values(op_object, state)?;
    match required_string(op_object.get("kind"), "op.kind")? {
        "let" => execute_let(op_object, name, &deps, plan_document, context, host, state),
        "call" => execute_call(op_object, name, &deps, context, host, state, effect_context),
        "write" => execute_write(op_object, name, &deps.values, context, host, state),
        other => Err(invalid_plan(format!(
            "unknown plan operation kind `{other}`"
        ))),
    }
}

fn execute_let(
    op_object: &Map<String, Value>,
    name: String,
    deps: &ResolvedDependencies,
    plan_document: &crate::PlanDocument,
    context: &PlanExecutionContext,
    host: &mut impl PlanExecutionHost,
    state: &mut ExecutionState,
) -> Result<(), PublicSeamError> {
    let expr = op_object
        .get("expr")
        .ok_or_else(|| invalid_plan("let op must carry expr"))?;
    let evaluated = evaluate_expr(expr, &name, deps, plan_document, context, host)?;
    if let Some(receipt) = evaluated.receipt {
        state.receipts.push(receipt);
    }
    let value_kind = evaluated
        .value
        .as_object()
        .and_then(|object| object.get("kind"))
        .and_then(Value::as_str);
    if matches!(
        value_kind,
        Some(
            "graph_set"
                | "case_record"
                | "workspace_snapshot"
                | "workspace_file"
                | "workspace_diff"
                | "workspace_listing"
        )
    ) {
        state.values.insert(name.clone(), evaluated.value.clone());
    }
    if !evaluated.data_classes.is_empty() {
        state
            .binding_data_classes
            .insert(name.clone(), evaluated.data_classes);
    }
    state.bindings.insert(name, evaluated.value);
    Ok(())
}

fn execute_call<H: PlanExecutionHost>(
    op_object: &Map<String, Value>,
    name: String,
    deps: &ResolvedDependencies,
    context: &PlanExecutionContext,
    host: &mut H,
    state: &mut ExecutionState,
    effect_context: EffectContext<'_>,
) -> Result<(), PublicSeamError> {
    let call = op_object
        .get("call")
        .ok_or_else(|| invalid_plan("call op must carry call"))?;
    let call_kind = nested_kind(call, "call")?;
    if let Some(capability) = effect_context.capability {
        crate::call_authority::validate_call_with_dependency_classes(
            call_kind,
            call,
            &deps.values,
            &deps.data_classes,
            capability,
        )?;
    }
    let request_hash = prefixed_jcs_hash(
        "fp_request_sha256_",
        &json!({
            "schema_version": "leaven.plan_call_request.v1",
            "name": name,
            "kind": call_kind,
            "call": call,
            "deps": deps.values
        }),
    )?;
    match call_kind {
        "lm_complete" => {
            validate_structured_output_contract(call, "lm_complete")?;
            let request = PlanLmCompleteRequest {
                name: &name,
                call,
                deps: &deps.values,
            };
            let (outcome, cache) = match effect_context.mode {
                EffectMode::Live => (host.lm_complete(request)?, None),
                EffectMode::RequireCached => {
                    let outcome = host.cached_lm_complete(request)?.ok_or_else(|| {
                        invalid_plan(format!(
                            "require_cached mode refused cache miss for `{name}` lm_complete call"
                        ))
                    })?;
                    (outcome, Some("hit"))
                }
            };
            validate_structured_output_outcome(call, outcome.parsed.as_ref(), "lm_complete")?;
            record_lm_call_outcome(
                name,
                call_kind,
                outcome,
                cache,
                &request_hash,
                context,
                state,
            )
        }
        "agent_run" => {
            if effect_context.mode == EffectMode::RequireCached {
                return Err(invalid_plan(
                    "require_cached mode cannot prove cached execution for `agent_run` calls",
                ));
            }
            execute_agent_run_call(host, name, call, deps, &request_hash, context, state)
        }
        "sandbox_exec" => {
            if effect_context.mode == EffectMode::RequireCached {
                return Err(invalid_plan(
                    "require_cached mode cannot prove cached execution for `sandbox_exec` calls",
                ));
            }
            execute_sandbox_exec_call(host, name, call, deps, &request_hash, context, state)
        }
        "workspace_materialize" => {
            if effect_context.mode == EffectMode::RequireCached {
                return Err(invalid_plan(
                    "require_cached mode cannot prove cached execution for `workspace_materialize` calls",
                ));
            }
            execute_workspace_materialize_call(
                host,
                name,
                call,
                deps,
                &request_hash,
                context,
                state,
            )
        }
        "workspace_release" if effect_context.mode == EffectMode::RequireCached => {
            Err(invalid_plan(
                "require_cached mode cannot prove cached execution for `workspace_release` calls",
            ))
        }
        "workspace_release" => {
            execute_workspace_release_call(host, name, call, deps, &request_hash, context, state)
        }
        _ => Err(invalid_plan(format!(
            "representative Plan IR harness does not execute `{call_kind}` calls"
        ))),
    }
}

fn execute_agent_run_call<H: PlanExecutionHost>(
    host: &mut H,
    name: String,
    call: &Value,
    deps: &ResolvedDependencies,
    request_hash: &str,
    context: &PlanExecutionContext,
    state: &mut ExecutionState,
) -> Result<(), PublicSeamError> {
    let request = PlanAgentRunRequest {
        name: &name,
        call,
        deps: &deps.values,
        live_workspaces: &deps.live_workspaces,
    };
    validate_structured_output_contract(call, "agent_run")?;
    request.live_workspace()?;
    let outcome = host.agent_run(request)?;
    validate_structured_output_outcome(call, outcome.parsed.as_ref(), "agent_run")?;
    record_agent_call_outcome(name, outcome, request_hash, context, state)
}

fn execute_sandbox_exec_call<H: PlanExecutionHost>(
    host: &mut H,
    name: String,
    call: &Value,
    deps: &ResolvedDependencies,
    request_hash: &str,
    context: &PlanExecutionContext,
    state: &mut ExecutionState,
) -> Result<(), PublicSeamError> {
    let request = PlanSandboxExecRequest {
        name: &name,
        call,
        deps: &deps.values,
        live_workspaces: &deps.live_workspaces,
    };
    request.live_workspace()?;
    let expected_output_files = request
        .to_workspace_command()?
        .output_files
        .into_iter()
        .map(|path| path.as_str().to_owned())
        .collect::<BTreeSet<_>>();
    let outcome = host.sandbox_exec(request)?;
    validate_sandbox_stream_outcome(call, &outcome, &expected_output_files)?;
    record_sandbox_call_outcome(name, outcome, request_hash, context, state)
}

fn validate_structured_output_outcome(
    call: &Value,
    parsed: Option<&Value>,
    call_kind: &str,
) -> Result<(), PublicSeamError> {
    if json_schema_output_schema(call, call_kind)?.is_some() {
        let parsed = parsed.ok_or_else(|| {
            invalid_plan(format!(
                "{call_kind} json_schema output must return parsed result payload"
            ))
        })?;
        validate_json_schema_output_payload(call_kind, call, parsed)?;
    }
    Ok(())
}

fn validate_structured_output_contract(
    call: &Value,
    call_kind: &str,
) -> Result<(), PublicSeamError> {
    if let Some((schema, schema_fingerprint)) = json_schema_output_schema(call, call_kind)? {
        validate_json_schema_fingerprint(call_kind, schema, schema_fingerprint)?;
        jsonschema::draft202012::options()
            .build(schema)
            .map_err(|error| {
                invalid_plan(format!(
                    "{call_kind} json_schema output schema is invalid: {error}"
                ))
            })?;
    }
    Ok(())
}

fn json_schema_output_schema<'a>(
    call: &'a Value,
    call_kind: &str,
) -> Result<Option<(&'a Value, &'a str)>, PublicSeamError> {
    let Some(output) = call.get("output").and_then(Value::as_object) else {
        return Ok(None);
    };
    if output.get("kind").and_then(Value::as_str) != Some("json_schema") {
        return Ok(None);
    }
    let schema_fingerprint = required_string(
        output.get("schema_fingerprint"),
        "output.schema_fingerprint",
    )?;
    let schema = output.get("schema").ok_or_else(|| {
        invalid_plan(format!(
            "{call_kind} json_schema output must carry inline schema for execution validation"
        ))
    })?;
    if schema.is_null() {
        return Err(invalid_plan(format!(
            "{call_kind} json_schema output must carry inline schema for execution validation"
        )));
    }
    Ok(Some((schema, schema_fingerprint)))
}

fn validate_json_schema_fingerprint(
    call_kind: &str,
    schema: &Value,
    schema_fingerprint: &str,
) -> Result<(), PublicSeamError> {
    let computed = SchemaFingerprint::for_json_value(schema).map_err(|error| {
        invalid_plan(format!(
            "{call_kind} json_schema output schema fingerprinting failed: {error}"
        ))
    })?;
    if computed.as_str() != schema_fingerprint {
        return Err(invalid_plan(format!(
            "{call_kind} json_schema output schema_fingerprint does not match inline schema"
        )));
    }
    Ok(())
}

fn validate_json_schema_output_payload(
    call_kind: &str,
    call: &Value,
    parsed: &Value,
) -> Result<(), PublicSeamError> {
    let Some((schema, schema_fingerprint)) = json_schema_output_schema(call, call_kind)? else {
        return Ok(());
    };
    validate_json_schema_fingerprint(call_kind, schema, schema_fingerprint)?;
    let validator = jsonschema::draft202012::options()
        .build(schema)
        .map_err(|error| {
            invalid_plan(format!(
                "{call_kind} json_schema output schema is invalid: {error}"
            ))
        })?;
    validator.validate(parsed).map_err(|error| {
        invalid_plan(format!(
            "{call_kind} parsed result payload failed json_schema output contract: {error}"
        ))
    })
}

fn validate_sandbox_stream_outcome(
    call: &Value,
    outcome: &PlanSandboxExecOutcome,
    expected_output_files: &BTreeSet<String>,
) -> Result<(), PublicSeamError> {
    if call
        .get("stream_policy")
        .and_then(Value::as_str)
        .unwrap_or("buffer")
        == "blob_refs_only"
        && (outcome.stdout_ref.is_none() || outcome.stderr_ref.is_none())
    {
        return Err(invalid_plan(
            "sandbox_exec blob_refs_only stream policy requires stdout_ref and stderr_ref",
        ));
    }
    if outcome.status == "completed"
        && (outcome.stdout_ref.is_none() || outcome.stderr_ref.is_none())
    {
        return Err(invalid_plan(
            "completed sandbox_exec result value must carry stdout_ref and stderr_ref",
        ));
    }
    let actual_output_files = outcome.files.keys().cloned().collect::<BTreeSet<_>>();
    if &actual_output_files != expected_output_files {
        let missing = expected_output_files
            .difference(&actual_output_files)
            .cloned()
            .collect::<Vec<_>>();
        let extra = actual_output_files
            .difference(expected_output_files)
            .cloned()
            .collect::<Vec<_>>();
        return Err(invalid_plan(format!(
            "sandbox_exec output file refs must match output contract paths; missing={missing:?} extra={extra:?}"
        )));
    }
    Ok(())
}

fn execute_workspace_materialize_call<H: PlanExecutionHost>(
    host: &mut H,
    name: String,
    call: &Value,
    deps: &ResolvedDependencies,
    request_hash: &str,
    context: &PlanExecutionContext,
    state: &mut ExecutionState,
) -> Result<(), PublicSeamError> {
    let request = PlanWorkspaceMaterializeRequest {
        name: &name,
        call,
        deps: &deps.values,
    };
    let lifetime = request.lifetime()?.to_owned();
    let outcome = host.workspace_materialize(request)?;
    if !is_workspace_id(&outcome.workspace) {
        return Err(invalid_plan(format!(
            "workspace_materialize host returned invalid workspace `{}`",
            outcome.workspace
        )));
    }
    if outcome.lifetime != lifetime {
        return Err(invalid_plan(format!(
            "workspace_materialize host returned lifetime `{}` for requested lifetime `{lifetime}`",
            outcome.lifetime
        )));
    }
    record_workspace_materialize_outcome(name, outcome, request_hash, context, state)
}

fn execute_workspace_release_call<H: PlanExecutionHost>(
    host: &mut H,
    name: String,
    call: &Value,
    deps: &ResolvedDependencies,
    request_hash: &str,
    context: &PlanExecutionContext,
    state: &mut ExecutionState,
) -> Result<(), PublicSeamError> {
    let request = PlanWorkspaceReleaseRequest {
        name: &name,
        call,
        deps: &deps.values,
        live_workspaces: &deps.live_workspaces,
    };
    let requested_workspace = request.workspace_ref()?;
    let workspace = request.live_workspace()?.to_owned();
    let lifetime = request.live_workspace_lifetime()?.to_owned();
    let outcome = host.workspace_release(request)?;
    if outcome.workspace != workspace {
        return Err(invalid_plan(format!(
            "workspace_release host returned workspace `{}` for requested workspace `{workspace}`",
            outcome.workspace
        )));
    }
    if outcome.lifetime != lifetime {
        return Err(invalid_plan(format!(
            "workspace_release host returned lifetime `{}` for live workspace lifetime `{lifetime}`",
            outcome.lifetime
        )));
    }
    record_workspace_release_outcome(
        name,
        outcome,
        &requested_workspace,
        request_hash,
        context,
        state,
    )
}

fn is_workspace_id(value: &str) -> bool {
    value.strip_prefix("ws_").is_some_and(|suffix| {
        !suffix.is_empty()
            && suffix.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b':' | b'-')
            })
    })
}

fn record_lm_call_outcome(
    name: String,
    call_kind: &str,
    outcome: PlanLmCompleteOutcome,
    cache: Option<&str>,
    request_hash: &str,
    context: &PlanExecutionContext,
    state: &mut ExecutionState,
) -> Result<(), PublicSeamError> {
    outcomes::record_lm_call_outcome(
        name,
        call_kind,
        outcome,
        cache,
        request_hash,
        context,
        state,
    )
}

fn record_agent_call_outcome(
    name: String,
    outcome: PlanAgentRunOutcome,
    request_hash: &str,
    context: &PlanExecutionContext,
    state: &mut ExecutionState,
) -> Result<(), PublicSeamError> {
    outcomes::record_agent_call_outcome(name, outcome, request_hash, context, state)
}

fn record_sandbox_call_outcome(
    name: String,
    outcome: PlanSandboxExecOutcome,
    request_hash: &str,
    context: &PlanExecutionContext,
    state: &mut ExecutionState,
) -> Result<(), PublicSeamError> {
    outcomes::record_sandbox_call_outcome(name, outcome, request_hash, context, state)
}

fn record_workspace_materialize_outcome(
    name: String,
    outcome: PlanWorkspaceMaterializeOutcome,
    request_hash: &str,
    context: &PlanExecutionContext,
    state: &mut ExecutionState,
) -> Result<(), PublicSeamError> {
    outcomes::record_workspace_materialize_outcome(name, outcome, request_hash, context, state)
}

fn record_workspace_release_outcome(
    name: String,
    outcome: PlanWorkspaceReleaseOutcome,
    requested_workspace: &effects::WorkspaceRefFacts,
    request_hash: &str,
    context: &PlanExecutionContext,
    state: &mut ExecutionState,
) -> Result<(), PublicSeamError> {
    outcomes::record_workspace_release_outcome(
        name,
        outcome,
        requested_workspace,
        request_hash,
        context,
        state,
    )
}

fn record_failed_lm_call(
    failure: outcomes::FailedLmCall<'_>,
    state: &mut ExecutionState,
) -> Result<(), PublicSeamError> {
    outcomes::record_failed_lm_call(failure, state)
}

fn execute_write<H: PlanExecutionHost>(
    op_object: &Map<String, Value>,
    name: String,
    dep_values: &BTreeMap<String, Value>,
    context: &PlanExecutionContext,
    host: &mut H,
    state: &mut ExecutionState,
) -> Result<(), PublicSeamError> {
    outcomes::execute_write(op_object, name, dep_values, context, host, state)
}

fn execute_graph_query_expr(
    expr: &Value,
    name: &str,
    plan_document: &crate::PlanDocument,
    context: &PlanExecutionContext,
    host: &mut impl PlanExecutionHost,
) -> Result<EvaluatedExpr, PublicSeamError> {
    evaluate::execute_graph_query_expr(expr, name, plan_document, context, host)
}

fn execute_case_query_expr(
    expr: &Value,
    name: &str,
    context: &PlanExecutionContext,
    host: &mut impl PlanExecutionHost,
) -> Result<EvaluatedExpr, PublicSeamError> {
    evaluate::execute_case_query_expr(expr, name, context, host)
}

fn execute_workspace_query_expr(
    expr: &Value,
    name: &str,
    deps: &ResolvedDependencies,
    context: &PlanExecutionContext,
    host: &mut impl PlanExecutionHost,
) -> Result<EvaluatedExpr, PublicSeamError> {
    evaluate::execute_workspace_query_expr(expr, name, deps, context, host)
}

fn dependency_values(
    op: &Map<String, Value>,
    bindings: &BTreeMap<String, Value>,
) -> Result<BTreeMap<String, Value>, PublicSeamError> {
    evaluate::dependency_values(op, bindings)
}

fn graph_read_scope<'a>(
    plan_document: &'a crate::PlanDocument,
    context: &'a PlanExecutionContext,
) -> Result<PlanGraphReadScope<'a>, PublicSeamError> {
    match plan_document.consistency_kind() {
        "latest_at_start" => Ok(PlanGraphReadScope::LatestAtStart {
            revision: &context.base_revision,
        }),
        "at_revision" => {
            let revision = plan_document
                .at_revision()
                .ok_or_else(|| invalid_plan("at_revision plans must carry a graph revision"))?;
            Ok(PlanGraphReadScope::AtRevision { revision })
        }
        "since_revision" => {
            let since = plan_document
                .since_revision()
                .ok_or_else(|| invalid_plan("since_revision plans must carry a base revision"))?;
            Ok(PlanGraphReadScope::SinceRevision {
                since,
                until: plan_document.until_revision(),
            })
        }
        other => Err(invalid_plan(format!(
            "unknown Plan consistency mode `{other}`"
        ))),
    }
}

fn graph_read_scope_value(scope: PlanGraphReadScope<'_>) -> Value {
    match scope {
        PlanGraphReadScope::LatestAtStart { revision } => json!({
            "kind": "latest_at_start",
            "revision": revision
        }),
        PlanGraphReadScope::AtRevision { revision } => json!({
            "kind": "at_revision",
            "revision": revision
        }),
        PlanGraphReadScope::SinceRevision { since, until } => json!({
            "kind": "since_revision",
            "since": since,
            "until": until
        }),
    }
}

fn object<'a>(value: &'a Value, field: &str) -> Result<&'a Map<String, Value>, PublicSeamError> {
    value
        .as_object()
        .ok_or_else(|| invalid_plan(format!("{field} must be an object")))
}

fn nested_kind<'a>(value: &'a Value, field: &str) -> Result<&'a str, PublicSeamError> {
    value
        .as_object()
        .and_then(|object| object.get("kind"))
        .and_then(Value::as_str)
        .filter(|kind| !kind.trim().is_empty())
        .ok_or_else(|| invalid_plan(format!("{field} must carry kind")))
}

fn required_string<'a>(value: Option<&'a Value>, field: &str) -> Result<&'a str, PublicSeamError> {
    value
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| invalid_plan(format!("{field} must be a string")))
}

fn prefixed_jcs_hash(prefix: &str, value: &Value) -> Result<String, PublicSeamError> {
    let digest = jcs_canonicalize::sha256_jcs_hex(value)
        .map_err(|error| invalid_plan(format!("plan execution hash failed: {error}")))?;
    Ok(format!("{prefix}{digest}"))
}

fn invalid_plan(message: impl Into<String>) -> PublicSeamError {
    PublicSeamError::InvalidPlan {
        message: message.into(),
    }
}
