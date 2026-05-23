use std::collections::BTreeMap;

use serde_json::{Map, Value, json};

use crate::{PlanResultDocument, PublicSeamError};

/// Execution metadata for the advanced public-seam Plan IR harness.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanExecutionContext {
    capability_fingerprint: String,
    policy_fingerprint: String,
    base_revision: String,
    started_at: String,
    completed_at: String,
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
        }
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
    /// Executes a typed `lm_complete` call.
    fn lm_complete(
        &mut self,
        request: PlanLmCompleteRequest<'_>,
    ) -> Result<PlanLmCompleteOutcome, PublicSeamError>;

    /// Executes a typed `emit_run_event` write.
    fn emit_run_event(
        &mut self,
        request: PlanEmitRunEventRequest<'_>,
    ) -> Result<PlanEmitRunEventOutcome, PublicSeamError>;
}

/// Lowered `lm_complete` request passed to a [`PlanExecutionHost`].
#[derive(Clone, Copy, Debug)]
pub struct PlanLmCompleteRequest<'a> {
    name: &'a str,
    call: &'a Value,
    deps: &'a BTreeMap<String, Value>,
}

impl<'a> PlanLmCompleteRequest<'a> {
    /// Operation binding name.
    pub const fn name(&self) -> &'a str {
        self.name
    }

    /// Typed `lm_complete` call body from the Plan IR.
    pub const fn call(&self) -> &'a Value {
        self.call
    }

    /// Resolved dependency bindings visible to this call.
    pub const fn deps(&self) -> &'a BTreeMap<String, Value> {
        self.deps
    }
}

/// Host outcome for a typed `lm_complete` call.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanLmCompleteOutcome {
    message: Value,
    data_classes: Vec<String>,
    replayability: String,
    runtime_fingerprint: String,
}

impl PlanLmCompleteOutcome {
    /// Creates an LM response outcome.
    pub fn new(message: Value, runtime_fingerprint: impl Into<String>) -> Self {
        Self {
            message,
            data_classes: vec!["public".to_owned()],
            replayability: "fully_managed".to_owned(),
            runtime_fingerprint: runtime_fingerprint.into(),
        }
    }

    /// Overrides the data classes carried by the LM response value.
    #[must_use]
    pub fn with_data_classes(mut self, data_classes: impl IntoIterator<Item = String>) -> Self {
        self.data_classes = data_classes.into_iter().collect();
        self
    }

    /// Overrides the replayability classification carried by the LM response value.
    #[must_use]
    pub fn with_replayability(mut self, replayability: impl Into<String>) -> Self {
        self.replayability = replayability.into();
        self
    }
}

/// Lowered `emit_run_event` request passed to a [`PlanExecutionHost`].
#[derive(Clone, Copy, Debug)]
pub struct PlanEmitRunEventRequest<'a> {
    name: &'a str,
    write: &'a Value,
    deps: &'a BTreeMap<String, Value>,
    base_revision: &'a str,
}

impl<'a> PlanEmitRunEventRequest<'a> {
    /// Operation binding name.
    pub const fn name(&self) -> &'a str {
        self.name
    }

    /// Typed `emit_run_event` write body from the Plan IR.
    pub const fn write(&self) -> &'a Value {
        self.write
    }

    /// Resolved dependency bindings visible to this write.
    pub const fn deps(&self) -> &'a BTreeMap<String, Value> {
        self.deps
    }

    /// Base graph revision supplied by the public-seam execution context.
    pub const fn base_revision(&self) -> &'a str {
        self.base_revision
    }
}

/// Host outcome for a typed `emit_run_event` write.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanEmitRunEventOutcome {
    event_id: String,
    committed_revision: String,
}

impl PlanEmitRunEventOutcome {
    /// Creates an emitted event outcome.
    pub fn new(event_id: impl Into<String>, committed_revision: impl Into<String>) -> Self {
        Self {
            event_id: event_id.into(),
            committed_revision: committed_revision.into(),
        }
    }
}

