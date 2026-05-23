use std::collections::BTreeMap;

use leaven_lm::{MessageContentPart, OutputMode, Role};
use leaven_public_seam::{
    CapabilityDocument, PlanCaseQueryOutcome, PlanCaseQueryRequest, PlanEmitRunEventOutcome,
    PlanEmitRunEventRequest, PlanExecutionContext, PlanExecutionHost, PlanGraphQueryOutcome,
    PlanGraphQueryRequest, PlanGraphReadScope, PlanLmCompleteOutcome, PlanLmCompleteRequest,
    PlanOperationKind, PublicSeamError, PublicSeamPackage,
};
use serde_json::{Value, json};

#[test]
fn plan_ir_family_accepts_typed_let_call_write_documents() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let plan = typed_let_call_write_plan();
    let document = package.validate_plan_document(&plan).unwrap();

    assert_eq!(
        document.operation_kinds(),
        &[
            PlanOperationKind::Let,
            PlanOperationKind::Call,
            PlanOperationKind::Write,
        ]
    );
    assert_eq!(document.return_names(), &["status"]);
    assert_eq!(document.consistency_kind(), "latest_at_start");
    assert_eq!(document.mode_kind(), "dry_run");
    assert_eq!(document.commit_kind(), "no_graph_writes");
}

#[test]
fn plan_ir_family_rejects_unknown_core_call_write_and_escape_hatch_ops() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();

    let mut unknown_core = typed_let_call_write_plan();
    unknown_core["ops"][0]["kind"] = json!("compute");
    assert!(matches!(
        package.validate_plan_document(&unknown_core).unwrap_err(),
        PublicSeamError::ExampleValidation { .. }
    ));

    let mut unknown_call = typed_let_call_write_plan();
    unknown_call["ops"][1]["call"]["kind"] = json!("provider_magic");
    assert!(matches!(
        package.validate_plan_document(&unknown_call).unwrap_err(),
        PublicSeamError::ExampleValidation { .. }
    ));

    let mut unknown_write = typed_let_call_write_plan();
    unknown_write["ops"][2]["write"]["kind"] = json!("mutate_graph_anyhow");
    assert!(matches!(
        package.validate_plan_document(&unknown_write).unwrap_err(),
        PublicSeamError::ExampleValidation { .. }
    ));

    let mut escape_hatch = typed_let_call_write_plan();
    escape_hatch["ops"][0] = json!({
        "kind": "extension",
        "namespace": "x.any",
        "op": "opaque.plan.node",
        "schema_fingerprint": "fp_schema_sha256_any",
        "payload": {
            "runtime_decides": true
        }
    });
    let error = package.validate_plan_document(&escape_hatch).unwrap_err();
    assert!(matches!(error, PublicSeamError::ExampleValidation { .. }));
}

#[test]
fn plan_ir_family_lowers_and_executes_let_call_write_through_public_seam_owner() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let mut plan = typed_let_call_write_plan();
    plan["mode"] = json!({"kind": "execute"});
    plan["commit"] = json!({
        "kind": "graph_writes_atomic",
        "on_stale": "reject"
    });
    let mut host = RecordingPlanHost::default();
    let context = plan_execution_context();

    let report = package
        .execute_plan_document(&plan, &context, &mut host)
        .unwrap();
    package
        .validate_plan_execution_result(&plan, &context, report.value())
        .unwrap();

    assert_eq!(host.calls, vec!["completion"]);
    assert_eq!(host.writes, vec!["status"]);
    assert_eq!(
        host.call_deps.get("prompt"),
        Some(&json!("Say ok")),
        "let binding must be lowered into the call host"
    );
    assert_eq!(
        host.write_deps.get("completion"),
        Some(&report.value()["values"]["completion"]),
        "call result must be lowered into the write host"
    );
    assert_eq!(report.document().receipt_kinds(), &["call", "write"]);
    assert_eq!(
        report.value()["values"]["completion"]["kind"].as_str(),
        Some("lm_response")
    );
    assert!(
        report.value()["receipts"][0]["request_hash"]
            .as_str()
            .unwrap()
            .starts_with("fp_request_sha256_")
    );
    assert_eq!(
        report.value()["receipts"][1]["write_kind"].as_str(),
        Some("emit_run_event")
    );
    assert_eq!(
        report.value()["final_revision"].as_str(),
        Some("rev_planexec_final")
    );
}

#[test]
fn plan_execution_result_rejects_receipt_hashes_unbound_from_plan_preimages() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let context = plan_execution_context();

    let mut query_host = RecordingPlanHost::default();
    let mut query_result = package
        .execute_plan_document(
            &latest_at_start_graph_query_plan(),
            &context,
            &mut query_host,
        )
        .unwrap()
        .value()
        .clone();
    query_result["receipts"][0]["op_hash"] =
        json!("fp_query_sha256_same_prefix_wrong_query_preimage");
    assert_plan_execution_result_rejected(
        &package,
        &latest_at_start_graph_query_plan(),
        &context,
        &query_result,
    );

    let mut plan = typed_let_call_write_plan();
    plan["mode"] = json!({"kind": "execute"});
    plan["commit"] = json!({
        "kind": "graph_writes_atomic",
        "on_stale": "reject"
    });
    let mut host = RecordingPlanHost::default();
    let result = package
        .execute_plan_document(&plan, &context, &mut host)
        .unwrap()
        .value()
        .clone();

    let mut wrong_call_request_hash = result.clone();
    wrong_call_request_hash["receipts"][0]["request_hash"] =
        json!("fp_request_sha256_same_prefix_wrong_call_preimage");
    assert_plan_execution_result_rejected(&package, &plan, &context, &wrong_call_request_hash);

    let mut missing_call_op_var = result.clone();
    missing_call_op_var["receipts"][0]
        .as_object_mut()
        .unwrap()
        .remove("op_var");
    assert_plan_execution_result_rejected(&package, &plan, &context, &missing_call_op_var);

    let mut wrong_write_request_hash = result.clone();
    wrong_write_request_hash["receipts"][1]["request_hash"] =
        json!("fp_request_sha256_same_prefix_wrong_write_preimage");
    assert_plan_execution_result_rejected(&package, &plan, &context, &wrong_write_request_hash);

    let mut wrong_write_result_hash = result.clone();
    wrong_write_result_hash["receipts"][1]["result_hash"] =
        json!("fp_result_sha256_same_prefix_wrong_write_result");
    assert_plan_execution_result_rejected(&package, &plan, &context, &wrong_write_result_hash);

    let mut tampered_plan = plan.clone();
    tampered_plan["ops"][1]["call"]["messages"][0]["content"][0]["text"] =
        json!("Say something else");
    assert_plan_execution_result_rejected(&package, &tampered_plan, &context, &result);
}

