use leaven_public_seam::{
    PublicSeamError, PublicSeamPackage, StagePayloadRole, StageProposalEffect,
};
use serde_json::{Value, json};

#[test]
fn stage_payloads_validate_all_role_specific_payload_shapes_with_provenance() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();

    let reflect = package
        .validate_stage_payload_document(&reflect_request())
        .unwrap();
    assert_eq!(reflect.role(), StagePayloadRole::Reflector);
    assert_eq!(reflect.reflective_example_count(), 1);
    assert_eq!(
        reflect.query_policy_fingerprint(),
        Some("fp_policy_sha256_stagepayload")
    );

    let reflection = package
        .validate_stage_payload_document(&reflection_result())
        .unwrap();
    assert_eq!(reflection.role(), StagePayloadRole::ReflectionResult);
    assert_eq!(reflection.read_receipt_count(), 1);
    assert_eq!(
        reflection.read_receipts(),
        &["qrec_stagepayload_lineage".to_owned()]
    );
    assert_eq!(reflection.data_classes(), &["optimizer.visible".to_owned()]);

    let propose = package
        .validate_stage_payload_document(&propose_request())
        .unwrap();
    assert_eq!(propose.role(), StagePayloadRole::Proposer);
    assert_eq!(propose.allowed_effects(), &[StageProposalEffect::Change]);
    assert_eq!(propose.allowed_change_schema_count(), 1);

    let runner = package
        .validate_stage_payload_document(&runner_request())
        .unwrap();
    assert_eq!(runner.role(), StagePayloadRole::Runner);

    let scorer = package
        .validate_stage_payload_document(&score_context())
        .unwrap();
    assert_eq!(scorer.role(), StagePayloadRole::Scorer);
    assert_eq!(scorer.output_count(), 1);

    let judge = package
        .validate_stage_payload_document(&judge_context())
        .unwrap();
    assert_eq!(judge.role(), StagePayloadRole::Judge);
    assert_eq!(judge.output_count(), 2);

    let callback = package
        .validate_stage_payload_document(&callback_request())
        .unwrap();
    assert_eq!(callback.role(), StagePayloadRole::Callback);
    assert_eq!(
        callback.payload_schema(),
        Some("fp_schema_sha256_callbackpayload")
    );

    let adapter = package
        .validate_stage_payload_document(&adapter_request("artifact_adapter"))
        .unwrap();
    assert_eq!(adapter.role(), StagePayloadRole::ArtifactAdapter);
    assert_eq!(
        adapter.payload_schema(),
        Some("fp_schema_sha256_adapterpayload")
    );

    let dataset_adapter = package
        .validate_stage_payload_document(&adapter_request("dataset_adapter"))
        .unwrap();
    assert_eq!(dataset_adapter.role(), StagePayloadRole::DatasetAdapter);

    let mut benign_target_word = reflect_request();
    benign_target_word["examples"][0]["side_info"]["target"] = json!("accuracy");
    package
        .validate_stage_payload_document(&benign_target_word)
        .unwrap();
}

#[test]
fn stage_payloads_preserve_object_form_info_and_receipt_refs() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();

    let mut reflect = reflect_request();
    reflect["parent"] = object_form_candidate_ref();
    reflect["source_refs"] = json!([object_form_candidate_ref()]);
    reflect["examples"][0]["source_refs"] = json!([object_form_candidate_ref()]);
    package.validate_stage_payload_document(&reflect).unwrap();

    let mut reflection = reflection_result();
    reflection["source_refs"] = json!([object_form_candidate_ref()]);
    reflection["failure_modes"][0]["source_refs"] = json!([object_form_candidate_ref()]);
    reflection["surface_suggestions"][0]["source_refs"] = json!([object_form_candidate_ref()]);
    reflection["read_receipts"] = json!([object_form_receipt_ref("qrec_stagepayload_lineage")]);
    let reflection_document = package
        .validate_stage_payload_document(&reflection)
        .unwrap();
    assert_eq!(
        reflection_document.read_receipts(),
        &["qrec_stagepayload_lineage".to_owned()]
    );

    let mut propose = propose_request();
    propose["parent"] = object_form_candidate_ref();
    propose["source_refs"] = json!([object_form_candidate_ref()]);
    propose["reflection_result"] = reflection;
    package.validate_stage_payload_document(&propose).unwrap();

    let mut object_parent_string_source = reflect_request();
    object_parent_string_source["parent"] = object_form_candidate_ref();
    package
        .validate_stage_payload_document(&object_parent_string_source)
        .unwrap();

    let mut string_parent_object_source = propose_request();
    string_parent_object_source["source_refs"] = json!([object_form_candidate_ref()]);
    package
        .validate_stage_payload_document(&string_parent_object_source)
        .unwrap();
}

