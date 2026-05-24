use std::collections::BTreeSet;

use serde_json::{Value, json};

use crate::{
    CapabilityDocument, CapabilityGrantRequest, PublicSeamError,
    execution_authority::{
        invalid_authority, limit_usage, output_authority, required_string, workspace_ref_id,
    },
};

/// Semantic call-authority facts validated from a Plan IR document.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CallAuthorityReport {
    lm_calls: usize,
    agent_calls: usize,
    sandbox_calls: usize,
    human_review_calls: usize,
    checked_input_classes: BTreeSet<String>,
}

impl CallAuthorityReport {
    /// Number of `lm_complete` calls checked.
    pub const fn lm_calls(&self) -> usize {
        self.lm_calls
    }

    /// Number of `agent_run` calls checked.
    pub const fn agent_calls(&self) -> usize {
        self.agent_calls
    }

    /// Number of `sandbox_exec` calls checked.
    pub const fn sandbox_calls(&self) -> usize {
        self.sandbox_calls
    }

    /// Number of `human_review` calls checked.
    pub const fn human_review_calls(&self) -> usize {
        self.human_review_calls
    }

    /// Union of input data classes checked across calls.
    pub fn checked_input_classes(&self) -> Vec<&str> {
        self.checked_input_classes
            .iter()
            .map(String::as_str)
            .collect()
    }
}

pub fn validate(
    plan: &Value,
    capability: &CapabilityDocument,
) -> Result<CallAuthorityReport, PublicSeamError> {
    let mut report = CallAuthorityReport::default();
    let ops = plan
        .get("ops")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid_authority("plan ops must be an array"))?;
    for op in ops {
        let Some(call) = op.get("call").and_then(Value::as_object) else {
            continue;
        };
        let call_kind = required_string(call.get("kind"), "call.kind")?;
        let input_classes = string_set(call.get("input_classes"), "input_classes")?;
        let forbidden = string_set(
            call.get("forbidden_input_classes"),
            "forbidden_input_classes",
        )?;
        if let Some(data_class) = input_classes.intersection(&forbidden).next() {
            return Err(invalid_authority(format!(
                "call `{call_kind}` input class `{data_class}` intersects declared forbidden_input_classes"
            )));
        }
        if call_kind == "lm_complete"
            && capability.subject_stage_role() == Some("reflector")
            && input_classes.contains("case.target")
        {
            return Err(invalid_authority(
                "reflector lm_complete calls must not carry case.target input classes",
            ));
        }
        validate_execution_policy(capability, call_kind, call)?;
        let mut request = CapabilityGrantRequest::for_action(action_for_call(call_kind)?);
        for data_class in &input_classes {
            report.checked_input_classes.insert(data_class.clone());
            request = request.with_input_class(data_class.clone());
        }
        request = add_call_dimensions(request, call_kind, call)?;
        capability
            .authorize_grant(request)
            .map_err(|denial| invalid_authority(format!("call `{call_kind}` denied: {denial}")))?;
        match call_kind {
            "lm_complete" => report.lm_calls += 1,
            "agent_run" => report.agent_calls += 1,
            "sandbox_exec" => report.sandbox_calls += 1,
            "human_review" => report.human_review_calls += 1,
            _ => {}
        }
    }
    Ok(report)
}

pub fn validate_call_with_dependency_classes(
    call_kind: &str,
    call: &Value,
    deps: &std::collections::BTreeMap<String, Value>,
    capability: &CapabilityDocument,
) -> Result<(), PublicSeamError> {
    let call = call
        .as_object()
        .ok_or_else(|| invalid_authority("call must be an object"))?;
    let declares_input_classes = call.contains_key("input_classes");
    let declared_input_classes = string_set(call.get("input_classes"), "input_classes")?;
    let dependency_input_classes = dependency_data_classes(deps)?;
    if declares_input_classes {
        for data_class in &dependency_input_classes {
            if !declared_input_classes.contains(data_class) {
                return Err(invalid_authority(format!(
                    "call `{call_kind}` omitted dependency data class `{data_class}` from input_classes"
                )));
            }
        }
    }
    let effective_input_classes = if declares_input_classes {
        declared_input_classes
            .union(&dependency_input_classes)
            .cloned()
            .collect::<BTreeSet<_>>()
    } else {
        declared_input_classes
    };
    let forbidden = string_set(
        call.get("forbidden_input_classes"),
        "forbidden_input_classes",
    )?;
    if let Some(data_class) = effective_input_classes.intersection(&forbidden).next() {
        return Err(invalid_authority(format!(
            "call `{call_kind}` input class `{data_class}` intersects declared forbidden_input_classes"
        )));
    }
    if call_kind == "lm_complete"
        && capability.subject_stage_role() == Some("reflector")
        && effective_input_classes.contains("case.target")
    {
        return Err(invalid_authority(
            "reflector lm_complete calls must not carry case.target input classes",
        ));
    }
    validate_execution_policy(capability, call_kind, call)?;
    let mut request = CapabilityGrantRequest::for_action(action_for_call(call_kind)?);
    for data_class in &effective_input_classes {
        request = request.with_input_class(data_class.clone());
    }
    request = add_call_dimensions(request, call_kind, call)?;
    capability
        .authorize_grant(request)
        .map_err(|denial| invalid_authority(format!("call `{call_kind}` denied: {denial}")))?;
    Ok(())
}

