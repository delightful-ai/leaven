use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value, json};

use super::{
    ExecutionState, LiveWorkspaceHandle, PlanAgentRunOutcome, PlanApplyProposalBatchRequest,
    PlanEmitRunEventRequest, PlanExecutionContext, PlanExecutionHost, PlanLmCompleteOutcome,
    PlanSandboxExecOutcome, PlanSubmitAssessmentsRequest, PlanSubmitProposalBatchRequest,
    PlanWorkspaceMaterializeOutcome, PlanWorkspaceReleaseOutcome, effects, invalid_plan,
    nested_kind, prefixed_jcs_hash,
};
use crate::PublicSeamError;

pub(super) fn record_lm_call_outcome(
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
        return super::record_failed_lm_call(
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
    let cost = outcome
        .cost
        .ok_or_else(|| invalid_plan("lm_complete host outcome must carry cost"))?;
    let mut value = json!({
        "kind": "lm_response",
        "message": outcome.message,
        "graph_revision": context.base_revision,
        "data_classes": outcome.data_classes,
        "replayability": outcome.replayability,
        "receipt": receipt_id,
        "cost": cost
    });
    if let Some(cache) = cache {
        value["cache"] = json!(cache);
    }
    if let Some(parsed) = outcome.parsed {
        value["parsed"] = parsed;
    }
    let result_hash = prefixed_jcs_hash(
        "fp_result_sha256_",
        &json!({
            "schema_version": "leaven.plan_call_result.v1",
            "name": name,
            "value": value
        }),
    )?;
    let mut receipt = json!({
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
    });
    if let Some(cost) = value.get("cost") {
        receipt["cost"] = cost.clone();
    }
    state.receipts.push(receipt);
    state.values.insert(name.clone(), value.clone());
    state.bindings.insert(name, value);
    Ok(())
}

pub(super) fn record_agent_call_outcome(
    name: String,
    outcome: PlanAgentRunOutcome,
    request_hash: &str,
    context: &PlanExecutionContext,
    state: &mut ExecutionState,
) -> Result<(), PublicSeamError> {
    let receipt_id = format!("agentrec_{name}");
    let runtime_fingerprint = outcome.runtime_fingerprint.clone();
    let mut value = json!({
        "kind": "agent_session",
        "status": outcome.status,
        "graph_revision": context.base_revision,
        "data_classes": outcome.data_classes,
        "replayability": outcome.replayability,
        "receipt": receipt_id,
        "commands": outcome.commands
    });
    if let Some(parsed) = outcome.parsed {
        value["parsed"] = parsed;
    }
    if let Some(transcript_ref) = outcome.transcript_ref {
        value["transcript_ref"] = transcript_ref;
    }
    if let Some(cost) = outcome.cost {
        value["cost"] = cost;
    }
    record_successful_external_call(
        name,
        "agent_run",
        value,
        &runtime_fingerprint,
        request_hash,
        context,
        state,
    )
}

pub(super) fn record_sandbox_call_outcome(
    name: String,
    outcome: PlanSandboxExecOutcome,
    request_hash: &str,
    context: &PlanExecutionContext,
    state: &mut ExecutionState,
) -> Result<(), PublicSeamError> {
    let receipt_id = format!("execrec_{name}");
    let runtime_fingerprint = outcome.runtime_fingerprint.clone();
    let mut value = json!({
        "kind": "sandbox_exec",
        "status": outcome.status,
        "graph_revision": context.base_revision,
        "data_classes": outcome.data_classes,
        "replayability": outcome.replayability,
        "receipt": receipt_id,
        "files": outcome.files
    });
    if let Some(exit_code) = outcome.exit_code {
        value["exit_code"] = json!(exit_code);
    }
    if let Some(stdout_ref) = outcome.stdout_ref {
        value["stdout_ref"] = stdout_ref;
    }
    if let Some(stderr_ref) = outcome.stderr_ref {
        value["stderr_ref"] = stderr_ref;
    }
    if let Some(cost) = outcome.cost {
        value["cost"] = cost;
    }
    record_successful_external_call(
        name,
        "sandbox_exec",
        value,
        &runtime_fingerprint,
        request_hash,
        context,
        state,
    )
}

pub(super) fn record_workspace_materialize_outcome(
    name: String,
    outcome: PlanWorkspaceMaterializeOutcome,
    request_hash: &str,
    context: &PlanExecutionContext,
    state: &mut ExecutionState,
) -> Result<(), PublicSeamError> {
    let PlanWorkspaceMaterializeOutcome {
        workspace,
        workspace_ref,
        lifetime,
        data_classes,
        replayability,
        runtime_fingerprint,
    } = outcome;
    let receipt_id = format!("wrec_{name}");
    let workspace_facts =
        effects::workspace_ref_facts(Some(&workspace_ref), "workspace_materialize result")?;
    if workspace_facts.id() != workspace {
        return Err(invalid_plan(format!(
            "workspace_materialize result workspace ref `{}` does not match host workspace `{workspace}`",
            workspace_facts.id()
        )));
    }
    state.live_workspaces.insert(
        name.clone(),
        LiveWorkspaceHandle::live_ref(workspace_facts, lifetime.clone()),
    );
    let value = json!({
        "kind": "workspace_handle",
        "workspace": workspace_ref,
        "lifetime": lifetime,
        "released": false,
        "graph_revision": context.base_revision,
        "data_classes": data_classes,
        "replayability": replayability,
        "receipt": receipt_id
    });
    record_successful_external_call(
        name,
        "workspace_materialize",
        value,
        &runtime_fingerprint,
        request_hash,
        context,
        state,
    )
}

pub(super) fn record_workspace_release_outcome(
    name: String,
    outcome: PlanWorkspaceReleaseOutcome,
    requested_workspace: &effects::WorkspaceRefFacts,
    request_hash: &str,
    context: &PlanExecutionContext,
    state: &mut ExecutionState,
) -> Result<(), PublicSeamError> {
    let PlanWorkspaceReleaseOutcome {
        workspace,
        workspace_ref,
        lifetime,
        runtime_fingerprint,
    } = outcome;
    let workspace_facts =
        effects::workspace_ref_facts(Some(&workspace_ref), "workspace_release result")?;
    if workspace_facts.id() != workspace {
        return Err(invalid_plan(format!(
            "workspace_release result workspace ref `{}` does not match host workspace `{workspace}`",
            workspace_facts.id()
        )));
    }
    if !workspace_facts.satisfies_request(requested_workspace) {
        return Err(invalid_plan(format!(
            "workspace_release result workspace `{}` does not match requested workspace `{}`",
            workspace_facts.id(),
            requested_workspace.id()
        )));
    }
    let receipt_id = format!("wrec_{name}");
    for handle in state.live_workspaces.values_mut() {
        if handle.satisfies_workspace(requested_workspace) {
            handle.release();
        }
    }
    state.live_workspaces.insert(
        name.clone(),
        LiveWorkspaceHandle::released_ref(workspace_facts, lifetime.clone()),
    );
    let value = json!({
        "kind": "workspace_handle",
        "workspace": workspace_ref,
        "lifetime": lifetime,
        "released": true,
        "receipt": receipt_id,
        "graph_revision": context.base_revision,
        "data_classes": ["public"],
        "replayability": "boundary_managed"
    });
    record_successful_external_call(
        name,
        "workspace_release",
        value,
        &runtime_fingerprint,
        request_hash,
        context,
        state,
    )
}

fn record_successful_external_call(
    name: String,
    call_kind: &str,
    value: Value,
    runtime_fingerprint: &str,
    request_hash: &str,
    context: &PlanExecutionContext,
    state: &mut ExecutionState,
) -> Result<(), PublicSeamError> {
    let result_hash = prefixed_jcs_hash(
        "fp_result_sha256_",
        &json!({
            "schema_version": "leaven.plan_call_result.v1",
            "name": name,
            "value": value
        }),
    )?;
    let mut receipt = json!({
        "kind": "call",
        "receipt": value["receipt"],
        "op_var": name,
        "started_at": context.started_at,
        "completed_at": context.completed_at,
        "call_kind": call_kind,
        "request_hash": request_hash,
        "result_hash": result_hash,
        "runtime_fingerprint": runtime_fingerprint,
        "status": "succeeded"
    });
    if let Some(cost) = value.get("cost") {
        receipt["cost"] = cost.clone();
    }
    state.receipts.push(receipt);
    state.values.insert(name.clone(), value.clone());
    state.bindings.insert(name, value);
    Ok(())
}

pub(super) struct FailedLmCall<'a> {
    name: &'a str,
    call_kind: &'a str,
    runtime_fingerprint: &'a str,
    cost: Option<&'a Value>,
    error: Value,
    request_hash: &'a str,
    context: &'a PlanExecutionContext,
}

