use leaven_agentic::{
    AdapterRequestPayload, CallbackRequestPayload, JudgeContextPayload, JudgeContextPayloadFields,
    ProposeRequestPayload, PublicStagePayloadError, PublicStagePayloadIdentity,
    PublicStagePayloadIdentityFields, ReflectProposeHandoffPayload, ReflectRequestPayload,
    ReflectionResultPayload, RunnerRequestPayload, ScorerContextPayload,
    ScorerContextPayloadFields,
};
use serde_json::{Value, json};

#[test]
fn agentic_reflect_propose_handoff_lowers_through_locked_public_seam_owner() {
    let package =
        leaven_public_seam::PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let reflect = ReflectRequestPayload::new(
        identity("sc_reflect_agentic"),
        "input_validation",
        [reflective_example()],
    )
    .unwrap();
    let reflection = reflection_result("qrec_lineage_agentic");
    let propose = ProposeRequestPayload::from_reflection(
        identity("sc_propose_agentic"),
        reflection.clone(),
        ["change".to_owned()],
        ["fp_schema_sha256_skill_patch".to_owned()],
    )
    .unwrap();
    let handoff = ReflectProposeHandoffPayload::new(
        &reflect,
        &reflection,
        &propose,
        "stagerec_reflect_agentic",
        "stagerec_propose_agentic",
    )
    .unwrap();

    let document = package
        .validate_reflect_propose_handoff_document(handoff.value())
        .unwrap();
    assert_eq!(document.reflect_stage_call_id(), "sc_reflect_agentic");
    assert_eq!(document.propose_stage_call_id(), "sc_propose_agentic");
    assert_eq!(
        document.reflection_result_fingerprint(),
        reflection.fingerprint()
    );
    assert_eq!(document.reflection_source_ref_count(), 1);
}

#[test]
fn agentic_reflect_propose_handoff_rejects_single_prompt_and_stale_reflection_fakes() {
    let package =
        leaven_public_seam::PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let reflect = ReflectRequestPayload::new(
        identity("sc_reflect_agentic"),
        "input_validation",
        [reflective_example()],
    )
    .unwrap();
    let reflection = reflection_result("qrec_lineage_agentic");

    let no_effects = ProposeRequestPayload::from_reflection(
        identity("sc_propose_agentic"),
        reflection.clone(),
        [],
        ["fp_schema_sha256_skill_patch".to_owned()],
    )
    .unwrap_err();
    assert!(matches!(
        no_effects,
        PublicStagePayloadError::EmptyField {
            field: "allowed_effects"
        }
    ));

    let target_leaking_reflect = ReflectRequestPayload::new(
        identity("sc_reflect_agentic"),
        "input_validation",
        [json!({
            "case": "case_visible_empty",
            "input": {"text": ""},
            "output": "materialized case.target answer",
            "score": {"value": 0.0, "output": output_record("leaked target")},
            "source_refs": ["cand_parent"],
            "data_classes": ["case.input", "case.target"]
        })],
    )
    .unwrap_err();
    assert!(matches!(
        target_leaking_reflect,
        PublicStagePayloadError::TargetLeakage { field: "examples" }
    ));

    let propose = ProposeRequestPayload::from_reflection(
        identity("sc_propose_agentic"),
        reflection.clone(),
        ["change".to_owned()],
        ["fp_schema_sha256_skill_patch".to_owned()],
    )
    .unwrap();
    let mut stale_handoff = ReflectProposeHandoffPayload::new(
        &reflect,
        &reflection,
        &propose,
        "stagerec_reflect_agentic",
        "stagerec_propose_agentic",
    )
    .unwrap()
    .into_value();
    stale_handoff["propose_request"]["reflection_result"]["summary"] = json!("stale shortcut");

    assert!(matches!(
        package
            .validate_reflect_propose_handoff_document(&stale_handoff)
            .unwrap_err(),
        leaven_public_seam::PublicSeamError::InvalidStagePayload { .. }
    ));
}