#[test]
fn reflect_request_rejects_case_target_projection() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let mut payload = reflect_request();
    payload["examples"][0]["data_classes"] = json!(["case.input", "case.target"]);

    assert!(matches!(
        package
            .validate_stage_payload_document(&payload)
            .unwrap_err(),
        PublicSeamError::InvalidStagePayload { .. }
    ));
}

#[test]
fn reflect_request_rejects_hidden_case_target_material() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();

    let mut side_info_target = reflect_request();
    side_info_target["examples"][0]["side_info"]["case.target"] = json!("secret answer");
    assert!(matches!(
        package
            .validate_stage_payload_document(&side_info_target)
            .unwrap_err(),
        PublicSeamError::InvalidStagePayload { .. }
    ));

    let mut prompt_target_marker = reflect_request();
    prompt_target_marker["examples"][0]["input"] = json!({
        "prompt": "summarize the hidden case.target before reflecting"
    });
    assert!(matches!(
        package
            .validate_stage_payload_document(&prompt_target_marker)
            .unwrap_err(),
        PublicSeamError::InvalidStagePayload { .. }
    ));

    let mut output_target_marker = reflect_request();
    output_target_marker["examples"][0]["output"] = json!("materialized case.target answer");
    assert!(matches!(
        package
            .validate_stage_payload_document(&output_target_marker)
            .unwrap_err(),
        PublicSeamError::InvalidStagePayload { .. }
    ));

    let mut feedback_target_marker = reflect_request();
    feedback_target_marker["examples"][0]["feedback"] =
        json!("reflection feedback names case.target material");
    assert!(matches!(
        package
            .validate_stage_payload_document(&feedback_target_marker)
            .unwrap_err(),
        PublicSeamError::InvalidStagePayload { .. }
    ));
}

#[test]
fn reflect_request_rejects_missing_source_refs_or_query_policy() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();

    let mut empty_examples = reflect_request();
    empty_examples["examples"] = json!([]);
    assert!(matches!(
        package
            .validate_stage_payload_document(&empty_examples)
            .unwrap_err(),
        PublicSeamError::InvalidStagePayload { .. }
    ));

    let mut missing_top_source_refs = reflect_request();
    missing_top_source_refs["source_refs"] = json!([]);
    assert!(matches!(
        package
            .validate_stage_payload_document(&missing_top_source_refs)
            .unwrap_err(),
        PublicSeamError::InvalidStagePayload { .. }
    ));

    let mut missing_example_source_refs = reflect_request();
    missing_example_source_refs["examples"][0]["source_refs"] = json!([]);
    assert!(matches!(
        package
            .validate_stage_payload_document(&missing_example_source_refs)
            .unwrap_err(),
        PublicSeamError::InvalidStagePayload { .. }
    ));

    let mut missing_example_data_classes = reflect_request();
    missing_example_data_classes["examples"][0]["data_classes"] = json!([]);
    assert!(matches!(
        package
            .validate_stage_payload_document(&missing_example_data_classes)
            .unwrap_err(),
        PublicSeamError::InvalidStagePayload { .. }
    ));

    let mut score_output_data_class_gap = reflect_request();
    score_output_data_class_gap["examples"][0]["score"]["output"]["data_classes"] =
        json!(["candidate.output", "external.secret"]);
    assert!(matches!(
        package
            .validate_stage_payload_document(&score_output_data_class_gap)
            .unwrap_err(),
        PublicSeamError::InvalidStagePayload { .. }
    ));

    let mut score_output_blob_ref_gap = reflect_request();
    score_output_blob_ref_gap["examples"][0]["score"]["output"]["blob_ref"] = json!({
        "kind": "blob_ref",
        "id": "blob_score_output",
        "sha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        "bytes": 32,
        "data_classes": ["external.secret"]
    });
    assert!(matches!(
        package
            .validate_stage_payload_document(&score_output_blob_ref_gap)
            .unwrap_err(),
        PublicSeamError::InvalidStagePayload { .. }
    ));

    let mut score_output_trace_ref_gap = reflect_request();
    score_output_trace_ref_gap["examples"][0]["score"]["output"]["trace_refs"] = json!([
        {
            "kind": "runner_trace",
            "id": "trace_score_output",
            "visibility": "redacted_completion",
            "data_classes": ["completion.raw"],
            "receipt": "lmrec_score"
        }
    ]);
    assert!(matches!(
        package
            .validate_stage_payload_document(&score_output_trace_ref_gap)
            .unwrap_err(),
        PublicSeamError::InvalidStagePayload { .. }
    ));

    let mut domain_value_data_classes = reflect_request();
    domain_value_data_classes["examples"][0]["score"]["output"]["value"] = json!({
        "domain_label": "classification",
        "data_classes": ["external.secret"]
    });
    package
        .validate_stage_payload_document(&domain_value_data_classes)
        .unwrap();

    let mut uncarried_example_source_ref = reflect_request();
    uncarried_example_source_ref["source_refs"] = json!(["cand_unrelated"]);
    assert!(matches!(
        package
            .validate_stage_payload_document(&uncarried_example_source_ref)
            .unwrap_err(),
        PublicSeamError::InvalidStagePayload { .. }
    ));

    let mut missing_query_policy = reflect_request();
    missing_query_policy
        .as_object_mut()
        .unwrap()
        .remove("query_policy_fingerprint");
    assert!(matches!(
        package
            .validate_stage_payload_document(&missing_query_policy)
            .unwrap_err(),
        PublicSeamError::InvalidStagePayload { .. }
    ));
}

