use crate::support::{
    FIXTURE_BLOB_SHA256, package, plan_call_result_hash, prefixed_jcs_hash, sha256_hex,
};
use std::collections::{BTreeMap, BTreeSet};

use leaven_lm::{MessageContentPart, OutputMode, Role};
use leaven_public_seam::{
    AgentCommandOutputRefs, CapabilityDocument, PlanAgentRunOutcome, PlanAgentRunRequest,
    PlanCallKind, PlanCaseQueryOutcome, PlanCaseQueryRequest, PlanCaseTargetValue, PlanCommitKind,
    PlanEmitRunEventOutcome, PlanEmitRunEventRequest, PlanEvaluationShape, PlanExecutionContext,
    PlanExecutionHost, PlanExpressionKind, PlanGraphEventFilter, PlanGraphQueryOutcome,
    PlanGraphQueryRequest, PlanGraphReadScope, PlanLmCompleteOutcome, PlanLmCompleteRequest,
    PlanMode, PlanOperationKind, PlanQueryKind, PlanSandboxExecOutcome, PlanSandboxExecRequest,
    PlanSchemaVersion, PlanWorkspaceMaterializeOutcome, PlanWorkspaceMaterializeRequest,
    PlanWorkspaceQueryOutcome, PlanWorkspaceQueryRequest, PlanWorkspaceReleaseOutcome,
    PlanWorkspaceReleaseRequest, PlanWriteKind, PublicSeamError, PublicSeamPackage,
    WorkspaceQueryOp,
};
use serde_json::{Value, json};

#[test]
fn plan_ir_family_accepts_typed_let_call_write_documents() {
    let package = package();
    let plan = typed_let_call_write_plan();
    let document = package.validate_plan_document(&plan).unwrap();

    assert_eq!(document.plan_id().as_str(), "plankind001");
    assert_eq!(document.schema_version(), PlanSchemaVersion::V1);
    assert_eq!(document.mode(), PlanMode::DryRun);
    assert_eq!(document.commit(), PlanCommitKind::NoGraphWrites);
    assert_eq!(
        document.operation_kinds(),
        &[
            PlanOperationKind::Let,
            PlanOperationKind::Call,
            PlanOperationKind::Write,
        ]
    );
    assert_eq!(document.return_names(), &["status"]);
    assert_eq!(document.return_bindings()[0].as_str(), "status");
    assert_eq!(document.consistency_kind(), "latest_at_start");
    assert_eq!(document.mode_kind(), "dry_run");
    assert_eq!(document.commit_kind(), "no_graph_writes");
    let [let_op, call_op, write_op] = document.operations() else {
        panic!("expected representative let/call/write plan");
    };
    assert_eq!(let_op.name(), "prompt");
    assert_eq!(let_op.kind(), PlanOperationKind::Let);
    assert_eq!(let_op.expression_kind(), Some(PlanExpressionKind::Literal));
    let Some(leaven_public_seam::PlanExpression::Literal {
        value,
        data_classes,
    }) = let_op.expression()
    else {
        panic!("literal let op must expose typed literal value");
    };
    assert_eq!(
        value.as_json(),
        &json!({
            "messages": [
                {"role": "user", "content": ["hello", {"topic": "typing"}]}
            ],
            "temperature": 0.2
        })
    );
    assert_eq!(data_classes, &["public".to_owned()]);
    assert_eq!(let_op.query_kind(), None);
    assert_eq!(call_op.name(), "completion");
    assert_eq!(call_op.kind(), PlanOperationKind::Call);
    assert_eq!(call_op.call_kind(), Some(PlanCallKind::LmComplete));
    assert_eq!(write_op.name(), "status");
    assert_eq!(write_op.kind(), PlanOperationKind::Write);
    assert_eq!(write_op.write_kind(), Some(PlanWriteKind::EmitRunEvent));
    let write = write_op
        .write()
        .expect("write op exposes typed write facts");
    assert_eq!(write.kind(), PlanWriteKind::EmitRunEvent);
    let event_write = write
        .emit_run_event()
        .expect("emit_run_event write exposes typed event payload");
    assert_eq!(event_write.event_kind(), "plan.ir.checked");
    assert_eq!(event_write.payload_schema(), "fp_schema_sha256_planir");
    assert_eq!(event_write.payload().as_json(), &json!({"ok": true}));
    assert_eq!(event_write.visibility(), "public");
    assert_eq!(write.assessment_score_output_count(), 0);
}

#[test]
fn request_evaluation_write_exposes_typed_request_facts() {
    let package = package();
    let mut plan = request_evaluation_plan();
    plan["ops"][0]["write"]["request"]["shape"] = json!("pairwise");
    plan["ops"][0]["write"]["request"]["candidates"] = json!([
        "cand_a",
        {"kind": "candidate", "run": "run_1", "id": "cand_b"}
    ]);
    let document = package.validate_plan_document(&plan).unwrap();
    let [op] = document.operations() else {
        panic!("expected one request_evaluation op");
    };
    assert_eq!(op.write_kind(), Some(PlanWriteKind::RequestEvaluation));
    let write = op
        .write()
        .and_then(|write| write.request_evaluation())
        .expect("request_evaluation write exposes typed request facts");

    assert_eq!(write.shape(), PlanEvaluationShape::Pairwise);
    assert_eq!(write.shape().as_str(), "pairwise");
    assert_eq!(write.candidate_ids(), &["cand_a", "cand_b"]);
    assert_eq!(write.set().kind(), "named");
    assert_eq!(write.set().named_set(), Some("validation"));
    assert_eq!(write.granularity(), "per_case");
    assert_eq!(write.purpose(), "validation");
    assert_eq!(write.evaluator(), Some("eval_score_v1"));
}

#[test]
fn request_evaluation_write_preserves_recursive_set_expr() {
    let package = package();
    let mut plan = request_evaluation_plan();
    plan["ops"][0]["write"]["request"]["set"] = json!({
        "kind": "sample",
        "base": {
            "kind": "union",
            "sets": [
                {"kind": "named", "name": "validation"},
                {
                    "kind": "cases",
                    "cases": [{"kind": "case", "id": "case_1"}],
                    "requires_partition_resolution": true
                }
            ]
        },
        "n": 3,
        "seed": 42
    });
    let document = package.validate_plan_document(&plan).unwrap();
    let write = document.operations()[0]
        .write()
        .and_then(|write| write.request_evaluation())
        .expect("request_evaluation write exposes typed request facts");

    let leaven_public_seam::PlanEvaluationSetExpr::Sample { base, n, seed } = write.set() else {
        panic!("expected sample set expression");
    };
    assert_eq!((*n, *seed), (3, 42));
    let leaven_public_seam::PlanEvaluationSetExpr::Union { sets } = base.as_ref() else {
        panic!("expected union base");
    };
    assert_eq!(sets[0].named_set(), Some("validation"));
    let leaven_public_seam::PlanEvaluationSetExpr::Cases {
        case_ids,
        requires_partition_resolution,
    } = &sets[1]
    else {
        panic!("expected cases branch");
    };
    assert_eq!(case_ids, &["case_1"]);
    assert!(*requires_partition_resolution);
}

#[test]
fn apply_proposal_batch_write_exposes_typed_batch_ref() {
    let package = package();
    let plan = json!({
        "schema_version": "leaven.plan.v1",
        "plan_id": "planapplytyped001",
        "consistency": {"kind": "latest_at_start"},
        "mode": {"kind": "execute"},
        "commit": {"kind": "graph_writes_atomic", "on_stale": "reject"},
        "ops": [{
            "kind": "write",
            "name": "apply",
            "idempotency_key": "apply-typed-0001",
            "write": {
                "kind": "apply_proposal_batch",
                "proposal_batch": "pb_typed_batch",
                "policy": "apply_first_valid"
            }
        }],
        "return": ["apply"]
    });

    let document = package.validate_plan_document(&plan).unwrap();
    let [op] = document.operations() else {
        panic!("expected one apply_proposal_batch op");
    };
    assert_eq!(op.write_kind(), Some(PlanWriteKind::ApplyProposalBatch));
    let write = op
        .write()
        .and_then(|write| write.apply_proposal_batch())
        .expect("apply_proposal_batch write exposes typed facts");

    assert_eq!(write.proposal_batch(), "pb_typed_batch");
}

#[test]
fn submit_assessments_write_exposes_typed_evaluation_request_ref() {
    let package = package();
    let document = package
        .validate_plan_document(&submit_assessments_plan())
        .unwrap();
    let write = document.operations()[0]
        .write()
        .and_then(|write| write.submit_assessments())
        .expect("submit_assessments write exposes typed facts");

    assert_eq!(write.evaluation_request_id(), "evalreq_score_output");
}

#[test]
fn plan_ir_extension_expression_preserves_typed_payload() {
    let package = package();
    let plan = json!({
        "schema_version": "leaven.plan.v1",
        "plan_id": "planextpayload001",
        "consistency": {"kind": "latest_at_start"},
        "mode": {"kind": "dry_run"},
        "ops": [{
            "kind": "let",
            "name": "extension_value",
            "expr": {
                "kind": "extension",
                "namespace": "x.test",
                "op": "literal_payload",
                "schema_fingerprint": "fp_schema_sha256_extension",
                "payload": {
                    "route": ["graph", {"candidate": "cand_1"}],
                    "enabled": true
                }
            }
        }],
        "return": ["extension_value"],
        "commit": {"kind": "no_graph_writes"}
    });
    let document = package.validate_plan_document(&plan).unwrap();
    let [let_op] = document.operations() else {
        panic!("expected one extension let op");
    };

    let Some(leaven_public_seam::PlanExpression::Extension {
        namespace,
        operation,
        schema_fingerprint,
        payload,
    }) = let_op.expression()
    else {
        panic!("extension let op must expose typed extension payload");
    };

    assert_eq!(namespace, "x.test");
    assert_eq!(operation, "literal_payload");
    assert_eq!(schema_fingerprint, "fp_schema_sha256_extension");
    assert_eq!(
        payload.as_json(),
        &json!({
            "route": ["graph", {"candidate": "cand_1"}],
            "enabled": true
        })
    );
}

#[test]
fn plan_ir_expression_preserves_projection_selector_and_cost_scope_owners() {
    let package = package();
    let plan = json!({
        "schema_version": "leaven.plan.v1",
        "plan_id": "planexprjsonowners001",
        "consistency": {"kind": "latest_at_start"},
        "mode": {"kind": "dry_run"},
        "ops": [
            {
                "kind": "let",
                "name": "artifact_view",
                "expr": {
                    "kind": "graph_query",
                    "source": {
                        "kind": "by_candidate",
                        "candidate": "cand_alpha"
                    },
                    "projection": {
                        "kind": "artifact_projection",
                        "artifact": {
                            "surface_fingerprint": "fp_surface_sha256_prompt",
                            "projection_schema": "fp_schema_sha256_projection",
                            "selector_schema": "fp_schema_sha256_selector",
                            "selector": {
                                "path": ["prompt", {"segment": 0}]
                            },
                            "data_classes": ["candidate.artifact"]
                        }
                    }
                }
            },
            {
                "kind": "let",
                "name": "costs",
                "expr": {
                    "kind": "graph_query",
                    "source": {
                        "kind": "costs",
                        "scope": {
                            "kind": "candidate",
                            "candidate": "cand_alpha",
                            "dimensions": ["lm", {"unit": "usd_micro"}]
                        }
                    },
                    "projection": {"kind": "summary"}
                }
            },
            {
                "kind": "let",
                "name": "extension_projection",
                "expr": {
                    "kind": "graph_query",
                    "source": {
                        "kind": "by_candidate",
                        "candidate": "cand_alpha"
                    },
                    "projection": {
                        "kind": "extension",
                        "namespace": "x.test",
                        "op": "opaque_projection",
                        "schema_fingerprint": "fp_schema_sha256_extension",
                        "payload": {
                            "surface_fingerprint": "fp_surface_decoy",
                            "projection_schema": "fp_schema_decoy",
                            "selector": {"must_not_be_collected": true},
                            "kind": "costs",
                            "scope": {"must_not_be_collected": true}
                        }
                    }
                }
            }
        ],
        "return": ["artifact_view", "costs", "extension_projection"],
        "commit": {"kind": "no_graph_writes"}
    });

    let document = package.validate_plan_document(&plan).unwrap();
    let artifact_expr = document.operations()[0]
        .expression()
        .expect("artifact query exposes typed expression");
    assert_eq!(artifact_expr.artifact_selectors().len(), 1);
    assert_eq!(
        artifact_expr.artifact_selectors()[0].as_json(),
        &json!({"path": ["prompt", {"segment": 0}]})
    );
    let costs_expr = document.operations()[1]
        .expression()
        .expect("cost query exposes typed expression");
    assert_eq!(costs_expr.cost_scopes().len(), 1);
    assert_eq!(
        costs_expr.cost_scopes()[0].as_json(),
        &json!({
            "kind": "candidate",
            "candidate": "cand_alpha",
            "dimensions": ["lm", {"unit": "usd_micro"}]
        })
    );
    let extension_expr = document.operations()[2]
        .expression()
        .expect("extension projection exposes typed expression");
    assert!(extension_expr.artifact_selectors().is_empty());
    assert!(extension_expr.cost_scopes().is_empty());
}

#[test]
fn plan_ir_accessors_classify_direct_query_operation_identity() {
    let package = package();
    let plan = latest_at_start_graph_query_plan();
    let document = package.validate_plan_document(&plan).unwrap();

    let query_op = &document.operations()[0];
    assert_eq!(query_op.name(), "events");
    assert_eq!(query_op.kind(), PlanOperationKind::Let);
    assert_eq!(
        query_op.expression_kind(),
        Some(PlanExpressionKind::GraphQuery)
    );
    assert_eq!(query_op.query_kind(), Some(PlanQueryKind::GraphQuery));
}

#[test]
fn graph_query_request_exposes_typed_run_context_event_filter() {
    let package = package();
    let context = plan_execution_context();
    let mut plan = latest_at_start_graph_query_plan();
    plan["plan_id"] = json!("planruncontextfilter001");
    plan["ops"][0]["expr"]["source"]["filter"] = json!({"kind": "run_context"});

    let mut host = RecordingPlanHost::default();
    package
        .execute_plan_document(&plan, &context, &mut host)
        .unwrap();

    assert_eq!(host.graph_read_plan_ids, vec!["planruncontextfilter001"]);
    assert_eq!(host.run_context_event_filter_reads, 1);
}

#[test]
fn plan_ir_family_rejects_unknown_core_call_write_and_escape_hatch_ops() {
    let package = package();

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
    let package = package();
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
        Some(&typed_prompt_literal()),
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
fn plan_execution_with_capability_checks_call_authority_before_host_effects() {
    let package = package();
    let mut plan = typed_let_call_write_plan();
    plan["mode"] = json!({"kind": "execute"});
    plan["commit"] = json!({
        "kind": "graph_writes_atomic",
        "on_stale": "reject"
    });

    let mut allowed_host = RecordingPlanHost::default();
    package
        .execute_plan_document_with_capability(
            &plan,
            &plan_execution_context(),
            &call_execution_capability(&["public"], &[], true),
            &mut allowed_host,
        )
        .unwrap();
    assert_eq!(allowed_host.calls, vec!["completion"]);
    assert_eq!(allowed_host.writes, vec!["status"]);

    let mut denied_plan = plan;
    denied_plan["ops"][1]["call"]["input_classes"] = json!(["public", "case.target"]);
    let mut denied_host = RecordingPlanHost::default();
    let error = package
        .execute_plan_document_with_capability(
            &denied_plan,
            &plan_execution_context(),
            &call_execution_capability(&["public"], &["case.target"], true),
            &mut denied_host,
        )
        .unwrap_err();
    assert!(
        error.to_string().contains("lm_complete"),
        "unexpected error: {error:?}"
    );
    assert!(denied_host.calls.is_empty());
    assert!(denied_host.writes.is_empty());

    let mut write_denied_host = RecordingPlanHost::default();
    let error = package
        .execute_plan_document_with_capability(
            &typed_let_call_write_execute_plan(),
            &plan_execution_context(),
            &call_execution_capability(&["public"], &[], false),
            &mut write_denied_host,
        )
        .unwrap_err();
    assert!(
        error.to_string().contains("event emit denied"),
        "unexpected error: {error:?}"
    );
    assert!(write_denied_host.calls.is_empty());
    assert!(write_denied_host.writes.is_empty());
}

#[test]
fn plan_execution_with_capability_allows_workspace_lifecycle_calls() {
    let package = package();
    let plan = workspace_materialize_release_plan();

    let mut host = RecordingPlanHost::default();
    package
        .execute_plan_document_with_capability(
            &plan,
            &plan_execution_context(),
            &workspace_lifecycle_capability(true),
            &mut host,
        )
        .unwrap();
    assert_eq!(
        host.calls,
        vec!["workspace_materialize", "workspace_release"]
    );

    let mut denied_host = RecordingPlanHost::default();
    let error = package
        .execute_plan_document_with_capability(
            &plan,
            &plan_execution_context(),
            &workspace_lifecycle_capability(false),
            &mut denied_host,
        )
        .unwrap_err();
    assert!(
        error.to_string().contains("workspace_release"),
        "unexpected error: {error:?}"
    );
    assert!(denied_host.calls.is_empty());
}

#[test]
fn plan_execution_with_capability_denies_ungranted_workspace_queries_before_host_effects() {
    let package = package();
    let plan = workspace_materialize_query_plan();

    let mut host = RecordingPlanHost::default();
    package
        .execute_plan_document_with_capability(
            &plan,
            &plan_execution_context(),
            &workspace_query_capability(
                &[
                    "read_file",
                    "list",
                    "stat",
                    "digest",
                    "snapshot",
                    "git_log",
                    "git_diff",
                    "git_status",
                    "capture_artifacts",
                ],
                &["candidate.artifact"],
            ),
            &mut host,
        )
        .unwrap();
    assert_eq!(
        host.calls,
        vec![
            "workspace_materialize",
            "workspace_read_file",
            "workspace_list",
            "workspace_stat",
            "workspace_digest",
            "workspace_snapshot",
            "workspace_git_log",
            "workspace_git_diff",
            "workspace_git_status",
            "workspace_capture_artifacts"
        ]
    );

    let mut ungranted_host = RecordingPlanHost::default();
    let error = package
        .execute_plan_document_with_capability(
            &plan,
            &plan_execution_context(),
            &workspace_lifecycle_capability(false),
            &mut ungranted_host,
        )
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("workspace_query `read_file` denied"),
        "unexpected error: {error:?}"
    );
    assert!(ungranted_host.calls.is_empty());

    let mut wrong_op_host = RecordingPlanHost::default();
    let error = package
        .execute_plan_document_with_capability(
            &plan,
            &plan_execution_context(),
            &workspace_query_capability(&["list"], &["candidate.artifact"]),
            &mut wrong_op_host,
        )
        .unwrap_err();
    assert!(
        error.to_string().contains("read_file"),
        "unexpected error: {error:?}"
    );
    assert!(wrong_op_host.calls.is_empty());

    let mut wrong_class_host = RecordingPlanHost::default();
    let error = package
        .execute_plan_document_with_capability(
            &plan,
            &plan_execution_context(),
            &workspace_query_capability(&["read_file"], &["public"]),
            &mut wrong_class_host,
        )
        .unwrap_err();
    assert!(
        error.to_string().contains("candidate.artifact"),
        "unexpected error: {error:?}"
    );
    assert!(wrong_class_host.calls.is_empty());
}

#[test]
fn workspace_query_denies_no_capability_execution_route() {
    let package = package();
    let mut host = RecordingPlanHost::default();

    let error = package
        .execute_plan_document(
            &workspace_materialize_query_plan(),
            &plan_execution_context(),
            &mut host,
        )
        .unwrap_err();

    assert!(
        error
            .to_string()
            .contains("workspace_query execution requires capability-authorized Plan execution"),
        "unexpected error: {error:?}"
    );
    assert!(host.calls.is_empty());
}

#[test]
fn plan_execution_with_capability_denies_sandbox_policy_before_host_effects() {
    let package = package();
    let mut capability = sandbox_exec_capability_value();
    capability["execution_policy"]["subprocess"] = json!("deny");
    let capability = CapabilityDocument::from_value(capability).unwrap();

    let mut host = RecordingPlanHost::default();
    let error = package
        .execute_plan_document_with_capability(
            &sandbox_exec_workspace_plan(),
            &plan_execution_context(),
            &capability,
            &mut host,
        )
        .unwrap_err();

    assert!(
        error
            .to_string()
            .contains("sandbox_exec denied by execution_policy.subprocess"),
        "unexpected error: {error:?}"
    );
    assert!(host.calls.is_empty());
    assert!(host.cached_calls.is_empty());
    assert!(host.writes.is_empty());

    let mut capability = sandbox_exec_capability_value();
    capability["execution_policy"]["network"] = json!("deny");
    let capability = CapabilityDocument::from_value(capability).unwrap();

    let mut host = RecordingPlanHost::default();
    let error = package
        .execute_plan_document_with_capability(
            &sandbox_exec_workspace_plan(),
            &plan_execution_context(),
            &capability,
            &mut host,
        )
        .unwrap_err();

    assert!(
        error
            .to_string()
            .contains("sandbox_exec denied by execution_policy.network"),
        "unexpected error: {error:?}"
    );
    assert!(host.calls.is_empty());
    assert!(host.cached_calls.is_empty());
    assert!(host.writes.is_empty());
}