#[test]
fn agentic_role_payloads_lower_remaining_stage_roles_through_locked_public_seam_owner() {
    let package =
        leaven_public_seam::PublicSeamPackage::active_from_repo(workspace_root()).unwrap();

    let runner = RunnerRequestPayload::new(
        "run_agentic",
        "sc_runner_agentic",
        "cand_agentic",
        "case_agentic",
        json!({"question": "target-free"}),
        "fp_cap_sha256_agentic",
    )
    .unwrap();
    let runner = package
        .validate_stage_payload_document(runner.value())
        .unwrap();
    assert_eq!(runner.role(), leaven_public_seam::StagePayloadRole::Runner);

    let scorer = ScorerContextPayload::new(ScorerContextPayloadFields {
        run: "run_agentic".to_owned(),
        stage_call_id: "sc_scorer_agentic".to_owned(),
        evaluation_request_id: "evalreq_agentic".to_owned(),
        candidate: "cand_agentic".to_owned(),
        case_ref: "case_agentic".to_owned(),
        output: output_record("candidate answer"),
        target_handle: Some("case_agentic".to_owned()),
        capability_fingerprint: "fp_cap_sha256_agentic".to_owned(),
    })
    .unwrap();
    let scorer = package
        .validate_stage_payload_document(scorer.value())
        .unwrap();
    assert_eq!(scorer.role(), leaven_public_seam::StagePayloadRole::Scorer);
    assert_eq!(
        scorer.capability_fingerprint(),
        Some("fp_cap_sha256_agentic")
    );
    assert_eq!(scorer.output_count(), 1);

    let judge = JudgeContextPayload::new(JudgeContextPayloadFields {
        run: "run_agentic".to_owned(),
        stage_call_id: "sc_judge_agentic".to_owned(),
        left: "cand_left".to_owned(),
        right: "cand_right".to_owned(),
        case_ref: Some("case_agentic".to_owned()),
        outputs: vec![output_record("left answer"), output_record("right answer")],
        rubric: json!({"kind": "preference"}),
        capability_fingerprint: "fp_cap_sha256_agentic".to_owned(),
    })
    .unwrap();
    let judge = package
        .validate_stage_payload_document(judge.value())
        .unwrap();
    assert_eq!(judge.role(), leaven_public_seam::StagePayloadRole::Judge);
    assert_eq!(judge.output_count(), 2);

    let callback = CallbackRequestPayload::new(
        "run_agentic",
        "sc_callback_agentic",
        json!({"kind": "stage_completed"}),
        "fp_schema_sha256_callback",
        "fp_cap_sha256_agentic",
    )
    .unwrap();
    let callback = package
        .validate_stage_payload_document(callback.value())
        .unwrap();
    assert_eq!(
        callback.role(),
        leaven_public_seam::StagePayloadRole::Callback
    );
    assert_eq!(callback.payload_schema(), Some("fp_schema_sha256_callback"));

    let artifact_adapter = AdapterRequestPayload::artifact(
        "run_agentic",
        "sc_artifact_adapter_agentic",
        json!({"kind": "project_artifact"}),
        "fp_schema_sha256_artifact_adapter",
        "fp_cap_sha256_agentic",
    )
    .unwrap();
    let artifact_adapter = package
        .validate_stage_payload_document(artifact_adapter.value())
        .unwrap();
    assert_eq!(
        artifact_adapter.role(),
        leaven_public_seam::StagePayloadRole::ArtifactAdapter
    );

    let dataset_adapter = AdapterRequestPayload::dataset(
        "run_agentic",
        "sc_dataset_adapter_agentic",
        json!({"kind": "project_dataset"}),
        "fp_schema_sha256_dataset_adapter",
        "fp_cap_sha256_agentic",
    )
    .unwrap();
    let dataset_adapter = package
        .validate_stage_payload_document(dataset_adapter.value())
        .unwrap();
    assert_eq!(
        dataset_adapter.role(),
        leaven_public_seam::StagePayloadRole::DatasetAdapter
    );
}

