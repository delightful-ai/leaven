use std::collections::BTreeMap;

use leaven_public_seam::{
    PlanEmitRunEventOutcome, PlanEmitRunEventRequest, PlanExecutionContext, PlanExecutionHost,
    PlanGraphQueryOutcome, PlanGraphQueryRequest, PlanGraphReadScope, PlanLmCompleteOutcome,
    PlanLmCompleteRequest, PlanOperationKind, PublicSeamError, PublicSeamPackage,
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

    let report = package
        .execute_plan_document(
            &plan,
            &PlanExecutionContext::new(
                "fp_cap_sha256_planexec",
                "fp_policy_sha256_planexec",
                "rev_planexec_base",
                "2026-05-23T12:00:00Z",
                "2026-05-23T12:00:01Z",
            ),
            &mut host,
        )
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
                "call": {
                    "kind": "lm_complete",
                    "purpose": "test.plan_ir",
                    "messages": [
                        {
                            "role": "user",
                            "content": [
                                {
                                    "kind": "text",
                                    "text": "Say ok"
                                }
                            ]
                        }
                    ],
                    "output": {
                        "kind": "final_message",
                        "max_bytes": 1024
                    },
                    "input_classes": ["public"]
                }
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

fn require_cached_call_plan() -> Value {
    let mut plan = typed_let_call_write_plan();
    plan["mode"] = json!({"kind": "require_cached"});
    plan["ops"].as_array_mut().unwrap().pop();
    plan["return"] = json!(["completion"]);
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
    calls: Vec<&'static str>,
    cached_calls: Vec<&'static str>,
    writes: Vec<&'static str>,
    replayed_receipts: Vec<String>,
    call_deps: BTreeMap<String, Value>,
    write_deps: BTreeMap<String, Value>,
    cached_hit: bool,
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

    fn lm_complete(
        &mut self,
        request: PlanLmCompleteRequest<'_>,
    ) -> Result<PlanLmCompleteOutcome, PublicSeamError> {
        assert_eq!(request.name(), "completion");
        assert_eq!(request.call()["kind"].as_str(), Some("lm_complete"));
        self.calls.push("completion");
        self.call_deps = request.deps().clone();
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