#[test]
fn reflect_request_rejects_parent_source_or_surface_context_gaps() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();

    let mut parent_not_in_source_refs = reflect_request();
    parent_not_in_source_refs["source_refs"] = json!(["cand_unrelated"]);
    parent_not_in_source_refs["examples"][0]["source_refs"] = json!(["cand_unrelated"]);
    assert!(matches!(
        package
            .validate_stage_payload_document(&parent_not_in_source_refs)
            .unwrap_err(),
        PublicSeamError::InvalidStagePayload { .. }
    ));

    let mut missing_surface_fingerprint = reflect_request();
    missing_surface_fingerprint
        .as_object_mut()
        .unwrap()
        .remove("surface_fingerprint");
    assert!(matches!(
        package
            .validate_stage_payload_document(&missing_surface_fingerprint)
            .unwrap_err(),
        PublicSeamError::InvalidStagePayload { .. }
    ));

    let mut missing_part_context = reflect_request();
    missing_part_context.as_object_mut().unwrap().remove("part");
    missing_part_context
        .as_object_mut()
        .unwrap()
        .remove("part_label");
    assert!(matches!(
        package
            .validate_stage_payload_document(&missing_part_context)
            .unwrap_err(),
        PublicSeamError::InvalidStagePayload { .. }
    ));
}

#[test]
fn reflection_result_requires_receipted_source_visible_evidence() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();

    let mut missing_receipt = reflection_result();
    missing_receipt["read_receipts"] = json!([]);
    assert!(matches!(
        package
            .validate_stage_payload_document(&missing_receipt)
            .unwrap_err(),
        PublicSeamError::InvalidStagePayload { .. }
    ));

    let mut effect_receipt_in_read_slot = reflection_result();
    effect_receipt_in_read_slot["read_receipts"] = json!(["lmrec_stagepayload"]);
    assert!(matches!(
        package
            .validate_stage_payload_document(&effect_receipt_in_read_slot)
            .unwrap_err(),
        PublicSeamError::InvalidStagePayload { .. }
    ));

    let mut object_form_effect_receipt_in_read_slot = reflection_result();
    object_form_effect_receipt_in_read_slot["read_receipts"] =
        json!([object_form_receipt_ref("agentrec_stagepayload")]);
    assert!(matches!(
        package
            .validate_stage_payload_document(&object_form_effect_receipt_in_read_slot)
            .unwrap_err(),
        PublicSeamError::InvalidStagePayload { .. }
    ));

    let mut missing_data_class = reflection_result();
    missing_data_class["data_classes"] = json!([]);
    assert!(matches!(
        package
            .validate_stage_payload_document(&missing_data_class)
            .unwrap_err(),
        PublicSeamError::InvalidStagePayload { .. }
    ));
}

