use crate::support::{bind_plan_result_hashes, fixture_blob_ref, package};
use leaven_public_seam::{
    PlanErrorCode, PlanResultGraphEventPayload, PlanResultGraphExtensionPayload,
    PlanResultProposalEffectKind, PlanResultReceiptKind, PlanResultValueKind, PublicSeamError,
};
use serde_json::{Value, json};

#[test]
fn plan_result_accepts_typed_success_and_failure_envelopes() {
    let package = package();

    let success = package
        .validate_plan_result_document(&typed_success_result())
        .unwrap();
    assert_eq!(success.plan_id(), "result001");
    assert_eq!(success.base_revision(), "rev_base");
    assert_eq!(success.final_revision(), "rev_base");
    assert_eq!(success.value_count(), 1);
    assert_eq!(success.receipt_count(), 1);
    assert_eq!(success.error_count(), 0);
    assert_eq!(success.charge_count(), 0);
    assert_eq!(success.values()[0].name(), "rows");
    assert_eq!(success.values()[0].kind(), PlanResultValueKind::GraphSet);
    assert_eq!(success.receipts()[0].kind(), PlanResultReceiptKind::Query);
    assert!(success.errors().is_empty());
    assert!(success.charges().is_empty());

    let failure = package
        .validate_plan_result_document(&typed_failure_result())
        .unwrap();
    assert_eq!(failure.plan_id(), "resultfail001");
    assert_eq!(failure.value_count(), 0);
    assert_eq!(failure.receipt_count(), 1);
    assert_eq!(failure.error_count(), 1);
    assert_eq!(failure.charge_count(), 1);
    assert_eq!(failure.receipts()[0].kind(), PlanResultReceiptKind::Call);
    assert_eq!(failure.errors().count(), 1);
    assert_eq!(failure.charges().count(), 1);
    assert_eq!(
        failure.plan_errors()[0].code(),
        PlanErrorCode::ProviderError
    );
    assert_eq!(failure.plan_errors()[0].message(), "provider failed");
    assert_eq!(failure.plan_errors()[0].receipt(), "lmrec_failed");
    assert_eq!(failure.plan_errors()[0].retryable(), Some(true));
}

#[test]
fn plan_result_preserves_graph_row_json_fragments_as_typed_owners() {
    let package = package();

    let result = package
        .validate_plan_result_document(&graph_row_fragment_result())
        .unwrap();
    let fragments = result.graph_row_fragments();

    assert_eq!(fragments.candidate_scores().len(), 1);
    let candidate_scores = &fragments.candidate_scores()[0];
    assert_eq!(
        candidate_scores.primary().map(ToString::to_string),
        Some("0.9".to_owned())
    );
    assert_eq!(candidate_scores.metrics()[0].0, "accuracy");
    assert_eq!(candidate_scores.metrics()[0].1.to_string(), "0.9");
    assert_eq!(candidate_scores.cases()[0].case_id(), "case_1");
    assert_eq!(candidate_scores.cases()[0].score().to_string(), "0.9");
    assert_eq!(fragments.candidate_artifacts().len(), 1);
    let candidate_artifact = &fragments.candidate_artifacts()[0];
    assert_eq!(candidate_artifact.kind(), "prompt");
    assert_eq!(candidate_artifact.identity(), Some("artifact_sha256_alpha"));
    assert_eq!(candidate_artifact.summary(), Some("answer concisely"));
    assert_eq!(candidate_artifact.body(), Some("answer concisely"));
    assert_eq!(fragments.proposal_effects().len(), 1);
    let proposal_effect = &fragments.proposal_effects()[0];
    assert_eq!(proposal_effect.kind(), PlanResultProposalEffectKind::Change);
    assert_eq!(proposal_effect.target_candidate_id(), Some("cand_alpha"));
    assert_eq!(proposal_effect.artifact_type(), None);
    assert_eq!(fragments.event_payloads().len(), 1);
    let PlanResultGraphEventPayload::ExternalEvent(event_payload) = &fragments.event_payloads()[0]
    else {
        panic!("expected typed external event payload");
    };
    assert!(event_payload.ok());
    assert_eq!(event_payload.stage_call_id(), None);
    assert_eq!(fragments.extension_payloads().len(), 1);
    let PlanResultGraphExtensionPayload::Summary(extension_payload) =
        &fragments.extension_payloads()[0]
    else {
        panic!("expected typed extension summary payload");
    };
    assert_eq!(extension_payload.summary(), "vendor score 7");
    assert_eq!(extension_payload.data_classes(), ["public"]);
    assert_eq!(extension_payload.source_ref(), Some("cand_alpha"));
}

#[test]
fn plan_result_rejects_open_candidate_summary_fragments() {
    let package = package();
    let mut result = graph_row_fragment_result();
    result["values"]["rows"]["items"][0]["scores"]["checks"] = json!(["format"]);

    let error = package.validate_plan_result_document(&result).unwrap_err();

    assert!(
        matches!(error, PublicSeamError::ExampleValidation { .. }),
        "{error:?}"
    );
}

