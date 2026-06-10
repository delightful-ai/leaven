use crate::support::package;
use leaven_public_seam::{LockedMethod, PublicSeamError, StagePayloadRole, StageRunKind};
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
    let case_input = request.payload().runner_case_input().unwrap();
    assert_eq!(case_input.candidate(), "cand_stagerun_parent");
    assert_eq!(case_input.case(), "case_stagerun");
    assert_eq!(
        case_input.case_input().as_json(),
        &json!({"question": "5 + 7"})
    );
    assert_eq!(case_input.case_input_keys(), &["question".to_owned()]);
    assert!(
        case_input
            .case_input_fingerprint()
            .starts_with("fp_stage_case_input_sha256_")
    );

    let result = package
        .validate_stage_run_result_document(&stage_run_result())
        .unwrap();
    assert_eq!(result.stage(), StageRunKind::Runner);
    assert_eq!(result.stage_call_id(), "sc_runner_stagerun");
    assert_eq!(result.output().kind(), "text");
    assert_eq!(result.output().visibility(), "optimizer_visible");
    assert!(result.effect_receipts().is_empty());
    assert!(result.proposal_receipts().is_empty());
}

#[test]
fn stage_run_result_preserves_worker_effect_receipts() {
    let package = package();
    let mut result_value = stage_run_result();
    result_value["effect_receipts"] = json!([
        {
            "method": "leaven/lm.complete",
            "receipt": "lmrec_completion",
            "call_kind": "lm_complete",
            "cost": {"usd_micro": 42, "input_tokens": 3, "output_tokens": 2},
            "blob_refs": [
                {
                    "kind": "blob_ref",
                    "id": "blob_stage_effect_transcript",
                    "sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                    "bytes": 12,
                    "data_classes": ["transcript.raw"]
                }
            ]
        }
    ]);

    let result = package
        .validate_stage_run_result_document(&result_value)
        .unwrap();

    let receipts = result.effect_receipts();
    assert_eq!(receipts.len(), 1);
    assert_eq!(receipts[0].method(), LockedMethod::LmComplete);
    assert_eq!(receipts[0].method().as_str(), "leaven/lm.complete");
    assert_eq!(receipts[0].receipt(), "lmrec_completion");
    assert_eq!(receipts[0].call_kind(), Some("lm_complete"));
    let cost = receipts[0].cost().unwrap();
    assert_eq!(cost.usd_micro(), Some(42));
    assert_eq!(cost.input_tokens(), Some(3));
    assert_eq!(cost.output_tokens(), Some(2));
    assert_eq!(cost.lm_calls(), None);

    let blob_ref = &receipts[0].blob_refs()[0];
    assert_eq!(blob_ref.id(), "blob_stage_effect_transcript");
    assert_eq!(
        blob_ref.sha256(),
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
    );
    assert_eq!(blob_ref.bytes(), 12);
    assert_eq!(blob_ref.media_type(), None);
    assert_eq!(blob_ref.uri(), None);
    assert_eq!(blob_ref.data_classes(), &["transcript.raw".to_owned()]);
}

#[test]
fn stage_run_result_preserves_worker_effect_blob_contents() {
    let package = package();
    let mut result_value = stage_run_result();
    result_value["effect_receipts"] = json!([
        {
            "method": "leaven/agent.run",
            "receipt": "agentrec_completion",
            "call_kind": "agent_run",
            "blob_refs": [
                {
                    "kind": "blob_ref",
                    "id": "blob_stage_effect_transcript",
                    "sha256": "54e6289e14c7b0e7ad9acc2dfc4c1e3d027d0eef7f5c4c3fe7c292761d0e06a6",
                    "bytes": 10,
                    "data_classes": ["transcript.raw"]
                }
            ],
            "blob_contents": [
                {
                    "blob_ref": {
                        "kind": "blob_ref",
                        "id": "blob_stage_effect_transcript",
                        "sha256": "54e6289e14c7b0e7ad9acc2dfc4c1e3d027d0eef7f5c4c3fe7c292761d0e06a6",
                        "bytes": 10,
                        "data_classes": ["transcript.raw"]
                    },
                    "content_base64": "dHJhbnNjcmlwdA=="
                }
            ]
        }
    ]);

    let result = package
        .validate_stage_run_result_document(&result_value)
        .unwrap();

    let contents = result.effect_receipts()[0].blob_contents();
    assert_eq!(contents.len(), 1);
    assert_eq!(contents[0].blob_ref().id(), "blob_stage_effect_transcript");
    assert_eq!(contents[0].content_bytes().unwrap(), b"transcript");
}

