use serde_json::Value;

use crate::{CapabilityDocument, CapabilityGrantRequest, PublicSeamError};

/// Semantic proposal-authority facts validated from a Plan IR document.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ProposalAuthorityReport {
    submit_batches: usize,
    apply_writes: usize,
    create_effects: usize,
    change_effects: usize,
    workspace_diff_effects: usize,
    agent_session_effects: usize,
}

impl ProposalAuthorityReport {
    /// Number of `submit_proposal_batch` writes checked.
    pub const fn submit_batches(&self) -> usize {
        self.submit_batches
    }

    /// Number of `apply_proposal_batch` writes checked.
    pub const fn apply_writes(&self) -> usize {
        self.apply_writes
    }

    /// Number of `create` effects checked.
    pub const fn create_effects(&self) -> usize {
        self.create_effects
    }

    /// Number of `change` effects checked.
    pub const fn change_effects(&self) -> usize {
        self.change_effects
    }

    /// Number of `change_from_workspace_diff` effects checked.
    pub const fn workspace_diff_effects(&self) -> usize {
        self.workspace_diff_effects
    }

    /// Number of `change_from_agent_session` effects checked.
    pub const fn agent_session_effects(&self) -> usize {
        self.agent_session_effects
    }
}

pub fn validate(
    plan: &Value,
    capability: &CapabilityDocument,
) -> Result<ProposalAuthorityReport, PublicSeamError> {
    let mut report = ProposalAuthorityReport::default();
    let ops = plan
        .get("ops")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid_authority("plan ops must be an array"))?;
    for op in ops {
        let Some(write) = op.get("write").and_then(Value::as_object) else {
            continue;
        };
        match required_string(write.get("kind"), "write.kind")? {
            "submit_proposal_batch" => {
                report.submit_batches += 1;
                validate_submit_proposal_batch(write, capability, &mut report)?;
            }
            "apply_proposal_batch" => {
                report.apply_writes += 1;
                validate_apply_proposal_batch(capability)?;
            }
            _ => {}
        }
    }
    Ok(report)
}

fn validate_submit_proposal_batch(
    write: &serde_json::Map<String, Value>,
    capability: &CapabilityDocument,
    report: &mut ProposalAuthorityReport,
) -> Result<(), PublicSeamError> {
    let proposals = write
        .get("proposals")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid_authority("submit_proposal_batch must carry proposals"))?;
    for proposal in proposals {
        let effect = proposal
            .get("effect")
            .and_then(Value::as_object)
            .ok_or_else(|| invalid_authority("proposal must carry effect"))?;
        let effect_kind = required_string(effect.get("kind"), "effect.kind")?;
        ensure_allowed_effect(capability, effect_kind)?;
        let mut request = CapabilityGrantRequest::for_action("proposal.submit_batch");
        match effect_kind {
            "create" => {
                report.create_effects += 1;
                request = request.with_schema(required_string(
                    effect.get("artifact_schema"),
                    "effect.artifact_schema",
                )?);
            }
            "change" => {
                report.change_effects += 1;
                request = add_change_authority(request, effect)?;
            }
            "change_from_workspace_diff" => {
                report.workspace_diff_effects += 1;
                request = add_change_authority(request, effect)?;
            }
            "change_from_agent_session" => {
                report.agent_session_effects += 1;
                request = add_change_authority(request, effect)?;
            }
            other => {
                return Err(invalid_authority(format!(
                    "unknown proposal effect `{other}`"
                )));
            }
        }
        capability
            .authorize_grant(request)
            .map_err(|denial| invalid_authority(format!("proposal submit denied: {denial}")))?;
    }
    Ok(())
}

fn add_change_authority(
    request: CapabilityGrantRequest,
    effect: &serde_json::Map<String, Value>,
) -> Result<CapabilityGrantRequest, PublicSeamError> {
    Ok(request
        .with_surface(required_string(
            effect.get("surface_fingerprint"),
            "effect.surface_fingerprint",
        )?)
        .with_schema(required_string(
            effect.get("change_schema"),
            "effect.change_schema",
        )?))
}

fn validate_apply_proposal_batch(capability: &CapabilityDocument) -> Result<(), PublicSeamError> {
    let grant = capability
        .grant("proposal.apply_batch")
        .ok_or_else(|| invalid_authority("proposal apply requires proposal.apply_batch grant"))?;
    match grant.constraints.get("may_apply").and_then(Value::as_bool) {
        Some(false) => Err(invalid_authority(
            "proposal apply grant has may_apply=false",
        )),
        _ => capability
            .authorize_grant(CapabilityGrantRequest::for_action("proposal.apply_batch"))
            .map(|_| ())
            .map_err(|denial| invalid_authority(format!("proposal apply denied: {denial}"))),
    }
}

fn ensure_allowed_effect(
    capability: &CapabilityDocument,
    effect: &str,
) -> Result<(), PublicSeamError> {
    let grant = capability
        .grant("proposal.submit_batch")
        .ok_or_else(|| invalid_authority("proposal submit requires proposal.submit_batch grant"))?;
    let Some(effects) = grant.constraints.get("effects") else {
        return Ok(());
    };
    let effects = effects
        .as_array()
        .ok_or_else(|| invalid_authority("grant effects must be an array"))?;
    if effects.iter().any(|item| item.as_str() == Some(effect)) {
        Ok(())
    } else {
        Err(invalid_authority(format!(
            "proposal effect `{effect}` is outside grant effects"
        )))
    }
}

fn required_string<'a>(value: Option<&'a Value>, field: &str) -> Result<&'a str, PublicSeamError> {
    value
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_authority(format!("proposal authority field `{field}` is required")))
}

fn invalid_authority(message: impl Into<String>) -> PublicSeamError {
    PublicSeamError::InvalidPlan {
        message: message.into(),
    }
}