#[test]
fn agentic_runner_and_scorer_payload_builders_reject_target_and_output_fakes() {
    let target_leak = RunnerRequestPayload::new(
        "run_agentic",
        "sc_runner_agentic",
        "cand_agentic",
        "case_agentic",
        json!({"case.target": "secret answer"}),
        "fp_cap_sha256_agentic",
    )
    .unwrap_err();
    assert!(matches!(
        target_leak,
        PublicStagePayloadError::TargetLeakage {
            field: "case_input"
        }
    ));

    let scorer_target_mismatch = ScorerContextPayload::new(ScorerContextPayloadFields {
        run: "run_agentic".to_owned(),
        stage_call_id: "sc_scorer_agentic".to_owned(),
        evaluation_request_id: "evalreq_agentic".to_owned(),
        candidate: "cand_agentic".to_owned(),
        case_ref: "case_agentic".to_owned(),
        output: output_record("candidate answer"),
        target_handle: Some("case_unrelated".to_owned()),
        capability_fingerprint: "fp_cap_sha256_agentic".to_owned(),
    })
    .unwrap_err();
    assert!(matches!(
        scorer_target_mismatch,
        PublicStagePayloadError::TargetHandleMismatch
    ));

    let scorer_without_assessed_output = ScorerContextPayload::new(ScorerContextPayloadFields {
        run: "run_agentic".to_owned(),
        stage_call_id: "sc_scorer_agentic".to_owned(),
        evaluation_request_id: "evalreq_agentic".to_owned(),
        candidate: "cand_agentic".to_owned(),
        case_ref: "case_agentic".to_owned(),
        output: public_only_output_record("candidate answer"),
        target_handle: Some("case_agentic".to_owned()),
        capability_fingerprint: "fp_cap_sha256_agentic".to_owned(),
    })
    .unwrap_err();
    assert!(matches!(
        scorer_without_assessed_output,
        PublicStagePayloadError::MissingAssessedOutputClass { field: "output" }
    ));
}

#[test]
fn agentic_judge_payload_builder_rejects_empty_and_public_only_outputs() {
    let judge_no_outputs = JudgeContextPayload::new(JudgeContextPayloadFields {
        run: "run_agentic".to_owned(),
        stage_call_id: "sc_judge_agentic".to_owned(),
        left: "cand_left".to_owned(),
        right: "cand_right".to_owned(),
        case_ref: None,
        outputs: Vec::new(),
        rubric: json!({"kind": "preference"}),
        capability_fingerprint: "fp_cap_sha256_agentic".to_owned(),
    })
    .unwrap_err();
    assert!(matches!(
        judge_no_outputs,
        PublicStagePayloadError::EmptyField { field: "outputs" }
    ));

    let judge_without_assessed_output = JudgeContextPayload::new(JudgeContextPayloadFields {
        run: "run_agentic".to_owned(),
        stage_call_id: "sc_judge_agentic".to_owned(),
        left: "cand_left".to_owned(),
        right: "cand_right".to_owned(),
        case_ref: Some("case_agentic".to_owned()),
        outputs: vec![public_only_output_record("left answer")],
        rubric: json!({"kind": "preference"}),
        capability_fingerprint: "fp_cap_sha256_agentic".to_owned(),
    })
    .unwrap_err();
    assert!(matches!(
        judge_without_assessed_output,
        PublicStagePayloadError::MissingAssessedOutputClass { field: "outputs" }
    ));
}