#[test]
fn stage_run_result_rejects_mismatched_worker_effect_blob_contents() {
    let package = package();
    let mut result_value = stage_run_result();
    result_value["effect_receipts"] = json!([
        {
            "method": "leaven/agent.run",
            "receipt": "agentrec_completion",
            "call_kind": "agent_run",
            "blob_refs": [
                {
                    "kind": "blob_ref",
                    "id": "blob_stage_effect_transcript",
                    "sha256": "54e6289e14c7b0e7ad9acc2dfc4c1e3d027d0eef7f5c4c3fe7c292761d0e06a6",
                    "bytes": 10,
                    "data_classes": ["transcript.raw"]
                }
            ],
            "blob_contents": [
                {
                    "blob_ref": {
                        "kind": "blob_ref",
                        "id": "blob_stage_effect_transcript",
                        "sha256": "54e6289e14c7b0e7ad9acc2dfc4c1e3d027d0eef7f5c4c3fe7c292761d0e06a6",
                        "bytes": 10,
                        "data_classes": ["transcript.raw"]
                    },
                    "content_base64": "bm90LXRyYW5zY3JpcHQ="
                }
            ]
        }
    ]);

    assert!(matches!(
        package
            .validate_stage_run_result_document(&result_value)
            .unwrap_err(),
        PublicSeamError::InvalidStageRun { .. } | PublicSeamError::ExampleValidation { .. }
    ));
}

#[test]
fn stage_run_result_preserves_worker_proposal_receipts() {
    let package = package();
    let mut result_value = proposer_stage_run_result();
    result_value["proposal_receipts"] = json!([
        {
            "method": "leaven/proposal.submit_batch",
            "receipt": "wrec_proposal_submit",
            "write_kind": "submit_proposal_batch",
            "proposal_ids": ["prop_stagerun_submit"]
        }
    ]);

    let result = package
        .validate_stage_run_result_document(&result_value)
        .unwrap();

    let receipts = result.proposal_receipts();
    assert_eq!(receipts.len(), 1);
    assert_eq!(receipts[0].method(), LockedMethod::ProposalSubmitBatch);
    assert_eq!(receipts[0].receipt(), "wrec_proposal_submit");
    assert_eq!(receipts[0].write_kind(), Some("submit_proposal_batch"));
    assert_eq!(
        receipts[0].proposal_ids(),
        &["prop_stagerun_submit".to_owned()]
    );
}

#[test]
fn stage_run_result_rejects_wrong_proposal_receipt_family() {
    let package = package();
    let mut wrong_receipt_family = proposer_stage_run_result();
    wrong_receipt_family["proposal_receipts"] = json!([
        {
            "method": "leaven/proposal.submit_batch",
            "receipt": "lmrec_completion",
            "write_kind": "submit_proposal_batch"
        }
    ]);

    assert!(matches!(
        package
            .validate_stage_run_result_document(&wrong_receipt_family)
            .unwrap_err(),
        PublicSeamError::InvalidStageRun { .. } | PublicSeamError::ExampleValidation { .. }
    ));
}