#[test]
fn plan_execution_with_capability_gates_evaluator_writes_before_host_effects() {
    let package = package();

    let mut assessment_host = RecordingPlanHost::default();
    let assessment_report = package
        .execute_plan_document_with_capability(
            &submit_assessments_plan(),
            &plan_execution_context(),
            &assessment_submit_capability("evalreq_score_output", Some(3)),
            &mut assessment_host,
        )
        .unwrap();
    assert_eq!(assessment_report.document().receipt_count(), 0);
    assert!(assessment_host.calls.is_empty());
    assert!(assessment_host.writes.is_empty());

    let mut wrong_eval_host = RecordingPlanHost::default();
    let error = package
        .execute_plan_document_with_capability(
            &submit_assessments_plan(),
            &plan_execution_context(),
            &assessment_submit_capability("evalreq_other", Some(3)),
            &mut wrong_eval_host,
        )
        .unwrap_err();
    assert!(
        error.to_string().contains("assessment submit denied"),
        "unexpected error: {error:?}"
    );
    assert!(wrong_eval_host.calls.is_empty());
    assert!(wrong_eval_host.writes.is_empty());

    let mut row_limit_host = RecordingPlanHost::default();
    let error = package
        .execute_plan_document_with_capability(
            &submit_assessments_plan(),
            &plan_execution_context(),
            &assessment_submit_capability("evalreq_score_output", Some(2)),
            &mut row_limit_host,
        )
        .unwrap_err();
    assert!(
        error.to_string().contains("assessment submit denied"),
        "unexpected error: {error:?}"
    );
    assert!(row_limit_host.calls.is_empty());
    assert!(row_limit_host.writes.is_empty());

    let mut evaluation_host = RecordingPlanHost::default();
    package
        .execute_plan_document_with_capability(
            &request_evaluation_plan(),
            &plan_execution_context(),
            &evaluation_request_capability(&["cand_a"], &["validation"]),
            &mut evaluation_host,
        )
        .unwrap();
    assert!(evaluation_host.calls.is_empty());
    assert!(evaluation_host.writes.is_empty());

    let mut wrong_candidate_host = RecordingPlanHost::default();
    let error = package
        .execute_plan_document_with_capability(
            &request_evaluation_plan(),
            &plan_execution_context(),
            &evaluation_request_capability(&["cand_other"], &["validation"]),
            &mut wrong_candidate_host,
        )
        .unwrap_err();
    assert!(
        error.to_string().contains("evaluation request denied"),
        "unexpected error: {error:?}"
    );
    assert!(wrong_candidate_host.calls.is_empty());
    assert!(wrong_candidate_host.writes.is_empty());

    let mut wrong_purpose_host = RecordingPlanHost::default();
    let error = package
        .execute_plan_document_with_capability(
            &request_evaluation_plan(),
            &plan_execution_context(),
            &evaluation_request_capability(&["cand_a"], &["diagnostic"]),
            &mut wrong_purpose_host,
        )
        .unwrap_err();
    assert!(
        error.to_string().contains("evaluation request denied"),
        "unexpected error: {error:?}"
    );
    assert!(wrong_purpose_host.calls.is_empty());
    assert!(wrong_purpose_host.writes.is_empty());
}