#[test]
fn plan_execution_result_rejects_missing_operation_receipts() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let context = plan_execution_context();

    let mut plan = typed_let_call_write_plan();
    plan["mode"] = json!({"kind": "execute"});
    plan["commit"] = json!({
        "kind": "graph_writes_atomic",
        "on_stale": "reject"
    });
    let result = package
        .execute_plan_document(&plan, &context, &mut RecordingPlanHost::default())
        .unwrap()
        .value()
        .clone();

    let mut missing_write_receipt = result;
    missing_write_receipt["receipts"]
        .as_array_mut()
        .unwrap()
        .remove(1);
    assert_plan_execution_result_rejected(&package, &plan, &context, &missing_write_receipt);

    let mut failed_host = RecordingPlanHost {
        fail_lm: true,
        ..RecordingPlanHost::default()
    };
    let failed_plan = execute_call_only_plan();
    let mut failed_result = package
        .execute_plan_document(&failed_plan, &context, &mut failed_host)
        .unwrap()
        .value()
        .clone();
    failed_result["receipts"] = json!([]);
    failed_result["charges"] = json!([]);
    failed_result["errors"] = json!([]);
    assert_plan_execution_result_rejected(&package, &failed_plan, &context, &failed_result);

    let mut missing_query_receipt = package
        .execute_plan_document(
            &latest_at_start_graph_query_plan(),
            &context,
            &mut RecordingPlanHost::default(),
        )
        .unwrap()
        .value()
        .clone();
    missing_query_receipt["values"] = json!({});
    missing_query_receipt["receipts"] = json!([]);
    assert_plan_execution_result_rejected(
        &package,
        &latest_at_start_graph_query_plan(),
        &context,
        &missing_query_receipt,
    );
}

#[test]
fn evaluator_target_reads_execute_case_query_load_with_query_receipts() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let plan = evaluator_target_case_query_plan();
    let context = evaluator_plan_execution_context();
    let mut host = RecordingPlanHost::default();

    let report = package
        .execute_plan_document_with_capability(
            &plan,
            &context,
            &evaluator_capability(&package),
            &mut host,
        )
        .unwrap();
    package
        .validate_plan_execution_result(&plan, &context, report.value())
        .unwrap();

    assert_eq!(host.case_reads, vec!["target:case_1"]);
    assert_eq!(
        report.value()["values"]["target"]["kind"].as_str(),
        Some("case_record")
    );
    assert_eq!(
        report.value()["values"]["target"]["target"],
        json!({"answer": "expected"})
    );
    assert_eq!(
        report.value()["values"]["target"]["data_classes"],
        json!(["case.target"])
    );
    assert_eq!(
        report.value()["receipts"][0]["kind"].as_str(),
        Some("query")
    );
    assert!(
        report.value()["receipts"][0]["op_hash"]
            .as_str()
            .unwrap()
            .starts_with("fp_query_sha256_")
    );
}

#[test]
fn evaluator_target_reads_reject_missing_or_unbound_case_query_receipts() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let plan = evaluator_target_case_query_plan();
    let context = evaluator_plan_execution_context();
    let result = package
        .execute_plan_document_with_capability(
            &plan,
            &context,
            &evaluator_capability(&package),
            &mut RecordingPlanHost::default(),
        )
        .unwrap()
        .value()
        .clone();

    let mut missing_receipt = result.clone();
    missing_receipt["receipts"] = json!([]);
    assert_plan_execution_or_result_rejected(&package, &plan, &context, &missing_receipt);

    let mut decorative_query_hash = result.clone();
    decorative_query_hash["receipts"][0]["op_hash"] =
        json!("fp_query_sha256_decorative_case_query");
    assert_plan_execution_or_result_rejected(&package, &plan, &context, &decorative_query_hash);

    let mut decorative_result_hash = result;
    decorative_result_hash["receipts"][0]["result_hash"] =
        json!("fp_result_sha256_decorative_case_query");
    assert_plan_execution_or_result_rejected(&package, &plan, &context, &decorative_result_hash);
}

#[test]
fn evaluator_target_reads_reject_unrequested_target_material() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let mut plan = evaluator_target_case_query_plan();
    plan["ops"][0]["expr"]["query"]["include"] = json!(["input"]);

    assert!(matches!(
        package
            .execute_plan_document_with_capability(
                &plan,
                &evaluator_plan_execution_context(),
                &evaluator_capability(&package),
                &mut RecordingPlanHost::default()
            )
            .unwrap_err(),
        PublicSeamError::InvalidPlan { .. }
    ));
}