#[test]
fn stage_run_validates_generic_proposer_dispatch_request_and_text_result() {
    let package = package();

    let request = package
        .validate_stage_run_request_document(&proposer_stage_run_request())
        .unwrap();
    assert_eq!(request.stage(), StageRunKind::Proposer);
    assert_eq!(request.stage().as_str(), "proposer");
    assert_eq!(request.payload().role(), StagePayloadRole::Proposer);
    assert!(request.payload().runner_case_input().is_none());

    let result = package
        .validate_stage_run_result_document(&proposer_stage_run_result())
        .unwrap();
    assert_eq!(result.stage(), StageRunKind::Proposer);
    assert_eq!(result.stage_call_id(), "sc_proposer_stagerun");
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

    let mut proposer_wrong_role = proposer_stage_run_request();
    proposer_wrong_role["payload"] = runner_payload();
    assert!(matches!(
        package
            .validate_stage_run_request_document(&proposer_wrong_role)
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

    let mut wrong_receipt_family = stage_run_result();
    wrong_receipt_family["effect_receipts"] = json!([
        {
            "method": "leaven/lm.complete",
            "receipt": "agentrec_completion",
            "call_kind": "lm_complete"
        }
    ]);
    assert!(matches!(
        package
            .validate_stage_run_result_document(&wrong_receipt_family)
            .unwrap_err(),
        PublicSeamError::InvalidStageRun { .. }
    ));
}

#[test]
fn stage_run_validates_scorer_dispatch_request_and_reward_vector_result() {
    let package = package();

    let request = package
        .validate_stage_run_request_document(&scorer_stage_run_request())
        .unwrap();
    assert_eq!(request.stage(), StageRunKind::Scorer);
    assert_eq!(request.stage().as_str(), "scorer");
    assert_eq!(request.payload().role(), StagePayloadRole::Scorer);

    let result = package
        .validate_stage_run_result_document(&scorer_stage_run_result())
        .unwrap();
    assert_eq!(result.stage(), StageRunKind::Scorer);
    assert_eq!(result.stage_call_id(), "sc_scorer_stagerun");
    assert_eq!(result.output().kind(), "text");

    let score = result.score().expect("scorer result must carry a score");
    assert_exact(score.value(), 0.5);
    let rewards = score.rewards();
    assert_eq!(rewards.len(), 2);
    assert_eq!(rewards[0].id(), "exact_match");
    assert_exact(rewards[0].value(), 1.0);
    assert_exact(rewards[0].weight(), 0.5);
    assert_eq!(rewards[0].feedback(), Some("matched the gold answer"));
    assert_eq!(rewards[1].id(), "format");
    assert_exact(rewards[1].value(), 0.0);
    assert_exact(rewards[1].weight(), 0.5);
    assert_eq!(rewards[1].feedback(), None);
}

/// Asserts a parsed reward/score number equals the exact wire value. JSON-decoded
/// `0.0`/`0.5`/`1.0` are bit-exact, so this catches a dropped or altered reward
/// without tolerating drift.
#[track_caller]
fn assert_exact(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < f64::EPSILON,
        "expected `{expected}`, got `{actual}`"
    );
}

#[test]
fn stage_run_request_rejects_runner_payload_on_scorer_stage() {
    let package = package();

    let mut wrong_role = scorer_stage_run_request();
    wrong_role["payload"] = runner_payload();
    assert!(matches!(
        package
            .validate_stage_run_request_document(&wrong_role)
            .unwrap_err(),
        PublicSeamError::InvalidStageRun { .. } | PublicSeamError::ExampleValidation { .. }
    ));
}

#[test]
fn stage_run_result_enforces_score_presence_by_stage_kind() {
    let package = package();

    // A scorer result without a score is refused: scoring without a reward
    // vector silently drops the optimizer signal.
    let mut scorer_without_score = scorer_stage_run_result();
    scorer_without_score
        .as_object_mut()
        .unwrap()
        .remove("score");
    assert!(matches!(
        package
            .validate_stage_run_result_document(&scorer_without_score)
            .unwrap_err(),
        PublicSeamError::InvalidStageRun { .. }
    ));

    // A runner result carrying a score is refused: runner stages must not smuggle
    // reward vectors.
    let mut runner_with_score = stage_run_result();
    runner_with_score["score"] = score_fact();
    assert!(matches!(
        package
            .validate_stage_run_result_document(&runner_with_score)
            .unwrap_err(),
        PublicSeamError::InvalidStageRun { .. } | PublicSeamError::ExampleValidation { .. }
    ));

    // A proposer result carrying a score is likewise refused.
    let mut proposer_with_score = proposer_stage_run_result();
    proposer_with_score["score"] = score_fact();
    assert!(matches!(
        package
            .validate_stage_run_result_document(&proposer_with_score)
            .unwrap_err(),
        PublicSeamError::InvalidStageRun { .. } | PublicSeamError::ExampleValidation { .. }
    ));
}

#[test]
fn stage_run_result_rejects_wrong_typed_reward_and_score_numbers() {
    let package = package();

    // A string reward value cannot pass as a number; a scorer cannot smuggle a
    // non-numeric reward past the typed reward vector.
    let mut string_reward = scorer_stage_run_result();
    string_reward["score"]["rewards"][0]["value"] = json!("1.0");
    assert!(matches!(
        package
            .validate_stage_run_result_document(&string_reward)
            .unwrap_err(),
        PublicSeamError::InvalidStageRun { .. } | PublicSeamError::ExampleValidation { .. }
    ));

    // A null collapsed score is not a finite number.
    let mut non_number_score = scorer_stage_run_result();
    non_number_score["score"]["value"] = serde_json::Value::Null;
    assert!(matches!(
        package
            .validate_stage_run_result_document(&non_number_score)
            .unwrap_err(),
        PublicSeamError::InvalidStageRun { .. } | PublicSeamError::ExampleValidation { .. }
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

fn scorer_stage_run_request() -> Value {
    json!({
        "schema_version": "leaven.stage_run.v1",
        "message": "stage_run_request",
        "stage": "scorer",
        "payload": score_context_payload()
    })
}

fn scorer_stage_run_result() -> Value {
    json!({
        "schema_version": "leaven.stage_run.v1",
        "message": "stage_run_result",
        "stage": "scorer",
        "stage_call_id": "sc_scorer_stagerun",
        "output": {
            "kind": "text",
            "summary": "exact match; wrong format",
            "value": "exact_match: pass\nformat: fail",
            "visibility": "optimizer_visible",
            "data_classes": ["candidate.output"]
        },
        "score": score_fact()
    })
}

fn score_fact() -> Value {
    json!({
        "value": 0.5,
        "rewards": [
            {
                "id": "exact_match",
                "value": 1.0,
                "weight": 0.5,
                "feedback": "matched the gold answer"
            },
            {
                "id": "format",
                "value": 0.0,
                "weight": 0.5
            }
        ]
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

fn proposer_stage_run_request() -> Value {
    json!({
        "schema_version": "leaven.stage_run.v1",
        "message": "stage_run_request",
        "stage": "proposer",
        "payload": proposer_payload()
    })
}

fn proposer_stage_run_result() -> Value {
    json!({
        "schema_version": "leaven.stage_run.v1",
        "message": "stage_run_result",
        "stage": "proposer",
        "stage_call_id": "sc_proposer_stagerun",
        "output": {
            "kind": "text",
            "summary": "submitted 1 proposal",
            "value": "wrec_proposal_submit",
            "visibility": "optimizer_visible",
            "data_classes": ["public"]
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
        "target_forbidden": true,
        "capability_fingerprint": "fp_cap_sha256_stagerun"
    })
}

fn proposer_payload() -> Value {
    json!({
        "schema_version": "leaven.stage_payloads.v1",
        "role": "proposer",
        "run": "run_stagerun",
        "stage_call_id": "sc_proposer_stagerun",
        "base_revision": "rev_stagerun_base",
        "parent": "cand_stagerun_parent",
        "surface_fingerprint": "fp_surface_sha256_stagerun",
        "reflection_result": {
            "schema_version": "leaven.stage_payloads.v1",
            "role": "reflection_result",
            "summary": "empty inputs fail",
            "failure_modes": [
                {
                    "label": "missing_empty_input_guard",
                    "description": "empty inputs fail",
                    "source_refs": ["cand_stagerun_parent"]
                }
            ],
            "surface_suggestions": [],
            "negative_constraints": [],
            "positive_constraints": [],
            "source_refs": ["cand_stagerun_parent"],
            "read_receipts": ["qrec_stagerun_reflection"],
            "data_classes": ["optimizer.visible"],
            "confidence": 0.8
        },
        "allowed_effects": ["change_from_agent_session"],
        "allowed_change_schemas": ["fp_schema_sha256_stagerun_patch"],
        "source_refs": ["cand_stagerun_parent"],
        "query_policy_fingerprint": "fp_policy_sha256_stagerun",
        "capability_fingerprint": "fp_cap_sha256_stagerun"
    })
}

fn score_context_payload() -> Value {
    json!({
        "schema_version": "leaven.stage_payloads.v1",
        "role": "scorer",
        "run": "run_stagerun",
        "stage_call_id": "sc_scorer_stagerun",
        "evaluation_request_id": "evalreq_stagerun",
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