pub(super) fn record_failed_lm_call(
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

pub(super) fn execute_write<H: PlanExecutionHost>(
    op_object: &Map<String, Value>,
    name: String,
    dep_values: &BTreeMap<String, Value>,
    dependency_data_classes: &BTreeSet<String>,
    context: &PlanExecutionContext,
    host: &mut H,
    state: &mut ExecutionState,
) -> Result<(), PublicSeamError> {
    let write = op_object
        .get("write")
        .ok_or_else(|| invalid_plan("write op must carry write"))?;
    let write_kind = nested_kind(write, "write")?;
    if write_kind == "submit_proposal_batch" {
        return execute_submit_proposal_batch_write(
            name,
            write,
            dep_values,
            dependency_data_classes,
            context,
            host,
            state,
        );
    }
    if write_kind == "apply_proposal_batch" {
        return execute_apply_proposal_batch_write(
            name,
            write,
            dep_values,
            dependency_data_classes,
            context,
            host,
            state,
        );
    }
    if write_kind == "submit_assessments" {
        return execute_submit_assessments_write(name, write, context, host, state);
    }
    if write_kind != "emit_run_event" {
        return Err(invalid_plan(format!(
            "representative Plan IR harness does not execute `{write_kind}` writes"
        )));
    }
    let outcome = host.emit_run_event(PlanEmitRunEventRequest {
        name: &name,
        write,
        deps: dep_values,
        dependency_data_classes,
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
            "dependency_data_classes": dependency_data_classes,
            "base_revision": context.base_revision
        }),
    )?;
    let value = json!({
        "kind": "emit_run_event",
        "event_id": outcome.event_id,
        "receipt": receipt_id,
        "data_classes": ["public"],
        "replayability": "fully_managed"
    });
    let result_hash = prefixed_jcs_hash(
        "fp_result_sha256_",
        &json!({
            "schema_version": "leaven.plan_write_result.v1",
            "name": name,
            "value": value
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
    state.bindings.insert(name, value);
    Ok(())
}

fn execute_submit_proposal_batch_write<H: PlanExecutionHost>(
    name: String,
    write: &Value,
    dep_values: &BTreeMap<String, Value>,
    dependency_data_classes: &BTreeSet<String>,
    context: &PlanExecutionContext,
    host: &mut H,
    state: &mut ExecutionState,
) -> Result<(), PublicSeamError> {
    let write_kind = "submit_proposal_batch";
    let outcome = host.submit_proposal_batch(PlanSubmitProposalBatchRequest::new(
        &name,
        write,
        &context.base_revision,
    ))?;
    let receipt_id = format!("wrec_{name}");
    let request_hash = prefixed_jcs_hash(
        "fp_request_sha256_",
        &json!({
            "schema_version": "leaven.plan_write_request.v1",
            "name": name,
            "kind": write_kind,
            "write": write,
            "deps": dep_values,
            "dependency_data_classes": dependency_data_classes,
            "base_revision": context.base_revision
        }),
    )?;
    let value = json!({
        "kind": "proposal_batch_receipt",
        "batch_id": outcome.batch_id(),
        "proposal_ids": outcome.proposal_ids(),
        "status": "committed",
        "graph_revision": outcome.committed_revision(),
        "data_classes": outcome.data_classes(),
        "replayability": outcome.replayability(),
        "receipt": receipt_id
    });
    let result_hash = prefixed_jcs_hash(
        "fp_result_sha256_",
        &json!({
            "schema_version": "leaven.plan_write_result.v1",
            "name": name,
            "value": value
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
        "committed_revision": value["graph_revision"],
        "status": "succeeded",
        "proposal_batch_id": value["batch_id"],
        "proposal_ids": value["proposal_ids"]
    }));
    state.final_revision = outcome.committed_revision().to_owned();
    state.values.insert(name.clone(), value.clone());
    state.bindings.insert(name, value);
    Ok(())
}

fn execute_apply_proposal_batch_write<H: PlanExecutionHost>(
    name: String,
    write: &Value,
    dep_values: &BTreeMap<String, Value>,
    dependency_data_classes: &BTreeSet<String>,
    context: &PlanExecutionContext,
    host: &mut H,
    state: &mut ExecutionState,
) -> Result<(), PublicSeamError> {
    let write_kind = "apply_proposal_batch";
    let outcome = host.apply_proposal_batch(PlanApplyProposalBatchRequest::new(
        &name,
        write,
        &context.base_revision,
    ))?;
    let receipt_id = format!("wrec_{name}");
    let request_hash = prefixed_jcs_hash(
        "fp_request_sha256_",
        &json!({
            "schema_version": "leaven.plan_write_request.v1",
            "name": name,
            "kind": write_kind,
            "write": write,
            "deps": dep_values,
            "dependency_data_classes": dependency_data_classes,
            "base_revision": context.base_revision
        }),
    )?;
    let value = json!({
        "kind": "apply_receipt",
        "created_candidates": outcome.created_candidates(),
        "status": "committed",
        "graph_revision": outcome.committed_revision(),
        "data_classes": outcome.data_classes(),
        "replayability": outcome.replayability(),
        "receipt": receipt_id
    });
    let result_hash = prefixed_jcs_hash(
        "fp_result_sha256_",
        &json!({
            "schema_version": "leaven.plan_write_result.v1",
            "name": name,
            "value": value
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
        "committed_revision": value["graph_revision"],
        "status": "succeeded",
        "created_candidates": value["created_candidates"]
    }));
    state.final_revision = outcome.committed_revision().to_owned();
    state.values.insert(name.clone(), value.clone());
    state.bindings.insert(name, value);
    Ok(())
}

fn execute_submit_assessments_write<H: PlanExecutionHost>(
    name: String,
    write: &Value,
    context: &PlanExecutionContext,
    host: &mut H,
    state: &mut ExecutionState,
) -> Result<(), PublicSeamError> {
    let write_kind = "submit_assessments";
    let request = PlanSubmitAssessmentsRequest::new(&name, write, &context.base_revision);
    let evaluation_request_id = request.evaluation_request_id()?.to_owned();
    let outcome = host.submit_assessments(request)?;
    let receipt_id = format!("wrec_{name}");
    let request_hash = prefixed_jcs_hash(
        "fp_request_sha256_",
        &json!({
            "schema_version": "leaven.submit_assessments_request.v1",
            "evaluation_request_id": evaluation_request_id,
            "assessment_ids": outcome.assessment_ids()
        }),
    )?;
    let per_assessment = outcome
        .assessment_ids()
        .iter()
        .map(|assessment| {
            json!({
                "assessment": assessment,
                "replayability": outcome.replayability()
            })
        })
        .collect::<Vec<_>>();
    let value = json!({
        "kind": "assessment_batch_receipt",
        "assessment_ids": outcome.assessment_ids(),
        "evaluation_request_id": evaluation_request_id,
        "per_assessment": per_assessment,
        "status": "committed",
        "graph_revision": outcome.committed_revision(),
        "data_classes": outcome.data_classes(),
        "replayability": outcome.replayability(),
        "receipt": receipt_id
    });
    let result_hash = prefixed_jcs_hash(
        "fp_result_sha256_",
        &json!({
            "schema_version": "leaven.plan_write_result.v1",
            "name": name,
            "value": value
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
        "committed_revision": value["graph_revision"],
        "status": "succeeded",
        "evaluation_request_id": value["evaluation_request_id"],
        "assessment_ids": value["assessment_ids"]
    }));
    state.final_revision = outcome.committed_revision().to_owned();
    state.values.insert(name.clone(), value.clone());
    state.bindings.insert(name, value);
    Ok(())
}