#[test]
fn evaluator_target_reads_reject_missing_evaluator_capability_before_host_read() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let plan = evaluator_target_case_query_plan();

    let mut bare_host = RecordingPlanHost::default();
    assert!(matches!(
        package
            .execute_plan_document(&plan, &evaluator_plan_execution_context(), &mut bare_host)
            .unwrap_err(),
        PublicSeamError::InvalidPlan { .. }
    ));
    assert!(bare_host.case_reads.is_empty());

    let mut denied_host = RecordingPlanHost::default();
    assert!(matches!(
        package
            .execute_plan_document_with_capability(
                &plan,
                &evaluator_plan_execution_context(),
                &target_denied_evaluator_capability(&package),
                &mut denied_host
            )
            .unwrap_err(),
        PublicSeamError::InvalidPlan { .. }
    ));
    assert!(denied_host.case_reads.is_empty());

    let wrong_context =
        evaluator_plan_execution_context().with_evaluation_request("run_other", "evalreq_01");
    let mut wrong_run_host = RecordingPlanHost::default();
    assert!(matches!(
        package
            .execute_plan_document_with_capability(
                &plan,
                &wrong_context,
                &evaluator_capability(&package),
                &mut wrong_run_host
            )
            .unwrap_err(),
        PublicSeamError::InvalidPlan { .. }
    ));
    assert!(wrong_run_host.case_reads.is_empty());
}

fn assert_plan_execution_result_rejected(
    package: &PublicSeamPackage,
    plan: &Value,
    context: &PlanExecutionContext,
    result: &Value,
) {
    assert!(matches!(
        package
            .validate_plan_execution_result(plan, context, result)
            .unwrap_err(),
        PublicSeamError::InvalidPlan { .. }
    ));
}

fn assert_plan_execution_or_result_rejected(
    package: &PublicSeamPackage,
    plan: &Value,
    context: &PlanExecutionContext,
    result: &Value,
) {
    assert!(matches!(
        package
            .validate_plan_execution_result(plan, context, result)
            .unwrap_err(),
        PublicSeamError::InvalidPlan { .. } | PublicSeamError::InvalidPlanResult { .. }
    ));
}

#[test]
fn plan_ir_family_execution_rejects_dry_run_or_no_graph_write_fake_execution() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let mut host = RecordingPlanHost::default();

    let dry_run = typed_let_call_write_plan();
    let dry_run_report = package
        .execute_plan_document(&dry_run, &plan_execution_context(), &mut host)
        .unwrap();
    assert_eq!(dry_run_report.document().value_count(), 0);
    assert_eq!(dry_run_report.document().receipt_count(), 0);
    assert_eq!(
        dry_run_report.document().base_revision(),
        "rev_planexec_base"
    );
    assert_eq!(
        dry_run_report.document().final_revision(),
        "rev_planexec_base"
    );
    assert!(host.calls.is_empty());
    assert!(host.writes.is_empty());

    let mut no_graph_write = typed_let_call_write_plan();
    no_graph_write["mode"] = json!({"kind": "execute"});
    assert!(matches!(
        package
            .execute_plan_document(&no_graph_write, &plan_execution_context(), &mut host,)
            .unwrap_err(),
        PublicSeamError::InvalidPlan { .. }
    ));
    assert!(host.calls.is_empty());
    assert!(host.writes.is_empty());
}

#[test]
fn plan_execution_modes_require_cached_uses_cache_and_refuses_live_misses() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let mut miss = RecordingPlanHost::default();
    let plan = require_cached_call_plan();

    assert!(matches!(
        package
            .execute_plan_document(&plan, &plan_execution_context(), &mut miss)
            .unwrap_err(),
        PublicSeamError::InvalidPlan { .. }
    ));
    assert!(miss.calls.is_empty());
    assert_eq!(miss.cached_calls, vec!["completion"]);

    let mut hit = RecordingPlanHost {
        cached_hit: true,
        ..RecordingPlanHost::default()
    };
    let report = package
        .execute_plan_document(&plan, &plan_execution_context(), &mut hit)
        .unwrap();

    assert!(hit.calls.is_empty());
    assert_eq!(hit.cached_calls, vec!["completion"]);
    assert_eq!(report.document().receipt_kinds(), &["call"]);
    assert_eq!(
        report.value()["values"]["completion"]["cache"].as_str(),
        Some("hit")
    );
}

#[test]
fn plan_execution_modes_require_cached_rejects_agent_and_sandbox_live_work() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();

    for call in [agent_run_call(), sandbox_exec_call()] {
        let plan = require_cached_external_call_plan(call);
        let mut host = RecordingPlanHost::default();
        assert!(matches!(
            package
                .execute_plan_document(&plan, &plan_execution_context(), &mut host)
                .unwrap_err(),
            PublicSeamError::InvalidPlan { .. }
        ));
        assert!(host.calls.is_empty());
        assert!(host.cached_calls.is_empty());
        assert!(host.writes.is_empty());
    }
}

#[test]
fn plan_execution_modes_replay_uses_receipts_without_live_host_effects() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let mut plan = typed_let_call_write_plan();
    plan["mode"] = json!({
        "kind": "replay",
        "receipts": ["lmrec_completion", "wrec_status"]
    });
    let mut host = RecordingPlanHost::default();

    let report = package
        .execute_plan_document(&plan, &plan_execution_context(), &mut host)
        .unwrap();

    assert!(host.calls.is_empty());
    assert!(host.writes.is_empty());
    assert_eq!(
        host.replayed_receipts,
        vec!["lmrec_completion", "wrec_status"]
    );
    assert_eq!(report.document().value_count(), 0);
    assert_eq!(report.document().receipt_kinds(), &["call", "write"]);
    assert_eq!(report.document().final_revision(), "rev_planexec_final");
}

#[test]
fn plan_execution_produces_failed_paid_lm_call_and_charge_receipts() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let plan = execute_call_only_plan();
    let mut host = RecordingPlanHost {
        fail_lm: true,
        ..RecordingPlanHost::default()
    };

    let report = package
        .execute_plan_document(&plan, &plan_execution_context(), &mut host)
        .unwrap();

    assert_eq!(host.calls, vec!["completion"]);
    assert_eq!(report.document().value_count(), 0);
    assert_eq!(report.document().receipt_kinds(), &["call"]);
    assert_eq!(report.document().charge_count(), 1);
    assert_eq!(report.document().error_count(), 1);
    assert_eq!(
        report.value()["receipts"][0]["status"].as_str(),
        Some("failed")
    );
    assert_eq!(
        report.value()["receipts"][0]["charge_receipts"][0].as_str(),
        Some("chargerec_completion")
    );
    assert_eq!(
        report.value()["charges"][0]["source_receipt"].as_str(),
        Some("lmrec_completion")
    );
}