#[test]
fn plan_result_rejects_open_extension_graph_row_payload() {
    let package = package();
    let mut result = graph_row_fragment_result();
    result["values"]["rows"]["items"][3]["payload"] = json!({
        "vendor": {"score": 7}
    });

    let error = package.validate_plan_result_document(&result).unwrap_err();

    assert!(
        matches!(error, PublicSeamError::ExampleValidation { .. }),
        "{error:?}"
    );
}

#[test]
fn plan_result_rejects_open_proposal_effect_summary_payload() {
    let package = package();
    let mut result = graph_row_fragment_result();
    result["values"]["rows"]["items"][1]["effect"]["prose"] =
        json!("proposal summaries are typed records");

    let error = package.validate_plan_result_document(&result).unwrap_err();

    assert!(
        matches!(error, PublicSeamError::ExampleValidation { .. }),
        "{error:?}"
    );
}

#[test]
fn plan_result_accepts_query_call_and_write_receipts_as_audit_currency() {
    let package = package();

    let result = package
        .validate_plan_result_document(&audit_currency_result())
        .unwrap();

    assert_eq!(result.receipt_count(), 3);
    assert_eq!(
        result
            .receipts()
            .iter()
            .map(|receipt| receipt.kind())
            .collect::<Vec<_>>(),
        vec![
            PlanResultReceiptKind::Query,
            PlanResultReceiptKind::Call,
            PlanResultReceiptKind::Write
        ]
    );
    assert!(
        result
            .values()
            .iter()
            .any(|value| value.kind() == PlanResultValueKind::GraphSet)
    );
    assert!(
        result
            .values()
            .iter()
            .any(|value| value.kind() == PlanResultValueKind::AgentSession)
    );
    assert!(
        result
            .values()
            .iter()
            .any(|value| value.kind() == PlanResultValueKind::ApplyReceipt)
    );
}

#[test]
fn plan_result_accepts_assessment_summary_with_score_output_and_evidence_envelope() {
    let package = package();

    let result = package
        .validate_plan_result_document(&assessment_summary_result())
        .unwrap();

    assert_eq!(result.values()[0].kind(), PlanResultValueKind::GraphSet);
    assert_eq!(
        result.values()[0].data_classes(),
        &["candidate.output".to_owned(), "public".to_owned()]
    );
    assert_eq!(
        result.value_data_classes(),
        &[(
            "rows".to_owned(),
            vec!["candidate.output".to_owned(), "public".to_owned()]
        )]
    );
}

#[test]
fn plan_result_accepts_assessment_summary_structured_or_numeric_score_output_values() {
    let package = package();

    let mut structured = assessment_summary_result();
    structured["values"]["rows"]["items"][0]["score"]["output"] = json!({
        "kind": "structured",
        "summary": "candidate alpha answer",
        "value": {
            "answer": "candidate alpha answer",
            "confidence": 0.9
        },
        "visibility": "public",
        "data_classes": ["candidate.output"]
    });
    bind_result_hashes_in_place(&mut structured);
    package.validate_plan_result_document(&structured).unwrap();

    let mut numeric = assessment_summary_result();
    numeric["values"]["rows"]["items"][0]["score"]["output"] = json!({
        "kind": "json",
        "summary": "candidate alpha answer",
        "value": 42,
        "visibility": "public",
        "data_classes": ["candidate.output"]
    });
    bind_result_hashes_in_place(&mut numeric);
    package.validate_plan_result_document(&numeric).unwrap();
}

#[test]
fn plan_result_rejects_generic_or_untyped_result_payloads() {
    let package = package();

    assert!(matches!(
        package
            .validate_plan_result_document(&json!({
                "status": "ok",
                "output": {
                    "whatever": true
                }
            }))
            .unwrap_err(),
        PublicSeamError::ExampleValidation { .. }
    ));

    let mut missing_fingerprint = typed_success_result();
    missing_fingerprint
        .as_object_mut()
        .unwrap()
        .remove("capability_fingerprint");
    assert!(matches!(
        package
            .validate_plan_result_document(&missing_fingerprint)
            .unwrap_err(),
        PublicSeamError::ExampleValidation { .. }
    ));

    let mut missing_policy_fingerprint = typed_success_result();
    missing_policy_fingerprint
        .as_object_mut()
        .unwrap()
        .remove("policy_fingerprint");
    assert!(matches!(
        package
            .validate_plan_result_document(&missing_policy_fingerprint)
            .unwrap_err(),
        PublicSeamError::ExampleValidation { .. }
    ));

    let mut untyped_error = typed_failure_result();
    untyped_error["errors"] = json!(["provider exploded"]);
    assert!(matches!(
        package
            .validate_plan_result_document(&untyped_error)
            .unwrap_err(),
        PublicSeamError::ExampleValidation { .. }
    ));

    let mut unknown_error = typed_failure_result();
    unknown_error["errors"][0]["code"] = json!("made_up_error");
    assert!(matches!(
        package
            .validate_plan_result_document(&unknown_error)
            .unwrap_err(),
        PublicSeamError::ExampleValidation { .. }
    ));
}