#[test]
fn reflection_result_rejects_unproven_diagnosis_without_sources() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();

    let mut no_source_backed_diagnosis = reflection_result();
    no_source_backed_diagnosis["failure_modes"] = json!([]);
    no_source_backed_diagnosis["surface_suggestions"] = json!([]);
    assert!(matches!(
        package
            .validate_stage_payload_document(&no_source_backed_diagnosis)
            .unwrap_err(),
        PublicSeamError::InvalidStagePayload { .. }
    ));

    let mut unproven_failure_mode = reflection_result();
    unproven_failure_mode["failure_modes"][0]["source_refs"] = json!([]);
    assert!(matches!(
        package
            .validate_stage_payload_document(&unproven_failure_mode)
            .unwrap_err(),
        PublicSeamError::InvalidStagePayload { .. }
    ));

    let mut missing_failure_source_refs = reflection_result();
    missing_failure_source_refs["failure_modes"][0]
        .as_object_mut()
        .unwrap()
        .remove("source_refs");
    assert!(matches!(
        package
            .validate_stage_payload_document(&missing_failure_source_refs)
            .unwrap_err(),
        PublicSeamError::InvalidStagePayload { .. }
    ));

    let mut unproven_surface_suggestion = reflection_result();
    unproven_surface_suggestion["surface_suggestions"][0]["source_refs"] = json!([]);
    assert!(matches!(
        package
            .validate_stage_payload_document(&unproven_surface_suggestion)
            .unwrap_err(),
        PublicSeamError::InvalidStagePayload { .. }
    ));

    let mut missing_surface_source_refs = reflection_result();
    missing_surface_source_refs["surface_suggestions"][0]
        .as_object_mut()
        .unwrap()
        .remove("source_refs");
    assert!(matches!(
        package
            .validate_stage_payload_document(&missing_surface_source_refs)
            .unwrap_err(),
        PublicSeamError::InvalidStagePayload { .. }
    ));

    let mut uncarried_diagnosis_source_ref = reflection_result();
    uncarried_diagnosis_source_ref["failure_modes"][0]["source_refs"] = json!(["cand_unrelated"]);
    assert!(matches!(
        package
            .validate_stage_payload_document(&uncarried_diagnosis_source_ref)
            .unwrap_err(),
        PublicSeamError::InvalidStagePayload { .. }
    ));
}

#[test]
fn score_context_rejects_target_handle_for_unrelated_case() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();

    let mut mismatched_target = score_context();
    mismatched_target["target_handle"] = json!("case_unrelated");
    assert!(matches!(
        package
            .validate_stage_payload_document(&mismatched_target)
            .unwrap_err(),
        PublicSeamError::InvalidStagePayload { .. }
    ));
}

#[test]
fn stage_score_contexts_reject_public_only_assessed_outputs() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();

    let mut reflect_public_only = reflect_request();
    reflect_public_only["examples"][0]["data_classes"] = json!(["case.input", "public"]);
    reflect_public_only["examples"][0]["score"]["output"]["data_classes"] = json!(["public"]);
    assert!(matches!(
        package
            .validate_stage_payload_document(&reflect_public_only)
            .unwrap_err(),
        PublicSeamError::InvalidStagePayload { .. }
    ));

    let mut scorer_public_only = score_context();
    scorer_public_only["output"]["data_classes"] = json!(["public"]);
    assert!(matches!(
        package
            .validate_stage_payload_document(&scorer_public_only)
            .unwrap_err(),
        PublicSeamError::InvalidStagePayload { .. }
    ));

    let mut judge_public_only = judge_context();
    judge_public_only["outputs"][1]["data_classes"] = json!(["public"]);
    assert!(matches!(
        package
            .validate_stage_payload_document(&judge_public_only)
            .unwrap_err(),
        PublicSeamError::InvalidStagePayload { .. }
    ));

    let mut scorer_artifact = score_context();
    scorer_artifact["output"]["data_classes"] = json!(["candidate.artifact", "public"]);
    package
        .validate_stage_payload_document(&scorer_artifact)
        .unwrap();
}