#[test]
fn plan_ir_family_execution_rejects_known_variants_outside_representative_harness() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let mut plan = typed_let_call_write_plan();
    plan["mode"] = json!({"kind": "execute"});
    plan["commit"] = json!({
        "kind": "graph_writes_atomic",
        "on_stale": "reject"
    });
    plan["ops"][1]["call"] = json!({
        "kind": "human_review",
        "queue": "qa",
        "prompt": "Review Say ok",
        "input_classes": ["public"]
    });
    let mut host = RecordingPlanHost::default();

    let error = package
        .execute_plan_document(&plan, &plan_execution_context(), &mut host)
        .unwrap_err();
    assert!(
        matches!(error, PublicSeamError::InvalidPlan { .. }),
        "unexpected error: {error:?}"
    );
    assert!(host.calls.is_empty());
    assert!(host.writes.is_empty());
}

#[test]
fn lm_complete_lowering_rejects_deferred_multimodal_or_extension_content() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let mut plan = typed_let_call_write_plan();
    plan["mode"] = json!({"kind": "execute"});
    plan["commit"] = json!({
        "kind": "graph_writes_atomic",
        "on_stale": "reject"
    });
    plan["ops"][1]["call"]["messages"][1]["content"] = json!([
        {
            "kind": "extension",
            "namespace": "leaven.media",
            "op": "image_input",
            "schema_fingerprint": "fp_schema_sha256_imageinput",
            "payload": {
                "image": "blob://image"
            }
        }
    ]);
    let mut host = RecordingPlanHost::default();

    let error = package
        .execute_plan_document(&plan, &plan_execution_context(), &mut host)
        .unwrap_err();

    assert!(
        error
            .to_string()
            .contains("supports text parts and tool_result tool messages only"),
        "unexpected error: {error:?}"
    );
    assert!(host.calls.is_empty());
    assert!(host.writes.is_empty());
}

#[test]
fn lm_complete_lowering_preserves_json_schema_output_and_provider_hints() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let mut plan = typed_let_call_write_plan();
    plan["mode"] = json!({"kind": "execute"});
    plan["commit"] = json!({
        "kind": "graph_writes_atomic",
        "on_stale": "reject"
    });
    plan["ops"][1]["call"]["output"] = json!({
        "kind": "json_schema",
        "schema_fingerprint": "fp_schema_sha256_lmanswer",
        "schema": {
            "type": "object",
            "properties": {
                "answer": {
                    "type": "string"
                }
            },
            "required": ["answer"],
            "additionalProperties": false
        }
    });
    let mut host = RecordingPlanHost::default();

    package
        .execute_plan_document(&plan, &plan_execution_context(), &mut host)
        .unwrap();

    assert_eq!(host.calls, vec!["completion"]);
    assert_eq!(host.writes, vec!["status"]);
}

#[test]
fn plan_ir_revision_modes_preserve_explicit_bases() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();

    let mut at_revision = typed_let_call_write_plan();
    at_revision["consistency"] = json!({
        "kind": "at_revision",
        "revision": "rev_pinned"
    });
    let document = package.validate_plan_document(&at_revision).unwrap();
    assert_eq!(document.consistency_kind(), "at_revision");
    assert_eq!(document.at_revision(), Some("rev_pinned"));

    let since_revision = package
        .validate_plan_document(&since_revision_event_diff_plan())
        .unwrap();
    assert_eq!(since_revision.consistency_kind(), "since_revision");
    assert_eq!(since_revision.since_revision(), Some("rev_base"));
    assert_eq!(since_revision.until_revision(), Some("rev_tip"));
    assert_eq!(since_revision.events_since_revision_queries(), 1);
}

#[test]
fn plan_revision_modes_reject_since_revision_fallback_to_latest() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();

    let mut mismatched_source = since_revision_event_diff_plan();
    mismatched_source["ops"][0]["expr"]["source"]["since_revision"] = json!("rev_other");
    assert!(matches!(
        package
            .validate_plan_document(&mismatched_source)
            .unwrap_err(),
        PublicSeamError::InvalidPlan { .. }
    ));

    let mut missing_source_base = since_revision_event_diff_plan();
    missing_source_base["ops"][0]["expr"]["source"]
        .as_object_mut()
        .unwrap()
        .remove("since_revision");
    assert!(matches!(
        package
            .validate_plan_document(&missing_source_base)
            .unwrap_err(),
        PublicSeamError::InvalidPlan { .. }
    ));
}

#[test]
fn plan_revision_modes_execute_graph_queries_at_declared_scope() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();

    let mut latest_host = RecordingPlanHost::default();
    let latest_report = package
        .execute_plan_document(
            &latest_at_start_graph_query_plan(),
            &plan_execution_context(),
            &mut latest_host,
        )
        .unwrap();
    assert_eq!(
        latest_host.graph_reads,
        vec!["latest_at_start:rev_planexec_base"]
    );
    assert_eq!(
        latest_report.value()["values"]["events"]["graph_revision"].as_str(),
        Some("rev_planexec_base")
    );
    assert_eq!(latest_report.document().receipt_kinds(), &["query"]);
    assert_eq!(
        latest_report.value()["values"]["events"]["receipt"].as_str(),
        Some("qrec_events")
    );
    assert_eq!(
        latest_report.document().final_revision(),
        "rev_planexec_base"
    );

    let mut at_host = RecordingPlanHost::default();
    let at_report = package
        .execute_plan_document(
            &at_revision_graph_query_plan(),
            &plan_execution_context(),
            &mut at_host,
        )
        .unwrap();
    assert_eq!(at_host.graph_reads, vec!["at_revision:rev_pinned"]);
    assert_eq!(
        at_report.value()["values"]["events"]["graph_revision"].as_str(),
        Some("rev_pinned")
    );
    assert_eq!(at_report.document().receipt_kinds(), &["query"]);
    assert_eq!(at_report.document().final_revision(), "rev_planexec_base");

    let mut since_host = RecordingPlanHost::default();
    let mut since_plan = since_revision_event_diff_plan();
    since_plan["mode"] = json!({"kind": "execute"});
    let since_report = package
        .execute_plan_document(&since_plan, &plan_execution_context(), &mut since_host)
        .unwrap();
    assert_eq!(
        since_host.graph_reads,
        vec!["since_revision:rev_base..rev_tip"]
    );
    assert_eq!(
        since_report.value()["values"]["events"]["items"][0]["revision"].as_str(),
        Some("rev_tip")
    );
    assert_eq!(since_report.document().receipt_kinds(), &["query"]);
    assert_eq!(
        since_report.document().final_revision(),
        "rev_planexec_base"
    );
}