pub fn execute_plan<H: PlanExecutionHost>(
    plan: &Value,
    context: &PlanExecutionContext,
    host: &mut H,
) -> Result<Value, PublicSeamError> {
    let plan_object = object(plan, "plan")?;
    let ops = plan_object
        .get("ops")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid_plan("plan ops must be an array"))?;
    let plan_id = required_string(plan_object.get("plan_id"), "plan_id")?;
    let mut state = ExecutionState::new(&context.base_revision);

    for op in ops {
        execute_op(op, context, host, &mut state)?;
    }

    Ok(json!({
        "schema_version": "leaven.plan_result.v1",
        "plan_id": plan_id,
        "capability_fingerprint": context.capability_fingerprint,
        "policy_fingerprint": context.policy_fingerprint,
        "base_revision": context.base_revision,
        "final_revision": state.final_revision,
        "replayability_summary": "fully_managed",
        "values": state.values,
        "receipts": state.receipts,
        "redactions": [],
        "charges": [],
        "errors": []
    }))
}

struct ExecutionState {
    bindings: BTreeMap<String, Value>,
    values: Map<String, Value>,
    receipts: Vec<Value>,
    final_revision: String,
}

impl ExecutionState {
    fn new(base_revision: &str) -> Self {
        Self {
            bindings: BTreeMap::new(),
            values: Map::new(),
            receipts: Vec::new(),
            final_revision: base_revision.to_owned(),
        }
    }
}

fn execute_op<H: PlanExecutionHost>(
    op: &Value,
    context: &PlanExecutionContext,
    host: &mut H,
    state: &mut ExecutionState,
) -> Result<(), PublicSeamError> {
    let op_object = object(op, "plan op")?;
    let name = required_string(op_object.get("name"), "op.name")?.to_owned();
    let dep_values = dependency_values(op_object, &state.bindings)?;
    match required_string(op_object.get("kind"), "op.kind")? {
        "let" => execute_let(op_object, name, state),
        "call" => execute_call(op_object, name, &dep_values, context, host, state),
        "write" => execute_write(op_object, name, &dep_values, context, host, state),
        other => Err(invalid_plan(format!(
            "unknown plan operation kind `{other}`"
        ))),
    }
}

fn execute_let(
    op_object: &Map<String, Value>,
    name: String,
    state: &mut ExecutionState,
) -> Result<(), PublicSeamError> {
    let expr = op_object
        .get("expr")
        .ok_or_else(|| invalid_plan("let op must carry expr"))?;
    let value = evaluate_expr(expr)?;
    state.bindings.insert(name, value);
    Ok(())
}

fn execute_call<H: PlanExecutionHost>(
    op_object: &Map<String, Value>,
    name: String,
    dep_values: &BTreeMap<String, Value>,
    context: &PlanExecutionContext,
    host: &mut H,
    state: &mut ExecutionState,
) -> Result<(), PublicSeamError> {
    let call = op_object
        .get("call")
        .ok_or_else(|| invalid_plan("call op must carry call"))?;
    let call_kind = nested_kind(call, "call")?;
    if call_kind != "lm_complete" {
        return Err(invalid_plan(format!(
            "representative Plan IR harness does not execute `{call_kind}` calls"
        )));
    }
    let outcome = host.lm_complete(PlanLmCompleteRequest {
        name: &name,
        call,
        deps: dep_values,
    })?;
    let receipt_id = format!("lmrec_{name}");
    let value = json!({
        "kind": "lm_response",
        "message": outcome.message,
        "graph_revision": context.base_revision,
        "data_classes": outcome.data_classes,
        "replayability": outcome.replayability,
        "receipt": receipt_id
    });
    let request_hash = prefixed_jcs_hash(
        "fp_request_sha256_",
        &json!({
            "schema_version": "leaven.plan_call_request.v1",
            "name": name,
            "kind": call_kind,
            "call": call,
            "deps": dep_values
        }),
    )?;
    let result_hash = prefixed_jcs_hash(
        "fp_result_sha256_",
        &json!({
            "schema_version": "leaven.plan_call_result.v1",
            "name": name,
            "value": value
        }),
    )?;
    state.receipts.push(json!({
        "kind": "call",
        "receipt": receipt_id,
        "op_var": name,
        "started_at": context.started_at,
        "completed_at": context.completed_at,
        "call_kind": call_kind,
        "request_hash": request_hash,
        "result_hash": result_hash,
        "runtime_fingerprint": outcome.runtime_fingerprint,
        "status": "succeeded"
    }));
    state.values.insert(name.clone(), value.clone());
    state.bindings.insert(name, value);
    Ok(())
}