#[test]
fn plan_result_parses_plan_error_details_as_typed_public_fields() {
    let package = package();

    let mut detailed = typed_failure_result();
    detailed["receipts"][0]["error"]["details"] = json!({
        "summary": "provider overloaded",
        "reason": "upstream_503",
        "retry_after_ms": 250
    });
    detailed["errors"][0]["details"] = json!({
        "summary": "provider overloaded",
        "reason": "upstream_503",
        "retry_after_ms": 250
    });

    let result = package.validate_plan_result_document(&detailed).unwrap();
    let details = result.plan_errors()[0].details().unwrap();
    assert_eq!(details.summary(), Some("provider overloaded"));
    assert_eq!(details.reason(), Some("upstream_503"));
    assert_eq!(details.retry_after_ms(), Some(250));
}

#[test]
fn plan_result_rejects_unowned_plan_error_details_payloads() {
    let package = package();

    let mut nested = typed_failure_result();
    nested["receipts"][0]["error"]["details"] = json!({
        "provider": {
            "raw": "unowned"
        }
    });
    nested["errors"][0]["details"] = json!({
        "provider": {
            "raw": "unowned"
        }
    });
    let error = package.validate_plan_result_document(&nested).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("PlanError.details carries unknown field `provider`"),
        "{error}"
    );
}

#[test]
fn plan_result_rejects_assessment_summary_without_score_output_or_evidence_truth() {
    let package = package();

    let mut missing_score = assessment_summary_result();
    missing_score["values"]["rows"]["items"][0]
        .as_object_mut()
        .unwrap()
        .remove("score");
    bind_result_hashes_in_place(&mut missing_score);
    assert!(matches!(
        package
            .validate_plan_result_document(&missing_score)
            .unwrap_err(),
        PublicSeamError::InvalidPlanResult { .. }
    ));

    let mut missing_evidence = assessment_summary_result();
    missing_evidence["values"]["rows"]["items"][0]
        .as_object_mut()
        .unwrap()
        .remove("evidence");
    bind_result_hashes_in_place(&mut missing_evidence);
    assert!(matches!(
        package
            .validate_plan_result_document(&missing_evidence)
            .unwrap_err(),
        PublicSeamError::InvalidPlanResult { .. }
    ));

    let mut unreceipted_evidence = assessment_summary_result();
    unreceipted_evidence["values"]["rows"]["items"][0]["evidence"]["source_receipts"]["read"] =
        json!([]);
    bind_result_hashes_in_place(&mut unreceipted_evidence);
    let error = package
        .validate_plan_result_document(&unreceipted_evidence)
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("evidence source_receipts must carry at least one receipt"),
        "{error}"
    );

    let mut dropped_output_class = assessment_summary_result();
    dropped_output_class["values"]["rows"]["items"][0]["score"]["output"]["data_classes"] =
        json!(["public"]);
    bind_result_hashes_in_place(&mut dropped_output_class);
    let error = package
        .validate_plan_result_document(&dropped_output_class)
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("candidate.output or candidate.artifact"),
        "{error}"
    );

    let mut structured_dummy = assessment_summary_result();
    structured_dummy["values"]["rows"]["items"][0]["score"]["output"] = json!({
        "kind": "structured",
        "value": {
            "looks_like": "schema-valid output",
            "but": "has no reportable projection"
        },
        "visibility": "public",
        "data_classes": ["candidate.output"]
    });
    bind_result_hashes_in_place(&mut structured_dummy);
    let error = package
        .validate_plan_result_document(&structured_dummy)
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("must carry a non-empty summary for evidence projection"),
        "{error}"
    );

    let mut missing_evidence_projection = assessment_summary_result();
    missing_evidence_projection["values"]["rows"]["items"][0]["score"]["output"] = json!({
        "kind": "structured",
        "summary": "schema-valid dummy output",
        "value": {
            "looks_like": "schema-valid output"
        },
        "visibility": "public",
        "data_classes": ["candidate.output"]
    });
    missing_evidence_projection["values"]["rows"]["items"][0]["evidence"]["public"]
        .as_object_mut()
        .unwrap()
        .remove("summary");
    bind_result_hashes_in_place(&mut missing_evidence_projection);
    let error = package
        .validate_plan_result_document(&missing_evidence_projection)
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("evidence.public.summary must project Score.output summary"),
        "{error}"
    );
}