#[test]
fn submit_assessments_score_outputs_cover_all_assessment_shapes() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();

    let document = package
        .validate_plan_document(&submit_assessments_plan())
        .unwrap();

    assert_eq!(document.assessment_score_output_count(), 3);
    assert_eq!(document.independent_assessment_score_output_count(), 1);
    assert_eq!(document.pairwise_assessment_score_output_count(), 1);
    assert_eq!(document.listwise_assessment_score_output_count(), 1);
}

#[test]
fn submit_assessments_accepts_candidate_artifact_score_output_class() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let mut document = submit_assessments_plan();
    document["ops"][0]["write"]["assessments"][0]["score"]["output"]["data_classes"] =
        json!(["candidate.artifact", "public"]);

    let document = package.validate_plan_document(&document).unwrap();

    assert_eq!(document.assessment_score_output_count(), 3);
}

#[test]
fn submit_assessments_rejects_missing_or_placeholder_score_output() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();

    let mut missing_output = submit_assessments_plan();
    missing_output["ops"][0]["write"]["assessments"][0]["score"]
        .as_object_mut()
        .unwrap()
        .remove("output");
    assert!(matches!(
        package.validate_plan_document(&missing_output).unwrap_err(),
        PublicSeamError::ExampleValidation { .. }
    ));

    let mut blank_text = submit_assessments_plan();
    blank_text["ops"][0]["write"]["assessments"][0]["score"]["output"] = json!({
        "kind": "text",
        "summary": "   ",
        "visibility": "public",
        "data_classes": ["public"]
    });
    assert!(matches!(
        package.validate_plan_document(&blank_text).unwrap_err(),
        PublicSeamError::InvalidPlan { .. }
    ));

    let mut null_json = submit_assessments_plan();
    null_json["ops"][0]["write"]["assessments"][1]["score"]["output"] = json!({
        "kind": "json",
        "value": null,
        "visibility": "public",
        "data_classes": ["public"]
    });
    assert!(matches!(
        package.validate_plan_document(&null_json).unwrap_err(),
        PublicSeamError::InvalidPlan { .. }
    ));

    let mut non_candidate_dummy = submit_assessments_plan();
    non_candidate_dummy["ops"][0]["write"]["assessments"][0]["score"]["output"] = json!({
        "kind": "text",
        "summary": "dummy output only present to satisfy schema",
        "value": "dummy output only present to satisfy schema",
        "visibility": "public",
        "data_classes": ["public"]
    });
    assert!(matches!(
        package
            .validate_plan_document(&non_candidate_dummy)
            .unwrap_err(),
        PublicSeamError::InvalidPlan { .. }
    ));
}

#[test]
fn submit_assessments_rejects_missing_assessment_score_or_replayability() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();

    let mut missing_score = submit_assessments_plan();
    missing_score["ops"][0]["write"]["assessments"][0]
        .as_object_mut()
        .unwrap()
        .remove("score");
    assert!(matches!(
        package.validate_plan_document(&missing_score).unwrap_err(),
        PublicSeamError::ExampleValidation { .. }
    ));

    let mut missing_replayability = submit_assessments_plan();
    missing_replayability["ops"][0]["write"]["assessments"][1]
        .as_object_mut()
        .unwrap()
        .remove("replayability");
    assert!(matches!(
        package
            .validate_plan_document(&missing_replayability)
            .unwrap_err(),
        PublicSeamError::ExampleValidation { .. }
    ));
}

fn typed_let_call_write_plan() -> Value {
    json!({
        "schema_version": "leaven.plan.v1",
        "plan_id": "plankind001",
        "consistency": {
            "kind": "latest_at_start"
        },
        "mode": {
            "kind": "dry_run"
        },
        "ops": [
            {
                "kind": "let",
                "name": "prompt",
                "expr": {
                    "kind": "literal",
                    "value": "Say ok",
                    "data_classes": ["public"]
                }
            },
            {
                "kind": "call",
                "name": "completion",
                "deps": ["prompt"],
                "idempotency_key": "plan-call-0001",
                "call": lm_complete_call()
            },
            {
                "kind": "write",
                "name": "status",
                "deps": ["completion"],
                "idempotency_key": "plan-write-0001",
                "write": {
                    "kind": "emit_run_event",
                    "event_kind": "plan.ir.checked",
                    "payload_schema": "fp_schema_sha256_planir",
                    "payload": {
                        "ok": true
                    },
                    "visibility": "public"
                }
            }
        ],
        "return": ["status"],
        "commit": {
            "kind": "no_graph_writes"
        }
    })
}

