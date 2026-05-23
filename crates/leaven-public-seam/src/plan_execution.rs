use std::collections::{BTreeMap, BTreeSet};

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

/// Lowered graph-read consistency scope for a Plan IR `graph_query`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlanGraphReadScope<'a> {
    /// Read from the graph revision captured when plan execution started.
    LatestAtStart { revision: &'a str },
    /// Read from an explicitly pinned graph revision.
    AtRevision { revision: &'a str },
    /// Read a finite graph-event diff over the declared revision interval.
    SinceRevision {
        since: &'a str,
        until: Option<&'a str>,
    },
}

/// Lowered `graph_query` request passed to a [`PlanExecutionHost`].
#[derive(Clone, Copy, Debug)]
pub struct PlanGraphQueryRequest<'a> {
    name: &'a str,
    expr: &'a Value,
    scope: PlanGraphReadScope<'a>,
}

impl<'a> PlanGraphQueryRequest<'a> {
    /// Operation binding name.
    pub const fn name(&self) -> &'a str {
        self.name
    }

    /// Typed `graph_query` expression body from the Plan IR.
    pub const fn expr(&self) -> &'a Value {
        self.expr
    }

    /// Consistency-derived graph read scope.
    pub const fn scope(&self) -> PlanGraphReadScope<'a> {
        self.scope
    }
}

/// Host outcome for a typed `graph_query` read.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanGraphQueryOutcome {
    items: Vec<Value>,
    graph_revision: String,
    data_classes: Vec<String>,
    next_cursor: Option<String>,
}

impl PlanGraphQueryOutcome {
    /// Creates a graph-set outcome for a pure graph read.
    pub fn new(items: impl IntoIterator<Item = Value>, graph_revision: impl Into<String>) -> Self {
        Self {
            items: items.into_iter().collect(),
            graph_revision: graph_revision.into(),
            data_classes: vec!["public".to_owned()],
            next_cursor: None,
        }
    }

    /// Overrides the data classes carried by the graph-set value.
    #[must_use]
    pub fn with_data_classes(mut self, data_classes: impl IntoIterator<Item = String>) -> Self {
        self.data_classes = data_classes.into_iter().collect();
        self
    }

    /// Adds the next cursor returned by the graph read.
    #[must_use]
    pub fn with_next_cursor(mut self, next_cursor: impl Into<String>) -> Self {
        self.next_cursor = Some(next_cursor.into());
        self
    }
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
    error: Option<Value>,
    cost: Option<Value>,
}

impl PlanLmCompleteOutcome {
    /// Creates an LM response outcome.
    pub fn new(message: Value, runtime_fingerprint: impl Into<String>) -> Self {
        Self {
            message,
            data_classes: vec!["public".to_owned()],
            replayability: "fully_managed".to_owned(),
            runtime_fingerprint: runtime_fingerprint.into(),
            error: None,
            cost: None,
        }
    }