#[test]
fn plan_result_rejects_receipts_without_audit_timing() {
    let package = package();

    let mut missing_started_at = typed_success_result();
    missing_started_at["receipts"][0]
        .as_object_mut()
        .unwrap()
        .remove("started_at");
    assert!(matches!(
        package
            .validate_plan_result_document(&missing_started_at)
            .unwrap_err(),
        PublicSeamError::InvalidPlanResult { .. }
    ));

    let mut missing_completed_at = typed_success_result();
    missing_completed_at["receipts"][0]
        .as_object_mut()
        .unwrap()
        .remove("completed_at");
    assert!(matches!(
        package
            .validate_plan_result_document(&missing_completed_at)
            .unwrap_err(),
        PublicSeamError::InvalidPlanResult { .. }
    ));
}

#[test]
fn plan_result_rejects_decorative_or_wrong_kind_receipt_refs() {
    let package = package();

    let mut missing_receipt = typed_success_result();
    missing_receipt["values"]["rows"]["receipt"] = json!("qrec_missing");
    assert!(matches!(
        package
            .validate_plan_result_document(&missing_receipt)
            .unwrap_err(),
        PublicSeamError::InvalidPlanResult { .. }
    ));

    let mut wrong_kind_receipt = typed_success_result();
    wrong_kind_receipt["receipts"][0]["kind"] = json!("call");
    wrong_kind_receipt["receipts"][0]["call_kind"] = json!("lm_complete");
    wrong_kind_receipt["receipts"][0]["request_hash"] = json!("fp_request_sha256_rows");
    wrong_kind_receipt["receipts"][0]
        .as_object_mut()
        .unwrap()
        .remove("op_hash");
    wrong_kind_receipt["receipts"][0]
        .as_object_mut()
        .unwrap()
        .remove("graph_revision");
    wrong_kind_receipt["receipts"][0]
        .as_object_mut()
        .unwrap()
        .remove("read_scope_fingerprint");
    wrong_kind_receipt["receipts"][0]
        .as_object_mut()
        .unwrap()
        .remove("projection_fingerprint");
    assert!(matches!(
        package
            .validate_plan_result_document(&wrong_kind_receipt)
            .unwrap_err(),
        PublicSeamError::InvalidPlanResult { .. }
    ));

    let mut duplicate_receipt = audit_currency_result();
    duplicate_receipt["receipts"][2]["receipt"] = json!("qrec_audit_rows");
    assert!(matches!(
        package
            .validate_plan_result_document(&duplicate_receipt)
            .unwrap_err(),
        PublicSeamError::InvalidPlanResult { .. }
    ));
}

#[test]
fn plan_result_rejects_decorative_audit_currency_receipts() {
    let package = package();

    let mut missing_query_policy = typed_success_result();
    missing_query_policy["receipts"][0]
        .as_object_mut()
        .unwrap()
        .remove("read_scope_fingerprint");
    assert!(matches!(
        package
            .validate_plan_result_document(&missing_query_policy)
            .unwrap_err(),
        PublicSeamError::InvalidPlanResult { .. }
    ));

    let mut missing_projection = typed_success_result();
    missing_projection["receipts"][0]
        .as_object_mut()
        .unwrap()
        .remove("projection_fingerprint");
    assert!(matches!(
        package
            .validate_plan_result_document(&missing_projection)
            .unwrap_err(),
        PublicSeamError::InvalidPlanResult { .. }
    ));

    let mut wrong_query_hash_role = typed_success_result();
    wrong_query_hash_role["receipts"][0]["op_hash"] = json!("fp_result_sha256_rows");
    assert!(matches!(
        package
            .validate_plan_result_document(&wrong_query_hash_role)
            .unwrap_err(),
        PublicSeamError::InvalidPlanResult { .. }
    ));

    let mut missing_runtime = typed_failure_result();
    missing_runtime["receipts"][0]
        .as_object_mut()
        .unwrap()
        .remove("runtime_fingerprint");
    assert!(matches!(
        package
            .validate_plan_result_document(&missing_runtime)
            .unwrap_err(),
        PublicSeamError::InvalidPlanResult { .. }
    ));

    let mut wrong_call_hash_role = typed_failure_result();
    wrong_call_hash_role["receipts"][0]["result_hash"] = json!("fp_request_sha256_lm");
    assert!(matches!(
        package
            .validate_plan_result_document(&wrong_call_hash_role)
            .unwrap_err(),
        PublicSeamError::InvalidPlanResult { .. }
    ));
}