fn dependency_data_classes(
    deps: &std::collections::BTreeMap<String, Value>,
) -> Result<BTreeSet<String>, PublicSeamError> {
    let mut classes = BTreeSet::new();
    for value in deps.values() {
        collect_data_classes(value, &mut classes)?;
    }
    Ok(classes)
}

fn collect_data_classes(
    value: &Value,
    classes: &mut BTreeSet<String>,
) -> Result<(), PublicSeamError> {
    let Some(object) = value.as_object() else {
        return Ok(());
    };
    if is_seam_data_class_carrier(object) {
        if let Some(data_classes) = object.get("data_classes") {
            classes.extend(string_set(Some(data_classes), "data_classes")?);
        }
    }
    for field in [
        "blob_ref",
        "transcript_ref",
        "stdout_ref",
        "stderr_ref",
        "payload_ref",
    ] {
        if let Some(value) = object.get(field) {
            collect_data_classes(value, classes)?;
        }
    }
    if let Some(values) = object.get("trace_refs").and_then(Value::as_array) {
        for value in values {
            collect_trace_ref_data_classes(value, classes)?;
        }
    }
    if let Some(files) = object.get("files").and_then(Value::as_object) {
        for value in files.values() {
            collect_data_classes(value, classes)?;
        }
    }
    Ok(())
}

fn is_seam_data_class_carrier(object: &serde_json::Map<String, Value>) -> bool {
    matches!(
        object.get("kind").and_then(Value::as_str),
        Some(
            "candidate_summary"
                | "proposal_summary"
                | "assessment_summary"
                | "event_summary"
                | "graph_set"
                | "case_record"
                | "workspace_handle"
                | "workspace_snapshot"
                | "workspace_file"
                | "workspace_diff"
                | "workspace_listing"
                | "lm_response"
                | "agent_session"
                | "sandbox_exec"
                | "proposal_batch_receipt"
                | "assessment_batch_receipt"
                | "evaluation_request_receipt"
                | "apply_receipt"
                | "blob_ref"
                | "text"
                | "json"
                | "structured"
        )
    ) && object.contains_key("data_classes")
}

fn collect_trace_ref_data_classes(
    value: &Value,
    classes: &mut BTreeSet<String>,
) -> Result<(), PublicSeamError> {
    let Some(object) = value.as_object() else {
        return Ok(());
    };
    if object.contains_key("visibility") {
        if let Some(data_classes) = object.get("data_classes") {
            classes.extend(string_set(Some(data_classes), "trace_refs.data_classes")?);
        }
    }
    Ok(())
}

fn validate_execution_policy(
    capability: &CapabilityDocument,
    call_kind: &str,
    call: &serde_json::Map<String, Value>,
) -> Result<(), PublicSeamError> {
    match call_kind {
        "agent_run" => validate_agent_execution_policy(capability, call),
        "sandbox_exec" => validate_sandbox_execution_policy(capability),
        _ => Ok(()),
    }
}

fn validate_agent_execution_policy(
    capability: &CapabilityDocument,
    call: &serde_json::Map<String, Value>,
) -> Result<(), PublicSeamError> {
    let Some(policy) = call.get("tool_policy").and_then(Value::as_object) else {
        return Ok(());
    };
    if policy
        .get("allow_shell")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        && capability.execution_policy_subprocess() != "allow"
    {
        return Err(invalid_authority(
            "agent_run allow_shell denied by execution_policy.subprocess",
        ));
    }
    if let Some(network) = policy.get("network").and_then(Value::as_str) {
        validate_network_request(capability.execution_policy_network(), network)?;
    }
    Ok(())
}

fn validate_sandbox_execution_policy(
    capability: &CapabilityDocument,
) -> Result<(), PublicSeamError> {
    if capability.execution_policy_subprocess() == "deny" {
        return Err(invalid_authority(
            "sandbox_exec denied by execution_policy.subprocess",
        ));
    }
    if capability.execution_policy_filesystem() != "workspace_handles_only" {
        return Err(invalid_authority(
            "sandbox_exec requires workspace_handles_only execution_policy.filesystem",
        ));
    }
    Ok(())
}