#[test]
fn propose_request_requires_reflection_result_and_change_schema_authority() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();

    let mut missing_source_refs = propose_request();
    missing_source_refs["source_refs"] = json!([]);
    assert!(matches!(
        package
            .validate_stage_payload_document(&missing_source_refs)
            .unwrap_err(),
        PublicSeamError::InvalidStagePayload { .. }
    ));

    let mut missing_query_policy = propose_request();
    missing_query_policy
        .as_object_mut()
        .unwrap()
        .remove("query_policy_fingerprint");
    assert!(matches!(
        package
            .validate_stage_payload_document(&missing_query_policy)
            .unwrap_err(),
        PublicSeamError::InvalidStagePayload { .. }
    ));

    let mut missing_surface_fingerprint = propose_request();
    missing_surface_fingerprint
        .as_object_mut()
        .unwrap()
        .remove("surface_fingerprint");
    assert!(matches!(
        package
            .validate_stage_payload_document(&missing_surface_fingerprint)
            .unwrap_err(),
        PublicSeamError::InvalidStagePayload { .. }
    ));

    let mut parent_not_in_source_refs = propose_request();
    parent_not_in_source_refs["source_refs"] = json!(["cand_unrelated"]);
    assert!(matches!(
        package
            .validate_stage_payload_document(&parent_not_in_source_refs)
            .unwrap_err(),
        PublicSeamError::InvalidStagePayload { .. }
    ));

    let mut missing_change_schema = propose_request();
    missing_change_schema["allowed_change_schemas"] = json!([]);
    assert!(matches!(
        package
            .validate_stage_payload_document(&missing_change_schema)
            .unwrap_err(),
        PublicSeamError::InvalidStagePayload { .. }
    ));

    let mut unsupported_effect = propose_request();
    unsupported_effect["allowed_effects"] = json!(["mutate_graph_directly"]);
    assert!(matches!(
        package
            .validate_stage_payload_document(&unsupported_effect)
            .unwrap_err(),
        PublicSeamError::ExampleValidation { .. } | PublicSeamError::InvalidStagePayload { .. }
    ));

    let mut wrong_bridge = propose_request();
    wrong_bridge["reflection_result"] = reflect_request();
    assert!(matches!(
        package
            .validate_stage_payload_document(&wrong_bridge)
            .unwrap_err(),
        PublicSeamError::ExampleValidation { .. } | PublicSeamError::InvalidStagePayload { .. }
    ));

    let mut unreceipted_reflection = propose_request();
    unreceipted_reflection["reflection_result"]["read_receipts"] = json!([]);
    assert!(matches!(
        package
            .validate_stage_payload_document(&unreceipted_reflection)
            .unwrap_err(),
        PublicSeamError::InvalidStagePayload { .. }
    ));

    let mut diagnosis_free_reflection = propose_request();
    diagnosis_free_reflection["reflection_result"]["failure_modes"] = json!([]);
    diagnosis_free_reflection["reflection_result"]["surface_suggestions"] = json!([]);
    assert!(matches!(
        package
            .validate_stage_payload_document(&diagnosis_free_reflection)
            .unwrap_err(),
        PublicSeamError::InvalidStagePayload { .. }
    ));
}

#[test]
fn propose_request_rejects_reflection_source_ref_drop() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();

    let mut dropped_reflection_source = propose_request();
    dropped_reflection_source["source_refs"] = json!(["cand_unrelated"]);
    assert!(matches!(
        package
            .validate_stage_payload_document(&dropped_reflection_source)
            .unwrap_err(),
        PublicSeamError::InvalidStagePayload { .. }
    ));
}

#[test]
fn reflect_propose_handoff_binds_distinct_stage_calls_and_exact_reflection_result() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();

    let handoff = package
        .validate_reflect_propose_handoff_document(&reflect_propose_handoff())
        .unwrap();
    assert_eq!(handoff.run(), "run_stagepayload");
    assert_eq!(handoff.reflect_stage_call_id(), "sc_reflect_stagepayload");
    assert_eq!(handoff.propose_stage_call_id(), "sc_propose_stagepayload");
    assert_eq!(handoff.base_revision(), "rev_stagepayload_base");
    assert_eq!(handoff.parent(), "candidate:cand_stagepayload_parent");
    assert_eq!(
        handoff.surface_fingerprint(),
        "fp_surface_sha256_stagepayload"
    );
    assert_eq!(
        handoff.capability_fingerprint(),
        "fp_cap_sha256_stagepayload"
    );
    assert_eq!(
        handoff.query_policy_fingerprint(),
        "fp_policy_sha256_stagepayload"
    );
    assert_eq!(handoff.reflection_source_ref_count(), 1);
}