#[test]
fn plan_result_rejects_same_prefix_result_hashes_that_do_not_bind_values() {
    let package = package();

    let mut wrong_query_result_hash = audit_currency_result();
    wrong_query_result_hash["receipts"][0]["result_hash"] =
        json!("fp_result_sha256_same_prefix_wrong_query_value");
    assert!(matches!(
        package
            .validate_plan_result_document(&wrong_query_result_hash)
            .unwrap_err(),
        PublicSeamError::InvalidPlanResult { .. }
    ));

    let mut wrong_call_result_hash = audit_currency_result();
    wrong_call_result_hash["receipts"][1]["result_hash"] =
        json!("fp_result_sha256_same_prefix_wrong_call_value");
    assert!(matches!(
        package
            .validate_plan_result_document(&wrong_call_result_hash)
            .unwrap_err(),
        PublicSeamError::InvalidPlanResult { .. }
    ));

    let mut wrong_write_result_hash = audit_currency_result();
    wrong_write_result_hash["receipts"][2]["result_hash"] =
        json!("fp_result_sha256_same_prefix_wrong_write_value");
    assert!(matches!(
        package
            .validate_plan_result_document(&wrong_write_result_hash)
            .unwrap_err(),
        PublicSeamError::InvalidPlanResult { .. }
    ));
}

#[test]
fn plan_result_rejects_failed_call_costs_without_charge_receipts() {
    let package = package();

    let mut missing_charge_id = typed_failure_result();
    missing_charge_id["receipts"][0]
        .as_object_mut()
        .unwrap()
        .remove("charge_receipts");
    assert!(matches!(
        package
            .validate_plan_result_document(&missing_charge_id)
            .unwrap_err(),
        PublicSeamError::InvalidPlanResult { .. }
    ));

    let mut missing_charge_record = typed_failure_result();
    missing_charge_record["charges"] = json!([]);
    assert!(matches!(
        package
            .validate_plan_result_document(&missing_charge_record)
            .unwrap_err(),
        PublicSeamError::InvalidPlanResult { .. }
    ));

    let mut wrong_charge_source = typed_failure_result();
    wrong_charge_source["charges"][0]["source_receipt"] = json!("lmrec_other");
    assert!(matches!(
        package
            .validate_plan_result_document(&wrong_charge_source)
            .unwrap_err(),
        PublicSeamError::InvalidPlanResult { .. }
    ));

    let mut partial_charge = typed_failure_result();
    partial_charge["charges"][0]["cost"]["usd_micro"] = json!(1);
    assert!(matches!(
        package
            .validate_plan_result_document(&partial_charge)
            .unwrap_err(),
        PublicSeamError::InvalidPlanResult { .. }
    ));
}

#[test]
fn plan_result_preserves_value_visibility_data_classes() {
    let package = package();

    let result = package
        .validate_plan_result_document(&workspace_listing_visibility_result())
        .unwrap();

    assert_eq!(
        result.value_data_classes(),
        &[(
            "listing".to_owned(),
            vec!["case.target".to_owned(), "public".to_owned()]
        )]
    );

    let mut weaker_value = workspace_listing_visibility_result();
    weaker_value["values"]["listing"]["data_classes"] = json!(["public"]);
    assert!(matches!(
        package
            .validate_plan_result_document(&weaker_value)
            .unwrap_err(),
        PublicSeamError::InvalidPlanResult { .. }
    ));
}

#[test]
fn plan_result_preserves_value_trace_and_blob_ref_data_classes() {
    let package = package();

    let mut trace_backed = typed_success_result();
    trace_backed["values"]["rows"]["trace_refs"] = json!([
        {
            "kind": "query_trace",
            "id": "trace_rows",
            "visibility": "redacted_transcript",
            "data_classes": ["transcript.raw"],
            "receipt": "qrec_rows"
        }
    ]);
    trace_backed["values"]["rows"]["data_classes"] = json!(["public", "transcript.raw"]);
    bind_result_hashes_in_place(&mut trace_backed);
    let trace_backed = package
        .validate_plan_result_document(&trace_backed)
        .unwrap();
    assert!(trace_backed.value_data_classes().contains(&(
        "rows".to_owned(),
        vec!["public".to_owned(), "transcript.raw".to_owned()]
    )));

    let mut blob_backed = typed_success_result();
    blob_backed["values"]["rows"] = json!({
        "kind": "workspace_file",
        "path": "artifacts/output.txt",
        "blob_ref": fixture_blob_ref("blob_workspace_file", &["workspace.file"]),
        "graph_revision": "rev_base",
        "data_classes": ["public", "workspace.file"],
        "replayability": "pure_read",
        "receipt": "qrec_rows"
    });
    bind_result_hashes_in_place(&mut blob_backed);
    let blob_backed = package.validate_plan_result_document(&blob_backed).unwrap();
    assert!(blob_backed.value_data_classes().contains(&(
        "rows".to_owned(),
        vec!["public".to_owned(), "workspace.file".to_owned()]
    )));
}