fn lm_complete_call() -> Value {
    json!({
        "kind": "lm_complete",
        "purpose": "test.plan_ir",
        "model": "gpt-4.1-mini",
        "model_role": "reflector",
        "messages": [
            {
                "role": "developer",
                "content": [
                    {
                        "kind": "text",
                        "text": "Return only the final answer"
                    }
                ]
            },
            {
                "role": "user",
                "content": [
                    {
                        "kind": "text",
                        "text": "Say ok"
                    }
                ]
            },
            {
                "role": "tool",
                "tool_call_id": "call_lookup_1",
                "content": [
                    {
                        "kind": "tool_result",
                        "tool_call_id": "call_lookup_1",
                        "content": "{\"hint\":\"ok\"}"
                    }
                ]
            }
        ],
        "tools": [
            {
                "name": "lookup",
                "description": "look up case facts",
                "input_schema": {
                    "type": "object"
                },
                "requires_capability_action": "case.read"
            }
        ],
        "sampling": {
            "temperature": 0.2,
            "top_p": 0.9,
            "max_output_tokens": 128,
            "seed": 7,
            "stop": ["DONE"]
        },
        "output": {
            "kind": "final_message",
            "max_bytes": 1024
        },
        "provider_hints": {
            "cache:key": "planexec-stable"
        },
        "input_classes": ["public"]
    })
}

fn require_cached_call_plan() -> Value {
    let mut plan = typed_let_call_write_plan();
    plan["mode"] = json!({"kind": "require_cached"});
    plan["ops"].as_array_mut().unwrap().pop();
    plan["return"] = json!(["completion"]);
    plan
}

fn execute_call_only_plan() -> Value {
    let mut plan = require_cached_call_plan();
    plan["mode"] = json!({"kind": "execute"});
    plan
}

fn require_cached_external_call_plan(call: Value) -> Value {
    let mut plan = typed_let_call_write_plan();
    plan["mode"] = json!({"kind": "require_cached"});
    plan["ops"].as_array_mut().unwrap().pop();
    plan["ops"][1]["call"] = call;
    plan["return"] = json!(["completion"]);
    plan
}

fn agent_run_call() -> Value {
    json!({
        "kind": "agent_run",
        "runtime": "codex",
        "workspace": "ws_planexec",
        "instructions": {
            "task": "Inspect the plan output."
        },
        "output": {
            "kind": "final_message",
            "max_bytes": 1024
        },
        "input_classes": ["public"]
    })
}

fn sandbox_exec_call() -> Value {
    json!({
        "kind": "sandbox_exec",
        "workspace": "ws_planexec",
        "argv": ["true"],
        "timeout_s": 1,
        "output": {
            "kind": "final_message",
            "max_bytes": 1024
        },
        "input_classes": ["public"]
    })
}

fn latest_at_start_graph_query_plan() -> Value {
    let mut plan = since_revision_event_diff_plan();
    plan["plan_id"] = json!("planrevisionlatest001");
    plan["consistency"] = json!({"kind": "latest_at_start"});
    plan["mode"] = json!({"kind": "execute"});
    plan["ops"][0]["expr"]["source"] = json!({"kind": "events"});
    plan
}

fn evaluator_target_case_query_plan() -> Value {
    json!({
        "schema_version": "leaven.plan.v1",
        "plan_id": "planevaltarget001",
        "consistency": {
            "kind": "latest_at_start"
        },
        "mode": {
            "kind": "execute"
        },
        "ops": [
            {
                "kind": "let",
                "name": "target",
                "expr": {
                        "kind": "case_query",
                        "query": {
                            "kind": "load",
                            "case": {
                                "kind": "case",
                                "run": "run_demo",
                                "id": "case_1"
                            },
                            "include": ["target"],
                            "projection_schema": "fp_schema_sha256_target_projection"
                        }
                }
            }
        ],
        "return": ["target"],
        "commit": {
            "kind": "no_graph_writes"
        }
    })
}

fn at_revision_graph_query_plan() -> Value {
    let mut plan = since_revision_event_diff_plan();
    plan["plan_id"] = json!("planrevisionpinned001");
    plan["consistency"] = json!({
        "kind": "at_revision",
        "revision": "rev_pinned"
    });
    plan["mode"] = json!({"kind": "execute"});
    plan["ops"][0]["expr"]["source"] = json!({"kind": "events"});
    plan
}

fn submit_assessments_plan() -> Value {
    json!({
        "schema_version": "leaven.plan.v1",
        "plan_id": "planscoreoutput001",
        "consistency": {
            "kind": "latest_at_start"
        },
        "mode": {
            "kind": "dry_run"
        },
        "ops": [
            {
                "kind": "write",
                "name": "assessments",
                "idempotency_key": "score-output-0001",
                "write": {
                    "kind": "submit_assessments",
                    "evaluation_request_id": "evalreq_score_output",
                    "assessments": [
                        {
                            "kind": "independent",
                            "candidate": "cand_a",
                            "target": {
                                "case": "case_1"
                            },
                            "score": score_with_output("independent answer"),
                            "evidence": evidence_envelope("independent answer"),
                            "replayability": "pure_read"
                        },
                        {
                            "kind": "pairwise",
                            "candidates": ["cand_a", "cand_b"],
                            "target": {
                                "case": "case_1"
                            },
                            "score": {
                                "value": 0.5,
                                "output": {
                                    "kind": "json",
                                    "value": {
                                        "left": "answer a",
                                        "right": "answer b"
                                    },
                                    "summary": "pairwise compared candidate outputs",
                                    "visibility": "public",
                                    "data_classes": ["candidate.output"]
                                }
                            },
                            "preference": {
                                "winner": "cand_a"
                            },
                            "evidence": evidence_envelope("pairwise compared candidate outputs"),
                            "replayability": "pure_read"
                        },
                        {
                            "kind": "listwise",
                            "candidates": ["cand_a", "cand_b", "cand_c"],
                            "target": {
                                "case": "case_1"
                            },
                            "score": {
                                "value": 0.75,
                                "output": {
                                    "kind": "structured",
                                    "value": [
                                        {"candidate": "cand_a", "output": "answer a"},
                                        {"candidate": "cand_b", "output": "answer b"},
                                        {"candidate": "cand_c", "output": "answer c"}
                                    ],
                                    "summary": "listwise ranked candidate outputs",
                                    "visibility": "public",
                                    "data_classes": ["candidate.output"]
                                }
                            },
                            "ranking": ["cand_a", "cand_b", "cand_c"],
                            "evidence": evidence_envelope("listwise ranked candidate outputs"),
                            "replayability": "pure_read"
                        }
                    ]
                }
            }
        ],
        "return": ["assessments"],
        "commit": {
            "kind": "no_graph_writes"
        }
    })
}