#[test]
fn plan_execution_result_rejects_receipt_hashes_unbound_from_plan_preimages() {
    let package = package();
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
fn plan_execution_result_rejects_workspace_query_value_forgery_with_valid_hashes() {
    let package = package();
    let context = plan_execution_context();
    let plan = workspace_materialize_query_plan();
    let mut result = package
        .execute_plan_document_with_capability(
            &plan,
            &context,
            &workspace_query_capability(
                &[
                    "read_file",
                    "list",
                    "stat",
                    "digest",
                    "snapshot",
                    "git_log",
                    "git_diff",
                    "git_status",
                    "capture_artifacts",
                ],
                &["candidate.artifact"],
            ),
            &mut RecordingPlanHost::default(),
        )
        .unwrap()
        .value()
        .clone();

    result["values"]["file"] = json!({
        "kind": "workspace_listing",
        "entries": [],
        "receipt": "qrec_file",
        "graph_revision": "rev_planexec_base",
        "data_classes": ["candidate.artifact", "public"],
        "replayability": "boundary_managed"
    });
    result["receipts"][1]["result_hash"] = json!(prefixed_jcs_hash(
        "fp_result_sha256_",
        &json!({
            "schema_version": "leaven.plan_query_result.v1",
            "name": "file",
            "value": result["values"]["file"]
        }),
    ));
    assert_plan_execution_result_rejected(&package, &plan, &context, &result);

    let mut missing_class = workspace_query_result_fixture(&package, &plan, &context);
    missing_class["values"]["file"]["data_classes"] = json!(["public"]);
    missing_class["receipts"][1]["result_hash"] = json!(prefixed_jcs_hash(
        "fp_result_sha256_",
        &json!({
            "schema_version": "leaven.plan_query_result.v1",
            "name": "file",
            "value": missing_class["values"]["file"]
        }),
    ));
    assert_plan_execution_result_rejected(&package, &plan, &context, &missing_class);

    let mut stat_wrong_path = workspace_query_result_fixture(&package, &plan, &context);
    stat_wrong_path["values"]["stat"]["entries"][0]["path"] = json!("src/lib.rs");
    stat_wrong_path["receipts"][3]["result_hash"] = json!(prefixed_jcs_hash(
        "fp_result_sha256_",
        &json!({
            "schema_version": "leaven.plan_query_result.v1",
            "name": "stat",
            "value": stat_wrong_path["values"]["stat"]
        }),
    ));
    assert_plan_execution_result_rejected(&package, &plan, &context, &stat_wrong_path);

    let mut digest_wrong_algorithm = workspace_query_result_fixture(&package, &plan, &context);
    digest_wrong_algorithm["values"]["digest"]["digest"] = json!("blake3:readme");
    digest_wrong_algorithm["receipts"][4]["result_hash"] = json!(prefixed_jcs_hash(
        "fp_result_sha256_",
        &json!({
            "schema_version": "leaven.plan_query_result.v1",
            "name": "digest",
            "value": digest_wrong_algorithm["values"]["digest"]
        }),
    ));
    assert_plan_execution_result_rejected(&package, &plan, &context, &digest_wrong_algorithm);

    let mut digest_wrong_workspace = workspace_query_result_fixture(&package, &plan, &context);
    digest_wrong_workspace["values"]["digest"]["workspace"] = json!("ws_planexec_other");
    digest_wrong_workspace["receipts"][4]["result_hash"] = json!(prefixed_jcs_hash(
        "fp_result_sha256_",
        &json!({
            "schema_version": "leaven.plan_query_result.v1",
            "name": "digest",
            "value": digest_wrong_workspace["values"]["digest"]
        }),
    ));
    assert_plan_execution_result_rejected(&package, &plan, &context, &digest_wrong_workspace);
}

fn workspace_query_result_fixture(
    package: &PublicSeamPackage,
    plan: &Value,
    context: &PlanExecutionContext,
) -> Value {
    package
        .execute_plan_document_with_capability(
            plan,
            context,
            &workspace_query_capability(
                &[
                    "read_file",
                    "list",
                    "stat",
                    "digest",
                    "snapshot",
                    "git_log",
                    "git_diff",
                    "git_status",
                    "capture_artifacts",
                ],
                &["candidate.artifact"],
            ),
            &mut RecordingPlanHost::default(),
        )
        .unwrap()
        .value()
        .clone()
}

#[test]
fn plan_execution_result_rejects_literal_workspace_handle_provenance_forgery() {
    let package = package();
    let context = plan_execution_context();
    let plan = forged_workspace_handle_query_plan();
    let value = json!({
        "kind": "workspace_file",
        "path": "README.md",
        "content": "forged file",
        "receipt": "qrec_file",
        "graph_revision": "rev_planexec_base",
        "data_classes": ["candidate.artifact", "public"],
        "replayability": "boundary_managed"
    });
    let scope = json!({
        "kind": "workspace_query",
        "workspace": "ws_planexec_materialized",
        "base_revision": "rev_planexec_base"
    });
    let result = json!({
        "schema_version": "leaven.plan_result.v1",
        "plan_id": "planforgedquery001",
        "capability_fingerprint": "fp_cap_sha256_planexec",
        "policy_fingerprint": "fp_policy_sha256_planexec",
        "base_revision": "rev_planexec_base",
        "final_revision": "rev_planexec_base",
        "replayability_summary": "boundary_managed",
        "values": {
            "file": value
        },
        "receipts": [
            {
                "kind": "query",
                "receipt": "qrec_file",
                "op_var": "file",
                "started_at": "2026-01-01T00:00:00Z",
                "completed_at": "2026-01-01T00:00:01Z",
                "op_hash": prefixed_jcs_hash(
                    "fp_query_sha256_",
                    &json!({
                        "schema_version": "leaven.plan_query_op.v1",
                        "name": "file",
                        "expr": plan["ops"][1]["expr"],
                        "scope": scope
                    }),
                ),
                "read_scope_fingerprint": prefixed_jcs_hash("fp_scope_sha256_", &scope),
                "projection_fingerprint": prefixed_jcs_hash(
                    "fp_projection_sha256_",
                    &json!({
                        "workspace": "ws_planexec_materialized",
                        "op": plan["ops"][1]["expr"]["op"]
                    }),
                ),
                "graph_revision": "rev_planexec_base",
                "result_hash": prefixed_jcs_hash(
                    "fp_result_sha256_",
                    &json!({
                        "schema_version": "leaven.plan_query_result.v1",
                        "name": "file",
                        "value": value
                    }),
                ),
                "status": "succeeded"
            }
        ],
        "redactions": [],
        "charges": [],
        "errors": []
    });

    assert_plan_execution_result_rejected(&package, &plan, &context, &result);
}

#[test]
fn plan_execution_result_rejects_workspace_lifecycle_state_forgery_with_valid_hashes() {
    let package = package();
    let context = plan_execution_context();
    let plan = workspace_materialize_release_plan();
    let result = package
        .execute_plan_document(&plan, &context, &mut RecordingPlanHost::default())
        .unwrap()
        .value()
        .clone();

    let mut wrong_materialize_lifetime = result.clone();
    wrong_materialize_lifetime["values"]["workspace"]["lifetime"] = json!("plan");
    rebind_call_result_hash(&mut wrong_materialize_lifetime, 0, "workspace");
    assert_plan_execution_result_rejected(&package, &plan, &context, &wrong_materialize_lifetime);

    let mut missing_materialize_released = result.clone();
    missing_materialize_released["values"]["workspace"]
        .as_object_mut()
        .unwrap()
        .remove("released");
    rebind_call_result_hash(&mut missing_materialize_released, 0, "workspace");
    assert_plan_execution_result_rejected(&package, &plan, &context, &missing_materialize_released);

    let mut already_released_materialize = result.clone();
    already_released_materialize["values"]["workspace"]["released"] = json!(true);
    rebind_call_result_hash(&mut already_released_materialize, 0, "workspace");
    assert_plan_execution_result_rejected(&package, &plan, &context, &already_released_materialize);

    let mut unreleased_release = result.clone();
    unreleased_release["values"]["release"]["released"] = json!(false);
    rebind_call_result_hash(&mut unreleased_release, 1, "release");
    assert_plan_execution_result_rejected(&package, &plan, &context, &unreleased_release);

    let mut missing_release_released = result;
    missing_release_released["values"]["release"]
        .as_object_mut()
        .unwrap()
        .remove("released");
    rebind_call_result_hash(&mut missing_release_released, 1, "release");
    assert_plan_execution_result_rejected(&package, &plan, &context, &missing_release_released);
}

#[test]
fn plan_execution_result_rejects_missing_operation_receipts() {
    let package = package();
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
    let package = package();
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
fn capability_execution_denies_calls_that_drop_dependency_data_classes() {
    let package = package();
    let mut plan = evaluator_target_case_query_plan();
    plan["ops"].as_array_mut().unwrap().push(json!({
        "kind": "call",
        "name": "completion",
        "deps": ["target"],
        "idempotency_key": "target-drop-0001",
        "call": {
            "kind": "lm_complete",
            "purpose": "test.plan_ir",
            "model": "gpt-4.1-mini",
            "model_role": "reflector",
            "messages": [
                {
                    "role": "user",
                    "content": [{"kind": "text", "text": "summarize"}]
                }
            ],
            "output": {"kind": "final_message"},
            "input_classes": ["public"]
        }
    }));
    plan["return"] = json!(["completion"]);
    let capability = CapabilityDocument::from_value(base_execution_capability(&[
        json!({
            "action": "case.read",
            "resource": {
                "run": "run_demo",
                "evaluation_request_id": "evalreq_01"
            },
            "constraints": {
                "case_fields": ["target"],
                "partitions": ["validation"],
                "allowed_input_classes": ["case.target"],
                "forbidden_input_classes": []
            }
        }),
        json!({
            "action": "lm.complete",
            "resource": {},
            "constraints": {
                "allowed_input_classes": ["public"],
                "forbidden_input_classes": ["case.target"],
                "purposes": ["test.plan_ir"],
                "models": ["gpt-4.1-mini"],
                "model_roles": ["reflector"]
            }
        }),
    ]))
    .unwrap();
    let mut host = RecordingPlanHost::default();

    let error = package
        .execute_plan_document_with_capability(
            &plan,
            &plan_execution_context()
                .with_evaluation_request("run_demo", "evalreq_01")
                .with_case_partition("validation"),
            &capability,
            &mut host,
        )
        .unwrap_err();

    assert!(
        error
            .to_string()
            .contains("call `lm_complete` omitted dependency data class `case.target`"),
        "unexpected error: {error:?}"
    );
    assert_eq!(host.case_reads, vec!["target:case_1"]);
    assert!(host.calls.is_empty());
}

#[test]
fn capability_execution_denies_calls_that_drop_nested_agent_command_data_classes() {
    let package = package();
    let mut plan = typed_let_call_write_plan();
    plan["mode"] = json!({"kind": "execute"});
    plan["ops"][0]["expr"]["value"] = literal_agent_session_with_command_output_classes();
    plan["ops"].as_array_mut().unwrap().pop();
    plan["return"] = json!(["completion"]);
    let mut host = RecordingPlanHost::default();

    let error = package
        .execute_plan_document_with_capability(
            &plan,
            &plan_execution_context(),
            &call_execution_capability(&["public"], &["transcript.raw"], false),
            &mut host,
        )
        .unwrap_err();

    assert!(
        error
            .to_string()
            .contains("call `lm_complete` omitted dependency data class `transcript.raw`"),
        "unexpected error: {error:?}"
    );
    assert!(host.calls.is_empty());

    plan["ops"][1]["call"]["input_classes"] = json!(["public", "transcript.raw"]);
    let mut host = RecordingPlanHost::default();
    package
        .execute_plan_document_with_capability(
            &plan,
            &plan_execution_context(),
            &call_execution_capability(&["public", "transcript.raw"], &[], false),
            &mut host,
        )
        .unwrap();
    assert_eq!(host.calls, vec!["completion"]);
}

#[test]
fn capability_execution_denies_calls_that_drop_nested_graph_row_data_classes() {
    let package = package();
    let mut plan = typed_let_call_write_plan();
    plan["mode"] = json!({"kind": "execute"});
    plan["ops"][0]["expr"]["value"] = literal_assessment_graph_set();
    plan["ops"].as_array_mut().unwrap().pop();
    plan["return"] = json!(["completion"]);
    let mut host = RecordingPlanHost::default();

    let error = package
        .execute_plan_document_with_capability(
            &plan,
            &plan_execution_context(),
            &call_execution_capability(&["public"], &["candidate.output"], false),
            &mut host,
        )
        .unwrap_err();

    assert!(
        error
            .to_string()
            .contains("call `lm_complete` omitted dependency data class `candidate.output`"),
        "unexpected error: {error:?}"
    );
    assert!(host.calls.is_empty());
}

#[test]
fn capability_execution_denies_calls_that_drop_literal_expr_data_classes() {
    let package = package();
    let mut plan = typed_let_call_write_plan();
    plan["mode"] = json!({"kind": "execute"});
    plan["ops"][0]["expr"]["data_classes"] = json!(["external.secret"]);
    plan["ops"].as_array_mut().unwrap().pop();
    plan["return"] = json!(["completion"]);
    let mut host = RecordingPlanHost::default();

    let error = package
        .execute_plan_document_with_capability(
            &plan,
            &plan_execution_context(),
            &call_execution_capability(&["public"], &["external.secret"], false),
            &mut host,
        )
        .unwrap_err();

    assert_call_authority_denied_redactions(&error, &["external.secret"]);
    assert!(
        error
            .to_string()
            .contains("call `lm_complete` omitted dependency data class `external.secret`"),
        "unexpected error: {error:?}"
    );
    assert!(host.calls.is_empty());

    plan["ops"][1]["call"]["input_classes"] = json!(["external.secret", "public"]);
    let mut host = RecordingPlanHost::default();
    package
        .execute_plan_document_with_capability(
            &plan,
            &plan_execution_context(),
            &call_execution_capability(&["external.secret", "public"], &[], false),
            &mut host,
        )
        .unwrap();
    assert_eq!(host.calls, vec!["completion"]);
    assert_eq!(host.call_deps["prompt"], typed_prompt_literal());
}

#[test]
fn capability_execution_denies_calls_that_drop_workspace_listing_entry_data_classes() {
    let package = package();
    let mut plan = typed_let_call_write_plan();
    plan["mode"] = json!({"kind": "execute"});
    plan["ops"][0]["expr"]["value"] = literal_workspace_listing_with_entry_classes();
    plan["ops"].as_array_mut().unwrap().pop();
    plan["return"] = json!(["completion"]);
    let mut host = RecordingPlanHost::default();

    let error = package
        .execute_plan_document_with_capability(
            &plan,
            &plan_execution_context(),
            &call_execution_capability(&["public"], &["workspace.secret"], false),
            &mut host,
        )
        .unwrap_err();

    assert!(
        error
            .to_string()
            .contains("call `lm_complete` omitted dependency data class `workspace.secret`"),
        "unexpected error: {error:?}"
    );
    assert!(host.calls.is_empty());
}

#[test]
fn write_receipts_bind_literal_dependency_data_classes_without_rewriting_values() {
    let package = package();
    let mut plan = emit_event_from_literal_dependency_plan();
    plan["ops"][0]["expr"]["data_classes"] = json!(["external.secret"]);
    let context = plan_execution_context();
    let mut host = RecordingPlanHost::default();

    let report = package
        .execute_plan_document(&plan, &context, &mut host)
        .unwrap();

    assert_eq!(host.writes, vec!["status"]);
    assert_eq!(host.write_deps["prompt"], typed_prompt_literal());
    assert_eq!(
        host.write_dependency_data_classes,
        BTreeSet::from(["external.secret".to_owned()])
    );
    package
        .validate_plan_execution_result(&plan, &context, report.value())
        .unwrap();

    let mut weaker_plan = plan.clone();
    weaker_plan["ops"][0]["expr"]
        .as_object_mut()
        .unwrap()
        .remove("data_classes");
    let error = package
        .validate_plan_execution_result(&weaker_plan, &context, report.value())
        .unwrap_err();
    assert!(matches!(&error, PublicSeamError::InvalidPlan { .. }));
    assert!(
        error.to_string().contains("request_hash"),
        "unexpected error: {error:?}"
    );
}

#[test]
fn capability_execution_ignores_domain_json_data_classes_inside_dependencies() {
    let package = package();
    let mut plan = typed_let_call_write_plan();
    plan["mode"] = json!({"kind": "execute"});
    plan["ops"][0]["expr"]["value"] = json!({
        "question": "Say ok",
        "payload": {
            "kind": "application_record",
            "data_classes": ["external.secret"]
        }
    });
    plan["ops"].as_array_mut().unwrap().pop();
    plan["return"] = json!(["completion"]);
    let mut host = RecordingPlanHost::default();

    package
        .execute_plan_document_with_capability(
            &plan,
            &plan_execution_context(),
            &call_execution_capability(&["public"], &["external.secret"], false),
            &mut host,
        )
        .unwrap();

    assert_eq!(host.calls, vec!["completion"]);
    assert_eq!(
        host.call_deps["prompt"]["payload"],
        json!({
            "kind": "application_record",
            "data_classes": ["external.secret"]
        })
    );

    plan["ops"][0]["expr"]["value"] = json!({
        "kind": "text",
        "data_classes": ["external.secret"],
        "body": "domain payload, not a public-seam OutputRecord"
    });
    let mut host = RecordingPlanHost::default();
    package
        .execute_plan_document_with_capability(
            &plan,
            &plan_execution_context(),
            &call_execution_capability(&["public"], &["external.secret"], false),
            &mut host,
        )
        .unwrap();
    assert_eq!(host.calls, vec!["completion"]);
}

#[test]
fn evaluator_target_reads_reject_missing_or_unbound_case_query_receipts() {
    let package = package();
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
    let package = package();
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
    let package = package();
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

fn assert_plan_execution_result_error_contains(
    package: &PublicSeamPackage,
    plan: &Value,
    context: &PlanExecutionContext,
    result: &Value,
    expected: &str,
) {
    let error = package
        .validate_plan_execution_result(plan, context, result)
        .unwrap_err();
    assert!(
        error.to_string().contains(expected),
        "expected `{expected}` in {error:?}"
    );
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

fn lm_answer_output_contract() -> Value {
    let schema = json!({
        "type": "object",
        "properties": {
            "answer": {
                "type": "string"
            }
        },
        "required": ["answer"],
        "additionalProperties": false
    });
    json!({
        "kind": "json_schema",
        "schema_fingerprint": prefixed_jcs_hash("fp_schema_sha256_", &schema),
        "schema": schema
    })
}

fn agent_status_output_contract() -> Value {
    let schema = json!({
        "type": "object",
        "properties": {
            "status": {
                "type": "string"
            }
        },
        "required": ["status"],
        "additionalProperties": false
    });
    json!({
        "kind": "json_schema",
        "schema_fingerprint": prefixed_jcs_hash("fp_schema_sha256_", &schema),
        "schema": schema
    })
}

fn rebind_call_result_hash(result: &mut Value, receipt_index: usize, name: &str) {
    let value = result["values"][name].clone();
    result["receipts"][receipt_index]["result_hash"] = json!(plan_call_result_hash(name, &value));
}

fn rebind_failed_call_result_hash(result: &mut Value, receipt_index: usize, name: &str) {
    let receipt = &result["receipts"][receipt_index];
    let error = receipt.get("error").cloned();
    let cost = receipt.get("cost").cloned();
    let charge_receipts = receipt
        .get("charge_receipts")
        .cloned()
        .unwrap_or_else(|| json!([]));
    result["receipts"][receipt_index]["result_hash"] = json!(prefixed_jcs_hash(
        "fp_result_sha256_",
        &json!({
            "schema_version": "leaven.plan_call_result.v1",
            "name": name,
            "error": error,
            "cost": cost,
            "charge_receipts": charge_receipts
        }),
    ));
}

#[test]
fn plan_ir_family_execution_rejects_dry_run_or_no_graph_write_fake_execution() {
    let package = package();
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
    let package = package();
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
    let package = package();

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
fn agent_run_and_sandbox_exec_lower_to_owned_runtime_primitives_and_emit_receipts() {
    let package = package();

    let mut agent_host = RecordingPlanHost::default();
    let agent_plan = agent_run_workspace_plan(&agent_run_call());
    let agent_report = package
        .execute_plan_document_with_capability(
            &agent_plan,
            &plan_execution_context(),
            &effect_execution_capability_for_plan(&agent_plan),
            &mut agent_host,
        )
        .unwrap();
    assert_eq!(agent_host.calls, vec!["workspace_materialize", "agent"]);
    assert_eq!(agent_report.document().receipt_kinds(), &["call", "call"]);
    assert_eq!(
        agent_report.value()["values"]["completion"]["kind"].as_str(),
        Some("agent_session")
    );
    assert_eq!(
        agent_report.value()["receipts"][1]["receipt"].as_str(),
        Some("agentrec_completion")
    );

    let mut sandbox_host = RecordingPlanHost::default();
    let sandbox_plan = sandbox_exec_workspace_plan();
    let sandbox_report = package
        .execute_plan_document_with_capability(
            &sandbox_plan,
            &plan_execution_context(),
            &effect_execution_capability_for_plan(&sandbox_plan),
            &mut sandbox_host,
        )
        .unwrap();
    assert_eq!(sandbox_host.calls, vec!["workspace_materialize", "sandbox"]);
    assert_eq!(sandbox_report.document().receipt_kinds(), &["call", "call"]);
    assert_eq!(
        sandbox_report.value()["values"]["completion"]["kind"].as_str(),
        Some("sandbox_exec")
    );
    assert_eq!(
        sandbox_report.value()["receipts"][1]["receipt"].as_str(),
        Some("execrec_completion")
    );
    assert_eq!(
        sandbox_report.value()["values"]["completion"]["files"]["out.txt"]["kind"].as_str(),
        Some("blob_ref")
    );
}

#[test]
fn agent_run_lowering_defaults_missing_tool_policy_to_no_shell() {
    let package = package();
    let mut call = agent_run_call();
    call.as_object_mut().unwrap().remove("tool_policy");
    let mut host = RecordingPlanHost::default();

    package
        .execute_plan_document_with_capability(
            &agent_run_workspace_plan(&call),
            &plan_execution_context(),
            &effect_execution_capability_for_plan(&agent_run_workspace_plan(&call)),
            &mut host,
        )
        .unwrap();

    assert_eq!(host.calls, vec!["workspace_materialize", "agent"]);
}

#[test]
fn agent_run_result_rejects_commands_outside_declared_allowed_commands() {
    let package = package();
    let mut call = agent_run_call();
    call["tool_policy"]["allowed_commands"] = json!(["python"]);
    let mut host = RecordingPlanHost::default();

    let error = package
        .execute_plan_document_with_capability(
            &agent_run_workspace_plan(&call),
            &plan_execution_context(),
            &effect_execution_capability_for_plan(&agent_run_workspace_plan(&call)),
            &mut host,
        )
        .unwrap_err();

    assert!(
        error
            .to_string()
            .contains("agent_run command `codex` is outside declared allowed_commands"),
        "unexpected error: {error:?}"
    );
    assert_eq!(host.calls, vec!["workspace_materialize", "agent"]);
}

#[test]
fn call_results_reject_missing_receipts_and_wrong_kinds_even_with_valid_hashes() {
    let package = package();
    let context = plan_execution_context();
    let plan = execute_call_only_plan();
    let mut result = package
        .execute_plan_document(&plan, &context, &mut RecordingPlanHost::default())
        .unwrap()
        .value()
        .clone();

    result["values"]["completion"]
        .as_object_mut()
        .unwrap()
        .remove("receipt");
    rebind_call_result_hash(&mut result, 0, "completion");
    assert_plan_execution_result_rejected(&package, &plan, &context, &result);

    let mut wrong_kind = package
        .execute_plan_document(&plan, &context, &mut RecordingPlanHost::default())
        .unwrap()
        .value()
        .clone();
    wrong_kind["values"]["completion"] = json!({
        "kind": "agent_session",
        "status": "completed",
        "graph_revision": "rev_planexec_base",
        "data_classes": ["public"],
        "replayability": "fully_managed",
        "receipt": "lmrec_completion"
    });
    rebind_call_result_hash(&mut wrong_kind, 0, "completion");
    assert_plan_execution_result_rejected(&package, &plan, &context, &wrong_kind);
}

#[test]
fn agent_run_rejects_unmaterialized_released_and_host_path_workspaces() {
    let package = package();

    let mut unmaterialized = agent_run_workspace_plan(&agent_run_call());
    unmaterialized["ops"][1]["call"]["workspace"] = json!("ws_unmaterialized");
    let mut host = RecordingPlanHost::default();
    let error = package
        .execute_plan_document_with_capability(
            &unmaterialized,
            &plan_execution_context(),
            &effect_execution_capability_for_plan(&unmaterialized),
            &mut host,
        )
        .unwrap_err();
    assert_eq!(host.calls, vec!["workspace_materialize"]);
    assert!(
        error
            .to_string()
            .contains("agent_run refused unmaterialized workspace"),
        "unexpected error: {error:?}"
    );

    let mut released = workspace_materialize_release_plan();
    released["ops"].as_array_mut().unwrap().push(json!({
        "kind": "call",
        "name": "completion",
        "deps": ["release"],
        "idempotency_key": "plan-agent-workspace-0003",
        "call": agent_run_call()
    }));
    released["return"] = json!(["workspace", "release", "completion"]);
    let mut host = RecordingPlanHost::default();
    let error = package
        .execute_plan_document_with_capability(
            &released,
            &plan_execution_context(),
            &effect_execution_capability_for_plan(&released),
            &mut host,
        )
        .unwrap_err();
    assert_eq!(
        host.calls,
        vec!["workspace_materialize", "workspace_release"]
    );
    assert!(
        error
            .to_string()
            .contains("agent_run refused already released workspace"),
        "unexpected error: {error:?}"
    );

    let mut host_path = agent_run_workspace_plan(&agent_run_call());
    host_path["ops"][1]["call"]["workspace"] = json!("/tmp/leaven-workspace");
    assert!(matches!(
        package
            .validate_plan_document(&host_path)
            .expect_err("host paths are not WorkspaceRef values"),
        PublicSeamError::ExampleValidation { .. }
    ));
}

#[test]
fn sandbox_exec_rejects_unmaterialized_released_and_host_path_workspaces() {
    let package = package();

    let mut unmaterialized = sandbox_exec_workspace_plan();
    unmaterialized["ops"][1]["call"]["workspace"] = json!("ws_unmaterialized");
    let mut host = RecordingPlanHost::default();
    let error = package
        .execute_plan_document_with_capability(
            &unmaterialized,
            &plan_execution_context(),
            &effect_execution_capability_for_plan(&unmaterialized),
            &mut host,
        )
        .unwrap_err();
    assert_eq!(host.calls, vec!["workspace_materialize"]);
    assert!(
        error
            .to_string()
            .contains("sandbox_exec refused unmaterialized workspace"),
        "unexpected error: {error:?}"
    );

    let mut released = workspace_materialize_release_plan();
    released["ops"].as_array_mut().unwrap().push(json!({
        "kind": "call",
        "name": "completion",
        "deps": ["release"],
        "idempotency_key": "plan-sandbox-workspace-0003",
        "call": sandbox_exec_call()
    }));
    released["return"] = json!(["workspace", "release", "completion"]);
    let mut host = RecordingPlanHost::default();
    let error = package
        .execute_plan_document_with_capability(
            &released,
            &plan_execution_context(),
            &effect_execution_capability_for_plan(&released),
            &mut host,
        )
        .unwrap_err();
    assert_eq!(
        host.calls,
        vec!["workspace_materialize", "workspace_release"]
    );
    assert!(
        error
            .to_string()
            .contains("sandbox_exec refused already released workspace"),
        "unexpected error: {error:?}"
    );

    let mut host_path = sandbox_exec_workspace_plan();
    host_path["ops"][1]["call"]["workspace"] = json!("/tmp/leaven-workspace");
    assert!(matches!(
        package
            .validate_plan_document(&host_path)
            .expect_err("host paths are not WorkspaceRef values"),
        PublicSeamError::ExampleValidation { .. }
    ));
}

#[test]
fn sandbox_exec_blob_refs_only_requires_stream_blob_refs() {
    let package = package();
    let mut plan = sandbox_exec_workspace_plan();
    plan["ops"][1]["call"]["stream_policy"] = json!("blob_refs_only");

    let mut host = RecordingPlanHost {
        sandbox_stream: SandboxStreamFixture::BlobRefsOnly,
        ..RecordingPlanHost::default()
    };
    let report = package
        .execute_plan_document_with_capability(
            &plan,
            &plan_execution_context(),
            &effect_execution_capability_for_plan(&plan),
            &mut host,
        )
        .unwrap();
    assert_eq!(
        report.value()["values"]["completion"]["stdout_ref"]["kind"].as_str(),
        Some("blob_ref")
    );
    assert_eq!(
        report.value()["values"]["completion"]["stderr_ref"]["kind"].as_str(),
        Some("blob_ref")
    );

    let mut missing_refs_host = RecordingPlanHost {
        sandbox_stream: SandboxStreamFixture::MissingBlobRefs,
        ..RecordingPlanHost::default()
    };
    let error = package
        .execute_plan_document_with_capability(
            &plan,
            &plan_execution_context(),
            &effect_execution_capability_for_plan(&plan),
            &mut missing_refs_host,
        )
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("blob_refs_only stream policy requires stdout_ref and stderr_ref"),
        "unexpected error: {error:?}"
    );

    assert_sandbox_stream_result_rejects_forgery(&package, &plan, report.value());
}

fn assert_sandbox_stream_result_rejects_forgery(
    package: &PublicSeamPackage,
    plan: &Value,
    result: &Value,
) {
    let mut forged = result.clone();
    forged["values"]["completion"]
        .as_object_mut()
        .unwrap()
        .remove("stdout_ref");
    rebind_call_result_hash(&mut forged, 1, "completion");
    assert_plan_execution_result_rejected(package, plan, &plan_execution_context(), &forged);
}

#[test]
fn sandbox_exec_outcome_propagates_stream_and_file_blob_data_classes() {
    let package = package();
    let mut host = RecordingPlanHost {
        sandbox_stream: SandboxStreamFixture::NonPublicBlobDataClasses,
        ..RecordingPlanHost::default()
    };
    let plan = sandbox_exec_workspace_plan();

    let report = package
        .execute_plan_document_with_capability(
            &plan,
            &plan_execution_context(),
            &effect_execution_capability_for_plan(&plan),
            &mut host,
        )
        .unwrap();

    assert_eq!(
        report.value()["values"]["completion"]["data_classes"],
        json!(["public", "transcript.raw", "workspace.file"])
    );

    let mut missing_stream_class = report.value().clone();
    missing_stream_class["values"]["completion"]["data_classes"] =
        json!(["public", "workspace.file"]);
    rebind_call_result_hash(&mut missing_stream_class, 1, "completion");
    assert_plan_execution_or_result_rejected(
        &package,
        &plan,
        &plan_execution_context(),
        &missing_stream_class,
    );
}

#[test]
fn workspace_materialize_and_release_emit_typed_handles_and_receipts() {
    let package = package();
    let plan = workspace_materialize_release_plan();
    let mut host = RecordingPlanHost::default();

    let report = package
        .execute_plan_document_with_capability(
            &plan,
            &plan_execution_context(),
            &effect_execution_capability_for_plan(&plan),
            &mut host,
        )
        .unwrap();

    assert_eq!(
        host.calls,
        vec!["workspace_materialize", "workspace_release"]
    );
    assert_eq!(report.document().receipt_kinds(), &["call", "call"]);
    assert_eq!(
        report.value()["values"]["workspace"]["kind"].as_str(),
        Some("workspace_handle")
    );
    assert_eq!(
        report.value()["values"]["workspace"]["released"].as_bool(),
        Some(false)
    );
    assert_eq!(
        report.value()["values"]["release"]["released"].as_bool(),
        Some(true)
    );
    assert_eq!(
        report.value()["receipts"][0]["call_kind"].as_str(),
        Some("workspace_materialize")
    );
    assert_eq!(
        report.value()["receipts"][1]["call_kind"].as_str(),
        Some("workspace_release")
    );
}

#[test]
fn workspace_materialize_rejects_host_path_and_lifetime_substitution() {
    let package = package();

    let mut host_path = workspace_materialize_only_plan();
    host_path["ops"][0]["name"] = json!("workspace_path");
    host_path["return"] = json!(["workspace_path"]);
    let mut host = RecordingPlanHost::default();
    let error = package
        .execute_plan_document(&host_path, &plan_execution_context(), &mut host)
        .unwrap_err();
    assert_eq!(host.calls, vec!["workspace_materialize"]);
    assert!(
        error
            .to_string()
            .contains("workspace_materialize host returned invalid workspace"),
        "unexpected error: {error:?}"
    );

    let mut wrong_lifetime = workspace_materialize_only_plan();
    wrong_lifetime["ops"][0]["name"] = json!("workspace_bad_lifetime");
    wrong_lifetime["ops"][0]["call"]["lifetime"] = json!("plan");
    wrong_lifetime["return"] = json!(["workspace_bad_lifetime"]);
    let mut host = RecordingPlanHost::default();
    let error = package
        .execute_plan_document(&wrong_lifetime, &plan_execution_context(), &mut host)
        .unwrap_err();
    assert_eq!(host.calls, vec!["workspace_materialize"]);
    assert!(
        error
            .to_string()
            .contains("workspace_materialize host returned lifetime `manual_release`"),
        "unexpected error: {error:?}"
    );
}

#[test]
fn workspace_handle_provenance_rejects_literal_forgery_and_released_reuse() {
    let package = package();

    for (name, plan, expected_error) in [
        (
            "agent_run",
            forged_workspace_handle_call_plan("plan-forged-agent-001", &agent_run_call()),
            "agent_run refused unmaterialized workspace",
        ),
        (
            "sandbox_exec",
            forged_workspace_handle_call_plan("plan-forged-sandbox-001", &sandbox_exec_call()),
            "sandbox_exec refused unmaterialized workspace",
        ),
        (
            "workspace_release",
            forged_workspace_handle_call_plan(
                "plan-forged-release-001",
                &json!({
                    "kind": "workspace_release",
                    "workspace": "ws_planexec_materialized",
                    "force": false
                }),
            ),
            "workspace_release refused unmaterialized workspace",
        ),
    ] {
        let mut host = RecordingPlanHost::default();
        let error = package
            .execute_plan_document_with_capability(
                &plan,
                &plan_execution_context(),
                &effect_execution_capability_for_plan(&plan),
                &mut host,
            )
            .unwrap_err();
        assert!(
            host.calls.is_empty(),
            "literal {name} handle forgery reached host: {:?}",
            host.calls
        );
        assert!(
            error.to_string().contains(expected_error),
            "unexpected {name} error: {error:?}"
        );
    }

    let query = forged_workspace_handle_query_plan();
    let mut host = RecordingPlanHost::default();
    let error = package
        .execute_plan_document_with_capability(
            &query,
            &plan_execution_context(),
            &workspace_query_capability(&["read_file"], &["candidate.artifact"]),
            &mut host,
        )
        .unwrap_err();
    assert!(host.calls.is_empty());
    assert!(
        error
            .to_string()
            .contains("workspace_query refused unmaterialized workspace"),
        "unexpected workspace_query error: {error:?}"
    );

    let mut reuse_after_release = workspace_materialize_release_plan();
    reuse_after_release["ops"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "kind": "call",
            "name": "completion",
            "deps": ["workspace"],
            "idempotency_key": "plan-workspace-reuse-0003",
            "call": sandbox_exec_call()
        }));
    reuse_after_release["return"] = json!(["workspace", "release", "completion"]);
    let mut host = RecordingPlanHost::default();
    let error = package
        .execute_plan_document_with_capability(
            &reuse_after_release,
            &plan_execution_context(),
            &effect_execution_capability_for_plan(&reuse_after_release),
            &mut host,
        )
        .unwrap_err();
    assert_eq!(
        host.calls,
        vec!["workspace_materialize", "workspace_release"]
    );
    assert!(
        error
            .to_string()
            .contains("sandbox_exec refused already released workspace"),
        "unexpected released-reuse error: {error:?}"
    );
}

#[test]
fn workspace_release_rejects_unmaterialized_handles_and_host_path_substitutes() {
    let package = package();
    let mut unmaterialized = workspace_materialize_release_plan();
    unmaterialized["ops"][1]["call"]["workspace"] = json!("ws_unmaterialized");
    let mut host = RecordingPlanHost::default();

    let error = package
        .execute_plan_document(&unmaterialized, &plan_execution_context(), &mut host)
        .unwrap_err();
    assert_eq!(host.calls, vec!["workspace_materialize"]);
    assert!(
        error
            .to_string()
            .contains("workspace_release refused unmaterialized workspace"),
        "unexpected error: {error:?}"
    );

    let mut host_path = workspace_materialize_release_plan();
    host_path["ops"][1]["call"]["workspace"] = json!("/tmp/leaven-workspace");
    assert!(matches!(
        package
            .validate_plan_document(&host_path)
            .expect_err("host paths are not WorkspaceRef values"),
        PublicSeamError::ExampleValidation { .. }
    ));

    let mut released = workspace_materialize_release_plan();
    released["ops"].as_array_mut().unwrap().push(json!({
        "kind": "call",
        "name": "release_again",
        "deps": ["release"],
        "idempotency_key": "plan-workspace-0003",
        "call": {
            "kind": "workspace_release",
            "workspace": "ws_planexec_materialized",
            "force": false
        }
    }));
    released["return"] = json!(["workspace", "release", "release_again"]);
    let mut host = RecordingPlanHost::default();
    let error = package
        .execute_plan_document(&released, &plan_execution_context(), &mut host)
        .unwrap_err();
    assert_eq!(
        host.calls,
        vec!["workspace_materialize", "workspace_release"]
    );
    assert!(
        error
            .to_string()
            .contains("workspace_release refused already released workspace"),
        "unexpected error: {error:?}"
    );

    let mut mismatched_host = workspace_materialize_release_plan();
    mismatched_host["ops"][1]["name"] = json!("release_wrong");
    mismatched_host["return"] = json!(["workspace", "release_wrong"]);
    let mut host = RecordingPlanHost::default();
    let error = package
        .execute_plan_document(&mismatched_host, &plan_execution_context(), &mut host)
        .unwrap_err();
    assert_eq!(
        host.calls,
        vec!["workspace_materialize", "workspace_release"]
    );
    assert!(
        error
            .to_string()
            .contains("host returned workspace `ws_planexec_other`"),
        "unexpected error: {error:?}"
    );

    let mut wrong_lifetime = workspace_materialize_release_plan();
    wrong_lifetime["ops"][1]["name"] = json!("release_bad_lifetime");
    wrong_lifetime["return"] = json!(["workspace", "release_bad_lifetime"]);
    let mut host = RecordingPlanHost::default();
    let error = package
        .execute_plan_document(&wrong_lifetime, &plan_execution_context(), &mut host)
        .unwrap_err();
    assert_eq!(
        host.calls,
        vec!["workspace_materialize", "workspace_release"]
    );
    assert!(
        error
            .to_string()
            .contains("workspace_release host returned lifetime `plan`"),
        "unexpected error: {error:?}"
    );
}

#[test]
fn workspace_query_reads_require_live_handles_and_emit_query_receipts() {
    let package = package();
    let mut host = RecordingPlanHost::default();

    let report =
        execute_workspace_query_plan(&package, &workspace_materialize_query_plan(), &mut host);

    assert_eq!(
        host.calls,
        vec![
            "workspace_materialize",
            "workspace_read_file",
            "workspace_list",
            "workspace_stat",
            "workspace_digest",
            "workspace_snapshot",
            "workspace_git_log",
            "workspace_git_diff",
            "workspace_git_status",
            "workspace_capture_artifacts"
        ]
    );
    assert_eq!(
        report.document().receipt_kinds(),
        &[
            "call", "query", "query", "query", "query", "query", "query", "query", "query", "query"
        ]
    );
    assert_eq!(
        report.value()["values"]["file"]["kind"].as_str(),
        Some("workspace_file")
    );
    assert_eq!(
        report.value()["values"]["file"]["data_classes"]
            .as_array()
            .unwrap(),
        &json!(["candidate.artifact", "public"]).as_array().unwrap()[..]
    );
    assert_eq!(
        report.value()["values"]["listing"]["kind"].as_str(),
        Some("workspace_listing")
    );
    assert_eq!(
        report.value()["values"]["listing"]["entries"][0]["data_classes"][0].as_str(),
        Some("candidate.artifact")
    );
    assert_eq!(
        report.value()["values"]["stat"]["kind"].as_str(),
        Some("workspace_listing")
    );
    assert_eq!(
        report.value()["values"]["digest"]["kind"].as_str(),
        Some("workspace_snapshot")
    );
    assert_eq!(
        report.value()["values"]["snapshot"]["kind"].as_str(),
        Some("workspace_snapshot")
    );
    assert_eq!(
        report.value()["values"]["log"]["kind"].as_str(),
        Some("workspace_diff")
    );
    assert_eq!(
        report.value()["values"]["diff"]["kind"].as_str(),
        Some("workspace_diff")
    );
    assert_eq!(
        report.value()["values"]["status"]["kind"].as_str(),
        Some("workspace_diff")
    );
    assert_eq!(
        report.value()["values"]["captured"]["kind"].as_str(),
        Some("workspace_listing")
    );
}

#[test]
fn workspace_query_rejects_unmaterialized_released_and_mismatched_results() {
    let package = package();

    let mut unmaterialized = workspace_materialize_query_plan();
    unmaterialized["ops"][1]["expr"]["workspace"] = json!("ws_unmaterialized");
    unmaterialized["ops"].as_array_mut().unwrap().truncate(2);
    unmaterialized["return"] = json!(["workspace", "file"]);
    let mut host = RecordingPlanHost::default();
    let error = package
        .execute_plan_document_with_capability(
            &unmaterialized,
            &plan_execution_context(),
            &workspace_query_capability_for_workspace(
                "ws_unmaterialized",
                &[
                    "read_file",
                    "list",
                    "stat",
                    "digest",
                    "snapshot",
                    "git_log",
                    "git_diff",
                    "git_status",
                    "capture_artifacts",
                ],
                &["candidate.artifact"],
            ),
            &mut host,
        )
        .unwrap_err();
    assert_eq!(host.calls, vec!["workspace_materialize"]);
    assert!(
        error
            .to_string()
            .contains("workspace_query refused unmaterialized workspace"),
        "unexpected error: {error:?}"
    );

    let mut released = workspace_materialize_release_plan();
    released["ops"].as_array_mut().unwrap().push(json!({
        "kind": "let",
        "name": "file",
        "deps": ["release"],
        "expr": {
            "kind": "workspace_query",
            "workspace": "ws_planexec_materialized",
            "op": {
                "kind": "read_file",
                "path": "README.md",
                "expected_data_classes": ["candidate.artifact"]
            }
        }
    }));
    released["return"] = json!(["workspace", "release", "file"]);
    let mut host = RecordingPlanHost::default();
    let error = package
        .execute_plan_document_with_capability(
            &released,
            &plan_execution_context(),
            &workspace_query_capability(&["read_file"], &["candidate.artifact"]),
            &mut host,
        )
        .unwrap_err();
    assert_eq!(
        host.calls,
        vec!["workspace_materialize", "workspace_release"]
    );
    assert!(
        error
            .to_string()
            .contains("workspace_query refused already released workspace"),
        "unexpected error: {error:?}"
    );

    let mut mismatch = workspace_materialize_query_plan();
    mismatch["ops"][1]["name"] = json!("wrong_kind");
    let mut host = RecordingPlanHost::default();
    let error = package
        .execute_plan_document_with_capability(
            &mismatch,
            &plan_execution_context(),
            &workspace_query_capability(
                &[
                    "read_file",
                    "list",
                    "stat",
                    "digest",
                    "snapshot",
                    "git_log",
                    "git_diff",
                    "git_status",
                    "capture_artifacts",
                ],
                &["candidate.artifact"],
            ),
            &mut host,
        )
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("host returned `workspace_listing` instead of `workspace_file`"),
        "unexpected error: {error:?}"
    );
}

#[test]
fn workspace_query_rejects_file_listing_stat_and_digest_result_mismatches() {
    let package = package();
    assert_file_listing_stat_workspace_query_mismatches(&package);
    assert_digest_workspace_query_mismatches(&package);
}

fn assert_file_listing_stat_workspace_query_mismatches(package: &PublicSeamPackage) {
    for (name, op, expected) in [
        (
            "read_file_wrong_path",
            json!({
                "kind": "read_file",
                "path": "README.md",
                "expected_data_classes": ["candidate.artifact"]
            }),
            "read_file result path `src/lib.rs` does not match requested `README.md`",
        ),
        (
            "read_file_missing_body",
            json!({
                "kind": "read_file",
                "path": "README.md",
                "expected_data_classes": ["candidate.artifact"]
            }),
            "read_file result must carry content or blob_ref",
        ),
        (
            "read_file_unsafe_result_path",
            json!({
                "kind": "read_file",
                "path": "README.md",
                "expected_data_classes": ["candidate.artifact"]
            }),
            "read_file result path must be a relative workspace path without traversal",
        ),
        (
            "list_outside_path",
            json!({
                "kind": "list",
                "path": "src"
            }),
            "list result path `README.md` is outside requested `src`",
        ),
        (
            "list_unsafe_request_path",
            json!({
                "kind": "list",
                "path": "src/../README.md"
            }),
            "list path must be a relative workspace path without traversal",
        ),
        (
            "stat_wrong_kind",
            json!({
                "kind": "stat",
                "path": "README.md"
            }),
            "host returned `workspace_file` instead of `workspace_listing`",
        ),
        (
            "digest_wrong_kind",
            json!({
                "kind": "digest",
                "path": "README.md",
                "algorithm": "sha256"
            }),
            "host returned `workspace_file` instead of `workspace_snapshot`",
        ),
        (
            "git_log_wrong_kind",
            json!({
                "kind": "git_log",
                "max_entries": 5
            }),
            "host returned `workspace_listing` instead of `workspace_diff`",
        ),
        (
            "stat_wrong_path",
            json!({
                "kind": "stat",
                "path": "README.md"
            }),
            "stat result path `src/lib.rs` does not match requested `README.md`",
        ),
        (
            "stat_multi_entry",
            json!({
                "kind": "stat",
                "path": "README.md"
            }),
            "stat result must carry exactly one listing entry",
        ),
    ] {
        assert_workspace_query_mismatch(package, name, op, expected);
    }
}

fn assert_digest_workspace_query_mismatches(package: &PublicSeamPackage) {
    for (name, op, expected) in [
        (
            "digest_wrong_algorithm",
            json!({
                "kind": "digest",
                "path": "README.md",
                "algorithm": "sha256"
            }),
            "digest result `blake3:readme` does not match requested algorithm `sha256`",
        ),
        (
            "digest_wrong_workspace",
            json!({
                "kind": "digest",
                "path": "README.md",
                "algorithm": "sha256"
            }),
            "digest result workspace `ws_planexec_other` does not match requested `ws_planexec_materialized`",
        ),
        (
            "digest_wrong_source_path",
            json!({
                "kind": "digest",
                "path": "README.md",
                "algorithm": "sha256"
            }),
            "digest result source_refs must include external `leaven.workspace.path` id `README.md`",
        ),
    ] {
        assert_workspace_query_mismatch(package, name, op, expected);
    }
}

#[test]
fn workspace_query_rejects_snapshot_git_and_artifact_result_mismatches() {
    let package = package();
    for (name, op, expected) in [
        (
            "snapshot_wrong_workspace",
            json!({
                "kind": "snapshot"
            }),
            "snapshot result workspace `ws_planexec_other` does not match requested `ws_planexec_materialized`",
        ),
        (
            "snapshot_missing_digest",
            json!({
                "kind": "snapshot"
            }),
            "snapshot result must carry digest",
        ),
        (
            "git_log_missing_body",
            json!({
                "kind": "git_log",
                "max_entries": 5
            }),
            "git_log result must carry text or blob_ref",
        ),
        (
            "git_log_wrong_source_limit",
            json!({
                "kind": "git_log",
                "max_entries": 5
            }),
            "git_log result source_refs must include external `leaven.workspace.git_log.max_entries` id `5`",
        ),
        (
            "git_diff_missing_body",
            json!({
                "kind": "git_diff",
                "against": "seed"
            }),
            "git_diff result must carry text or blob_ref",
        ),
        (
            "git_diff_wrong_source_against",
            json!({
                "kind": "git_diff",
                "against": "seed"
            }),
            "git_diff result source_refs must include external `leaven.workspace.git_diff.against` id `seed`",
        ),
        (
            "git_status_missing_body",
            json!({
                "kind": "git_status"
            }),
            "git_status result must carry text or blob_ref",
        ),
        (
            "git_status_wrong_source_porcelain",
            json!({
                "kind": "git_status",
                "porcelain": true
            }),
            "git_status result source_refs must include external `leaven.workspace.git_status.porcelain` id `true`",
        ),
        (
            "capture_unrequested_path",
            json!({
                "kind": "capture_artifacts",
                "paths": ["README.md"]
            }),
            "capture_artifacts result path `src/lib.rs` was not requested",
        ),
        (
            "capture_unsafe_request_path",
            json!({
                "kind": "capture_artifacts",
                "paths": ["src/../README.md"]
            }),
            "capture_artifacts path must be a relative workspace path without traversal",
        ),
        (
            "capture_unsafe_result_path",
            json!({
                "kind": "capture_artifacts",
                "paths": ["README.md"]
            }),
            "capture_artifacts entry path must be a relative workspace path without traversal",
        ),
        (
            "capture_empty_paths",
            json!({
                "kind": "capture_artifacts",
                "paths": []
            }),
            "capture_artifacts must request at least one path",
        ),
    ] {
        assert_workspace_query_mismatch(&package, name, op, expected);
    }
}

fn assert_workspace_query_mismatch(
    package: &PublicSeamPackage,
    name: &str,
    op: Value,
    expected: &str,
) {
    let mut mismatch = workspace_materialize_query_plan();
    mismatch["ops"]
        .as_array_mut()
        .unwrap()
        .push(workspace_query_let_op(
            name,
            json!("ws_planexec_materialized"),
            op,
        ));
    mismatch["return"] = json!(["workspace", name]);
    let mut host = RecordingPlanHost::default();
    let error = package
        .execute_plan_document_with_capability(
            &mismatch,
            &plan_execution_context(),
            &workspace_query_capability(
                &[
                    "read_file",
                    "list",
                    "stat",
                    "digest",
                    "snapshot",
                    "git_log",
                    "git_diff",
                    "git_status",
                    "capture_artifacts",
                ],
                &["candidate.artifact"],
            ),
            &mut host,
        )
        .unwrap_err();
    assert!(
        error.to_string().contains(expected),
        "unexpected error for {name}: {error:?}"
    );
}

#[test]
fn agent_run_lowering_preserves_json_schema_output_contract() {
    let package = package();
    let mut call = agent_run_call();
    call["output"] = agent_status_output_contract();
    let plan = agent_run_workspace_plan(&call);
    let mut host = RecordingPlanHost::default();

    let report = package
        .execute_plan_document_with_capability(
            &plan,
            &plan_execution_context(),
            &effect_execution_capability_for_plan(&plan),
            &mut host,
        )
        .unwrap();

    assert_eq!(host.calls, vec!["workspace_materialize", "agent"]);
    assert_eq!(
        report.value()["values"]["completion"]["parsed"],
        json!({"status": "ok"})
    );

    let mut missing_parsed_host = RecordingPlanHost {
        structured_parsed: StructuredParsedFixture::Omit,
        ..RecordingPlanHost::default()
    };
    let error = package
        .execute_plan_document_with_capability(
            &plan,
            &plan_execution_context(),
            &effect_execution_capability_for_plan(&plan),
            &mut missing_parsed_host,
        )
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("agent_run json_schema output must return parsed result payload"),
        "unexpected error: {error:?}"
    );

    let mut missing_inline_schema = plan.clone();
    missing_inline_schema["ops"][1]["call"]["output"]
        .as_object_mut()
        .unwrap()
        .remove("schema");
    let mut host = RecordingPlanHost::default();
    let error = package
        .execute_plan_document_with_capability(
            &missing_inline_schema,
            &plan_execution_context(),
            &effect_execution_capability_for_plan(&missing_inline_schema),
            &mut host,
        )
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("agent_run json_schema output must carry inline schema"),
        "unexpected error: {error:?}"
    );
    assert_eq!(host.calls, vec!["workspace_materialize"]);

    let mut mismatched_fingerprint = plan.clone();
    mismatched_fingerprint["ops"][1]["call"]["output"]["schema_fingerprint"] =
        json!("fp_schema_sha256_0000000000000000000000000000000000000000000000000000000000000000");
    let mut host = RecordingPlanHost::default();
    let error = package
        .execute_plan_document_with_capability(
            &mismatched_fingerprint,
            &plan_execution_context(),
            &effect_execution_capability_for_plan(&mismatched_fingerprint),
            &mut host,
        )
        .unwrap_err();
    assert!(
        error.to_string().contains(
            "agent_run json_schema output schema_fingerprint does not match inline schema"
        ),
        "unexpected error: {error:?}"
    );
    assert_eq!(host.calls, vec!["workspace_materialize"]);

    let mut invalid_parsed_host = RecordingPlanHost {
        structured_parsed: StructuredParsedFixture::Invalid,
        ..RecordingPlanHost::default()
    };
    let error = package
        .execute_plan_document_with_capability(
            &plan,
            &plan_execution_context(),
            &effect_execution_capability_for_plan(&plan),
            &mut invalid_parsed_host,
        )
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("agent_run parsed result payload failed json_schema output contract"),
        "unexpected error: {error:?}"
    );
    assert_eq!(
        invalid_parsed_host.calls,
        vec!["workspace_materialize", "agent"]
    );

    assert_agent_json_schema_result_rejects_forgery(&package, &plan, report.value());
}

