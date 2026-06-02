use crate::support::package;
use leaven_public_seam::{PublicSeamError, StagePayloadRole, StageRunKind};
use serde_json::{Value, json};

#[test]
fn stage_run_validates_generic_runner_dispatch_request_and_text_result() {
    let package = package();

    let request = package
        .validate_stage_run_request_document(&stage_run_request())
        .unwrap();
    assert_eq!(request.stage(), StageRunKind::Runner);
    assert_eq!(request.stage().as_str(), "runner");
    assert_eq!(request.payload().role(), StagePayloadRole::Runner);

    let result = package
        .validate_stage_run_result_document(&stage_run_result())
        .unwrap();
    assert_eq!(result.stage(), StageRunKind::Runner);
    assert_eq!(result.stage_call_id(), "sc_runner_stagerun");
    assert_eq!(result.output().kind(), "text");
    assert_eq!(result.output().visibility(), "optimizer_visible");
}

#[test]
fn stage_run_request_rejects_target_material_and_wrong_payload_role() {
    let package = package();

    let mut hidden_target = stage_run_request();
    hidden_target["payload"]["case_input"]["case.target"] = json!("secret answer");
    assert!(matches!(
        package
            .validate_stage_run_request_document(&hidden_target)
            .unwrap_err(),
        PublicSeamError::InvalidStageRun { .. }
    ));

    // A target-bearing scorer payload cannot ride the runner stage dispatch even
    // when its own role/shape is otherwise valid: the stage kind must match.
    let mut wrong_role = stage_run_request();
    wrong_role["payload"] = score_context_payload();
    assert!(matches!(
        package
            .validate_stage_run_request_document(&wrong_role)
            .unwrap_err(),
        PublicSeamError::InvalidStageRun { .. } | PublicSeamError::ExampleValidation { .. }
    ));

    let mut bad_message = stage_run_request();
    bad_message["message"] = json!("stage_run_result");
    assert!(matches!(
        package
            .validate_stage_run_request_document(&bad_message)
            .unwrap_err(),
        PublicSeamError::ExampleValidation { .. }
    ));
}

#[test]
fn stage_run_result_rejects_non_text_output_and_plan_result_smuggling() {
    let package = package();

    let mut json_output = stage_run_result();
    json_output["output"]["kind"] = json!("json");
    json_output["output"]["value"] = json!({"answer": "4"});
    assert!(matches!(
        package
            .validate_stage_run_result_document(&json_output)
            .unwrap_err(),
        PublicSeamError::InvalidStageRun { .. }
    ));

    // The stage-run result schema is not the Plan Result schema: a Plan Result
    // envelope cannot pass as a stage-run result.
    let plan_result = json!({
        "schema_version": "leaven.plan_result.v1",
        "values": {"primary": {"kind": "literal", "value": "4"}}
    });
    assert!(matches!(
        package
            .validate_stage_run_result_document(&plan_result)
            .unwrap_err(),
        PublicSeamError::ExampleValidation { .. }
    ));
}

fn stage_run_request() -> Value {
    json!({
        "schema_version": "leaven.stage_run.v1",
        "message": "stage_run_request",
        "stage": "runner",
        "payload": runner_payload()
    })
}

fn stage_run_result() -> Value {
    json!({
        "schema_version": "leaven.stage_run.v1",
        "message": "stage_run_result",
        "stage": "runner",
        "stage_call_id": "sc_runner_stagerun",
        "output": {
            "kind": "text",
            "summary": "5 + 7 = 12",
            "value": "12",
            "visibility": "optimizer_visible",
            "data_classes": ["candidate.output"]
        }
    })
}

fn runner_payload() -> Value {
    json!({
        "schema_version": "leaven.stage_payloads.v1",
        "role": "runner",
        "run": "run_stagerun",
        "stage_call_id": "sc_runner_stagerun",
        "candidate": "cand_stagerun_parent",
        "case": "case_stagerun",
        "case_input": {"question": "5 + 7"},
        "target_forbidden": true
    })
}

fn score_context_payload() -> Value {
    json!({
        "schema_version": "leaven.stage_payloads.v1",
        "role": "scorer",
        "run": "run_stagerun",
        "stage_call_id": "sc_scorer_stagerun",
        "evaluation_request_id": "erq_stagerun",
        "candidate": "cand_stagerun_parent",
        "case": "case_stagerun",
        "output": {
            "kind": "text",
            "value": "12",
            "visibility": "optimizer_visible",
            "data_classes": ["candidate.output"]
        },
        "capability_fingerprint": "fp_cap_sha256_stagerun"
    })
}
