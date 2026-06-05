use leaven_public_seam::{
    CapabilityDocument, CapabilityGrantRequest, EvaluationJobDocument, LockedMethod,
    PlanCommitKind, PlanDocument, PlanEvaluationShape, PlanExecutionContext, PlanMode,
    PlanRequestEvaluationWrite, PlanWriteKind, PublicSeamError,
};
use serde_json::{Value, json};

pub(crate) fn extension_result_for_plan_report(
    method: LockedMethod,
    plan: &Value,
    result: &Value,
) -> Result<Value, PublicSeamError> {
    if method == LockedMethod::EventEmit {
        return event_emit_result_for_plan_report(method, result);
    }
    let values = result
        .get("values")
        .and_then(Value::as_object)
        .ok_or_else(|| PublicSeamError::InvalidPlan {
            message: "public seam method result missing values".to_owned(),
        })?;
    let primary_kind = method_primary_kind(method);
    let primary = plan
        .get("return")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .filter_map(|name| values.get(name))
        .find(|value| value.get("kind").and_then(Value::as_str) == Some(primary_kind))
        .or_else(|| {
            plan.get("return")
                .and_then(Value::as_array)
                .and_then(|returns| returns.first())
                .and_then(Value::as_str)
                .and_then(|name| values.get(name))
        })
        .ok_or_else(|| PublicSeamError::InvalidPlan {
            message: format!("public seam method result missing returned `{primary_kind}` value"),
        })?;
    let data_classes = primary
        .get("data_classes")
        .cloned()
        .unwrap_or_else(|| serde_json::json!([]));
    let receipts = acp_extension_receipts_for_plan_report(method, result, primary)?;
    Ok(serde_json::json!({
        "method": method.as_str(),
        "primary": primary,
        "receipts": receipts,
        "redactions": result.get("redactions").cloned().unwrap_or_else(|| serde_json::json!([])),
        "capability_fingerprint": result.get("capability_fingerprint").cloned().unwrap_or_else(|| serde_json::json!("fp_cap_sha256_missing")),
        "policy_fingerprint": result.get("policy_fingerprint").cloned().unwrap_or_else(|| serde_json::json!("fp_policy_sha256_missing")),
        "data_classes": data_classes
    }))
}

pub(crate) struct RequestEvaluationWriteSelection {
    pub(crate) name: String,
    pub(crate) write: PlanRequestEvaluationWrite,
}

pub(crate) fn single_request_evaluation_write(
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

pub(crate) fn authorize_evaluation_request_write(
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

pub(crate) fn evaluation_job_value_from_write(
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

pub(crate) fn evaluation_request_plan_result(
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

fn acp_extension_receipts_for_plan_report(
    method: LockedMethod,
    result: &Value,
    primary: &Value,
) -> Result<Value, PublicSeamError> {
    let Some(receipts) = result.get("receipts").cloned() else {
        return Ok(serde_json::json!([]));
    };
    if method != LockedMethod::EvaluationRequest {
        return Ok(receipts);
    }
    let mut receipts = receipts;
    let Some(receipt_items) = receipts.as_array_mut() else {
        return Ok(receipts);
    };
    for receipt in receipt_items {
        if receipt.get("write_kind").and_then(Value::as_str) == Some("request_evaluation") {
            let op_name = receipt
                .get("op_var")
                .and_then(Value::as_str)
                .unwrap_or("primary");
            receipt["result_hash"] = json!(acp_primary_result_hash(
                "leaven.plan_write_result.v1",
                op_name,
                primary
            )?);
        }
    }
    Ok(receipts)
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
    let receipt = result
        .get("receipts")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .find(|receipt| receipt.get("write_kind").and_then(Value::as_str) == Some("emit_run_event"))
        .ok_or_else(|| PublicSeamError::InvalidPlan {
            message: "public seam event.emit result missing emit_run_event receipt".to_owned(),
        })?;
    let primary = serde_json::json!({
        "kind": "emit_run_event",
        "event_id": receipt.get("event_id").cloned().unwrap_or_else(|| serde_json::json!("event_missing")),
        "receipt": receipt.get("receipt").cloned().unwrap_or_else(|| serde_json::json!("wrec_missing")),
        "data_classes": ["public"],
        "replayability": "fully_managed"
    });
    Ok(serde_json::json!({
        "method": method.as_str(),
        "primary": primary,
        "receipts": result.get("receipts").cloned().unwrap_or_else(|| serde_json::json!([])),
        "redactions": result.get("redactions").cloned().unwrap_or_else(|| serde_json::json!([])),
        "capability_fingerprint": result.get("capability_fingerprint").cloned().unwrap_or_else(|| serde_json::json!("fp_cap_sha256_missing")),
        "policy_fingerprint": result.get("policy_fingerprint").cloned().unwrap_or_else(|| serde_json::json!("fp_policy_sha256_missing")),
        "data_classes": ["public"]
    }))
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