fn assert_agent_json_schema_result_rejects_forgery(
    package: &PublicSeamPackage,
    plan: &Value,
    result: &Value,
) {
    let mut forged = result.clone();
    forged["values"]["completion"]
        .as_object_mut()
        .unwrap()
        .remove("parsed");
    rebind_call_result_hash(&mut forged, 1, "completion");
    assert_plan_execution_result_rejected(package, plan, &plan_execution_context(), &forged);

    let mut forged_invalid = result.clone();
    forged_invalid["values"]["completion"]["parsed"] = json!(["not", "an", "object"]);
    rebind_call_result_hash(&mut forged_invalid, 1, "completion");
    assert_plan_execution_result_rejected(
        package,
        plan,
        &plan_execution_context(),
        &forged_invalid,
    );
}

#[test]
fn agent_run_lowering_preserves_workspace_diff_surface_fingerprint() {
    let package = package();
    let mut call = agent_run_call();
    call["output"] = json!({
        "kind": "workspace_diff",
        "surface_fingerprint": "fp_surface_sha256_agentdiff",
        "max_bytes": 4096
    });
    let plan = agent_run_workspace_plan(&call);
    let mut host = RecordingPlanHost::default();

    package
        .execute_plan_document_with_capability(
            &plan,
            &plan_execution_context(),
            &effect_execution_capability_for_plan(&plan),
            &mut host,
        )
        .unwrap();

    assert_eq!(host.calls, vec!["workspace_materialize", "agent"]);
}

#[test]
fn agent_run_preserves_runtime_selector_and_rejects_fingerprint_mismatch() {
    let package = package();
    let mut plan = agent_run_workspace_plan(&agent_run_call());
    plan["ops"][1]["call"]["runtime"] = json!("codex/app-server");
    let mut host = RecordingPlanHost::default();

    package
        .execute_plan_document_with_capability(
            &plan,
            &plan_execution_context(),
            &effect_execution_capability_for_plan(&plan),
            &mut host,
        )
        .unwrap();

    let mut mismatched = agent_run_workspace_plan(&agent_run_call());
    mismatched["ops"][1]["call"]["runtime_fingerprint"] =
        json!("fp_runtime_sha256_ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff");
    let mut host = RecordingPlanHost::default();
    let error = package
        .execute_plan_document_with_capability(
            &mismatched,
            &plan_execution_context(),
            &effect_execution_capability_for_plan(&mismatched),
            &mut host,
        )
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("agent_run outcome runtime_fingerprint"),
        "unexpected error: {error:?}"
    );
}

#[test]
fn agent_run_rejects_schema_valid_sessions_without_audit_facts() {
    let package = package();
    let plan = agent_run_workspace_plan(&agent_run_call());
    for (agent_audit, expected) in [
        (
            AgentAuditFixture::MissingTranscript,
            "agent_run result value must carry transcript_ref",
        ),
        (
            AgentAuditFixture::MissingCommands,
            "agent_run result value must carry at least one command record",
        ),
        (
            AgentAuditFixture::MissingCommandReceipt,
            "agent_run command receipt",
        ),
        (
            AgentAuditFixture::MissingCommandArgv,
            "agent_run command record must carry argv",
        ),
        (
            AgentAuditFixture::EmptyCommandArgv,
            "agent_run command record argv must not be empty",
        ),
        (
            AgentAuditFixture::NonStringCommandArgv,
            "agent_run command argv",
        ),
        (
            AgentAuditFixture::MissingCommandStatus,
            "agent_run command status",
        ),
        (
            AgentAuditFixture::InvalidCommandStatus,
            "agent_run command status `success` is not a V1 command status",
        ),
        (
            AgentAuditFixture::MissingCommandStdoutRef,
            "agent_run command record must carry stdout_ref",
        ),
        (
            AgentAuditFixture::MissingCommandStderrRef,
            "agent_run command record must carry stderr_ref",
        ),
        (
            AgentAuditFixture::InvalidCommandOutputRef,
            "agent_run command stdout_ref must be a blob_ref",
        ),
        (
            AgentAuditFixture::ForgedCommandReceipt,
            "agent_run command record receipt",
        ),
        (
            AgentAuditFixture::MissingCost,
            "agent_run result value must carry cost",
        ),
    ] {
        let mut result = package
            .execute_plan_document_with_capability(
                &plan,
                &plan_execution_context(),
                &effect_execution_capability_for_plan(&plan),
                &mut RecordingPlanHost::default(),
            )
            .unwrap()
            .value()
            .clone();
        apply_agent_audit_fixture(&mut result, agent_audit);
        let error = package
            .validate_plan_execution_result(&plan, &plan_execution_context(), &result)
            .unwrap_err();
        assert!(
            error.to_string().contains(expected),
            "unexpected error for {agent_audit:?}: {error:?}"
        );
    }
}