#[test]
fn plan_result_rejects_value_trace_and_blob_ref_data_class_gaps() {
    let package = package();

    let mut missing_value_trace_class = typed_success_result();
    missing_value_trace_class["values"]["rows"]["trace_refs"] = json!([
        {
            "kind": "query_trace",
            "id": "trace_rows",
            "visibility": "redacted_transcript",
            "data_classes": ["transcript.raw"],
            "receipt": "qrec_rows"
        }
    ]);
    bind_result_hashes_in_place(&mut missing_value_trace_class);
    let trace_error = package
        .validate_plan_result_document(&missing_value_trace_class)
        .unwrap_err();
    assert!(matches!(
        &trace_error,
        PublicSeamError::InvalidPlanResult { .. }
    ));
    assert!(
        trace_error
            .to_string()
            .contains("nested visibility data class `transcript.raw`"),
        "{trace_error}"
    );

    let mut missing_workspace_blob_class = typed_success_result();
    missing_workspace_blob_class["values"]["rows"] = json!({
        "kind": "workspace_file",
        "path": "artifacts/output.txt",
        "blob_ref": fixture_blob_ref("blob_workspace_file", &["workspace.file"]),
        "graph_revision": "rev_base",
        "data_classes": ["public"],
        "replayability": "pure_read",
        "receipt": "qrec_rows"
    });
    bind_result_hashes_in_place(&mut missing_workspace_blob_class);
    let blob_error = package
        .validate_plan_result_document(&missing_workspace_blob_class)
        .unwrap_err();
    assert!(matches!(
        &blob_error,
        PublicSeamError::InvalidPlanResult { .. }
    ));
    assert!(
        blob_error
            .to_string()
            .contains("nested visibility data class `workspace.file`"),
        "{blob_error}"
    );
}

fn typed_success_result() -> Value {
    bind_result_hashes(json!({
        "schema_version": "leaven.plan_result.v1",
        "plan_id": "result001",
        "capability_fingerprint": "fp_cap_sha256_resultcap",
        "policy_fingerprint": "fp_policy_sha256_resultpolicy",
        "base_revision": "rev_base",
        "final_revision": "rev_base",
        "replayability_summary": "pure_read",
        "values": {
            "rows": {
                "kind": "graph_set",
                "items": [
                    {
                        "kind": "candidate_summary",
                        "candidate": "cand_alpha",
                        "artifact_identity": "artifact_sha256_alpha"
                    }
                ],
                "graph_revision": "rev_base",
                "data_classes": ["public"],
                "replayability": "pure_read",
                "receipt": "qrec_rows"
            }
        },
        "receipts": [
            {
                "kind": "query",
                "receipt": "qrec_rows",
                "op_var": "rows",
                "started_at": "2026-05-23T12:00:00Z",
                "completed_at": "2026-05-23T12:00:01Z",
                "op_hash": "fp_query_sha256_rows",
                "result_hash": "fp_result_sha256_rows",
                "graph_revision": "rev_base",
                "status": "succeeded",
                "read_scope_fingerprint": "fp_scope_sha256_read",
                "projection_fingerprint": "fp_projection_sha256_rows"
            }
        ],
        "redactions": [],
        "charges": [],
        "errors": []
    }))
}

fn graph_row_fragment_result() -> Value {
    bind_result_hashes(json!({
        "schema_version": "leaven.plan_result.v1",
        "plan_id": "result_graph_fragments001",
        "capability_fingerprint": "fp_cap_sha256_resultcap",
        "policy_fingerprint": "fp_policy_sha256_resultpolicy",
        "base_revision": "rev_base",
        "final_revision": "rev_base",
        "replayability_summary": "pure_read",
        "values": {
            "rows": {
                "kind": "graph_set",
                "items": [
                    {
                        "kind": "candidate_summary",
                        "candidate": "cand_alpha",
                        "artifact_identity": "artifact_sha256_alpha",
                        "scores": {
                            "primary": 0.9,
                            "metrics": {
                                "accuracy": 0.9
                            },
                            "cases": [
                                {
                                    "case": "case_1",
                                    "score": 0.9
                                }
                            ]
                        },
                        "artifact": {
                            "kind": "prompt",
                            "identity": "artifact_sha256_alpha",
                            "summary": "answer concisely",
                            "body": "answer concisely"
                        }
                    },
                    {
                        "kind": "proposal_summary",
                        "proposal": "prop_alpha",
                        "batch": "pb_alpha",
                        "effect": {
                            "kind": "change",
                            "target": "cand_alpha"
                        }
                    },
                    {
                        "kind": "event_summary",
                        "event_kind": "case.loaded",
                        "revision": "rev_base",
                        "payload": {
                            "kind": "external_event",
                            "ok": true
                        }
                    },
                    {
                        "kind": "extension",
                        "namespace": "vendor.eval",
                        "op": "row",
                        "schema_fingerprint": "fp_schema_sha256_vendor_row",
                        "payload": {
                            "kind": "summary",
                            "summary": "vendor score 7",
                            "data_classes": ["public"],
                            "source_ref": {
                                "kind": "candidate",
                                "id": "cand_alpha"
                            }
                        }
                    }
                ],
                "graph_revision": "rev_base",
                "data_classes": ["public"],
                "replayability": "pure_read",
                "receipt": "qrec_rows"
            }
        },
        "receipts": [
            {
                "kind": "query",
                "receipt": "qrec_rows",
                "op_var": "rows",
                "started_at": "2026-05-23T12:00:00Z",
                "completed_at": "2026-05-23T12:00:01Z",
                "op_hash": "fp_query_sha256_rows",
                "result_hash": "fp_result_sha256_rows",
                "graph_revision": "rev_base",
                "status": "succeeded",
                "read_scope_fingerprint": "fp_scope_sha256_read",
                "projection_fingerprint": "fp_projection_sha256_rows"
            }
        ],
        "redactions": [],
        "charges": [],
        "errors": []
    }))
}