    /// Creates a failed paid LM outcome that still emits audit and charge receipts.
    pub fn failed_provider_error(
        message: impl Into<String>,
        runtime_fingerprint: impl Into<String>,
        usd_micro: u64,
    ) -> Self {
        Self {
            message: Value::Null,
            data_classes: Vec::new(),
            replayability: "has_declared_external_effects".to_owned(),
            runtime_fingerprint: runtime_fingerprint.into(),
            error: Some(json!({
                "code": "provider_error",
                "message": message.into(),
                "retryable": true
            })),
            cost: Some(json!({
                "usd_micro": usd_micro
            })),
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
    plan_document: &crate::PlanDocument,
    context: &PlanExecutionContext,
    host: &mut H,
) -> Result<Value, PublicSeamError> {
    match plan_document.mode_kind() {
        "execute" => execute_effects(plan, plan_document, context, host, EffectMode::Live),
        "dry_run" => dry_run_result(plan, context),
        "require_cached" => execute_effects(
            plan,
            plan_document,
            context,
            host,
            EffectMode::RequireCached,
        ),
        "replay" => replay_result(plan, context, host),
        other => Err(invalid_plan(format!(
            "unknown Plan execution mode `{other}`"
        ))),
    }
}

pub fn validate_plan_result_receipts(
    plan: &Value,
    plan_document: &crate::PlanDocument,
    context: &PlanExecutionContext,
    result: &Value,
) -> Result<(), PublicSeamError> {
    let plan_object = object(plan, "plan")?;
    let ops = plan_object
        .get("ops")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid_plan("plan ops must be an array"))?;
    let result_object = object(result, "plan result")?;
    let values = result_object
        .get("values")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid_plan("plan result values must be an object"))?;
    let receipts = result_object
        .get("receipts")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid_plan("plan result receipts must be an array"))?;
    let receipts_by_op = receipts_by_op_var(receipts)?;
    let mut bindings = BTreeMap::new();
    let mut seen_receipt_ops = BTreeSet::new();
    for op in ops {
        let name = required_string(object(op, "plan op")?.get("name"), "op.name")?;
        if receipts_by_op.contains_key(name) {
            seen_receipt_ops.insert(name.to_owned());
        }
        validate_op_receipt(
            op,
            plan_document,
            context,
            values,
            &receipts_by_op,
            &mut bindings,
        )?;
    }
    for op_var in receipts_by_op.keys() {
        if !seen_receipt_ops.contains(op_var) {
            return Err(invalid_plan(format!(
                "receipt claims operation `{op_var}` that is not present in the Plan IR"
            )));
        }
    }
    Ok(())
}

fn validate_op_receipt(
    op: &Value,
    plan_document: &crate::PlanDocument,
    context: &PlanExecutionContext,
    values: &Map<String, Value>,
    receipts_by_op: &BTreeMap<String, &Map<String, Value>>,
    bindings: &mut BTreeMap<String, Value>,
) -> Result<(), PublicSeamError> {
    let op_object = object(op, "plan op")?;
    let name = required_string(op_object.get("name"), "op.name")?;
    match required_string(op_object.get("kind"), "op.kind")? {
        "let" => validate_let_receipt(
            op_object,
            name,
            plan_document,
            context,
            values,
            receipts_by_op,
            bindings,
        ),
        "call" => validate_call_receipt(op_object, name, values, receipts_by_op, bindings),
        "write" => validate_write_receipt(op_object, name, context, receipts_by_op, bindings),
        other => Err(invalid_plan(format!(
            "unknown plan operation kind `{other}`"
        ))),
    }
}

fn validate_let_receipt(
    op_object: &Map<String, Value>,
    name: &str,
    plan_document: &crate::PlanDocument,
    context: &PlanExecutionContext,
    values: &Map<String, Value>,
    receipts_by_op: &BTreeMap<String, &Map<String, Value>>,
    bindings: &mut BTreeMap<String, Value>,
) -> Result<(), PublicSeamError> {
    let expr = op_object
        .get("expr")
        .ok_or_else(|| invalid_plan("let op must carry expr"))?;
    match nested_kind(expr, "expr")? {
        "literal" => {
            let value = object(expr, "literal expr")?
                .get("value")
                .cloned()
                .ok_or_else(|| invalid_plan("literal expr must carry value"))?;
            bindings.insert(name.to_owned(), value);
            Ok(())
        }
        "graph_query" => {
            let Some(receipt) = receipts_by_op.get(name) else {
                return Ok(());
            };
            require_receipt_field(receipt, "kind", "query")?;
            let value = values.get(name).ok_or_else(|| {
                invalid_plan(format!(
                    "query receipt for `{name}` must have a matching result value"
                ))
            })?;
            let scope = graph_read_scope(plan_document, context)?;
            let scope_value = graph_read_scope_value(scope);
            let projection = object(expr, "graph_query")?
                .get("projection")
                .ok_or_else(|| invalid_plan("graph_query must carry projection"))?;
            require_receipt_field(
                receipt,
                "op_hash",
                &prefixed_jcs_hash(
                    "fp_query_sha256_",
                    &json!({
                        "schema_version": "leaven.plan_query_op.v1",
                        "name": name,
                        "expr": expr,
                        "scope": scope_value
                    }),
                )?,
            )?;
            require_receipt_field(
                receipt,
                "read_scope_fingerprint",
                &prefixed_jcs_hash("fp_scope_sha256_", &scope_value)?,
            )?;
            require_receipt_field(
                receipt,
                "projection_fingerprint",
                &prefixed_jcs_hash("fp_projection_sha256_", projection)?,
            )?;
            bindings.insert(name.to_owned(), value.clone());
            Ok(())
        }
        other => Err(invalid_plan(format!(
            "representative Plan IR receipt verifier does not inspect `{other}` let expressions"
        ))),
    }
}