#[test]
fn agentic_callback_and_adapter_payload_builders_reject_schema_and_capability_fakes() {
    let package =
        leaven_public_seam::PublicSeamPackage::active_from_repo(workspace_root()).unwrap();

    let callback_no_schema = CallbackRequestPayload::new(
        "run_agentic",
        "sc_callback_agentic",
        json!({"kind": "stage_completed"}),
        "",
        "fp_cap_sha256_agentic",
    )
    .unwrap_err();
    assert!(matches!(
        callback_no_schema,
        PublicStagePayloadError::EmptyField {
            field: "payload_schema"
        }
    ));

    let mut forged_callback = CallbackRequestPayload::new(
        "run_agentic",
        "sc_callback_agentic",
        json!({"kind": "stage_completed"}),
        "fp_schema_sha256_callback",
        "fp_cap_sha256_agentic",
    )
    .unwrap()
    .value()
    .clone();
    forged_callback
        .as_object_mut()
        .unwrap()
        .remove("payload_schema");
    assert!(matches!(
        package
            .validate_stage_payload_document(&forged_callback)
            .unwrap_err(),
        leaven_public_seam::PublicSeamError::ExampleValidation { .. }
            | leaven_public_seam::PublicSeamError::InvalidStagePayload { .. }
    ));

    let adapter_no_capability = AdapterRequestPayload::artifact(
        "run_agentic",
        "sc_artifact_adapter_agentic",
        json!({"kind": "project_artifact"}),
        "fp_schema_sha256_artifact_adapter",
        "",
    )
    .unwrap_err();
    assert!(matches!(
        adapter_no_capability,
        PublicStagePayloadError::EmptyField {
            field: "capability_fingerprint"
        }
    ));
}

fn identity(stage_call_id: &str) -> PublicStagePayloadIdentity {
    PublicStagePayloadIdentity::new(PublicStagePayloadIdentityFields {
        run: "run_agentic".to_owned(),
        stage_call_id: stage_call_id.to_owned(),
        base_revision: "rev_agentic_base".to_owned(),
        parent: json!("cand_parent"),
        source_refs: vec![json!("cand_parent")],
        surface_fingerprint: "fp_surface_sha256_skill".to_owned(),
        query_policy_fingerprint: "fp_policy_sha256_agentic".to_owned(),
        capability_fingerprint: "fp_cap_sha256_agentic".to_owned(),
    })
    .unwrap()
}

fn reflection_result(receipt: &str) -> ReflectionResultPayload {
    ReflectionResultPayload::new(
        "The candidate fails empty-input cases because its guard accepts blanks.",
        [json!({
            "label": "empty_input_guard",
            "description": "No early return for empty user input.",
            "severity": "high",
            "source_refs": ["cand_parent"]
        })],
        [json!({
            "surface_fingerprint": "fp_surface_sha256_skill",
            "part_label": "input_validation",
            "diagnosis": "guard missing",
            "suggested_direction": "add explicit empty input guard",
            "source_refs": ["cand_parent"]
        })],
        [json!("cand_parent")],
        [json!(receipt)],
        ["optimizer.visible".to_owned()],
        0.82,
    )
    .unwrap()
}

fn reflective_example() -> Value {
    json!({
        "case": "case_visible_empty",
        "input": {"text": ""},
        "output": {
            "kind": "text",
            "summary": "empty input rejected",
            "value": "empty input rejected",
            "visibility": "public",
            "data_classes": ["candidate.output"]
        },
        "score": {
            "value": 0.25,
            "output": {
                "kind": "text",
                "summary": "empty input rejected",
                "value": "empty input rejected",
                "visibility": "public",
                "data_classes": ["candidate.output"]
            }
        },
        "feedback": "The candidate does not reject empty input.",
        "side_info": {"difficulty": "edge_case"},
        "source_refs": ["cand_parent"],
        "data_classes": ["case.input", "candidate.output"],
        "evidence_visibility": "score_and_feedback"
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

fn public_only_output_record(summary: &str) -> Value {
    json!({
        "kind": "text",
        "summary": summary,
        "value": summary,
        "visibility": "public",
        "data_classes": ["public"]
    })
}

fn workspace_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("crate lives under crates/<name>")
        .to_path_buf()
}