fn assessment_summary_result() -> Value {
    bind_result_hashes(json!({
        "schema_version": "leaven.plan_result.v1",
        "plan_id": "result_assessment_summary001",
        "capability_fingerprint": "fp_cap_sha256_resultcap",
        "policy_fingerprint": "fp_policy_sha256_resultpolicy",
        "base_revision": "rev_base",
        "final_revision": "rev_base",
        "replayability_summary": "pure_read",
        "values": {
            "rows": {
                "kind": "graph_set",
                "items": [
                    {
                        "kind": "assessment_summary",
                        "assessment": "assess_alpha",
                        "score": {
                            "value": 1.0,
                            "output": {
                                "kind": "text",
                                "summary": "candidate alpha answer",
                                "value": "candidate alpha answer",
                                "visibility": "public",
                                "data_classes": ["candidate.output"]
                            }
                        },
                        "evidence": {
                            "schema_version": "leaven.evidence_envelope.v1",
                            "target_derived": false,
                            "public": {
                                "summary": "candidate alpha answer",
                                "data_classes": ["public"]
                            },
                            "redaction_policy": {
                                "optimizer": "score_only",
                                "reflector": "score_only",
                                "operator": "score_only"
                            },
                            "producer": {
                                "stage_call_id": "sc_assessment_summary"
                            },
                            "source_receipts": {
                                "read": ["qrec_rows"],
                                "effect": []
                            }
                        }
                    }
                ],
                "graph_revision": "rev_base",
                "data_classes": ["candidate.output", "public"],
                "replayability": "pure_read",
                "receipt": "qrec_rows"
            }
        },
        "receipts": [
            {
                "kind": "query",
                "receipt": "qrec_rows",
                "op_var": "rows",
                "started_at": "2026-05-23T12:00:00Z",
                "completed_at": "2026-05-23T12:00:01Z",
                "op_hash": "fp_query_sha256_rows",
                "result_hash": "fp_result_sha256_rows",
                "graph_revision": "rev_base",
                "status": "succeeded",
                "read_scope_fingerprint": "fp_scope_sha256_read",
                "projection_fingerprint": "fp_projection_sha256_rows"
            }
        ],
        "redactions": [],
        "charges": [],
        "errors": []
    }))
}

fn audit_currency_result() -> Value {
    bind_result_hashes(json!({
        "schema_version": "leaven.plan_result.v1",
        "plan_id": "result_audit_currency",
        "capability_fingerprint": "fp_cap_sha256_auditcurrency",
        "policy_fingerprint": "fp_policy_sha256_auditcurrency",
        "base_revision": "rev_base",
        "final_revision": "rev_final",
        "replayability_summary": "fully_managed",
        "values": {
            "rows": {
                "kind": "graph_set",
                "items": [
                    {
                        "kind": "candidate_summary",
                        "candidate": "cand_audit",
                        "artifact_identity": "artifact_sha256_audit"
                    }
                ],
                "graph_revision": "rev_base",
                "data_classes": ["public"],
                "replayability": "pure_read",
                "receipt": "qrec_audit_rows"
            },
            "agent": {
                "kind": "agent_session",
                "status": "completed",
                "graph_revision": "rev_base",
                "data_classes": ["public"],
                "replayability": "fully_managed",
                "receipt": "agentrec_audit_agent"
            },
            "apply": {
                "kind": "apply_receipt",
                "created_candidates": ["cand_audit_result"],
                "status": "committed",
                "graph_revision": "rev_final",
                "data_classes": ["public"],
                "replayability": "fully_managed",
                "receipt": "wrec_audit_apply"
            }
        },
        "receipts": [
            {
                "kind": "query",
                "receipt": "qrec_audit_rows",
                "started_at": "2026-05-23T12:00:00Z",
                "completed_at": "2026-05-23T12:00:01Z",
                "op_hash": "fp_query_sha256_audit_rows",
                "result_hash": "fp_result_sha256_audit_rows",
                "graph_revision": "rev_base",
                "status": "succeeded",
                "read_scope_fingerprint": "fp_scope_sha256_audit_rows",
                "projection_fingerprint": "fp_projection_sha256_audit_rows"
            },
            {
                "kind": "call",
                "receipt": "agentrec_audit_agent",
                "started_at": "2026-05-23T12:00:01Z",
                "completed_at": "2026-05-23T12:00:02Z",
                "call_kind": "agent_run",
                "request_hash": "fp_request_sha256_audit_agent",
                "result_hash": "fp_result_sha256_audit_agent",
                "runtime_fingerprint": "fp_runtime_sha256_audit_agent",
                "status": "succeeded"
            },
            {
                "kind": "write",
                "receipt": "wrec_audit_apply",
                "op_var": "apply",
                "started_at": "2026-05-23T12:00:02Z",
                "completed_at": "2026-05-23T12:00:03Z",
                "write_kind": "apply_proposal_batch",
                "request_hash": "fp_request_sha256_audit_apply",
                "result_hash": "fp_result_sha256_audit_apply",
                "base_revision": "rev_base",
                "committed_revision": "rev_final",
                "status": "succeeded",
                "created_candidates": ["cand_audit_result"]
            }
        ],
        "redactions": [],
        "charges": [],
        "errors": []
    }))
}