fn execute_write<H: PlanExecutionHost>(
    op_object: &Map<String, Value>,
    name: String,
    dep_values: &BTreeMap<String, Value>,
    context: &PlanExecutionContext,
    host: &mut H,
    state: &mut ExecutionState,
) -> Result<(), PublicSeamError> {
    let write = op_object
        .get("write")
        .ok_or_else(|| invalid_plan("write op must carry write"))?;
    let write_kind = nested_kind(write, "write")?;
    if write_kind != "emit_run_event" {
        return Err(invalid_plan(format!(
            "representative Plan IR harness does not execute `{write_kind}` writes"
        )));
    }
    let outcome = host.emit_run_event(PlanEmitRunEventRequest {
        name: &name,
        write,
        deps: dep_values,
        base_revision: &context.base_revision,
    })?;
    let receipt_id = format!("wrec_{name}");
    let request_hash = prefixed_jcs_hash(
        "fp_request_sha256_",
        &json!({
            "schema_version": "leaven.plan_write_request.v1",
            "name": name,
            "kind": write_kind,
            "write": write,
            "deps": dep_values,
            "base_revision": context.base_revision
        }),
    )?;
    let result_hash = prefixed_jcs_hash(
        "fp_result_sha256_",
        &json!({
            "schema_version": "leaven.plan_write_result.v1",
            "name": name,
            "event_id": outcome.event_id,
            "committed_revision": outcome.committed_revision
        }),
    )?;
    state.receipts.push(json!({
        "kind": "write",
        "receipt": receipt_id,
        "op_var": name,
        "started_at": context.started_at,
        "completed_at": context.completed_at,
        "write_kind": write_kind,
        "request_hash": request_hash,
        "result_hash": result_hash,
        "base_revision": context.base_revision,
        "committed_revision": outcome.committed_revision,
        "status": "succeeded",
        "event_id": outcome.event_id
    }));
    state.final_revision.clone_from(&outcome.committed_revision);
    state.bindings.insert(
        name,
        json!({
            "kind": "emit_run_event",
            "event_id": outcome.event_id
        }),
    );
    Ok(())
}

fn dependency_values(
    op: &Map<String, Value>,
    bindings: &BTreeMap<String, Value>,
) -> Result<BTreeMap<String, Value>, PublicSeamError> {
    let mut deps = BTreeMap::new();
    let Some(raw) = op.get("deps") else {
        return Ok(deps);
    };
    let raw = raw
        .as_array()
        .ok_or_else(|| invalid_plan("op deps must be an array"))?;
    for dep in raw {
        let dep = dep
            .as_str()
            .ok_or_else(|| invalid_plan("op deps must be binding names"))?;
        let value = bindings
            .get(dep)
            .ok_or_else(|| invalid_plan(format!("op references unknown dependency `{dep}`")))?;
        deps.insert(dep.to_owned(), value.clone());
    }
    Ok(deps)
}

fn evaluate_expr(expr: &Value) -> Result<Value, PublicSeamError> {
    let object = object(expr, "expr")?;
    match required_string(object.get("kind"), "expr.kind")? {
        "literal" => object
            .get("value")
            .cloned()
            .ok_or_else(|| invalid_plan("literal expr must carry value")),
        other => Err(invalid_plan(format!(
            "representative Plan IR harness does not execute `{other}` let expressions"
        ))),
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