fn score_with_output(summary: &'static str) -> Value {
    json!({
        "value": 1.0,
        "output": {
            "kind": "text",
            "summary": summary,
            "value": summary,
            "visibility": "public",
            "data_classes": ["candidate.output"]
        }
    })
}

fn evidence_envelope(summary: &'static str) -> Value {
    json!({
        "schema_version": "leaven.evidence_envelope.v1",
        "target_derived": false,
        "public": {
            "summary": summary,
            "data_classes": ["public"]
        },
        "redaction_policy": {
            "optimizer": "score_only",
            "reflector": "score_only",
            "operator": "score_only"
        },
        "producer": {
            "stage_call_id": "sc_score_output"
        },
        "source_receipts": {
            "read": [],
            "effect": []
        }
    })
}

#[derive(Default)]
struct RecordingPlanHost {
    graph_reads: Vec<String>,
    case_reads: Vec<String>,
    calls: Vec<&'static str>,
    cached_calls: Vec<&'static str>,
    writes: Vec<&'static str>,
    replayed_receipts: Vec<String>,
    call_deps: BTreeMap<String, Value>,
    write_deps: BTreeMap<String, Value>,
    cached_hit: bool,
    fail_lm: bool,
}

impl PlanExecutionHost for RecordingPlanHost {
    fn graph_query(
        &mut self,
        request: PlanGraphQueryRequest<'_>,
    ) -> Result<PlanGraphQueryOutcome, PublicSeamError> {
        assert_eq!(request.name(), "events");
        assert_eq!(request.expr()["kind"].as_str(), Some("graph_query"));
        match request.scope() {
            PlanGraphReadScope::LatestAtStart { revision } => {
                self.graph_reads.push(format!("latest_at_start:{revision}"));
                Ok(PlanGraphQueryOutcome::new(
                    [json!({
                        "kind": "event_summary",
                        "event_kind": "plan.started",
                        "revision": revision,
                        "payload": {
                            "scope": "latest_at_start"
                        }
                    })],
                    revision,
                ))
            }
            PlanGraphReadScope::AtRevision { revision } => {
                self.graph_reads.push(format!("at_revision:{revision}"));
                Ok(PlanGraphQueryOutcome::new(
                    [json!({
                        "kind": "event_summary",
                        "event_kind": "plan.pinned",
                        "revision": revision,
                        "payload": {
                            "scope": "at_revision"
                        }
                    })],
                    revision,
                ))
            }
            PlanGraphReadScope::SinceRevision { since, until } => {
                let graph_revision = until.unwrap_or(since);
                self.graph_reads.push(format!(
                    "since_revision:{since}..{}",
                    until.unwrap_or("<latest>")
                ));
                Ok(PlanGraphQueryOutcome::new(
                    [json!({
                        "kind": "event_summary",
                        "event_kind": "plan.changed",
                        "revision": graph_revision,
                        "payload": {
                            "since": since,
                            "until": until
                        }
                    })],
                    graph_revision,
                ))
            }
        }
    }

    fn case_query_load(
        &mut self,
        request: PlanCaseQueryRequest<'_>,
    ) -> Result<PlanCaseQueryOutcome, PublicSeamError> {
        assert_eq!(request.name(), "target");
        assert_eq!(request.query()["kind"].as_str(), Some("load"));
        assert_eq!(
            request.query()["case"],
            json!({"kind": "case", "run": "run_demo", "id": "case_1"})
        );
        self.case_reads.push("target:case_1".to_owned());
        Ok(PlanCaseQueryOutcome::new("case_1", "rev_planexec_base")
            .with_target(json!({"answer": "expected"}))
            .with_data_classes(["case.target".to_owned()]))
    }

    fn lm_complete(
        &mut self,
        request: PlanLmCompleteRequest<'_>,
    ) -> Result<PlanLmCompleteOutcome, PublicSeamError> {
        assert_eq!(request.name(), "completion");
        assert_eq!(request.call()["kind"].as_str(), Some("lm_complete"));
        let lm_request = request.to_lm_request()?;
        assert_eq!(lm_request.model.as_str(), "gpt-4.1-mini");
        assert_eq!(
            lm_request
                .model_role
                .as_ref()
                .map(leaven_lm::ModelRole::as_str),
            Some("reflector")
        );
        assert_eq!(
            lm_request
                .messages
                .iter()
                .map(leaven_lm::Message::role)
                .collect::<Vec<_>>(),
            vec![Role::Developer, Role::User, Role::Tool]
        );
        assert!(matches!(
            lm_request.messages.as_slice()[2].content_parts(),
            [MessageContentPart::ToolResult {
                tool_call_id,
                content
            }] if tool_call_id == "call_lookup_1" && content == "{\"hint\":\"ok\"}"
        ));
        assert_eq!(lm_request.tools[0].name, "lookup");
        assert_eq!(lm_request.sampling.max_output_tokens, Some(128));
        assert_eq!(lm_request.sampling.stop, vec!["DONE".to_owned()]);
        assert_eq!(
            lm_request.provider_hints.values.get("cache:key"),
            Some(&json!("planexec-stable"))
        );
        match request.call()["output"]["kind"].as_str() {
            Some("final_message") => assert!(matches!(
                lm_request.output,
                OutputMode::FinalMessage {
                    max_bytes: Some(1024)
                }
            )),
            Some("json_schema") => match lm_request.output {
                OutputMode::JsonSchema(schema) => {
                    assert_eq!(schema.name, "fp_schema_sha256_lmanswer");
                    assert_eq!(schema.schema["required"], json!(["answer"]));
                    assert!(schema.strict);
                }
                other => panic!("unexpected LM output mode: {other:?}"),
            },
            other => panic!("unexpected plan output kind: {other:?}"),
        }
        self.calls.push("completion");
        self.call_deps = request.deps().clone();
        if self.fail_lm {
            return Ok(PlanLmCompleteOutcome::failed_provider_error(
                "provider failed after spending budget",
                "fp_runtime_sha256_planexec",
                100,
            ));
        }
        Ok(PlanLmCompleteOutcome::new(
            json!({
                "role": "assistant",
                "content": [
                    {
                        "kind": "text",
                        "text": "ok"
                    }
                ]
            }),
            "fp_runtime_sha256_planexec",
        ))
    }