#[test]
fn sandbox_exec_rejects_schema_valid_results_without_audit_facts() {
    let package = package();
    {
        let sandbox_stream = SandboxStreamFixture::MissingStreamRefs;
        let expected = "completed sandbox_exec result value must carry stdout_ref and stderr_ref";
        let mut host = RecordingPlanHost {
            sandbox_stream,
            ..RecordingPlanHost::default()
        };
        let error = package
            .execute_plan_document_with_capability(
                &sandbox_exec_workspace_plan(),
                &plan_execution_context(),
                &effect_execution_capability_for_plan(&sandbox_exec_workspace_plan()),
                &mut host,
            )
            .unwrap_err();
        assert!(
            error.to_string().contains(expected),
            "unexpected error for {sandbox_stream:?}: {error:?}"
        );
    }

    let plan = sandbox_exec_workspace_plan();
    let mut forged = package
        .execute_plan_document_with_capability(
            &plan,
            &plan_execution_context(),
            &effect_execution_capability_for_plan(&plan),
            &mut RecordingPlanHost::default(),
        )
        .unwrap()
        .value()
        .clone();
    forged["values"]["completion"]
        .as_object_mut()
        .unwrap()
        .remove("exit_code");
    rebind_call_result_hash(&mut forged, 1, "completion");
    assert_plan_execution_result_rejected(&package, &plan, &plan_execution_context(), &forged);

    let mut no_stream_refs = package
        .execute_plan_document_with_capability(
            &plan,
            &plan_execution_context(),
            &effect_execution_capability_for_plan(&plan),
            &mut RecordingPlanHost::default(),
        )
        .unwrap()
        .value()
        .clone();
    no_stream_refs["values"]["completion"]
        .as_object_mut()
        .unwrap()
        .remove("stdout_ref");
    rebind_call_result_hash(&mut no_stream_refs, 1, "completion");
    assert_plan_execution_result_rejected(
        &package,
        &plan,
        &plan_execution_context(),
        &no_stream_refs,
    );

    assert_sandbox_exec_rejects_missing_receipt_cost(&package, &plan);

    let mut no_artifacts = package
        .execute_plan_document_with_capability(
            &plan,
            &plan_execution_context(),
            &effect_execution_capability_for_plan(&plan),
            &mut RecordingPlanHost::default(),
        )
        .unwrap()
        .value()
        .clone();
    no_artifacts["values"]["completion"]
        .as_object_mut()
        .unwrap()
        .remove("files");
    rebind_call_result_hash(&mut no_artifacts, 1, "completion");
    package
        .validate_plan_execution_result(&plan, &plan_execution_context(), &no_artifacts)
        .unwrap();
}

fn assert_sandbox_exec_rejects_missing_receipt_cost(package: &PublicSeamPackage, plan: &Value) {
    let mut missing_receipt_cost = package
        .execute_plan_document_with_capability(
            plan,
            &plan_execution_context(),
            &effect_execution_capability_for_plan(plan),
            &mut RecordingPlanHost::default(),
        )
        .unwrap()
        .value()
        .clone();
    missing_receipt_cost["receipts"][1]
        .as_object_mut()
        .unwrap()
        .remove("cost");
    assert_plan_execution_result_rejected(
        package,
        plan,
        &plan_execution_context(),
        &missing_receipt_cost,
    );
}

#[test]
fn plan_execution_modes_replay_uses_receipts_without_live_host_effects() {
    let package = package();
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
    let package = package();
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
fn plan_execution_rejects_failed_call_receipts_without_typed_error_and_charge_audit() {
    let package = package();
    let plan = execute_call_only_plan();
    let context = plan_execution_context();
    let mut host = RecordingPlanHost {
        fail_lm: true,
        ..RecordingPlanHost::default()
    };
    let report = package
        .execute_plan_document(&plan, &context, &mut host)
        .unwrap();

    let mut missing_error = report.value().clone();
    missing_error["receipts"][0]
        .as_object_mut()
        .unwrap()
        .remove("error");
    missing_error["errors"] = json!([]);
    rebind_failed_call_result_hash(&mut missing_error, 0, "completion");
    assert_plan_execution_result_error_contains(
        &package,
        &plan,
        &context,
        &missing_error,
        "failed call receipt for `completion` must carry typed PlanError",
    );

    let mut mismatched_error_receipt = report.value().clone();
    mismatched_error_receipt["receipts"][0]["error"]["receipt"] = json!("lmrec_other");
    mismatched_error_receipt["errors"][0]["receipt"] = json!("lmrec_other");
    rebind_failed_call_result_hash(&mut mismatched_error_receipt, 0, "completion");
    assert_plan_execution_result_error_contains(
        &package,
        &plan,
        &context,
        &mismatched_error_receipt,
        "failed call PlanError receipt must match call receipt",
    );

    let mut missing_top_level_error = report.value().clone();
    missing_top_level_error["errors"] = json!([]);
    assert_plan_execution_result_error_contains(
        &package,
        &plan,
        &context,
        &missing_top_level_error,
        "failed call receipt for `completion` PlanError must appear in top-level errors",
    );
}

#[test]
fn plan_ir_family_execution_rejects_known_variants_outside_representative_harness() {
    let package = package();
    let mut plan = typed_let_call_write_plan();
    plan["mode"] = json!({"kind": "execute"});
    plan["commit"] = json!({
        "kind": "graph_writes_atomic",
        "on_stale": "reject"
    });
    plan["ops"][1]["call"] = json!({
        "kind": "operator_review",
        "queue": "qa",
        "prompt": "Review Say ok",
        "input_classes": ["public"]
    });
    let mut host = RecordingPlanHost::default();

    let error = package
        .execute_plan_document(&plan, &plan_execution_context(), &mut host)
        .unwrap_err();
    assert!(
        matches!(error, PublicSeamError::ExampleValidation { .. }),
        "unexpected error: {error:?}"
    );
    assert!(host.calls.is_empty());
    assert!(host.writes.is_empty());
}

#[test]
fn lm_complete_lowering_rejects_deferred_multimodal_or_extension_content() {
    let package = package();
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
fn lm_complete_rejects_streaming_request_shapes_before_provider_call() {
    let package = package();
    for (field, value) in [
        ("stream", json!(true)),
        ("output", json!({"kind": "streaming"})),
    ] {
        let mut plan = typed_let_call_write_plan();
        plan["mode"] = json!({"kind": "execute"});
        plan["commit"] = json!({
            "kind": "graph_writes_atomic",
            "on_stale": "reject"
        });
        plan["ops"][1]["call"][field] = value;
        let mut host = RecordingPlanHost::default();

        let error = package
            .execute_plan_document(&plan, &plan_execution_context(), &mut host)
            .unwrap_err();

        assert!(
            matches!(
                error,
                PublicSeamError::ExampleValidation { .. } | PublicSeamError::InvalidPlan { .. }
            ),
            "unexpected error for {field}: {error:?}"
        );
        assert!(host.calls.is_empty());
        assert!(host.writes.is_empty());
    }
}

#[test]
fn lm_complete_rejects_schema_valid_non_final_or_oversized_responses() {
    let package = package();
    assert_lm_forged_response_fakes_rejected(&package);
    assert_lm_cost_binding_fakes_rejected(&package);
}

fn assert_lm_forged_response_fakes_rejected(package: &PublicSeamPackage) {
    let mut plan = typed_let_call_write_plan();
    plan["mode"] = json!({"kind": "execute"});
    plan["commit"] = json!({
        "kind": "graph_writes_atomic",
        "on_stale": "reject"
    });
    let report = package
        .execute_plan_document(
            &plan,
            &plan_execution_context(),
            &mut RecordingPlanHost::default(),
        )
        .unwrap();
    let mut forged = report.value().clone();
    forged["values"]["completion"]["message"]["role"] = json!("tool");
    rebind_call_result_hash(&mut forged, 0, "completion");
    assert_plan_execution_result_rejected(package, &plan, &plan_execution_context(), &forged);

    let mut forged_tool_result = report.value().clone();
    forged_tool_result["values"]["completion"]["message"]["content"] = json!([
        {
            "kind": "tool_result",
            "tool_call_id": "call_lookup_1",
            "content": "{\"hint\":\"ok\"}"
        }
    ]);
    rebind_call_result_hash(&mut forged_tool_result, 0, "completion");
    assert_plan_execution_result_rejected(
        package,
        &plan,
        &plan_execution_context(),
        &forged_tool_result,
    );

    let mut forged_extension = report.value().clone();
    forged_extension["values"]["completion"]["message"]["content"] = json!([
        {
            "kind": "extension",
            "namespace": "leaven.media",
            "op": "image_output",
            "schema_fingerprint": "fp_schema_sha256_imageoutput",
            "payload": {
                "image": "blob://image"
            }
        }
    ]);
    rebind_call_result_hash(&mut forged_extension, 0, "completion");
    assert_plan_execution_result_rejected(
        package,
        &plan,
        &plan_execution_context(),
        &forged_extension,
    );

    let mut forged_tool_metadata = report.value().clone();
    forged_tool_metadata["values"]["completion"]["message"]["tool_call_id"] =
        json!("call_lookup_1");
    rebind_call_result_hash(&mut forged_tool_metadata, 0, "completion");
    assert_plan_execution_result_rejected(
        package,
        &plan,
        &plan_execution_context(),
        &forged_tool_metadata,
    );

    let mut forged_name_metadata = report.value().clone();
    forged_name_metadata["values"]["completion"]["message"]["name"] = json!("assistant_alias");
    rebind_call_result_hash(&mut forged_name_metadata, 0, "completion");
    assert_plan_execution_result_rejected(
        package,
        &plan,
        &plan_execution_context(),
        &forged_name_metadata,
    );

    let mut max_bytes_plan = plan.clone();
    max_bytes_plan["ops"][1]["call"]["output"]["max_bytes"] = json!(2);
    let max_bytes_report = package
        .execute_plan_document(
            &max_bytes_plan,
            &plan_execution_context(),
            &mut RecordingPlanHost::default(),
        )
        .unwrap();
    let mut forged_oversized = max_bytes_report.value().clone();
    forged_oversized["values"]["completion"]["message"]["content"][0]["text"] = json!("too long");
    rebind_call_result_hash(&mut forged_oversized, 0, "completion");
    assert_plan_execution_result_rejected(
        package,
        &max_bytes_plan,
        &plan_execution_context(),
        &forged_oversized,
    );
}

fn assert_lm_cost_binding_fakes_rejected(package: &PublicSeamPackage) {
    let mut plan = typed_let_call_write_plan();
    plan["mode"] = json!({"kind": "execute"});
    plan["commit"] = json!({
        "kind": "graph_writes_atomic",
        "on_stale": "reject"
    });

    let report = package
        .execute_plan_document(
            &plan,
            &plan_execution_context(),
            &mut RecordingPlanHost::default(),
        )
        .unwrap();
    let mut missing_value_cost = report.value().clone();
    missing_value_cost["values"]["completion"]
        .as_object_mut()
        .unwrap()
        .remove("cost");
    rebind_call_result_hash(&mut missing_value_cost, 0, "completion");
    assert_plan_execution_result_rejected(
        package,
        &plan,
        &plan_execution_context(),
        &missing_value_cost,
    );

    let mut missing_receipt_cost = report.value().clone();
    missing_receipt_cost["receipts"][0]
        .as_object_mut()
        .unwrap()
        .remove("cost");
    assert_plan_execution_result_rejected(
        package,
        &plan,
        &plan_execution_context(),
        &missing_receipt_cost,
    );

    let mut mismatched_receipt_cost = report.value().clone();
    mismatched_receipt_cost["receipts"][0]["cost"] = json!({"input_tokens": 1, "output_tokens": 1});
    assert_plan_execution_result_rejected(
        package,
        &plan,
        &plan_execution_context(),
        &mismatched_receipt_cost,
    );
}

#[test]
fn lm_complete_lowering_preserves_json_schema_output_and_provider_hints() {
    let package = package();
    let mut plan = typed_let_call_write_plan();
    plan["mode"] = json!({"kind": "execute"});
    plan["commit"] = json!({
        "kind": "graph_writes_atomic",
        "on_stale": "reject"
    });
    plan["ops"][1]["call"]["output"] = lm_answer_output_contract();
    let mut host = RecordingPlanHost::default();

    let report = package
        .execute_plan_document(&plan, &plan_execution_context(), &mut host)
        .unwrap();

    assert_eq!(host.calls, vec!["completion"]);
    assert_eq!(host.writes, vec!["status"]);
    assert_eq!(
        report.value()["values"]["completion"]["parsed"],
        json!({"answer": "ok"})
    );

    let mut missing_parsed_host = RecordingPlanHost {
        structured_parsed: StructuredParsedFixture::Omit,
        ..RecordingPlanHost::default()
    };
    let error = package
        .execute_plan_document(&plan, &plan_execution_context(), &mut missing_parsed_host)
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("lm_complete json_schema output must return parsed result payload"),
        "unexpected error: {error:?}"
    );

    let mut invalid_parsed_host = RecordingPlanHost {
        structured_parsed: StructuredParsedFixture::Invalid,
        ..RecordingPlanHost::default()
    };
    let error = package
        .execute_plan_document(&plan, &plan_execution_context(), &mut invalid_parsed_host)
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("lm_complete parsed result payload failed json_schema output contract"),
        "unexpected error: {error:?}"
    );
    assert_eq!(invalid_parsed_host.calls, vec!["completion"]);

    let mut missing_inline_schema = plan.clone();
    missing_inline_schema["ops"][1]["call"]["output"]
        .as_object_mut()
        .unwrap()
        .remove("schema");
    let mut host = RecordingPlanHost::default();
    let error = package
        .execute_plan_document(&missing_inline_schema, &plan_execution_context(), &mut host)
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("lm_complete json_schema output must carry inline schema"),
        "unexpected error: {error:?}"
    );
    assert!(host.calls.is_empty());

    let mut mismatched_fingerprint = plan.clone();
    mismatched_fingerprint["ops"][1]["call"]["output"]["schema_fingerprint"] =
        json!("fp_schema_sha256_0000000000000000000000000000000000000000000000000000000000000000");
    let mut host = RecordingPlanHost::default();
    let error = package
        .execute_plan_document(
            &mismatched_fingerprint,
            &plan_execution_context(),
            &mut host,
        )
        .unwrap_err();
    assert!(
        error.to_string().contains(
            "lm_complete json_schema output schema_fingerprint does not match inline schema"
        ),
        "unexpected error: {error:?}"
    );
    assert!(host.calls.is_empty());

    let mut forged = report.value().clone();
    forged["values"]["completion"]
        .as_object_mut()
        .unwrap()
        .remove("parsed");
    rebind_call_result_hash(&mut forged, 0, "completion");
    assert_plan_execution_result_rejected(&package, &plan, &plan_execution_context(), &forged);

    let mut forged_invalid = report.value().clone();
    forged_invalid["values"]["completion"]["parsed"] = json!({"answer": 42});
    rebind_call_result_hash(&mut forged_invalid, 0, "completion");
    assert_plan_execution_result_rejected(
        &package,
        &plan,
        &plan_execution_context(),
        &forged_invalid,
    );
}

#[test]
fn plan_ir_revision_modes_preserve_explicit_bases() {
    let package = package();

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
    let package = package();

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
    let package = package();

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
    let package = package();

    let document = package
        .validate_plan_document(&submit_assessments_plan())
        .unwrap();

    assert_eq!(document.assessment_score_output_count(), 3);
    assert_eq!(document.assessment_evidence_count(), 3);
    assert_eq!(document.independent_assessment_score_output_count(), 1);
    assert_eq!(document.pairwise_assessment_score_output_count(), 1);
    assert_eq!(document.listwise_assessment_score_output_count(), 1);
    let write = document.operations()[0]
        .write()
        .expect("submit_assessments op exposes typed write facts");
    assert_eq!(write.kind(), PlanWriteKind::SubmitAssessments);
    assert_eq!(write.assessment_score_output_count(), 3);
    assert_eq!(write.assessment_evidence_count(), 3);
    assert_eq!(write.independent_assessment_score_output_count(), 1);
    assert_eq!(write.pairwise_assessment_score_output_count(), 1);
    assert_eq!(write.listwise_assessment_score_output_count(), 1);
    assert_eq!(write.assessment_score_output_values().len(), 3);
    assert_eq!(
        write.assessment_score_output_values()[0].as_json(),
        &json!({"candidate": "cand_a", "output": "independent answer"})
    );
    assert_eq!(
        write.assessment_score_output_values()[1].as_json(),
        &json!([
            {"candidate": "cand_a", "output": "answer a"},
            {"candidate": "cand_b", "output": "answer b"}
        ])
    );
    assert_eq!(write.assessment_target_values().len(), 3);
    assert_eq!(
        write.assessment_target_values()[0].as_json(),
        &json!({"case": "case_1"})
    );
    assert_eq!(
        write.assessment_target_values()[1].as_json(),
        &json!({"case": "case_1"})
    );
    assert_eq!(
        write.assessment_target_values()[2].as_json(),
        &json!({"case": "case_1"})
    );
    assert_eq!(write.assessment_preference_values().len(), 1);
    assert_eq!(
        write.assessment_preference_values()[0].as_json(),
        &json!({"winner": "cand_a"})
    );
    assert_eq!(write.assessment_ranking_values().len(), 1);
    assert_eq!(
        write.assessment_ranking_values()[0].as_json(),
        &json!(["cand_a", "cand_b", "cand_c"])
    );
}

#[test]
fn submit_assessments_accepts_candidate_artifact_score_output_class() {
    let package = package();
    let mut document = submit_assessments_plan();
    document["ops"][0]["write"]["assessments"][0]["score"]["output"]["data_classes"] =
        json!(["candidate.artifact", "public"]);
    document["ops"][0]["write"]["assessments"][0]["score"]["output"]["value"] = json!({
        "candidate": "cand_a",
        "artifact": "artifact snapshot for cand_a"
    });

    let document = package.validate_plan_document(&document).unwrap();

    assert_eq!(document.assessment_score_output_count(), 3);
}

#[test]
fn submit_assessments_accepts_evidence_source_receipts_without_duplicate_assessment_declarations() {
    let package = package();
    let mut document = submit_assessments_plan();
    document["ops"][0]["write"]["assessments"][0]
        .as_object_mut()
        .unwrap()
        .remove("read_receipts");
    document["ops"][0]["write"]["assessments"][0]["evidence"]["source_receipts"]["write"] =
        json!(["wrec_prior_assessment_context"]);

    let document = package.validate_plan_document(&document).unwrap();

    assert_eq!(document.assessment_evidence_count(), 3);
}

#[test]
fn submit_assessments_rejects_missing_or_placeholder_score_output() {
    let package = package();

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

    let mut candidate_labeled_dummy = submit_assessments_plan();
    candidate_labeled_dummy["ops"][0]["write"]["assessments"][0]["score"]["output"] = json!({
        "kind": "text",
        "summary": "dummy output only present to satisfy schema",
        "value": "dummy output only present to satisfy schema",
        "visibility": "public",
        "data_classes": ["candidate.output"]
    });
    candidate_labeled_dummy["ops"][0]["write"]["assessments"][0]["evidence"] =
        evidence_envelope("dummy output only present to satisfy schema");
    assert!(matches!(
        package
            .validate_plan_document(&candidate_labeled_dummy)
            .unwrap_err(),
        PublicSeamError::InvalidPlan { .. }
    ));

    let mut mismatched_candidate_binding = submit_assessments_plan();
    mismatched_candidate_binding["ops"][0]["write"]["assessments"][1]["score"]["output"]["value"]
        [0]["candidate"] = json!("cand_b");
    assert!(matches!(
        package
            .validate_plan_document(&mismatched_candidate_binding)
            .unwrap_err(),
        PublicSeamError::InvalidPlan { .. }
    ));
}

#[test]
fn submit_assessments_rejects_summary_only_score_output_dummies() {
    let package = package();

    for (assessment_index, data_class, summary) in [
        (
            0,
            "candidate.output",
            "dummy independent output only present to satisfy schema",
        ),
        (
            1,
            "candidate.output",
            "dummy pairwise output only present to satisfy schema",
        ),
        (
            2,
            "candidate.output",
            "dummy listwise output only present to satisfy schema",
        ),
        (
            0,
            "candidate.artifact",
            "dummy artifact output only present to satisfy schema",
        ),
    ] {
        let mut document = submit_assessments_plan();
        document["ops"][0]["write"]["assessments"][assessment_index]["score"]["output"] = json!({
            "kind": "text",
            "summary": summary,
            "visibility": "public",
            "data_classes": [data_class]
        });
        document["ops"][0]["write"]["assessments"][assessment_index]["evidence"] =
            evidence_envelope(summary);
        assert!(matches!(
            package.validate_plan_document(&document).unwrap_err(),
            PublicSeamError::InvalidPlan { .. }
        ));
    }
}

