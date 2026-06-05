use std::collections::BTreeMap;

use leaven_public_seam::{
    CapabilityDocument, CapabilityGrantRequest, EvaluationJobDocument, LockedMethod,
    PlanCommitKind, PlanDocument, PlanEvaluationShape, PlanExecutionContext, PlanMode,
    PlanRequestEvaluationWrite, PlanWriteKind, PublicSeamError,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub fn extension_result_for_plan_report(
    method: LockedMethod,
    plan: &Value,
    result: &Value,
) -> Result<Value, PublicSeamError> {
    if method == LockedMethod::EventEmit {
        return event_emit_result_for_plan_report(method, result);
    }
    let plan = ConfiguredPlanProjection::parse(plan)?;
    let result = ConfiguredPlanResultProjection::parse(result)?;
    let primary_kind = method_primary_kind(method);
    let primary = result.primary_from_return(plan.return_values(), primary_kind)?;
    let receipts = result.extension_receipts(method, &primary)?;
    let projection = ConfiguredExtensionResultProjection {
        method: method.as_str(),
        primary: &primary.value,
        receipts,
        redactions: result.redactions,
        capability_fingerprint: result.capability_fingerprint,
        policy_fingerprint: result.policy_fingerprint,
        data_classes: primary.data_classes,
    };
    serde_json::to_value(projection).map_err(|error| PublicSeamError::InvalidPlan {
        message: format!("configured extension projection failed: {error}"),
    })
}

pub struct RequestEvaluationWriteSelection {
    pub(crate) name: String,
    pub(crate) write: PlanRequestEvaluationWrite,
}

pub fn single_request_evaluation_write(
    plan: &PlanDocument,
) -> Result<RequestEvaluationWriteSelection, PublicSeamError> {
    if plan.mode() != PlanMode::Execute {
        return Err(PublicSeamError::InvalidPlan {
            message: "request_evaluation execution requires execute mode".to_owned(),
        });
    }
    if plan.commit() != PlanCommitKind::GraphWritesAtomic {
        return Err(PublicSeamError::InvalidPlan {
            message: "request_evaluation execution requires graph_writes_atomic commit".to_owned(),
        });
    }
    let mut found = None;
    for op in plan.operations() {
        if op.write_kind() == Some(PlanWriteKind::RequestEvaluation) {
            let write = op
                .write()
                .and_then(|write| write.request_evaluation())
                .ok_or_else(|| PublicSeamError::InvalidPlan {
                    message: "request_evaluation op must expose typed write".to_owned(),
                })?;
            let selected = RequestEvaluationWriteSelection {
                name: op.name().to_owned(),
                write: write.clone(),
            };
            if found.replace(selected).is_some() {
                return Err(PublicSeamError::InvalidPlan {
                    message: "configured service executes one request_evaluation write at a time"
                        .to_owned(),
                });
            }
        }
    }
    found.ok_or_else(|| PublicSeamError::InvalidPlan {
        message: "evaluation.request method must carry a request_evaluation write".to_owned(),
    })
}

pub fn authorize_evaluation_request_write(
    write: &PlanRequestEvaluationWrite,
    capability: &CapabilityDocument,
) -> Result<(), PublicSeamError> {
    let mut grant = CapabilityGrantRequest::for_action("evaluation.request")
        .with_resource("candidate_ids", json!(write.candidate_ids()));
    grant = grant.with_purpose(write.purpose());
    capability
        .authorize_grant(grant)
        .map_err(|denial| PublicSeamError::InvalidPlan {
            message: format!("evaluation request denied: {denial}"),
        })?;
    Ok(())
}

pub fn evaluation_job_value_from_write(
    write: &PlanRequestEvaluationWrite,
    context: &PlanExecutionContext,
) -> Result<Value, PublicSeamError> {
    let kind = evaluation_job_kind(write.shape(), write.candidate_ids())?;
    let set_name = write.set().named_set().unwrap_or("validation");
    let evaluator = write.evaluator().unwrap_or("eval_configured");
    Ok(json!({
        "schema_version": "leaven.evaluation_job.v1",
        "run": "run_demo",
        "stage_call_id": "sc_request_evaluation",
        "evaluation_request_id": "evalreq_configured",
        "evaluator_id": evaluator,
        "evaluator_fingerprint": "fp_eval_sha256_configured",
        "base_revision": context.base_revision(),
        "deadline_at": "2026-05-23T00:20:00Z",
        "kind": kind,
        "granularity": write.granularity(),
        "purpose": write.purpose(),
        "resolved_set": {
            "id": format!("rset_{}", sanitize_id_fragment(set_name)),
            "case_ids": ["case_1"],
            "case_count": 1,
            "case_set_version": "v1",
            "partition_summary": {
                set_name: 1
            }
        },
        "capability_fingerprint": context.capability_fingerprint()
    }))
}

pub fn evaluation_request_plan_result(
    plan: &PlanDocument,
    name: &str,
    context: &PlanExecutionContext,
    job: &EvaluationJobDocument,
) -> Result<Value, PublicSeamError> {
    let receipt = format!("wrec_{name}");
    let value = json!({
        "kind": "evaluation_request_receipt",
        "evaluation_request_id": job.request_id(),
        "status": "recorded",
        "graph_revision": job.base_revision(),
        "data_classes": ["public"],
        "replayability": "fully_managed",
        "receipt": receipt
    });
    Ok(json!({
        "schema_version": "leaven.plan_result.v1",
        "plan_id": plan.plan_id().as_str(),
        "capability_fingerprint": context.capability_fingerprint(),
        "policy_fingerprint": context.policy_fingerprint(),
        "base_revision": context.base_revision(),
        "final_revision": context.base_revision(),
        "replayability_summary": "fully_managed",
        "values": {
            name: value
        },
        "receipts": [{
            "kind": "write",
            "receipt": receipt,
            "op_var": name,
            "started_at": context.started_at(),
            "completed_at": context.completed_at(),
            "write_kind": "request_evaluation",
            "request_hash": job.request_hash()?,
            "result_hash": job.result_hash()?,
            "base_revision": context.base_revision(),
            "committed_revision": context.base_revision(),
            "status": "succeeded",
            "evaluation_request_id": job.request_id()
        }],
        "redactions": [],
        "charges": [],
        "errors": []
    }))
}

fn acp_primary_result_hash(
    schema_version: &str,
    op_name: &str,
    primary: &Value,
) -> Result<String, PublicSeamError> {
    let digest = jcs_canonicalize::sha256_jcs_hex(&json!({
        "schema_version": schema_version,
        "name": op_name,
        "value": primary
    }))
    .map_err(|error| PublicSeamError::InvalidPlan {
        message: format!("ACP extension result hash failed: {error}"),
    })?;
    Ok(format!("fp_result_sha256_{digest}"))
}

fn evaluation_job_kind(
    shape: PlanEvaluationShape,
    candidates: &[String],
) -> Result<Value, PublicSeamError> {
    match shape {
        PlanEvaluationShape::Independent => Ok(json!({
            "kind": "independent",
            "candidates": candidates
        })),
        PlanEvaluationShape::Listwise => Ok(json!({
            "kind": "listwise",
            "candidates": candidates
        })),
        PlanEvaluationShape::Pairwise => {
            if candidates.len() < 2 {
                return Err(PublicSeamError::InvalidPlan {
                    message: "pairwise evaluation request requires at least two candidates"
                        .to_owned(),
                });
            }
            let mut pairs = Vec::new();
            for left_index in 0..candidates.len() {
                for right in candidates.iter().skip(left_index + 1) {
                    pairs.push(json!({
                        "left": candidates[left_index],
                        "right": right
                    }));
                }
            }
            Ok(json!({
                "kind": "pairwise",
                "pairs": pairs
            }))
        }
    }
}

fn event_emit_result_for_plan_report(
    method: LockedMethod,
    result: &Value,
) -> Result<Value, PublicSeamError> {
    let result = ConfiguredPlanResultProjection::parse(result)?;
    let receipt = result.event_emit_receipt()?;
    let event_id = receipt.event_id()?;
    let primary = json!({
        "kind": "emit_run_event",
        "event_id": event_id,
        "receipt": receipt.receipt.as_str(),
        "data_classes": ["public"],
        "replayability": "fully_managed"
    });
    let projection = ConfiguredExtensionResultProjection {
        method: method.as_str(),
        primary: &primary,
        receipts: result.receipt_values()?,
        redactions: result.redactions,
        capability_fingerprint: result.capability_fingerprint,
        policy_fingerprint: result.policy_fingerprint,
        data_classes: vec!["public".to_owned()],
    };
    serde_json::to_value(projection).map_err(|error| PublicSeamError::InvalidPlan {
        message: format!("configured event.emit projection failed: {error}"),
    })
}

#[derive(Deserialize)]
struct ConfiguredPlanProjection {
    #[serde(default, rename = "return")]
    return_values: Vec<String>,
}

impl ConfiguredPlanProjection {
    fn parse(value: &Value) -> Result<Self, PublicSeamError> {
        serde_json::from_value(value.clone()).map_err(|error| PublicSeamError::InvalidPlan {
            message: format!("configured extension plan projection failed: {error}"),
        })
    }

    fn return_values(&self) -> &[String] {
        &self.return_values
    }
}

#[derive(Deserialize)]
struct ConfiguredPlanResultProjection {
    values: BTreeMap<String, Value>,
    #[serde(default)]
    receipts: Vec<ConfiguredReceiptProjection>,
    #[serde(default)]
    redactions: Vec<Value>,
    #[serde(default = "missing_capability_fingerprint")]
    capability_fingerprint: String,
    #[serde(default = "missing_policy_fingerprint")]
    policy_fingerprint: String,
}

impl ConfiguredPlanResultProjection {
    fn parse(value: &Value) -> Result<Self, PublicSeamError> {
        serde_json::from_value(value.clone()).map_err(|error| PublicSeamError::InvalidPlan {
            message: format!("configured extension result projection failed: {error}"),
        })
    }

    fn primary_from_return(
        &self,
        return_values: &[String],
        primary_kind: &'static str,
    ) -> Result<ConfiguredPrimaryProjection, PublicSeamError> {
        let mut first_returned_kind = None;
        for name in return_values {
            let Some((_, value)) = self
                .values
                .iter()
                .find(|(value_name, _)| value_name.as_str() == name.as_str())
            else {
                continue;
            };
            let primary = ConfiguredPrimaryProjection::parse(value)?;
            if primary.kind == primary_kind {
                return Ok(primary);
            }
            first_returned_kind.get_or_insert(primary.kind);
        }
        if let Some(kind) = first_returned_kind {
            return Err(PublicSeamError::InvalidPlan {
                message: format!(
                    "public seam method returned `{kind}` without required `{primary_kind}` value"
                ),
            });
        }
        Err(PublicSeamError::InvalidPlan {
            message: format!("public seam method result missing returned `{primary_kind}` value"),
        })
    }

    fn extension_receipts(
        &self,
        method: LockedMethod,
        primary: &ConfiguredPrimaryProjection,
    ) -> Result<Vec<Value>, PublicSeamError> {
        self.receipts
            .iter()
            .map(|receipt| receipt.extension_value(method, primary))
            .collect()
    }

    fn receipt_values(&self) -> Result<Vec<Value>, PublicSeamError> {
        self.receipts
            .iter()
            .map(ConfiguredReceiptProjection::to_value)
            .collect()
    }

    fn event_emit_receipt(&self) -> Result<&ConfiguredReceiptProjection, PublicSeamError> {
        self.receipts
            .iter()
            .find(|receipt| receipt.write_kind.as_deref() == Some("emit_run_event"))
            .ok_or_else(|| PublicSeamError::InvalidPlan {
                message: "public seam event.emit result missing emit_run_event receipt".to_owned(),
            })?
            .require_event_emit()
    }
}

struct ConfiguredPrimaryProjection {
    kind: String,
    data_classes: Vec<String>,
    value: Value,
}

impl ConfiguredPrimaryProjection {
    fn parse(value: &Value) -> Result<Self, PublicSeamError> {
        let facts: ConfiguredPrimaryFacts =
            serde_json::from_value(value.clone()).map_err(|error| {
                PublicSeamError::InvalidPlan {
                    message: format!("configured extension primary projection failed: {error}"),
                }
            })?;
        Ok(Self {
            kind: facts.kind,
            data_classes: facts.data_classes,
            value: value.clone(),
        })
    }
}

#[derive(Deserialize)]
struct ConfiguredPrimaryFacts {
    kind: String,
    #[serde(default)]
    data_classes: Vec<String>,
}

#[derive(Deserialize, Serialize)]
struct ConfiguredReceiptProjection {
    #[serde(skip_serializing_if = "Option::is_none")]
    kind: Option<String>,
    receipt: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    op_var: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    write_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    event_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result_hash: Option<String>,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

impl ConfiguredReceiptProjection {
    fn extension_value(
        &self,
        method: LockedMethod,
        primary: &ConfiguredPrimaryProjection,
    ) -> Result<Value, PublicSeamError> {
        let mut receipt = self.clone_for_projection();
        if method == LockedMethod::EvaluationRequest
            && receipt.write_kind.as_deref() == Some("request_evaluation")
        {
            let op_name = receipt.op_var.as_deref().unwrap_or("primary");
            receipt.result_hash = Some(acp_primary_result_hash(
                "leaven.plan_write_result.v1",
                op_name,
                &primary.value,
            )?);
        }
        receipt.to_value()
    }

    fn to_value(&self) -> Result<Value, PublicSeamError> {
        serde_json::to_value(self).map_err(|error| PublicSeamError::InvalidPlan {
            message: format!("configured extension receipt projection failed: {error}"),
        })
    }

    fn require_event_emit(&self) -> Result<&Self, PublicSeamError> {
        self.event_id()?;
        Ok(self)
    }

    fn event_id(&self) -> Result<&str, PublicSeamError> {
        self.event_id
            .as_deref()
            .ok_or_else(|| PublicSeamError::InvalidPlan {
                message: "public seam event.emit receipt missing event_id".to_owned(),
            })
    }

    fn clone_for_projection(&self) -> Self {
        Self {
            kind: self.kind.clone(),
            receipt: self.receipt.clone(),
            op_var: self.op_var.clone(),
            write_kind: self.write_kind.clone(),
            event_id: self.event_id.clone(),
            result_hash: self.result_hash.clone(),
            extra: self.extra.clone(),
        }
    }
}

#[derive(Serialize)]
struct ConfiguredExtensionResultProjection<'a> {
    method: &'static str,
    primary: &'a Value,
    receipts: Vec<Value>,
    redactions: Vec<Value>,
    capability_fingerprint: String,
    policy_fingerprint: String,
    data_classes: Vec<String>,
}

fn missing_capability_fingerprint() -> String {
    "fp_cap_sha256_missing".to_owned()
}

fn missing_policy_fingerprint() -> String {
    "fp_policy_sha256_missing".to_owned()
}

fn method_primary_kind(method: LockedMethod) -> &'static str {
    match method {
        LockedMethod::LmComplete => "lm_response",
        LockedMethod::AgentRun => "agent_session",
        LockedMethod::ProposalSubmitBatch => "proposal_batch_receipt",
        LockedMethod::ProposalApply => "apply_receipt",
        LockedMethod::AssessmentSubmit => "assessment_batch_receipt",
        LockedMethod::EvaluationRequest => "evaluation_request_receipt",
        LockedMethod::GraphQuery => "graph_set",
        LockedMethod::CaseLoad
        | LockedMethod::CaseInput
        | LockedMethod::CaseTarget
        | LockedMethod::CaseMetadata => "case_record",
        LockedMethod::WorkspaceMaterialize | LockedMethod::WorkspaceRelease => "workspace_handle",
        LockedMethod::WorkspaceSnapshot | LockedMethod::WorkspaceDigest => "workspace_snapshot",
        LockedMethod::WorkspaceList
        | LockedMethod::WorkspaceStat
        | LockedMethod::WorkspaceCaptureArtifacts => "workspace_listing",
        LockedMethod::WorkspaceReadFile => "workspace_file",
        LockedMethod::WorkspaceGitLog
        | LockedMethod::WorkspaceGitDiff
        | LockedMethod::WorkspaceGitStatus => "workspace_diff",
        LockedMethod::SandboxExec => "sandbox_exec",
        LockedMethod::EventEmit => "emit_run_event",
        LockedMethod::StageRun => "stage_run_text_output",
    }
}