#[test]
fn reflect_propose_handoff_rejects_single_prompt_and_stale_reflection_fakes() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();

    let mut same_stage_call = reflect_propose_handoff();
    same_stage_call["propose_request"]["stage_call_id"] = json!("sc_reflect_stagepayload");
    assert!(matches!(
        package
            .validate_reflect_propose_handoff_document(&same_stage_call)
            .unwrap_err(),
        PublicSeamError::InvalidStagePayload { .. }
    ));

    let mut stale_embedded_reflection = reflect_propose_handoff();
    stale_embedded_reflection["propose_request"]["reflection_result"]["summary"] =
        json!("A stale summary from a different reflection.");
    assert!(matches!(
        package
            .validate_reflect_propose_handoff_document(&stale_embedded_reflection)
            .unwrap_err(),
        PublicSeamError::InvalidStagePayload { .. }
    ));

    let mut mismatched_run = reflect_propose_handoff();
    mismatched_run["propose_request"]["run"] = json!("run_other");
    assert!(matches!(
        package
            .validate_reflect_propose_handoff_document(&mismatched_run)
            .unwrap_err(),
        PublicSeamError::InvalidStagePayload { .. }
    ));

    let mut mismatched_base_revision = reflect_propose_handoff();
    mismatched_base_revision["propose_request"]["base_revision"] = json!("rev_other");
    assert!(matches!(
        package
            .validate_reflect_propose_handoff_document(&mismatched_base_revision)
            .unwrap_err(),
        PublicSeamError::InvalidStagePayload { .. }
    ));

    let mut mismatched_parent = reflect_propose_handoff();
    mismatched_parent["propose_request"]["parent"] = json!("cand_other");
    mismatched_parent["propose_request"]["source_refs"] =
        json!(["cand_stagepayload_parent", "cand_other"]);
    assert!(matches!(
        package
            .validate_reflect_propose_handoff_document(&mismatched_parent)
            .unwrap_err(),
        PublicSeamError::InvalidStagePayload { .. }
    ));

    let mut mismatched_surface = reflect_propose_handoff();
    mismatched_surface["propose_request"]["surface_fingerprint"] = json!("fp_surface_sha256_other");
    assert!(matches!(
        package
            .validate_reflect_propose_handoff_document(&mismatched_surface)
            .unwrap_err(),
        PublicSeamError::InvalidStagePayload { .. }
    ));

    let mut mismatched_capability = reflect_propose_handoff();
    mismatched_capability["propose_request"]["capability_fingerprint"] =
        json!("fp_cap_sha256_other");
    assert!(matches!(
        package
            .validate_reflect_propose_handoff_document(&mismatched_capability)
            .unwrap_err(),
        PublicSeamError::InvalidStagePayload { .. }
    ));

    let mut mismatched_query_policy = reflect_propose_handoff();
    mismatched_query_policy["propose_request"]["query_policy_fingerprint"] =
        json!("fp_policy_sha256_other");
    assert!(matches!(
        package
            .validate_reflect_propose_handoff_document(&mismatched_query_policy)
            .unwrap_err(),
        PublicSeamError::InvalidStagePayload { .. }
    ));

    let mut uncovered_reflection_source = reflect_propose_handoff();
    uncovered_reflection_source["reflection_result"]["source_refs"] = json!(["cand_unrelated"]);
    uncovered_reflection_source["reflection_result"]["failure_modes"][0]["source_refs"] =
        json!(["cand_unrelated"]);
    uncovered_reflection_source["reflection_result"]["surface_suggestions"][0]["source_refs"] =
        json!(["cand_unrelated"]);
    uncovered_reflection_source["propose_request"]["reflection_result"] =
        uncovered_reflection_source["reflection_result"].clone();
    uncovered_reflection_source["propose_request"]["source_refs"] =
        json!(["cand_stagepayload_parent", "cand_unrelated"]);
    assert!(matches!(
        package
            .validate_reflect_propose_handoff_document(&uncovered_reflection_source)
            .unwrap_err(),
        PublicSeamError::InvalidStagePayload { .. }
    ));
}

