use std::collections::BTreeSet;

use serde_json::Value;

use crate::{CapabilityDocument, CapabilityGrantRequest, PublicSeamError};

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
        if call_kind == "lm_complete" {
            if let Some(purpose) = call.get("purpose").and_then(Value::as_str) {
                request = request.with_purpose(purpose);
            }
            if let Some(model_role) = call.get("model_role").and_then(Value::as_str) {
                request = request.with_model_role(model_role);
            }
        }
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
        "human_review" => Ok("human.review"),
        other => Err(invalid_authority(format!(
            "call kind `{other}` has no V1 capability action mapping"
        ))),
    }
}

fn required_string<'a>(value: Option<&'a Value>, field: &str) -> Result<&'a str, PublicSeamError> {
    value
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_authority(format!("call authority field `{field}` is required")))
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

fn invalid_authority(message: impl Into<String>) -> PublicSeamError {
    PublicSeamError::InvalidPlan {
        message: message.into(),
    }
}
