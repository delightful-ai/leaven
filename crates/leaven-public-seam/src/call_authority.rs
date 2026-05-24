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