#[test]
fn runner_request_rejects_case_target_material() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();

    let mut hidden_target = runner_request();
    hidden_target["case_input"]["case.target"] = json!("secret answer");
    assert!(matches!(
        package
            .validate_stage_payload_document(&hidden_target)
            .unwrap_err(),
        PublicSeamError::InvalidStagePayload { .. }
    ));

    let mut target_marker = runner_request();
    target_marker["case_input"] = json!({
        "question": "q",
        "evidence": "runner prompt asks for case.target"
    });
    assert!(matches!(
        package
            .validate_stage_payload_document(&target_marker)
            .unwrap_err(),
        PublicSeamError::InvalidStagePayload { .. }
    ));
}

#[test]
fn callback_and_adapter_payloads_reject_missing_payload_schema() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();

    let mut callback = callback_request();
    callback.as_object_mut().unwrap().remove("payload_schema");
    assert!(matches!(
        package
            .validate_stage_payload_document(&callback)
            .unwrap_err(),
        PublicSeamError::ExampleValidation { .. } | PublicSeamError::InvalidStagePayload { .. }
    ));

    let mut adapter = adapter_request("artifact_adapter");
    adapter.as_object_mut().unwrap().remove("payload_schema");
    assert!(matches!(
        package
            .validate_stage_payload_document(&adapter)
            .unwrap_err(),
        PublicSeamError::ExampleValidation { .. } | PublicSeamError::InvalidStagePayload { .. }
    ));
}

#[test]
fn active_reflect_then_propose_example_validates_through_semantic_stage_payloads() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let example: Value = serde_json::from_str(
        &std::fs::read_to_string(
            workspace_root()
                .join("docs/specs/public-seam-v1/examples/reflect_then_propose.example.json"),
        )
        .unwrap(),
    )
    .unwrap();

    package
        .validate_stage_payload_document(&example["reflect_request"])
        .unwrap();
    package
        .validate_stage_payload_document(&example["reflection_result"])
        .unwrap();
    package
        .validate_stage_payload_document(&example["propose_request"])
        .unwrap();
    let handoff = package
        .validate_reflect_propose_handoff_document(&example)
        .unwrap();
    assert_eq!(handoff.reflect_stage_call_id(), "sc_reflect_01");
    assert_eq!(handoff.propose_stage_call_id(), "sc_propose_01");
}

fn reflect_propose_handoff() -> Value {
    json!({
        "reflect_request": reflect_request(),
        "reflection_result": reflection_result(),
        "propose_request": propose_request()
    })
}

fn reflect_request() -> Value {
    json!({
        "schema_version": "leaven.stage_payloads.v1",
        "role": "reflector",
        "run": "run_stagepayload",
        "stage_call_id": "sc_reflect_stagepayload",
        "base_revision": "rev_stagepayload_base",
        "parent": "cand_stagepayload_parent",
        "part": {"path": "/prompt"},
        "part_label": "prompt",
        "surface_fingerprint": "fp_surface_sha256_stagepayload",
        "examples": [
            {
                "case": "case_stagepayload",
                "input": "question",
                "output": "answer",
                "score": {
                    "value": 0.25,
                    "output": {
                        "kind": "text",
                        "summary": "answer",
                        "value": "answer",
                        "visibility": "public",
                        "data_classes": ["candidate.output"]
                    }
                },
                "feedback": "missed edge case",
                "side_info": {"difficulty": "easy"},
                "source_refs": ["cand_stagepayload_parent"],
                "data_classes": ["case.input", "candidate.output"],
                "evidence_visibility": "score_and_feedback"
            }
        ],
        "source_refs": ["cand_stagepayload_parent"],
        "attempt_index": 1,
        "target_safety": "target_safe_projection",
        "query_policy_fingerprint": "fp_policy_sha256_stagepayload",
        "capability_fingerprint": "fp_cap_sha256_stagepayload"
    })
}