fn validate_network_request(allowed: &str, requested: &str) -> Result<(), PublicSeamError> {
    let allowed_rank = network_rank(allowed).ok_or_else(|| {
        invalid_authority(format!(
            "capability execution_policy.network `{allowed}` is not in the V1 policy lattice"
        ))
    })?;
    let requested_rank = network_rank(requested).ok_or_else(|| {
        invalid_authority(format!(
            "agent_run network policy `{requested}` is not in the V1 policy lattice"
        ))
    })?;
    if requested_rank <= allowed_rank {
        Ok(())
    } else {
        Err(invalid_authority(format!(
            "agent_run network policy `{requested}` exceeds execution_policy.network `{allowed}`"
        )))
    }
}

const fn network_rank(policy: &str) -> Option<u8> {
    match policy.as_bytes() {
        b"deny" => Some(0),
        b"leaven_endpoint_only" => Some(1),
        b"allowlist" => Some(2),
        b"unrestricted" => Some(3),
        _ => None,
    }
}

fn action_for_call(call_kind: &str) -> Result<&'static str, PublicSeamError> {
    match call_kind {
        "lm_complete" => Ok("lm.complete"),
        "agent_run" => Ok("agent.run"),
        "sandbox_exec" => Ok("sandbox.exec"),
        "workspace_materialize" => Ok("workspace.materialize"),
        "workspace_release" => Ok("workspace.release"),
        "human_review" => Ok("human.review"),
        other => Err(invalid_authority(format!(
            "call kind `{other}` has no V1 capability action mapping"
        ))),
    }
}

fn add_call_dimensions(
    request: CapabilityGrantRequest,
    call_kind: &str,
    call: &serde_json::Map<String, Value>,
) -> Result<CapabilityGrantRequest, PublicSeamError> {
    let mut request = output_authority(request, call.get("output"))?;
    match call_kind {
        "lm_complete" => {
            if let Some(purpose) = call.get("purpose").and_then(Value::as_str) {
                request = request.with_purpose(purpose);
            }
            if let Some(model) = call.get("model").and_then(Value::as_str) {
                request = request.with_model(model);
            }
            if let Some(model_role) = call.get("model_role").and_then(Value::as_str) {
                request = request.with_model_role(model_role);
            }
        }
        "agent_run" => {
            if let Some(workspace) = call.get("workspace") {
                request = request.with_resource(
                    "workspace_ids",
                    json!(workspace_ref_id(Some(workspace), "agent_run")?),
                );
            }
            if let Some(tool_policy) = call.get("tool_policy").and_then(Value::as_object)
                && let Some(commands) = tool_policy
                    .get("allowed_commands")
                    .and_then(Value::as_array)
            {
                for command in commands {
                    request = request.with_command(command.as_str().ok_or_else(|| {
                        invalid_authority("agent_run allowed_commands entries must be strings")
                    })?);
                }
            }
            if let Some(limits) = call.get("limits").and_then(Value::as_object) {
                request = request.with_limits(limit_usage(limits));
            }
        }
        "sandbox_exec" => {
            request = request
                .with_resource(
                    "workspace_ids",
                    json!(workspace_ref_id(call.get("workspace"), "sandbox_exec")?),
                )
                .with_workspace_op("exec");
            if let Some(command) = call
                .get("argv")
                .and_then(Value::as_array)
                .and_then(|argv| argv.first())
                .and_then(Value::as_str)
            {
                request = request.with_command(command);
            }
            request = request.with_limits(limit_usage(call));
        }
        "workspace_materialize" => {
            if let Some(candidate) = call.get("candidate") {
                request = request.with_resource("candidate_ids", candidate.clone());
            }
            request = request.with_workspace_op("materialize");
        }
        "workspace_release" => {
            request = request
                .with_resource(
                    "workspace_ids",
                    json!(workspace_ref_id(
                        call.get("workspace"),
                        "workspace_release"
                    )?),
                )
                .with_workspace_op("release");
        }
        _ => {}
    }
    Ok(request)
}

fn string_set(value: Option<&Value>, field: &str) -> Result<BTreeSet<String>, PublicSeamError> {
    value.map_or_else(
        || Ok(BTreeSet::new()),
        |value| {
            value
                .as_array()
                .ok_or_else(|| invalid_authority(format!("call field `{field}` must be an array")))?
                .iter()
                .map(|item| {
                    item.as_str().map(ToOwned::to_owned).ok_or_else(|| {
                        invalid_authority(format!("call field `{field}` entries must be strings"))
                    })
                })
                .collect()
        },
    )
}