#[test]
fn submit_assessments_rejects_missing_assessment_score_or_replayability() {
    let package = package();

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

#[test]
fn submit_assessments_rejects_schema_valid_but_semantically_invalid_evidence() {
    let package = package();

    let mut no_source_receipts = submit_assessments_plan();
    no_source_receipts["ops"][0]["write"]["assessments"][0]["evidence"]["source_receipts"]["read"] =
        json!([]);
    let error = package
        .validate_plan_document(&no_source_receipts)
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("evidence source_receipts must carry at least one receipt")
    );

    let mut hidden_target = submit_assessments_plan();
    hidden_target["ops"][0]["write"]["assessments"][0]["evidence"]["public"]["data_classes"] =
        json!(["case.target"]);
    let error = package.validate_plan_document(&hidden_target).unwrap_err();
    assert!(error.to_string().contains("target_derived=true"));

    let mut target_without_classes = submit_assessments_plan();
    target_without_classes["ops"][0]["write"]["assessments"][0]["evidence"]["target_derived"] =
        json!(true);
    target_without_classes["ops"][0]["write"]["assessments"][0]["evidence"]["data_classes"] =
        json!(["public"]);
    let error = package
        .validate_plan_document(&target_without_classes)
        .unwrap_err();
    assert!(error.to_string().contains("target-derived evidence"));

    let mut wrong_source_receipt_family = submit_assessments_plan();
    wrong_source_receipt_family["ops"][0]["write"]["assessments"][0]["evidence"]["source_receipts"]
        ["effect"] = json!(["qrec_not_an_effect"]);
    let error = package
        .validate_plan_document(&wrong_source_receipt_family)
        .unwrap_err();
    assert!(
        error.to_string().contains("source_receipts.effect"),
        "{error}"
    );
}