fn sanitize_id_fragment(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use leaven_public_seam::LockedMethod;
    use serde_json::json;

    use super::extension_result_for_plan_report;

    #[test]
    fn configured_projection_rejects_wrong_returned_primary_kind() {
        let plan = json!({
            "return": ["primary"]
        });
        let result = json!({
            "values": {
                "primary": {
                    "kind": "agent_session",
                    "data_classes": ["public"]
                }
            },
            "receipts": [],
            "redactions": [],
            "capability_fingerprint": "fp_cap_sha256_configured",
            "policy_fingerprint": "fp_policy_sha256_configured"
        });

        let error = extension_result_for_plan_report(LockedMethod::LmComplete, &plan, &result)
            .expect_err("configured projection must reject wrong primary kind");

        assert!(
            error
                .to_string()
                .contains("without required `lm_response` value"),
            "{error}"
        );
    }

    #[test]
    fn configured_event_projection_requires_event_id_receipt_field() {
        let plan = json!({
            "return": ["event"]
        });
        let result = json!({
            "values": {},
            "receipts": [{
                "kind": "write",
                "receipt": "wrec_event",
                "write_kind": "emit_run_event",
                "op_var": "event",
                "started_at": "2026-06-05T00:00:00Z",
                "completed_at": "2026-06-05T00:00:01Z",
                "request_hash": "fp_request_sha256_event",
                "result_hash": "fp_result_sha256_event",
                "base_revision": "rev_base",
                "committed_revision": "rev_base",
                "status": "succeeded"
            }],
            "redactions": [],
            "capability_fingerprint": "fp_cap_sha256_configured",
            "policy_fingerprint": "fp_policy_sha256_configured"
        });

        let error = extension_result_for_plan_report(LockedMethod::EventEmit, &plan, &result)
            .expect_err("configured event projection must not synthesize event ids");

        assert!(error.to_string().contains("missing event_id"), "{error}");
    }
}