fn workspace_listing_visibility_result() -> Value {
    bind_result_hashes(json!({
        "schema_version": "leaven.plan_result.v1",
        "plan_id": "result_listing_visibility",
        "capability_fingerprint": "fp_cap_sha256_listingvisibility",
        "policy_fingerprint": "fp_policy_sha256_listingvisibility",
        "base_revision": "rev_base",
        "final_revision": "rev_final",
        "replayability_summary": "pure_read",
        "values": {
            "listing": {
                "kind": "workspace_listing",
                "entries": [
                    {
                        "path": "public.txt",
                        "kind": "file",
                        "data_classes": ["public"]
                    },
                    {
                        "path": "target.txt",
                        "kind": "file",
                        "data_classes": ["case.target"]
                    }
                ],
                "graph_revision": "rev_final",
                "data_classes": ["case.target", "public"],
                "replayability": "pure_read",
                "receipt": "qrec_listing_visibility"
            }
        },
        "receipts": [
            {
                "kind": "query",
                "receipt": "qrec_listing_visibility",
                "started_at": "2026-05-23T12:00:00Z",
                "completed_at": "2026-05-23T12:00:01Z",
                "op_hash": "fp_query_sha256_listingvisibility",
                "result_hash": "fp_result_sha256_listingvisibility",
                "graph_revision": "rev_base",
                "status": "succeeded",
                "read_scope_fingerprint": "fp_scope_sha256_listingvisibility",
                "projection_fingerprint": "fp_projection_sha256_listingvisibility"
            }
        ],
        "redactions": [],
        "charges": [],
        "errors": []
    }))
}

fn bind_result_hashes(result: Value) -> Value {
    bind_plan_result_hashes(result)
}

fn bind_result_hashes_in_place(result: &mut Value) {
    *result = bind_result_hashes(std::mem::take(result));
}

fn typed_failure_result() -> Value {
    json!({
        "schema_version": "leaven.plan_result.v1",
        "plan_id": "resultfail001",
        "capability_fingerprint": "fp_cap_sha256_resultcap",
        "policy_fingerprint": "fp_policy_sha256_resultpolicy",
        "base_revision": "rev_base",
        "final_revision": "rev_base",
        "replayability_summary": "has_declared_external_effects",
        "values": {},
        "receipts": [
            {
                "kind": "call",
                "receipt": "lmrec_failed",
                "op_var": "completion",
                "started_at": "2026-05-23T12:00:00Z",
                "completed_at": "2026-05-23T12:00:02Z",
                "call_kind": "lm_complete",
                "request_hash": "fp_request_sha256_lm",
                "result_hash": "fp_result_sha256_lm_failed",
                "runtime_fingerprint": "fp_runtime_sha256_lm",
                "status": "failed",
                "error": {
                    "code": "provider_error",
                    "message": "provider failed",
                    "receipt": "lmrec_failed",
                    "retryable": true
                },
                "cost": {
                    "usd_micro": 100
                },
                "charge_receipts": ["chargerec_lm_failed"]
            }
        ],
        "redactions": [],
        "charges": [
            {
                "receipt": "chargerec_lm_failed",
                "source_receipt": "lmrec_failed",
                "cost": {
                    "usd_micro": 100
                },
                "ledger_scope": "plan",
                "charged_at": "2026-05-23T12:00:02Z"
            }
        ],
        "errors": [
            {
                "code": "provider_error",
                "message": "provider failed",
                "receipt": "lmrec_failed",
                "retryable": true
            }
        ]
    })
}