    fn cached_lm_complete(
        &mut self,
        request: PlanLmCompleteRequest<'_>,
    ) -> Result<Option<PlanLmCompleteOutcome>, PublicSeamError> {
        assert_eq!(request.name(), "completion");
        assert_eq!(request.call()["kind"].as_str(), Some("lm_complete"));
        self.cached_calls.push("completion");
        self.call_deps = request.deps().clone();
        if self.cached_hit {
            Ok(Some(PlanLmCompleteOutcome::new(
                json!({
                    "role": "assistant",
                    "content": [
                        {
                            "kind": "text",
                            "text": "cached ok"
                        }
                    ]
                }),
                "fp_runtime_sha256_planexec",
            )))
        } else {
            Ok(None)
        }
    }

    fn emit_run_event(
        &mut self,
        request: PlanEmitRunEventRequest<'_>,
    ) -> Result<PlanEmitRunEventOutcome, PublicSeamError> {
        assert_eq!(request.name(), "status");
        assert_eq!(request.write()["kind"].as_str(), Some("emit_run_event"));
        assert_eq!(request.base_revision(), "rev_planexec_base");
        self.writes.push("status");
        self.write_deps = request.deps().clone();
        Ok(PlanEmitRunEventOutcome::new(
            "event_plan_ir_checked",
            "rev_planexec_final",
        ))
    }

    fn replay_receipt(&mut self, receipt: &str) -> Result<Value, PublicSeamError> {
        self.replayed_receipts.push(receipt.to_owned());
        match receipt {
            "lmrec_completion" => Ok(json!({
                "kind": "call",
                "receipt": "lmrec_completion",
                "op_var": "completion",
                "started_at": "2026-05-23T12:00:00Z",
                "completed_at": "2026-05-23T12:00:01Z",
                "call_kind": "lm_complete",
                "request_hash": "fp_request_sha256_replay_lm",
                "result_hash": "fp_result_sha256_replay_lm",
                "runtime_fingerprint": "fp_runtime_sha256_planexec",
                "status": "succeeded"
            })),
            "wrec_status" => Ok(json!({
                "kind": "write",
                "receipt": "wrec_status",
                "op_var": "status",
                "started_at": "2026-05-23T12:00:01Z",
                "completed_at": "2026-05-23T12:00:02Z",
                "write_kind": "emit_run_event",
                "request_hash": "fp_request_sha256_replay_write",
                "result_hash": "fp_result_sha256_replay_write",
                "base_revision": "rev_planexec_base",
                "committed_revision": "rev_planexec_final",
                "status": "succeeded",
                "event_id": "event_plan_ir_checked"
            })),
            _ => Err(PublicSeamError::InvalidPlan {
                message: format!("unexpected replay receipt `{receipt}`"),
            }),
        }
    }
}

fn plan_execution_context() -> PlanExecutionContext {
    PlanExecutionContext::new(
        "fp_cap_sha256_planexec",
        "fp_policy_sha256_planexec",
        "rev_planexec_base",
        "2026-05-23T12:00:00Z",
        "2026-05-23T12:00:01Z",
    )
}

fn evaluator_plan_execution_context() -> PlanExecutionContext {
    PlanExecutionContext::new(
        "fp_cap_sha256_eval01",
        "fp_policy_sha256_01",
        "rev_planexec_base",
        "2026-05-23T12:00:00Z",
        "2026-05-23T12:00:01Z",
    )
    .with_evaluation_request("run_demo", "evalreq_01")
    .with_case_partition("validation")
}

fn evaluator_capability(package: &PublicSeamPackage) -> CapabilityDocument {
    CapabilityDocument::from_value(evaluator_capability_value(package)).unwrap()
}

fn target_denied_evaluator_capability(package: &PublicSeamPackage) -> CapabilityDocument {
    let mut value = evaluator_capability_value(package);
    value["grants"][0]["constraints"]["case_fields"] = json!(["input", "metadata"]);
    CapabilityDocument::from_value(value).unwrap()
}

fn evaluator_capability_value(package: &PublicSeamPackage) -> Value {
    let path = package
        .root()
        .join("examples")
        .join("evaluator_capability.v0.3.example.json");
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn since_revision_event_diff_plan() -> Value {
    json!({
        "schema_version": "leaven.plan.v1",
        "plan_id": "planrevision001",
        "consistency": {
            "kind": "since_revision",
            "since": "rev_base",
            "until": "rev_tip"
        },
        "mode": {
            "kind": "dry_run"
        },
        "ops": [
            {
                "kind": "let",
                "name": "events",
                "expr": {
                    "kind": "graph_query",
                    "source": {
                        "kind": "events",
                        "since_revision": "rev_base",
                        "until_revision": "rev_tip"
                    },
                    "projection": {
                        "kind": "ids"
                    },
                    "page": {
                        "limit": 100
                    }
                }
            }
        ],
        "return": ["events"],
        "commit": {
            "kind": "no_graph_writes"
        }
    })
}

fn workspace_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .unwrap()
        .to_path_buf()
}