fn typed_prompt_literal() -> Value {
    json!({
        "messages": [
            {"role": "user", "content": ["hello", {"topic": "typing"}]}
        ],
        "temperature": 0.2
    })
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
                    "value": typed_prompt_literal(),
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
            "prompt_cache_key": "planexec-stable"
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

fn typed_let_call_write_execute_plan() -> Value {
    let mut plan = typed_let_call_write_plan();
    plan["mode"] = json!({"kind": "execute"});
    plan["commit"] = json!({
        "kind": "graph_writes_atomic",
        "on_stale": "reject"
    });
    plan
}

fn emit_event_from_literal_dependency_plan() -> Value {
    let mut plan = typed_let_call_write_execute_plan();
    plan["ops"].as_array_mut().unwrap().remove(1);
    plan["ops"][1]["deps"] = json!(["prompt"]);
    plan["return"] = json!(["status"]);
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

fn workspace_materialize_release_plan() -> Value {
    json!({
        "schema_version": "leaven.plan.v1",
        "plan_id": "planworkspace001",
        "consistency": {
            "kind": "latest_at_start"
        },
        "mode": {
            "kind": "execute"
        },
        "ops": [
            {
                "kind": "call",
                "name": "workspace",
                "idempotency_key": "plan-workspace-0001",
                "call": {
                    "kind": "workspace_materialize",
                    "candidate": "cand_planexec",
                    "surface": "program",
                    "mode": "copy_on_write",
                    "lifetime": "manual_release"
                }
            },
            {
                "kind": "call",
                "name": "release",
                "deps": ["workspace"],
                "idempotency_key": "plan-workspace-0002",
                "call": {
                    "kind": "workspace_release",
                    "workspace": "ws_planexec_materialized",
                    "force": false
                }
            }
        ],
        "return": ["workspace", "release"],
        "commit": {
            "kind": "graph_writes_atomic",
            "on_stale": "reject"
        }
    })
}

fn workspace_materialize_only_plan() -> Value {
    let mut plan = workspace_materialize_release_plan();
    plan["ops"].as_array_mut().unwrap().truncate(1);
    plan["return"] = json!(["workspace"]);
    plan
}

fn forged_workspace_handle_call_plan(plan_id: &str, call: &Value) -> Value {
    json!({
        "schema_version": "leaven.plan.v1",
        "plan_id": plan_id,
        "consistency": {
            "kind": "latest_at_start"
        },
        "mode": {
            "kind": "execute"
        },
        "ops": [
            forged_workspace_handle_let_op(),
            {
                "kind": "call",
                "name": "completion",
                "deps": ["forged"],
                "idempotency_key": format!("{plan_id}-call"),
                "call": call
            }
        ],
        "return": ["completion"],
        "commit": {
            "kind": "graph_writes_atomic",
            "on_stale": "reject"
        }
    })
}

fn forged_workspace_handle_query_plan() -> Value {
    json!({
        "schema_version": "leaven.plan.v1",
        "plan_id": "planforgedquery001",
        "consistency": {
            "kind": "latest_at_start"
        },
        "mode": {
            "kind": "execute"
        },
        "ops": [
            forged_workspace_handle_let_op(),
            {
                "kind": "let",
                "name": "file",
                "deps": ["forged"],
                "expr": {
                    "kind": "workspace_query",
                    "workspace": "ws_planexec_materialized",
                    "op": {
                        "kind": "read_file",
                        "path": "README.md",
                        "expected_data_classes": ["candidate.artifact"]
                    }
                }
            }
        ],
        "return": ["file"],
        "commit": {
            "kind": "graph_writes_atomic",
            "on_stale": "reject"
        }
    })
}

fn forged_workspace_handle_let_op() -> Value {
    json!({
        "kind": "let",
        "name": "forged",
        "expr": {
            "kind": "literal",
            "value": {
                "kind": "workspace_handle",
                "workspace": "ws_planexec_materialized",
                "lifetime": "manual_release",
                "released": false,
                "graph_revision": "rev_base",
                "data_classes": ["public"],
                "replayability": "boundary_managed",
                "receipt": "wrec_forged"
            }
        }
    })
}

fn workspace_materialize_query_plan() -> Value {
    json!({
        "schema_version": "leaven.plan.v1",
        "plan_id": "planworkspacequery001",
        "consistency": {
            "kind": "latest_at_start"
        },
        "mode": {
            "kind": "execute"
        },
        "ops": workspace_query_plan_ops(),
        "return": [
            "workspace",
            "file",
            "listing",
            "stat",
            "digest",
            "snapshot",
            "log",
            "diff",
            "status",
            "captured"
        ],
        "commit": {
            "kind": "graph_writes_atomic",
            "on_stale": "reject"
        }
    })
}

fn execute_workspace_query_plan(
    package: &PublicSeamPackage,
    plan: &Value,
    host: &mut RecordingPlanHost,
) -> leaven_public_seam::PlanExecutionReport {
    package
        .execute_plan_document_with_capability(
            plan,
            &plan_execution_context(),
            &workspace_query_capability(
                &[
                    "read_file",
                    "list",
                    "stat",
                    "digest",
                    "snapshot",
                    "git_log",
                    "git_diff",
                    "git_status",
                    "capture_artifacts",
                ],
                &["candidate.artifact"],
            ),
            host,
        )
        .unwrap()
}

fn workspace_query_plan_ops() -> Vec<Value> {
    let workspace = json!("ws_planexec_materialized");
    vec![
        workspace_materialize_call_op("plan-workspace-query-0001"),
        workspace_query_let_op(
            "file",
            workspace.clone(),
            json!({
                "kind": "read_file",
                "path": "README.md",
                "expected_data_classes": ["candidate.artifact"]
            }),
        ),
        workspace_query_let_op(
            "listing",
            workspace.clone(),
            json!({"kind": "list", "path": ".", "recursive": false, "max_entries": 10}),
        ),
        workspace_query_let_op(
            "stat",
            workspace.clone(),
            json!({"kind": "stat", "path": "README.md"}),
        ),
        workspace_query_let_op(
            "digest",
            workspace.clone(),
            json!({"kind": "digest", "path": "README.md", "algorithm": "sha256"}),
        ),
        workspace_query_let_op("snapshot", workspace.clone(), json!({"kind": "snapshot"})),
        workspace_query_let_op(
            "log",
            workspace.clone(),
            json!({"kind": "git_log", "max_entries": 5}),
        ),
        workspace_query_let_op(
            "diff",
            workspace.clone(),
            json!({"kind": "git_diff", "against": "seed", "max_bytes": 4096}),
        ),
        workspace_query_let_op(
            "status",
            workspace.clone(),
            json!({"kind": "git_status", "porcelain": true}),
        ),
        workspace_query_let_op(
            "captured",
            workspace,
            json!({"kind": "capture_artifacts", "paths": ["README.md"], "max_bytes": 4096}),
        ),
    ]
}

fn workspace_materialize_call_op(idempotency_key: &str) -> Value {
    json!({
        "kind": "call",
        "name": "workspace",
        "idempotency_key": idempotency_key,
        "call": {
            "kind": "workspace_materialize",
            "candidate": "cand_planexec",
            "surface": "program",
            "mode": "copy_on_write",
            "lifetime": "manual_release"
        }
    })
}

fn agent_run_workspace_plan(call: &Value) -> Value {
    json!({
        "schema_version": "leaven.plan.v1",
        "plan_id": "planagentworkspace001",
        "consistency": {
            "kind": "latest_at_start"
        },
        "mode": {
            "kind": "execute"
        },
        "ops": [
            workspace_materialize_call_op("plan-agent-workspace-0001"),
            {
                "kind": "call",
                "name": "completion",
                "deps": ["workspace"],
                "idempotency_key": "plan-agent-workspace-0002",
                "call": call
            }
        ],
        "return": ["workspace", "completion"],
        "commit": {
            "kind": "graph_writes_atomic",
            "on_stale": "reject"
        }
    })
}

fn sandbox_exec_workspace_plan() -> Value {
    json!({
        "schema_version": "leaven.plan.v1",
        "plan_id": "plansandboxworkspace001",
        "consistency": {
            "kind": "latest_at_start"
        },
        "mode": {
            "kind": "execute"
        },
        "ops": [
            workspace_materialize_call_op("plan-sandbox-workspace-0001"),
            {
                "kind": "call",
                "name": "completion",
                "deps": ["workspace"],
                "idempotency_key": "plan-sandbox-workspace-0002",
                "call": sandbox_exec_call()
            }
        ],
        "return": ["workspace", "completion"],
        "commit": {
            "kind": "graph_writes_atomic",
            "on_stale": "reject"
        }
    })
}

fn workspace_query_let_op(
    name: &str,
    workspace: impl serde::Serialize,
    op: impl serde::Serialize,
) -> Value {
    json!({
        "kind": "let",
        "name": name,
        "deps": ["workspace"],
        "expr": {
            "kind": "workspace_query",
            "workspace": workspace,
            "op": op
        }
    })
}

fn agent_run_call() -> Value {
    json!({
        "kind": "agent_run",
        "runtime": "codex",
        "workspace": "ws_planexec_materialized",
        "instructions": {
            "system": "Stay within the workspace.",
            "task": "Inspect the plan output."
        },
        "tool_policy": {
            "allow_shell": false,
            "allowed_tools": ["read_file"]
        },
        "output": {
            "kind": "final_message",
            "max_bytes": 1024
        },
        "limits": {
            "timeout_s": 30,
            "max_turns": 4,
            "max_usd_micro": 1000
        },
        "input_classes": ["public"]
    })
}

fn sandbox_exec_call() -> Value {
    json!({
        "kind": "sandbox_exec",
        "workspace": "ws_planexec_materialized",
        "argv": ["python", "-c", "print('ok')"],
        "cwd": "work",
        "env": {
            "LEAVEN_CASE": "case_1"
        },
        "timeout_s": 1,
        "output": {
            "kind": "files",
            "paths": ["out.txt"],
            "max_bytes": 1024
        },
        "stream_policy": "buffer",
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
                                    "kind": "structured",
                                    "value": [
                                        {"candidate": "cand_a", "output": "answer a"},
                                        {"candidate": "cand_b", "output": "answer b"}
                                    ],
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

fn request_evaluation_plan() -> Value {
    json!({
        "schema_version": "leaven.plan.v1",
        "plan_id": "planevalrequest001",
        "consistency": {
            "kind": "latest_at_start"
        },
        "mode": {
            "kind": "dry_run"
        },
        "ops": [
            {
                "kind": "write",
                "name": "evaluation",
                "idempotency_key": "evaluation-request-0001",
                "write": {
                    "kind": "request_evaluation",
                    "request": {
                        "shape": "independent",
                        "candidates": ["cand_a"],
                        "set": {
                            "kind": "named",
                            "name": "validation"
                        },
                        "granularity": "per_case",
                        "purpose": "validation",
                        "evaluator": "eval_score_v1"
                    }
                }
            }
        ],
        "return": ["evaluation"],
        "commit": {
            "kind": "no_graph_writes"
        }
    })
}

fn score_with_output(summary: &'static str) -> Value {
    json!({
        "value": 1.0,
        "output": {
            "kind": "structured",
            "summary": summary,
            "value": {
                "candidate": "cand_a",
                "output": summary
            },
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
            "read": ["qrec_score_output_source"],
            "effect": []
        }
    })
}

#[derive(Default)]
struct RecordingPlanHost {
    graph_reads: Vec<String>,
    graph_read_plan_ids: Vec<String>,
    run_context_event_filter_reads: usize,
    case_reads: Vec<String>,
    calls: Vec<&'static str>,
    cached_calls: Vec<&'static str>,
    writes: Vec<&'static str>,
    replayed_receipts: Vec<String>,
    call_deps: BTreeMap<String, Value>,
    write_deps: BTreeMap<String, Value>,
    write_dependency_data_classes: BTreeSet<String>,
    workspaces: BTreeSet<String>,
    cached_hit: bool,
    fail_lm: bool,
    structured_parsed: StructuredParsedFixture,
    sandbox_stream: SandboxStreamFixture,
}

#[derive(Clone, Copy, Default)]
enum StructuredParsedFixture {
    #[default]
    Valid,
    Omit,
    Invalid,
}

fn agent_command_fixture(agent_audit: AgentAuditFixture) -> Value {
    let mut command = match agent_audit {
        AgentAuditFixture::MissingCommandReceipt => json!({
            "argv": ["codex"],
            "status": "completed"
        }),
        AgentAuditFixture::MissingCommandArgv => json!({
            "status": "completed",
            "receipt": "agentrec_completion"
        }),
        AgentAuditFixture::EmptyCommandArgv => json!({
            "argv": [],
            "status": "completed",
            "receipt": "agentrec_completion"
        }),
        AgentAuditFixture::NonStringCommandArgv => json!({
            "argv": ["codex", 42],
            "status": "completed",
            "receipt": "agentrec_completion"
        }),
        AgentAuditFixture::MissingCommandStatus => json!({
            "argv": ["codex"],
            "receipt": "agentrec_completion"
        }),
        AgentAuditFixture::InvalidCommandStatus => json!({
            "argv": ["codex"],
            "status": "success",
            "receipt": "agentrec_completion"
        }),
        AgentAuditFixture::ForgedCommandReceipt => json!({
            "argv": ["codex"],
            "status": "completed",
            "receipt": "agentrec_forged"
        }),
        _ => json!({
            "argv": ["codex"],
            "status": "completed",
            "receipt": "agentrec_completion"
        }),
    };
    if !matches!(
        agent_audit,
        AgentAuditFixture::MissingCommandArgv
            | AgentAuditFixture::EmptyCommandArgv
            | AgentAuditFixture::NonStringCommandArgv
            | AgentAuditFixture::MissingCommandStatus
            | AgentAuditFixture::InvalidCommandStatus
    ) {
        command["stdout_ref"] = agent_command_blob_ref("blob_agent_stdout");
        command["stderr_ref"] = agent_command_blob_ref("blob_agent_stderr");
    }
    match agent_audit {
        AgentAuditFixture::MissingCommandStdoutRef => {
            command.as_object_mut().unwrap().remove("stdout_ref");
        }
        AgentAuditFixture::MissingCommandStderrRef => {
            command.as_object_mut().unwrap().remove("stderr_ref");
        }
        AgentAuditFixture::InvalidCommandOutputRef => {
            command["stdout_ref"] = json!({"kind": "not_blob_ref"});
        }
        _ => {}
    }
    command
}

fn apply_agent_audit_fixture(result: &mut Value, agent_audit: AgentAuditFixture) {
    match agent_audit {
        AgentAuditFixture::MissingTranscript => {
            result["values"]["completion"]
                .as_object_mut()
                .unwrap()
                .remove("transcript_ref");
        }
        AgentAuditFixture::MissingCommands => {
            result["values"]["completion"]["commands"] = json!([]);
        }
        AgentAuditFixture::MissingCost => {
            result["values"]["completion"]
                .as_object_mut()
                .unwrap()
                .remove("cost");
            result["receipts"][1]
                .as_object_mut()
                .unwrap()
                .remove("cost");
        }
        other => {
            result["values"]["completion"]["commands"] = json!([agent_command_fixture(other)]);
        }
    }
    rebind_call_result_hash(result, 1, "completion");
}

fn agent_command_blob_ref(id: &'static str) -> Value {
    json!({
        "kind": "blob_ref",
        "id": id,
        "sha256": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        "bytes": 0,
        "data_classes": ["transcript.raw"]
    })
}

fn literal_agent_session_with_command_output_classes() -> Value {
    json!({
        "kind": "agent_session",
        "session": "ags_literalseam",
        "status": "succeeded",
        "transcript_ref": agent_command_blob_ref("blob_literal_agent_transcript"),
        "commands": [
            {
                "argv": ["codex"],
                "status": "completed",
                "receipt": "agentrec_literal_command",
                "stdout_ref": agent_command_blob_ref("blob_literal_agent_stdout"),
                "stderr_ref": agent_command_blob_ref("blob_literal_agent_stderr")
            }
        ],
        "graph_revision": "rev_planexec_base",
        "data_classes": ["public"],
        "replayability": "has_declared_external_effects",
        "receipt": "agentrec_literal_session",
        "cost": {"usd_micro": 1}
    })
}

fn literal_assessment_graph_set() -> Value {
    json!({
        "kind": "graph_set",
        "items": [
            {
                "kind": "assessment_summary",
                "assessment": "assess_literal_dependency",
                "score": {
                    "value": 1.0,
                    "output": {
                        "kind": "text",
                        "summary": "candidate answer",
                        "value": "candidate answer",
                        "visibility": "public",
                        "data_classes": ["candidate.output"]
                    }
                },
                "evidence": evidence_envelope("candidate answer")
            }
        ],
        "graph_revision": "rev_planexec_base",
        "data_classes": ["public"],
        "replayability": "pure_read",
        "receipt": "qrec_literal_assessment"
    })
}

fn literal_workspace_listing_with_entry_classes() -> Value {
    json!({
        "kind": "workspace_listing",
        "entries": [
            {
                "path": "secrets.txt",
                "kind": "file",
                "bytes": 12,
                "data_classes": ["workspace.secret"]
            }
        ],
        "graph_revision": "rev_planexec_base",
        "data_classes": ["public"],
        "replayability": "pure_read",
        "receipt": "qrec_literal_listing"
    })
}

fn agent_session() -> leaven_kernel::Metered<leaven_agent::AgentSession> {
    let mut session = leaven_agent::AgentSession::succeeded(leaven_kernel::AgentSessionId::new());
    let mut command = leaven_workspace::Command::new("codex");
    command.args = vec!["exec".to_owned(), "--json".to_owned()];
    session.commands.push(leaven_agent::CommandRecord {
        command,
        output: leaven_workspace::CommandOutput::new(
            leaven_workspace::ExitStatus { code: Some(0) },
            leaven_workspace::CapturedOutput::empty(),
            leaven_workspace::CapturedOutput::empty(),
            std::time::Duration::from_millis(10),
        ),
    });
    leaven_kernel::Metered::new(
        session,
        leaven_kernel::Cost::custom("usd_micro", 1000.0).unwrap(),
    )
}

fn lm_response(text: &str) -> leaven_kernel::Metered<leaven_lm::LmResponse> {
    let usage = leaven_lm::TokenUsage {
        input_tokens: 7,
        output_tokens: 3,
        ..leaven_lm::TokenUsage::default()
    };
    leaven_kernel::Metered::new(
        leaven_lm::LmResponse::new(leaven_lm::Message::assistant(text), usage).unwrap(),
        leaven_kernel::Cost {
            llm_calls: 1,
            prompt_tokens: 7,
            completion_tokens: 3,
            ..leaven_kernel::Cost::zero()
        },
    )
}

fn agent_command_output_refs() -> AgentCommandOutputRefs {
    AgentCommandOutputRefs::new(
        agent_command_blob_ref("blob_agent_stdout"),
        agent_command_blob_ref("blob_agent_stderr"),
    )
}

#[derive(Clone, Copy, Debug, Default)]
enum AgentAuditFixture {
    #[default]
    Complete,
    MissingTranscript,
    MissingCommands,
    MissingCommandReceipt,
    MissingCommandArgv,
    EmptyCommandArgv,
    NonStringCommandArgv,
    MissingCommandStatus,
    InvalidCommandStatus,
    MissingCommandStdoutRef,
    MissingCommandStderrRef,
    InvalidCommandOutputRef,
    ForgedCommandReceipt,
    MissingCost,
}

#[derive(Clone, Copy, Debug, Default)]
enum SandboxStreamFixture {
    #[default]
    Buffered,
    BlobRefsOnly,
    MissingBlobRefs,
    MissingStreamRefs,
    NonPublicBlobDataClasses,
}

impl SandboxStreamFixture {
    fn expected_policy(self) -> &'static str {
        match self {
            Self::Buffered | Self::MissingStreamRefs | Self::NonPublicBlobDataClasses => "buffer",
            Self::BlobRefsOnly | Self::MissingBlobRefs => "blob_refs_only",
        }
    }

    fn includes_stream_refs(self) -> bool {
        match self {
            Self::Buffered | Self::BlobRefsOnly | Self::NonPublicBlobDataClasses => true,
            Self::MissingBlobRefs | Self::MissingStreamRefs => false,
        }
    }
}

#[allow(
    clippy::unnecessary_wraps,
    reason = "test fixture implements fallible PlanExecutionHost methods even when a specific branch cannot fail"
)]
impl PlanExecutionHost for RecordingPlanHost {
    fn graph_query(
        &mut self,
        request: PlanGraphQueryRequest<'_>,
    ) -> Result<PlanGraphQueryOutcome, PublicSeamError> {
        assert_eq!(request.name(), "events");
        assert_eq!(request.expr()["kind"].as_str(), Some("graph_query"));
        self.graph_read_plan_ids.push(request.plan_id().to_owned());
        if matches!(
            request.source(),
            leaven_public_seam::PlanGraphQuerySource::Events {
                filter: Some(PlanGraphEventFilter::RunContext),
                ..
            }
        ) {
            self.run_context_event_filter_reads += 1;
        }
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
            .with_target(PlanCaseTargetValue::from_json(
                json!({"answer": "expected"}),
            ))
            .with_data_classes(["case.target".to_owned()]))
    }

    fn lm_complete(
        &mut self,
        request: PlanLmCompleteRequest<'_>,
    ) -> Result<PlanLmCompleteOutcome, PublicSeamError> {
        assert_eq!(request.name(), "completion");
        let lm_request = request.lm_request();
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
            lm_request.provider_hints.prompt_cache_key.as_deref(),
            Some("planexec-stable")
        );
        assert!(lm_request.provider_hints.values.is_empty());
        match &lm_request.output {
            OutputMode::FinalMessage { max_bytes } => {
                assert!(
                    matches!(*max_bytes, Some(2 | 1024)),
                    "unexpected final_message max_bytes: {max_bytes:?}"
                );
            }
            OutputMode::JsonSchema(schema) => {
                assert_eq!(schema.schema["required"], json!(["answer"]));
                assert!(schema.strict);
            }
            other => panic!("unexpected LM output mode: {other:?}"),
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
        let mut outcome = PlanLmCompleteOutcome::from_lm_response(
            lm_response("ok"),
            leaven_kernel::Fingerprint::from_bytes([0x1e; 32]),
        );
        if matches!(lm_request.output, OutputMode::JsonSchema(_)) {
            match self.structured_parsed {
                StructuredParsedFixture::Valid => {
                    outcome = outcome.with_parsed(json!({"answer": "ok"}));
                }
                StructuredParsedFixture::Invalid => {
                    outcome = outcome.with_parsed(json!({"answer": 42}));
                }
                StructuredParsedFixture::Omit => {}
            }
        }
        Ok(outcome)
    }

    fn cached_lm_complete(
        &mut self,
        request: PlanLmCompleteRequest<'_>,
    ) -> Result<Option<PlanLmCompleteOutcome>, PublicSeamError> {
        assert_eq!(request.name(), "completion");
        let lm_request = request.lm_request();
        self.cached_calls.push("completion");
        self.call_deps = request.deps().clone();
        if self.cached_hit {
            let mut outcome = PlanLmCompleteOutcome::from_lm_response(
                lm_response("cached ok"),
                leaven_kernel::Fingerprint::from_bytes([0x1e; 32]),
            );
            if matches!(lm_request.output, OutputMode::JsonSchema(_)) {
                match self.structured_parsed {
                    StructuredParsedFixture::Valid => {
                        outcome = outcome.with_parsed(json!({"answer": "cached ok"}));
                    }
                    StructuredParsedFixture::Invalid => {
                        outcome = outcome.with_parsed(json!({"answer": 42}));
                    }
                    StructuredParsedFixture::Omit => {}
                }
            }
            Ok(Some(outcome))
        } else {
            Ok(None)
        }
    }

    fn agent_run(
        &mut self,
        request: PlanAgentRunRequest<'_>,
    ) -> Result<PlanAgentRunOutcome, PublicSeamError> {
        assert_eq!(request.name(), "completion");
        assert_eq!(request.live_workspace()?, "ws_planexec_materialized");
        assert_eq!(
            request.deps()["workspace"]["workspace"].as_str(),
            Some("ws_planexec_materialized")
        );
        let agent_request = request.agent_run_request();
        assert_eq!(
            request.runtime().map(leaven_kernel::AgentRuntimeId::as_str),
            Some(agent_request.runtime.as_ref().unwrap().as_str())
        );
        assert!(matches!(
            agent_request
                .runtime
                .as_ref()
                .map(leaven_kernel::AgentRuntimeId::as_str),
            Some("codex" | "codex/app-server")
        ));
        assert_eq!(
            agent_request.instructions.system.as_deref(),
            Some("Stay within the workspace.")
        );
        assert_eq!(agent_request.instructions.task, "Inspect the plan output.");
        assert_eq!(agent_request.cwd.as_str(), "");
        assert!(!agent_request.tool_policy.allow_shell);
        assert!(
            agent_request.tool_policy.allowed_tools.is_empty()
                || agent_request.tool_policy.allowed_tools == vec!["read_file".to_owned()],
            "unexpected allowed tools: {:?}",
            agent_request.tool_policy.allowed_tools
        );
        assert!(
            agent_request.tool_policy.allowed_commands.is_empty()
                || agent_request.tool_policy.allowed_commands == vec!["python".to_owned()],
            "unexpected allowed commands: {:?}",
            agent_request.tool_policy.allowed_commands
        );
        assert_eq!(agent_request.limits.max_turns, Some(4));
        let expects_json_schema = match &agent_request.output_contract {
            leaven_agent::OutputContract::FinalMessage => false,
            leaven_agent::OutputContract::JsonSchema { schema, .. } => {
                assert_eq!(schema["type"], "object");
                true
            }
            leaven_agent::OutputContract::WorkspaceDiff {
                roots,
                surface_fingerprint,
            } => {
                assert_eq!(
                    roots.first().map(leaven_workspace::WorkspacePath::as_str),
                    Some("")
                );
                assert_eq!(
                    surface_fingerprint.as_deref(),
                    Some("fp_surface_sha256_agentdiff")
                );
                false
            }
            other => panic!("unexpected agent output contract: {other:?}"),
        };
        self.calls.push("agent");
        let mut outcome = PlanAgentRunOutcome::from_agent_session_with_command_output_refs(
            agent_session(),
            leaven_kernel::Fingerprint::from_bytes([0xa7; 32]),
            agent_transcript_blob_ref("blob_agent_transcript"),
            "agentrec_completion",
            [agent_command_output_refs()],
        )?;
        if expects_json_schema {
            match self.structured_parsed {
                StructuredParsedFixture::Valid => {
                    outcome = outcome.with_parsed(json!({"status": "ok"}));
                }
                StructuredParsedFixture::Invalid => {
                    outcome = outcome.with_parsed(json!(["not", "an", "object"]));
                }
                StructuredParsedFixture::Omit => {}
            }
        }
        Ok(outcome)
    }

    fn sandbox_exec(
        &mut self,
        request: PlanSandboxExecRequest<'_>,
    ) -> Result<PlanSandboxExecOutcome, PublicSeamError> {
        assert_eq!(request.name(), "completion");
        assert_eq!(request.live_workspace()?, "ws_planexec_materialized");
        assert_eq!(
            request.deps()["workspace"]["workspace"].as_str(),
            Some("ws_planexec_materialized")
        );
        assert_eq!(
            request.stream_policy(),
            self.sandbox_stream.expected_policy()
        );
        let command = request.workspace_command();
        assert_eq!(command.program, "python");
        assert_eq!(command.args, vec!["-c", "print('ok')"]);
        assert_eq!(
            command
                .cwd
                .as_ref()
                .map(leaven_workspace::WorkspacePath::as_str),
            Some("work")
        );
        assert_eq!(command.env["LEAVEN_CASE"], "case_1");
        assert_eq!(command.limits.timeout.unwrap().as_secs(), 1);
        assert_eq!(
            command
                .output_files
                .iter()
                .map(leaven_workspace::WorkspacePath::as_str)
                .collect::<Vec<_>>(),
            vec!["out.txt"]
        );
        assert_eq!(command.limits.max_output_file_bytes, Some(1024));
        self.calls.push("sandbox");
        if !self.sandbox_stream.includes_stream_refs() {
            return Ok(
                PlanSandboxExecOutcome::completed("fp_runtime_sha256_sandbox")
                    .with_cost(json!({"usd_micro": 10})),
            );
        }
        let output_path = leaven_workspace::WorkspacePath::new("out.txt").unwrap();
        let command_output = leaven_workspace::CommandOutput::new(
            leaven_workspace::ExitStatus { code: Some(0) },
            leaven_workspace::CapturedOutput::new(b"stdout".to_vec(), None),
            leaven_workspace::CapturedOutput::new(b"stderr".to_vec(), None),
            std::time::Duration::from_millis(10),
        )
        .with_output_file(
            output_path.clone(),
            leaven_workspace::CapturedOutput::new(b"sandbox file".to_vec(), None),
        );
        PlanSandboxExecOutcome::from_command_output_with_file_refs(
            leaven_kernel::Metered::new(
                command_output,
                leaven_kernel::Cost::custom("usd_micro", 10.0).unwrap(),
            ),
            leaven_kernel::Fingerprint::from_bytes([0x5a; 32]),
            self.sandbox_blob_ref("blob_sandbox_stdout"),
            self.sandbox_blob_ref("blob_sandbox_stderr"),
            [(
                output_path,
                self.sandbox_blob_ref("blob_sandbox_output_file"),
            )],
        )
    }

    fn workspace_materialize(
        &mut self,
        request: PlanWorkspaceMaterializeRequest<'_>,
    ) -> Result<PlanWorkspaceMaterializeOutcome, PublicSeamError> {
        assert!(matches!(
            request.name(),
            "workspace" | "workspace_path" | "workspace_bad_lifetime"
        ));
        assert_eq!(
            request.call()["kind"].as_str(),
            Some("workspace_materialize")
        );
        assert_eq!(request.candidate()?, "cand_planexec");
        assert_eq!(request.surface(), Some("program"));
        assert_eq!(request.mode()?, "copy_on_write");
        let requested_lifetime = request.lifetime()?;
        if request.name() == "workspace_bad_lifetime" {
            assert_eq!(requested_lifetime, "plan");
        } else {
            assert_eq!(requested_lifetime, "manual_release");
        }
        self.calls.push("workspace_materialize");
        if request.name() == "workspace_path" {
            return Ok(PlanWorkspaceMaterializeOutcome::new(
                "/tmp/leaven-workspace",
                requested_lifetime,
                "fp_runtime_sha256_workspace",
            ));
        }
        if request.name() == "workspace_bad_lifetime" {
            return Ok(PlanWorkspaceMaterializeOutcome::new(
                "ws_planexec_materialized",
                "manual_release",
                "fp_runtime_sha256_workspace",
            ));
        }
        self.workspaces
            .insert("ws_planexec_materialized".to_owned());
        Ok(PlanWorkspaceMaterializeOutcome::new(
            "ws_planexec_materialized",
            requested_lifetime,
            "fp_runtime_sha256_workspace",
        ))
    }

    fn workspace_release(
        &mut self,
        request: PlanWorkspaceReleaseRequest<'_>,
    ) -> Result<PlanWorkspaceReleaseOutcome, PublicSeamError> {
        assert!(matches!(
            request.name(),
            "release" | "release_wrong" | "release_bad_lifetime"
        ));
        assert_eq!(request.call()["kind"].as_str(), Some("workspace_release"));
        assert!(!request.force());
        let workspace = request.workspace()?;
        if request.name() == "release_wrong" {
            self.calls.push("workspace_release");
            return Ok(PlanWorkspaceReleaseOutcome::new(
                "ws_planexec_other",
                "manual_release",
                "fp_runtime_sha256_workspace",
            ));
        }
        if request.name() == "release_bad_lifetime" {
            self.calls.push("workspace_release");
            return Ok(PlanWorkspaceReleaseOutcome::new(
                workspace,
                "plan",
                "fp_runtime_sha256_workspace",
            ));
        }
        if !self.workspaces.remove(workspace) {
            return Err(PublicSeamError::InvalidPlan {
                message: format!(
                    "workspace_release refused unmaterialized workspace `{workspace}`"
                ),
            });
        }
        assert_eq!(
            request.deps()["workspace"]["workspace"].as_str(),
            Some(workspace)
        );
        self.calls.push("workspace_release");
        Ok(PlanWorkspaceReleaseOutcome::new(
            workspace,
            "manual_release",
            "fp_runtime_sha256_workspace",
        ))
    }

    fn workspace_query(
        &mut self,
        request: PlanWorkspaceQueryRequest<'_>,
    ) -> Result<PlanWorkspaceQueryOutcome, PublicSeamError> {
        assert_eq!(request.workspace(), "ws_planexec_materialized");
        assert_eq!(
            request.deps()["workspace"]["workspace"].as_str(),
            Some("ws_planexec_materialized")
        );
        match (request.name(), request.op_kind()) {
            ("file", "read_file") => self.workspace_read_file(&request),
            ("listing", "list") => self.workspace_list(&request),
            ("stat", "stat") => self.workspace_stat(&request),
            ("digest", "digest") => self.workspace_digest(&request),
            ("snapshot", "snapshot") => Ok(self.workspace_snapshot()),
            ("log", "git_log") => Ok(self.workspace_git_log(&request)),
            ("diff", "git_diff") => Ok(self.workspace_git_diff()),
            ("status", "git_status") => Ok(self.workspace_git_status()),
            ("captured", "capture_artifacts") => Ok(self.workspace_capture_artifacts()),
            other => Ok(self.workspace_query_mismatch(other)),
        }
    }

    fn emit_run_event(
        &mut self,
        request: PlanEmitRunEventRequest<'_>,
    ) -> Result<PlanEmitRunEventOutcome, PublicSeamError> {
        assert_eq!(request.name(), "status");
        assert_eq!(request.write().event_kind(), "plan.ir.checked");
        assert_eq!(request.write().payload_schema(), "fp_schema_sha256_planir");
        assert_eq!(request.write().payload().as_json(), &json!({"ok": true}));
        assert_eq!(request.write().visibility(), "public");
        assert_eq!(request.base_revision(), "rev_planexec_base");
        self.writes.push("status");
        self.write_deps = request.deps().clone();
        self.write_dependency_data_classes
            .clone_from(request.dependency_data_classes());
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

impl RecordingPlanHost {
    fn sandbox_blob_ref(&self, id: &'static str) -> Value {
        if id == "blob_sandbox_output_file" {
            return blob_ref_with_hash_and_data_classes(
                id,
                "21e919af86c8fc1c44863d251f1bf9a492c0ee668796f528bd4cc372d3ea93ab",
                12,
                if matches!(
                    self.sandbox_stream,
                    SandboxStreamFixture::NonPublicBlobDataClasses
                ) {
                    &["workspace.file"][..]
                } else {
                    &["public"][..]
                },
            );
        }
        if id == "blob_sandbox_stdout" {
            return blob_ref_with_hash_and_data_classes(
                id,
                "63d42d26156fcc761e57da4128e9881d5bdf3bf933f0f6e9c93d6e26b9b90ae7",
                6,
                if matches!(
                    self.sandbox_stream,
                    SandboxStreamFixture::NonPublicBlobDataClasses
                ) {
                    &["transcript.raw"][..]
                } else {
                    &["public"][..]
                },
            );
        }
        if id == "blob_sandbox_stderr" {
            return blob_ref_with_hash_and_data_classes(
                id,
                "7e6b710b765404cccbad9eedcff7615fc37b269d6db12cd81a58be541d93083c",
                6,
                if matches!(
                    self.sandbox_stream,
                    SandboxStreamFixture::NonPublicBlobDataClasses
                ) {
                    &["transcript.raw"][..]
                } else {
                    &["public"][..]
                },
            );
        }
        if matches!(
            self.sandbox_stream,
            SandboxStreamFixture::NonPublicBlobDataClasses
        ) {
            let data_classes = if id == "blob_sandbox_output_file" {
                &["workspace.file"][..]
            } else {
                &["transcript.raw"][..]
            };
            blob_ref_with_data_classes(id, data_classes)
        } else {
            blob_ref(id)
        }
    }

    fn workspace_read_file(
        &mut self,
        request: &PlanWorkspaceQueryRequest<'_>,
    ) -> Result<PlanWorkspaceQueryOutcome, PublicSeamError> {
        ensure_workspace_fixture(
            request.path() == Some("README.md"),
            "read_file path must be README.md",
        )?;
        ensure_workspace_fixture(
            request.expected_data_classes() == BTreeSet::from(["candidate.artifact"]),
            "read_file data classes must match fixture",
        )?;
        self.calls.push("workspace_read_file");
        Ok(workspace_query_outcome(json!({
            "kind": "workspace_file",
            "path": "README.md",
            "content": "workspace file"
        })))
    }

    fn workspace_list(
        &mut self,
        request: &PlanWorkspaceQueryRequest<'_>,
    ) -> Result<PlanWorkspaceQueryOutcome, PublicSeamError> {
        ensure_workspace_fixture(request.path() == Some("."), "list path must be .")?;
        self.calls.push("workspace_list");
        Ok(workspace_query_outcome(workspace_listing_value()))
    }

    fn workspace_stat(
        &mut self,
        request: &PlanWorkspaceQueryRequest<'_>,
    ) -> Result<PlanWorkspaceQueryOutcome, PublicSeamError> {
        ensure_workspace_fixture(
            request.path() == Some("README.md"),
            "stat path must be README.md",
        )?;
        self.calls.push("workspace_stat");
        Ok(workspace_query_outcome(json!({
            "kind": "workspace_listing",
            "entries": [{
                "path": "README.md",
                "kind": "file",
                "bytes": 128,
                "data_classes": ["candidate.artifact"]
            }]
        })))
    }

    fn workspace_digest(
        &mut self,
        request: &PlanWorkspaceQueryRequest<'_>,
    ) -> Result<PlanWorkspaceQueryOutcome, PublicSeamError> {
        ensure_workspace_fixture(
            request.path() == Some("README.md"),
            "digest path must be README.md",
        )?;
        ensure_workspace_fixture(
            matches!(
                request.op(),
                WorkspaceQueryOp::Digest { algorithm, .. } if algorithm.as_str() == "sha256"
            ),
            "digest algorithm must be sha256",
        )?;
        self.calls.push("workspace_digest");
        Ok(workspace_query_outcome(json!({
            "kind": "workspace_snapshot",
            "workspace": "ws_planexec_materialized",
            "digest": "sha256:readme",
            "source_refs": [{
                "kind": "external",
                "namespace": "leaven.workspace.path",
                "id": "README.md"
            }]
        })))
    }

    fn workspace_snapshot(&mut self) -> PlanWorkspaceQueryOutcome {
        self.calls.push("workspace_snapshot");
        workspace_query_outcome(json!({
            "kind": "workspace_snapshot",
            "workspace": "ws_planexec_materialized",
            "digest": "sha256:planexec"
        }))
    }

    fn workspace_git_log(
        &mut self,
        request: &PlanWorkspaceQueryRequest<'_>,
    ) -> PlanWorkspaceQueryOutcome {
        assert!(matches!(
            request.op(),
            WorkspaceQueryOp::GitLog {
                max_entries: Some(5),
            }
        ));
        self.calls.push("workspace_git_log");
        workspace_query_outcome(json!({
            "kind": "workspace_diff",
            "text": "commit abc123 public-seam test",
            "source_refs": [{
                "kind": "external",
                "namespace": "leaven.workspace.git_log.max_entries",
                "id": "5"
            }]
        }))
    }

    fn workspace_git_diff(&mut self) -> PlanWorkspaceQueryOutcome {
        self.calls.push("workspace_git_diff");
        workspace_query_outcome(json!({
            "kind": "workspace_diff",
            "text": "diff --git a/README.md b/README.md",
            "source_refs": [{
                "kind": "external",
                "namespace": "leaven.workspace.git_diff.against",
                "id": "seed"
            }]
        }))
    }

    fn workspace_git_status(&mut self) -> PlanWorkspaceQueryOutcome {
        self.calls.push("workspace_git_status");
        workspace_query_outcome(json!({
            "kind": "workspace_diff",
            "text": " M README.md",
            "source_refs": [{
                "kind": "external",
                "namespace": "leaven.workspace.git_status.porcelain",
                "id": "true"
            }]
        }))
    }

    fn workspace_capture_artifacts(&mut self) -> PlanWorkspaceQueryOutcome {
        self.calls.push("workspace_capture_artifacts");
        workspace_query_outcome(workspace_listing_value())
    }

    fn workspace_query_mismatch(&mut self, query: (&str, &str)) -> PlanWorkspaceQueryOutcome {
        match query.1 {
            "read_file" | "list" | "stat" | "digest" => self.workspace_file_listing_mismatch(query),
            "snapshot" | "git_log" | "git_diff" | "git_status" | "capture_artifacts" => {
                self.workspace_snapshot_artifact_mismatch(query)
            }
            _ => panic!("unexpected workspace query: {query:?}"),
        }
    }

    fn workspace_file_listing_mismatch(
        &mut self,
        query: (&str, &str),
    ) -> PlanWorkspaceQueryOutcome {
        match query {
            ("wrong_kind", "read_file") => {
                self.calls.push("workspace_read_file");
                PlanWorkspaceQueryOutcome::new(
                    json!({"kind": "workspace_listing", "entries": []}),
                    "rev_planexec_base",
                )
            }
            ("read_file_wrong_path", "read_file") => {
                self.calls.push("workspace_read_file");
                workspace_query_outcome(json!({
                    "kind": "workspace_file",
                    "path": "src/lib.rs",
                    "content": "wrong file"
                }))
            }
            ("read_file_missing_body", "read_file") => {
                self.calls.push("workspace_read_file");
                workspace_query_outcome(json!({
                    "kind": "workspace_file",
                    "path": "README.md"
                }))
            }
            ("read_file_unsafe_result_path", "read_file") => {
                self.calls.push("workspace_read_file");
                workspace_query_outcome(json!({
                    "kind": "workspace_file",
                    "path": "/tmp/README.md",
                    "content": "host path"
                }))
            }
            ("list_outside_path", "list") => {
                self.calls.push("workspace_list");
                workspace_query_outcome(workspace_listing_value())
            }
            ("stat_wrong_kind", "stat") | ("digest_wrong_kind", "digest") => {
                self.calls.push("workspace_read_file");
                workspace_query_outcome(json!({
                    "kind": "workspace_file",
                    "path": "README.md",
                    "content": "wrong type"
                }))
            }
            ("stat_wrong_path", "stat") => {
                self.calls.push("workspace_stat");
                workspace_query_outcome(json!({
                    "kind": "workspace_listing",
                    "entries": [{
                        "path": "src/lib.rs",
                        "kind": "file",
                        "bytes": 128,
                        "data_classes": ["candidate.artifact"]
                    }]
                }))
            }
            ("stat_multi_entry", "stat") => {
                self.calls.push("workspace_stat");
                workspace_query_outcome(json!({
                    "kind": "workspace_listing",
                    "entries": [
                        {
                            "path": "README.md",
                            "kind": "file",
                            "bytes": 128,
                            "data_classes": ["candidate.artifact"]
                        },
                        {
                            "path": "src/lib.rs",
                            "kind": "file",
                            "bytes": 256,
                            "data_classes": ["candidate.artifact"]
                        }
                    ]
                }))
            }
            (name, "digest") => self.workspace_digest_mismatch(name),
            other => panic!("unexpected workspace query: {other:?}"),
        }
    }

    fn workspace_digest_mismatch(&mut self, name: &str) -> PlanWorkspaceQueryOutcome {
        self.calls.push("workspace_digest");
        match name {
            "digest_wrong_algorithm" => workspace_query_outcome(json!({
                "kind": "workspace_snapshot",
                "workspace": "ws_planexec_materialized",
                "digest": "blake3:readme",
                "source_refs": [workspace_source_ref("leaven.workspace.path", "README.md")]
            })),
            "digest_wrong_workspace" => workspace_query_outcome(json!({
                "kind": "workspace_snapshot",
                "workspace": "ws_planexec_other",
                "digest": "sha256:readme",
                "source_refs": [workspace_source_ref("leaven.workspace.path", "README.md")]
            })),
            "digest_wrong_source_path" => workspace_query_outcome(json!({
                "kind": "workspace_snapshot",
                "workspace": "ws_planexec_materialized",
                "digest": "sha256:readme",
                "source_refs": [workspace_source_ref("leaven.workspace.path", "src/lib.rs")]
            })),
            other => panic!("unexpected workspace digest query: {other}"),
        }
    }

    fn workspace_snapshot_artifact_mismatch(
        &mut self,
        query: (&str, &str),
    ) -> PlanWorkspaceQueryOutcome {
        match query {
            ("snapshot_wrong_workspace", "snapshot") => {
                self.calls.push("workspace_snapshot");
                workspace_query_outcome(json!({
                    "kind": "workspace_snapshot",
                    "workspace": "ws_planexec_other",
                    "digest": "sha256:planexec"
                }))
            }
            ("snapshot_missing_digest", "snapshot") => {
                self.calls.push("workspace_snapshot");
                workspace_query_outcome(json!({
                    "kind": "workspace_snapshot",
                    "workspace": "ws_planexec_materialized"
                }))
            }
            ("git_log_wrong_kind", "git_log") => {
                self.calls.push("workspace_git_log");
                workspace_query_outcome(workspace_listing_value())
            }
            ("git_log_missing_body", "git_log") => {
                self.calls.push("workspace_git_log");
                workspace_query_outcome(json!({"kind": "workspace_diff"}))
            }
            ("git_log_wrong_source_limit", "git_log") => {
                self.calls.push("workspace_git_log");
                workspace_query_outcome(json!({
                    "kind": "workspace_diff",
                    "text": "commit abc123 public-seam test",
                    "source_refs": [{
                        "kind": "external",
                        "namespace": "leaven.workspace.git_log.max_entries",
                        "id": "10"
                    }]
                }))
            }
            ("git_diff_missing_body", "git_diff") => {
                self.calls.push("workspace_git_diff");
                workspace_query_outcome(json!({"kind": "workspace_diff"}))
            }
            ("git_diff_wrong_source_against", "git_diff") => {
                self.calls.push("workspace_git_diff");
                workspace_query_outcome(json!({
                    "kind": "workspace_diff",
                    "text": "diff --git a/README.md b/README.md",
                    "source_refs": [{
                        "kind": "external",
                        "namespace": "leaven.workspace.git_diff.against",
                        "id": "parent"
                    }]
                }))
            }
            ("git_status_missing_body", "git_status") => {
                self.calls.push("workspace_git_status");
                workspace_query_outcome(json!({"kind": "workspace_diff"}))
            }
            ("git_status_wrong_source_porcelain", "git_status") => {
                self.calls.push("workspace_git_status");
                workspace_query_outcome(json!({
                    "kind": "workspace_diff",
                    "text": " M README.md",
                    "source_refs": [{
                        "kind": "external",
                        "namespace": "leaven.workspace.git_status.porcelain",
                        "id": "false"
                    }]
                }))
            }
            ("capture_unrequested_path", "capture_artifacts") => {
                self.calls.push("workspace_capture_artifacts");
                workspace_query_outcome(json!({
                    "kind": "workspace_listing",
                    "entries": [{
                        "path": "src/lib.rs",
                        "kind": "file",
                        "bytes": 256,
                        "data_classes": ["candidate.artifact"]
                    }]
                }))
            }
            ("capture_unsafe_result_path", "capture_artifacts") => {
                self.calls.push("workspace_capture_artifacts");
                workspace_query_outcome(json!({
                    "kind": "workspace_listing",
                    "entries": [{
                        "path": "README.md/../secret.txt",
                        "kind": "file",
                        "bytes": 64,
                        "data_classes": ["candidate.artifact"]
                    }]
                }))
            }
            ("capture_empty_paths", "capture_artifacts") => {
                self.calls.push("workspace_capture_artifacts");
                workspace_query_outcome(workspace_listing_value())
            }
            other => panic!("unexpected workspace query: {other:?}"),
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

fn call_execution_capability(
    allowed: &[&str],
    forbidden: &[&str],
    include_event_emit: bool,
) -> CapabilityDocument {
    let grants = vec![json!({
        "action": "lm.complete",
        "resource": {},
        "constraints": {
            "allowed_input_classes": allowed,
            "forbidden_input_classes": forbidden,
            "purposes": ["test.plan_ir"],
            "models": ["gpt-4.1-mini"],
            "model_roles": ["reflector"]
        }
    })];
    let mut value = base_execution_capability(&grants);
    if include_event_emit {
        value["grants"].as_array_mut().unwrap().push(json!({
            "action": "event.emit",
            "resource": {},
            "constraints": {}
        }));
    }
    CapabilityDocument::from_value(value).unwrap()
}

fn workspace_lifecycle_capability(include_release: bool) -> CapabilityDocument {
    let mut grants = vec![json!({
        "action": "workspace.materialize",
        "resource": {
            "candidate_ids": ["cand_planexec"]
        },
        "constraints": {
            "workspace_ops": ["materialize"]
        }
    })];
    if include_release {
        grants.push(json!({
            "action": "workspace.release",
            "resource": {
                "workspace_ids": ["ws_planexec_materialized"]
            },
            "constraints": {
                "workspace_ops": ["release"]
            }
        }));
    }
    CapabilityDocument::from_value(base_execution_capability(&grants)).unwrap()
}

fn workspace_query_capability(
    read_ops: &[&str],
    allowed_read_classes: &[&str],
) -> CapabilityDocument {
    workspace_query_capability_for_workspace(
        "ws_planexec_materialized",
        read_ops,
        allowed_read_classes,
    )
}

fn workspace_query_capability_for_workspace(
    workspace_id: &str,
    read_ops: &[&str],
    allowed_read_classes: &[&str],
) -> CapabilityDocument {
    CapabilityDocument::from_value(base_execution_capability(&[
        json!({
            "action": "workspace.materialize",
            "resource": {
                "candidate_ids": ["cand_planexec"]
            },
            "constraints": {
                "workspace_ops": ["materialize"]
            }
        }),
        json!({
            "action": "workspace.read",
            "resource": {
                "workspace_ids": [workspace_id]
            },
            "constraints": {
                "allowed_input_classes": allowed_read_classes,
                "workspace_ops": read_ops
            }
        }),
        json!({
            "action": "workspace.release",
            "resource": {
                "workspace_ids": [workspace_id]
            },
            "constraints": {
                "workspace_ops": ["release"]
            }
        }),
    ]))
    .unwrap()
}

fn sandbox_exec_capability_value() -> Value {
    base_execution_capability(&[
        json!({
            "action": "workspace.materialize",
            "resource": {
                "candidate_ids": ["cand_planexec"]
            },
            "constraints": {
                "workspace_ops": ["materialize"]
            }
        }),
        json!({
            "action": "sandbox.exec",
            "resource": {
                "workspace_ids": ["ws_planexec_materialized"]
            },
            "constraints": {
                "allowed_input_classes": ["public"],
                "workspace_ops": ["exec"],
                "allowed_commands": ["python"]
            },
            "limits": {
                "timeout_s": 1
            }
        }),
    ])
}

struct EffectCapabilityFacts {
    agent_workspaces: BTreeSet<String>,
    sandbox_workspaces: BTreeSet<String>,
    agent_commands: BTreeSet<String>,
    schemas: Vec<String>,
    surfaces: Vec<String>,
}

fn effect_execution_capability_for_plan(plan: &Value) -> CapabilityDocument {
    let facts = effect_capability_facts(plan);
    let mut agent_constraints = json!({"allowed_input_classes": ["public"]});
    if !facts.agent_commands.is_empty() {
        agent_constraints["allowed_commands"] = json!(facts.agent_commands);
    }
    if !facts.schemas.is_empty() {
        agent_constraints["schemas"] = json!(facts.schemas);
    }
    if !facts.surfaces.is_empty() {
        agent_constraints["allowed_surfaces"] = json!(facts.surfaces);
    }

    CapabilityDocument::from_value(base_execution_capability(&[
        json!({
            "action": "workspace.materialize",
            "resource": {
                "candidate_ids": ["cand_planexec"]
            },
            "constraints": {
                "workspace_ops": ["materialize"]
            }
        }),
        json!({
            "action": "workspace.release",
            "resource": {
                "workspace_ids": ["ws_planexec_materialized"]
            },
            "constraints": {
                "workspace_ops": ["release"]
            }
        }),
        json!({
            "action": "agent.run",
            "resource": {
                "workspace_ids": facts.agent_workspaces
            },
            "constraints": agent_constraints,
            "limits": {
                "timeout_s": 30,
                "max_usd_micro": 1000
            }
        }),
        json!({
            "action": "sandbox.exec",
            "resource": {
                "workspace_ids": facts.sandbox_workspaces
            },
            "constraints": {
                "allowed_input_classes": ["public"],
                "workspace_ops": ["exec"],
                "allowed_commands": ["python"]
            },
            "limits": {
                "timeout_s": 1
            }
        }),
    ]))
    .unwrap()
}

fn effect_capability_facts(plan: &Value) -> EffectCapabilityFacts {
    let mut facts = EffectCapabilityFacts {
        agent_workspaces: BTreeSet::new(),
        sandbox_workspaces: BTreeSet::new(),
        agent_commands: BTreeSet::new(),
        schemas: Vec::new(),
        surfaces: Vec::new(),
    };
    for op in plan
        .get("ops")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let Some(call) = op.get("call").and_then(Value::as_object) else {
            continue;
        };
        match call.get("kind").and_then(Value::as_str) {
            Some("agent_run") => {
                if let Some(workspace) = call.get("workspace").and_then(Value::as_str) {
                    facts.agent_workspaces.insert(workspace.to_owned());
                }
                collect_agent_capability_constraints(call, &mut facts);
            }
            Some("sandbox_exec") => {
                if let Some(workspace) = call.get("workspace").and_then(Value::as_str) {
                    facts.sandbox_workspaces.insert(workspace.to_owned());
                }
            }
            _ => {}
        }
    }
    if facts.agent_workspaces.is_empty() {
        facts
            .agent_workspaces
            .insert("ws_planexec_materialized".to_owned());
    }
    if facts.sandbox_workspaces.is_empty() {
        facts
            .sandbox_workspaces
            .insert("ws_planexec_materialized".to_owned());
    }
    facts
}

fn collect_agent_capability_constraints(
    call: &serde_json::Map<String, Value>,
    facts: &mut EffectCapabilityFacts,
) {
    if let Some(commands) = call
        .get("tool_policy")
        .and_then(Value::as_object)
        .and_then(|policy| policy.get("allowed_commands"))
        .and_then(Value::as_array)
    {
        facts
            .agent_commands
            .extend(commands.iter().filter_map(Value::as_str).map(str::to_owned));
    }
    let Some(output) = call.get("output").and_then(Value::as_object) else {
        return;
    };
    match output.get("kind").and_then(Value::as_str) {
        Some("json_schema") => {
            if let Some(schema) = output.get("schema_fingerprint").and_then(Value::as_str) {
                facts.schemas.push(schema.to_owned());
            }
        }
        Some("workspace_diff") => {
            if let Some(surface) = output.get("surface_fingerprint").and_then(Value::as_str) {
                facts.surfaces.push(surface.to_owned());
            }
        }
        _ => {}
    }
}

fn assessment_submit_capability(
    evaluation_request_id: &str,
    max_rows: Option<u64>,
) -> CapabilityDocument {
    let mut grant = json!({
        "action": "assessment.submit",
        "resource": {
            "evaluation_request_id": evaluation_request_id
        },
        "constraints": {}
    });
    if let Some(max_rows) = max_rows {
        grant["limits"] = json!({
            "max_rows": max_rows
        });
    }
    CapabilityDocument::from_value(base_execution_capability(&[grant])).unwrap()
}

fn evaluation_request_capability(candidate_ids: &[&str], purposes: &[&str]) -> CapabilityDocument {
    CapabilityDocument::from_value(base_execution_capability(&[json!({
        "action": "evaluation.request",
        "resource": {
            "candidate_ids": candidate_ids
        },
        "constraints": {
            "purposes": purposes
        }
    })]))
    .unwrap()
}

fn base_execution_capability(grants: &[Value]) -> Value {
    json!({
        "schema_version": "leaven.capability.v1",
        "jti": "jti_planexec_call_authority",
        "capability_fingerprint": "fp_cap_sha256_planexec",
        "policy_fingerprint": "fp_policy_sha256_planexec",
        "subject_fingerprint": "fp_subject_sha256_planexec",
        "issuer": {
            "kind": "run_engine",
            "id": "engine_local"
        },
        "subject": {
            "kind": "stage_call",
            "run": "run_demo",
            "stage_call_id": "sc_planexec_call_authority",
            "role": "scorer"
        },
        "audience": ["leaven.acp.worker"],
        "issued_at": "2026-05-23T00:00:00Z",
        "expires_at": "2026-05-23T00:20:00Z",
        "expiry_behavior": "drain_inflight_no_new_ops",
        "token_binding": {
            "kind": "opaque_lookup",
            "token_id": "ltok_planexec_call_authority"
        },
        "revocation": {
            "mode": "issuer_epoch",
            "revocation_epoch": 7,
            "check": "on_every_request"
        },
        "renewal": {
            "mode": "renew_before_expiry",
            "max_extensions": 2,
            "max_total_lifetime_s": 3600
        },
        "grants": grants,
        "budgets": {},
        "execution_policy": {
            "profile": "managed_sandbox",
            "network": "leaven_endpoint_only",
            "subprocess": "deny_except_sandbox_exec",
            "filesystem": "workspace_handles_only",
            "byo_effects": "forbidden"
        },
        "delegation": {
            "may_delegate": false,
            "max_depth": 0,
            "must_attenuate": true,
            "allowed_actions": []
        }
    })
}

fn evaluator_capability_value(package: &PublicSeamPackage) -> Value {
    let path = package
        .root()
        .join("examples")
        .join("evaluator_capability.v0.3.example.json");
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn blob_ref(id: &'static str) -> Value {
    blob_ref_with_data_classes(id, &["public"])
}

fn blob_ref_with_data_classes(id: &'static str, data_classes: &[&str]) -> Value {
    blob_ref_with_hash_and_data_classes(id, FIXTURE_BLOB_SHA256, 12, data_classes)
}

fn agent_transcript_blob_ref(id: &'static str) -> Value {
    let transcript_bytes = serde_json::to_vec(&leaven_agent::AgentTranscript::default()).unwrap();
    json!({
        "kind": "blob_ref",
        "id": id,
        "sha256": sha256_hex(&transcript_bytes),
        "bytes": transcript_bytes.len(),
        "data_classes": ["transcript.raw"]
    })
}

fn blob_ref_with_hash_and_data_classes(
    id: &'static str,
    sha256: &'static str,
    bytes: u64,
    data_classes: &[&str],
) -> Value {
    json!({
        "kind": "blob_ref",
        "id": id,
        "sha256": sha256,
        "bytes": bytes,
        "data_classes": data_classes
    })
}

fn workspace_query_outcome(value: Value) -> PlanWorkspaceQueryOutcome {
    PlanWorkspaceQueryOutcome::new(value, "rev_planexec_base")
        .with_data_classes(["candidate.artifact".to_owned(), "public".to_owned()])
}

fn workspace_source_ref(namespace: &str, id: &str) -> Value {
    json!({
        "kind": "external",
        "namespace": namespace,
        "id": id
    })
}

fn workspace_listing_value() -> Value {
    json!({
        "kind": "workspace_listing",
        "entries": [
            {
                "path": "README.md",
                "kind": "file",
                "bytes": 14,
                "data_classes": ["candidate.artifact"]
            }
        ]
    })
}

fn ensure_workspace_fixture(condition: bool, message: &'static str) -> Result<(), PublicSeamError> {
    if condition {
        Ok(())
    } else {
        Err(PublicSeamError::InvalidPlan {
            message: message.to_owned(),
        })
    }
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

fn assert_call_authority_denied_redactions(error: &PublicSeamError, expected: &[&str]) {
    match error {
        PublicSeamError::CallAuthorityDenied {
            kind, redactions, ..
        } => {
            assert_eq!(kind, "data_class");
            assert_eq!(
                redactions,
                &expected
                    .iter()
                    .map(|redaction| (*redaction).to_owned())
                    .collect::<Vec<_>>()
            );
        }
        other => panic!("expected call authority denial, got {other:?}"),
    }
}
