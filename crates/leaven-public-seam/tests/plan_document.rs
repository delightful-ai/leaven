use std::collections::BTreeMap;

use leaven_public_seam::{
    PlanEmitRunEventOutcome, PlanEmitRunEventRequest, PlanExecutionContext, PlanExecutionHost,
    PlanLmCompleteOutcome, PlanLmCompleteRequest, PlanOperationKind, PublicSeamError,
    PublicSeamPackage,
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
fn plan_ir_family_execution_rejects_known_variants_outside_representative_harness() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let mut plan = typed_let_call_write_plan();
    plan["mode"] = json!({"kind": "execute"});
    plan["ops"][1]["call"] = json!({
        "kind": "human_review",
        "queue": "qa",
        "prompt": "Review Say ok",
        "input_classes": ["public"]
    });
    let mut host = RecordingPlanHost::default();

    let error = package
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
                            "evidence": evidence_envelope(),
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
                            "evidence": evidence_envelope(),
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
                            "evidence": evidence_envelope(),
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

fn evidence_envelope() -> Value {
    json!({
        "schema_version": "leaven.evidence_envelope.v1",
        "target_derived": false,
        "public": {
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
    calls: Vec<&'static str>,
    writes: Vec<&'static str>,
    call_deps: BTreeMap<String, Value>,
    write_deps: BTreeMap<String, Value>,
}

impl PlanExecutionHost for RecordingPlanHost {
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