fn validate_call_receipt(
    op_object: &Map<String, Value>,
    name: &str,
    values: &Map<String, Value>,
    receipts_by_op: &BTreeMap<String, &Map<String, Value>>,
    bindings: &mut BTreeMap<String, Value>,
) -> Result<(), PublicSeamError> {
    let Some(receipt) = receipts_by_op.get(name) else {
        return Ok(());
    };
    let call = op_object
        .get("call")
        .ok_or_else(|| invalid_plan("call op must carry call"))?;
    let call_kind = nested_kind(call, "call")?;
    require_receipt_field(receipt, "kind", "call")?;
    require_receipt_field(receipt, "call_kind", call_kind)?;
    let deps = dependency_values(op_object, bindings)?;
    require_receipt_field(
        receipt,
        "request_hash",
        &prefixed_jcs_hash(
            "fp_request_sha256_",
            &json!({
                "schema_version": "leaven.plan_call_request.v1",
                "name": name,
                "kind": call_kind,
                "call": call,
                "deps": deps
            }),
        )?,
    )?;
    match required_string(receipt.get("status"), "receipt.status")? {
        "succeeded" => {
            let value = values.get(name).ok_or_else(|| {
                invalid_plan(format!(
                    "succeeded call receipt for `{name}` must have a matching result value"
                ))
            })?;
            bindings.insert(name.to_owned(), value.clone());
        }
        "failed" => {
            require_receipt_field(
                receipt,
                "result_hash",
                &prefixed_jcs_hash(
                    "fp_result_sha256_",
                    &json!({
                        "schema_version": "leaven.plan_call_result.v1",
                        "name": name,
                        "error": receipt.get("error"),
                        "cost": receipt.get("cost"),
                        "charge_receipts": receipt.get("charge_receipts").cloned().unwrap_or_else(|| json!([]))
                    }),
                )?,
            )?;
        }
        _ => {}
    }
    Ok(())
}

fn validate_write_receipt(
    op_object: &Map<String, Value>,
    name: &str,
    context: &PlanExecutionContext,
    receipts_by_op: &BTreeMap<String, &Map<String, Value>>,
    bindings: &mut BTreeMap<String, Value>,
) -> Result<(), PublicSeamError> {
    let Some(receipt) = receipts_by_op.get(name) else {
        return Ok(());
    };
    let write = op_object
        .get("write")
        .ok_or_else(|| invalid_plan("write op must carry write"))?;
    let write_kind = nested_kind(write, "write")?;
    require_receipt_field(receipt, "kind", "write")?;
    require_receipt_field(receipt, "write_kind", write_kind)?;
    require_receipt_field(receipt, "base_revision", &context.base_revision)?;
    let deps = dependency_values(op_object, bindings)?;
    require_receipt_field(
        receipt,
        "request_hash",
        &prefixed_jcs_hash(
            "fp_request_sha256_",
            &json!({
                "schema_version": "leaven.plan_write_request.v1",
                "name": name,
                "kind": write_kind,
                "write": write,
                "deps": deps,
                "base_revision": context.base_revision
            }),
        )?,
    )?;
    if write_kind == "emit_run_event" {
        require_receipt_field(
            receipt,
            "result_hash",
            &prefixed_jcs_hash(
                "fp_result_sha256_",
                &json!({
                    "schema_version": "leaven.plan_write_result.v1",
                    "name": name,
                    "event_id": required_string(receipt.get("event_id"), "receipt.event_id")?,
                    "committed_revision": receipt.get("committed_revision").cloned().unwrap_or(Value::Null)
                }),
            )?,
        )?;
        bindings.insert(
            name.to_owned(),
            json!({
                "kind": "emit_run_event",
                "event_id": required_string(receipt.get("event_id"), "receipt.event_id")?
            }),
        );
    }
    Ok(())
}