fn reflection_result() -> Value {
    json!({
        "schema_version": "leaven.stage_payloads.v1",
        "role": "reflection_result",
        "summary": "The parent misses an empty-input guard.",
        "failure_modes": [
            {
                "label": "empty_input_guard",
                "description": "No early return for empty input.",
                "severity": "high",
                "source_refs": ["cand_stagepayload_parent"]
            }
        ],
        "surface_suggestions": [
            {
                "surface_fingerprint": "fp_surface_sha256_stagepayload",
                "part_label": "prompt",
                "diagnosis": "guard missing",
                "suggested_direction": "add explicit empty input guard",
                "constraints": ["keep public behavior unchanged"],
                "source_refs": ["cand_stagepayload_parent"]
            }
        ],
        "negative_constraints": ["do not read case targets"],
        "positive_constraints": ["preserve output shape"],
        "source_refs": ["cand_stagepayload_parent"],
        "read_receipts": ["qrec_stagepayload_lineage"],
        "data_classes": ["optimizer.visible"],
        "confidence": 0.82
    })
}

fn propose_request() -> Value {
    json!({
        "schema_version": "leaven.stage_payloads.v1",
        "role": "proposer",
        "run": "run_stagepayload",
        "stage_call_id": "sc_propose_stagepayload",
        "base_revision": "rev_stagepayload_base",
        "parent": "cand_stagepayload_parent",
        "surface_fingerprint": "fp_surface_sha256_stagepayload",
        "reflection_result": reflection_result(),
        "allowed_effects": ["change"],
        "allowed_change_schemas": ["fp_schema_sha256_stagepatch"],
        "source_refs": ["cand_stagepayload_parent"],
        "query_policy_fingerprint": "fp_policy_sha256_stagepayload",
        "capability_fingerprint": "fp_cap_sha256_stagepayload"
    })
}

fn runner_request() -> Value {
    json!({
        "schema_version": "leaven.stage_payloads.v1",
        "role": "runner",
        "run": "run_stagepayload",
        "stage_call_id": "sc_runner_stagepayload",
        "candidate": "cand_stagepayload_parent",
        "case": "case_stagepayload",
        "case_input": {"question": "target-free"},
        "target_forbidden": true
    })
}

fn score_context() -> Value {
    json!({
        "schema_version": "leaven.stage_payloads.v1",
        "role": "scorer",
        "run": "run_stagepayload",
        "stage_call_id": "sc_score_stagepayload",
        "evaluation_request_id": "evalreq_stagepayload",
        "candidate": "cand_stagepayload_parent",
        "case": "case_stagepayload",
        "output": output_record("answer"),
        "target_handle": "case_stagepayload",
        "capability_fingerprint": "fp_cap_sha256_stagepayload"
    })
}

fn judge_context() -> Value {
    json!({
        "schema_version": "leaven.stage_payloads.v1",
        "role": "judge",
        "run": "run_stagepayload",
        "stage_call_id": "sc_judge_stagepayload",
        "left": "cand_stagepayload_left",
        "right": "cand_stagepayload_right",
        "case": "case_stagepayload",
        "outputs": [output_record("left answer"), output_record("right answer")],
        "rubric": {"kind": "preference"},
        "capability_fingerprint": "fp_cap_sha256_stagepayload"
    })
}

fn callback_request() -> Value {
    json!({
        "schema_version": "leaven.stage_payloads.v1",
        "role": "callback",
        "run": "run_stagepayload",
        "stage_call_id": "sc_callback_stagepayload",
        "event": {"kind": "stage_completed"},
        "payload_schema": "fp_schema_sha256_callbackpayload",
        "capability_fingerprint": "fp_cap_sha256_stagepayload"
    })
}

fn adapter_request(role: &str) -> Value {
    json!({
        "schema_version": "leaven.stage_payloads.v1",
        "role": role,
        "run": "run_stagepayload",
        "stage_call_id": "sc_adapter_stagepayload",
        "payload_schema": "fp_schema_sha256_adapterpayload",
        "payload": {"kind": "lower"},
        "capability_fingerprint": "fp_cap_sha256_stagepayload"
    })
}

fn output_record(summary: &str) -> Value {
    json!({
        "kind": "text",
        "summary": summary,
        "value": summary,
        "visibility": "public",
        "data_classes": ["candidate.output"]
    })
}

fn object_form_candidate_ref() -> Value {
    json!({
        "kind": "candidate",
        "run": "run_stagepayload",
        "id": "cand_stagepayload_parent"
    })
}

fn object_form_receipt_ref(id: &str) -> Value {
    json!({
        "kind": "receipt",
        "id": id,
        "fingerprint": "fp_receipt_sha256_stagepayload"
    })
}

fn workspace_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .unwrap()
        .to_path_buf()
}
