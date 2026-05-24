fn execute_agent_run_call<H: PlanExecutionHost>(
    host: &mut H,
    name: String,
    call: &Value,
    deps: &ResolvedDependencies,
    request_hash: &str,
    context: &PlanExecutionContext,
    state: &mut ExecutionState,
) -> Result<(), PublicSeamError> {
    validate_structured_output_contract(call, "agent_run")?;
    let request = PlanAgentRunRequest::new(&name, call, &deps.values, &deps.live_workspaces)?;
    request.live_workspace()?;
    let expected_runtime_fingerprint = request.runtime_fingerprint().map(str::to_owned);
    let outcome = host.agent_run(request)?;
    if let Some(expected_runtime_fingerprint) = expected_runtime_fingerprint
        && outcome.runtime_fingerprint != expected_runtime_fingerprint
    {
        return Err(invalid_plan(format!(
            "agent_run outcome runtime_fingerprint `{}` did not match requested `{expected_runtime_fingerprint}`",
            outcome.runtime_fingerprint
        )));
    }
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
    let request = PlanSandboxExecRequest::new(&name, call, &deps.values, &deps.live_workspaces)?;
    request.live_workspace()?;
    let expected_output_files = request
        .workspace_command()
        .output_files
        .iter()
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
    deps: &ResolvedDependencies,
    context: &PlanExecutionContext,
    host: &mut H,
    state: &mut ExecutionState,
) -> Result<(), PublicSeamError> {
    outcomes::execute_write(
        op_object,
        name,
        &deps.values,
        &deps.data_classes,
        context,
        host,
        state,
    )
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
