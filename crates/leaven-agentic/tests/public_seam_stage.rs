use leaven_agentic::{
    ProposeRequestPayload, PublicStagePayloadError, PublicStagePayloadIdentity,
    PublicStagePayloadIdentityFields, ReflectProposeHandoffPayload, ReflectRequestPayload,
    ReflectionResultPayload,
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

fn workspace_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("crate lives under crates/<name>")
        .to_path_buf()
}