fn receipts_by_op_var(
    receipts: &[Value],
) -> Result<BTreeMap<String, &Map<String, Value>>, PublicSeamError> {
    let mut by_op = BTreeMap::new();
    for receipt in receipts {
        let receipt = object(receipt, "receipt")?;
        let op_var = required_string(receipt.get("op_var"), "receipt.op_var")?;
        if by_op.insert(op_var.to_owned(), receipt).is_some() {
            return Err(invalid_plan(format!(
                "multiple receipts claim operation `{op_var}`"
            )));
        }
    }
    Ok(by_op)
}

fn require_receipt_field(
    receipt: &Map<String, Value>,
    field: &str,
    expected: &str,
) -> Result<(), PublicSeamError> {
    let actual = required_string(receipt.get(field), field)?;
    if actual != expected {
        return Err(invalid_plan(format!(
            "receipt {field} for `{}` does not match Plan IR preimage",
            receipt
                .get("receipt")
                .and_then(Value::as_str)
                .unwrap_or("<unknown>")
        )));
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EffectMode {
    Live,
    RequireCached,
}

fn execute_effects<H: PlanExecutionHost>(
    plan: &Value,
    plan_document: &crate::PlanDocument,
    context: &PlanExecutionContext,
    host: &mut H,
    mode: EffectMode,
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

    for op in ops {
        execute_op(op, plan_document, context, host, &mut state, mode)?;
    }

    Ok(plan_result_value(&PlanResultValue {
        plan_id,
        context,
        final_revision: &state.final_revision,
        replayability_summary: replayability_summary(&state.values, &state.receipts),
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

fn replayability_summary(values: &Map<String, Value>, receipts: &[Value]) -> &'static str {
    let mut rank = receipts
        .iter()
        .filter_map(|receipt| {
            receipt
                .as_object()
                .and_then(|object| object.get("kind"))
                .and_then(Value::as_str)
        })
        .filter(|kind| *kind != "query")
        .map(|_| 1)
        .max()
        .unwrap_or(0);
    for value in values.values() {
        rank = rank.max(
            value
                .as_object()
                .and_then(|object| object.get("replayability"))
                .and_then(Value::as_str)
                .map(replayability_rank)
                .unwrap_or(0),
        );
    }
    match rank {
        0 => "pure_read",
        1 => "fully_managed",
        2 => "boundary_managed",
        3 => "has_declared_external_effects",
        _ => "has_untracked_external_effects",
    }
}

fn replayability_rank(replayability: &str) -> usize {
    match replayability {
        "pure_read" => 0,
        "fully_managed" => 1,
        "boundary_managed" => 2,
        "has_declared_external_effects" => 3,
        "has_untracked_external_effects" => 4,
        _ => 5,
    }
}

struct ExecutionState {
    bindings: BTreeMap<String, Value>,
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
    mode: EffectMode,
) -> Result<(), PublicSeamError> {
    let op_object = object(op, "plan op")?;
    let name = required_string(op_object.get("name"), "op.name")?.to_owned();
    let dep_values = dependency_values(op_object, &state.bindings)?;
    match required_string(op_object.get("kind"), "op.kind")? {
        "let" => execute_let(op_object, name, plan_document, context, host, state),
        "call" => execute_call(op_object, name, &dep_values, context, host, state, mode),
        "write" => execute_write(op_object, name, &dep_values, context, host, state),
        other => Err(invalid_plan(format!(
            "unknown plan operation kind `{other}`"
        ))),
    }
}

fn execute_let(
    op_object: &Map<String, Value>,
    name: String,
    plan_document: &crate::PlanDocument,
    context: &PlanExecutionContext,
    host: &mut impl PlanExecutionHost,
    state: &mut ExecutionState,
) -> Result<(), PublicSeamError> {
    let expr = op_object
        .get("expr")
        .ok_or_else(|| invalid_plan("let op must carry expr"))?;
    let evaluated = evaluate_expr(expr, &name, plan_document, context, host)?;
    if let Some(receipt) = evaluated.receipt {
        state.receipts.push(receipt);
    }
    if evaluated
        .value
        .as_object()
        .and_then(|object| object.get("kind"))
        .and_then(Value::as_str)
        == Some("graph_set")
    {
        state.values.insert(name.clone(), evaluated.value.clone());
    }
    state.bindings.insert(name, evaluated.value);
    Ok(())
}

fn execute_call<H: PlanExecutionHost>(
    op_object: &Map<String, Value>,
    name: String,
    dep_values: &BTreeMap<String, Value>,
    context: &PlanExecutionContext,
    host: &mut H,
    state: &mut ExecutionState,
    mode: EffectMode,
) -> Result<(), PublicSeamError> {
    let call = op_object
        .get("call")
        .ok_or_else(|| invalid_plan("call op must carry call"))?;
    let call_kind = nested_kind(call, "call")?;
    if call_kind != "lm_complete" {
        if mode == EffectMode::RequireCached {
            return Err(invalid_plan(format!(
                "require_cached mode cannot prove cached execution for `{call_kind}` calls"
            )));
        }
        return Err(invalid_plan(format!(
            "representative Plan IR harness does not execute `{call_kind}` calls"
        )));
    }
    let request = PlanLmCompleteRequest {
        name: &name,
        call,
        deps: dep_values,
    };
    let (outcome, cache) = match mode {
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

fn record_lm_call_outcome(
    name: String,
    call_kind: &str,
    outcome: PlanLmCompleteOutcome,
    cache: Option<&str>,
    request_hash: &str,
    context: &PlanExecutionContext,
    state: &mut ExecutionState,
) -> Result<(), PublicSeamError> {
    let receipt_id = format!("lmrec_{name}");
    if let Some(mut error) = outcome.error {
        error["receipt"] = json!(receipt_id);
        error["op"] = json!(name);
        return record_failed_lm_call(
            FailedLmCall {
                name: &name,
                call_kind,
                runtime_fingerprint: &outcome.runtime_fingerprint,
                cost: outcome.cost.as_ref(),
                error,
                request_hash,
                context,
            },
            state,
        );
    }
    let mut value = json!({
        "kind": "lm_response",
        "message": outcome.message,
        "graph_revision": context.base_revision,
        "data_classes": outcome.data_classes,
        "replayability": outcome.replayability,
        "receipt": receipt_id
    });
    if let Some(cache) = cache {
        value["cache"] = json!(cache);
    }
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

struct FailedLmCall<'a> {
    name: &'a str,
    call_kind: &'a str,
    runtime_fingerprint: &'a str,
    cost: Option<&'a Value>,
    error: Value,
    request_hash: &'a str,
    context: &'a PlanExecutionContext,
}

fn record_failed_lm_call(
    failure: FailedLmCall<'_>,
    state: &mut ExecutionState,
) -> Result<(), PublicSeamError> {
    let receipt_id = format!("lmrec_{}", failure.name);
    let charge_receipts = if let Some(cost) = failure.cost {
        let charge_id = format!("chargerec_{}", failure.name);
        state.charges.push(json!({
            "receipt": charge_id,
            "source_receipt": receipt_id,
            "cost": cost,
            "ledger_scope": "plan",
            "charged_at": failure.context.completed_at
        }));
        vec![charge_id]
    } else {
        Vec::new()
    };
    let result_hash = prefixed_jcs_hash(
        "fp_result_sha256_",
        &json!({
            "schema_version": "leaven.plan_call_result.v1",
            "name": failure.name,
            "error": failure.error,
            "cost": failure.cost,
            "charge_receipts": charge_receipts
        }),
    )?;
    state.receipts.push(json!({
        "kind": "call",
        "receipt": receipt_id,
        "op_var": failure.name,
        "started_at": failure.context.started_at,
        "completed_at": failure.context.completed_at,
        "call_kind": failure.call_kind,
        "request_hash": failure.request_hash,
        "result_hash": result_hash,
        "runtime_fingerprint": failure.runtime_fingerprint,
        "status": "failed",
        "error": failure.error,
        "cost": failure.cost,
        "charge_receipts": charge_receipts
    }));
    state.errors.push(failure.error);
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

struct EvaluatedExpr {
    value: Value,
    receipt: Option<Value>,
}

fn evaluate_expr(
    expr: &Value,
    name: &str,
    plan_document: &crate::PlanDocument,
    context: &PlanExecutionContext,
    host: &mut impl PlanExecutionHost,
) -> Result<EvaluatedExpr, PublicSeamError> {
    let object = object(expr, "expr")?;
    match required_string(object.get("kind"), "expr.kind")? {
        "literal" => Ok(EvaluatedExpr {
            value: object
                .get("value")
                .cloned()
                .ok_or_else(|| invalid_plan("literal expr must carry value"))?,
            receipt: None,
        }),
        "graph_query" => execute_graph_query_expr(expr, name, plan_document, context, host),
        other => Err(invalid_plan(format!(
            "representative Plan IR harness does not execute `{other}` let expressions"
        ))),
    }
}

fn execute_graph_query_expr(
    expr: &Value,
    name: &str,
    plan_document: &crate::PlanDocument,
    context: &PlanExecutionContext,
    host: &mut impl PlanExecutionHost,
) -> Result<EvaluatedExpr, PublicSeamError> {
    let scope = graph_read_scope(plan_document, context)?;
    let outcome = host.graph_query(PlanGraphQueryRequest { name, expr, scope })?;
    let receipt_id = format!("qrec_{name}");
    let mut value = json!({
        "kind": "graph_set",
        "items": outcome.items,
        "graph_revision": outcome.graph_revision,
        "data_classes": outcome.data_classes,
        "replayability": "pure_read",
        "receipt": receipt_id
    });
    if let Some(next_cursor) = outcome.next_cursor {
        value["next_cursor"] = json!(next_cursor);
    }
    let scope_value = graph_read_scope_value(scope);
    let projection = object(expr, "graph_query")?
        .get("projection")
        .ok_or_else(|| invalid_plan("graph_query must carry projection"))?;
    let op_hash = prefixed_jcs_hash(
        "fp_query_sha256_",
        &json!({
            "schema_version": "leaven.plan_query_op.v1",
            "name": name,
            "expr": expr,
            "scope": scope_value
        }),
    )?;
    let result_hash = prefixed_jcs_hash(
        "fp_result_sha256_",
        &json!({
            "schema_version": "leaven.plan_query_result.v1",
            "name": name,
            "value": value
        }),
    )?;
    let read_scope_fingerprint = prefixed_jcs_hash("fp_scope_sha256_", &scope_value)?;
    let projection_fingerprint = prefixed_jcs_hash("fp_projection_sha256_", projection)?;
    let graph_revision = required_string(value.get("graph_revision"), "graph_revision")?.to_owned();
    Ok(EvaluatedExpr {
        value,
        receipt: Some(json!({
            "kind": "query",
            "receipt": receipt_id,
            "op_var": name,
            "started_at": context.started_at,
            "completed_at": context.completed_at,
            "op_hash": op_hash,
            "result_hash": result_hash,
            "graph_revision": graph_revision,
            "read_scope_fingerprint": read_scope_fingerprint,
            "projection_fingerprint": projection_fingerprint,
            "status": "succeeded"
        })),
    })
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
