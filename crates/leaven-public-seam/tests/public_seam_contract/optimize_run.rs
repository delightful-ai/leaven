use crate::support::package;
use leaven_public_seam::{OptimizeObjective, OptimizeReflection, OptimizeSplit, PublicSeamError};
use serde_json::{Value, json};

#[test]
fn optimize_run_validates_request_seed_cases_optimizer_and_reflection() {
    let package = package();

    let request = package
        .validate_optimize_run_request_document(&optimize_run_request())
        .unwrap();

    assert_eq!(request.run_id(), "run_optimize");
    assert_eq!(request.seed().artifact_type(), "prompt");
    assert_eq!(request.seed().artifact_schema(), "fp_schema_sha256_prompt");
    assert_eq!(
        request.seed().artifact(),
        &json!({"template": "Answer the question: {{question}}"})
    );

    let cases = request.cases();
    assert_eq!(cases.len(), 2);
    assert_eq!(cases[0].case(), "case_optimize_1");
    assert_eq!(cases[0].input(), &json!({"question": "2 + 2"}));
    assert_eq!(cases[0].target(), &json!({"answer": "4"}));
    assert!(cases[0].has_target());
    assert_eq!(cases[0].split(), Some(OptimizeSplit::Train));
    // The second case has a null target and no split.
    assert_eq!(cases[1].target(), &Value::Null);
    assert!(!cases[1].has_target());
    assert_eq!(cases[1].split(), None);

    let optimizer = request.optimizer();
    assert_eq!(optimizer.max_metric_calls(), 30);
    assert_eq!(optimizer.population_size(), Some(4));
    assert_eq!(optimizer.minibatch_size(), Some(3));
    assert_eq!(optimizer.objective(), OptimizeObjective::Instance);

    assert_eq!(
        request.reflection(),
        &OptimizeReflection::Lm {
            model: "gpt-5.4-mini".to_owned()
        }
    );
    assert_eq!(request.capability_fingerprint(), "fp_cap_sha256_optimize");
}

#[test]
fn optimize_run_validates_result_best_frontier_run_and_receipts() {
    let package = package();

    let result = package
        .validate_optimize_run_result_document(&optimize_run_result())
        .unwrap();

    assert_eq!(result.best().candidate_id(), "cand_optimize_child");
    assert_exact(result.best().score(), 0.75);
    assert_eq!(result.best().parent(), Some(&json!("cand_optimize_seed")));
    assert_eq!(result.best().artifact().artifact_type(), "prompt");

    assert_eq!(result.frontier().len(), 2);
    assert_eq!(result.frontier()[0].candidate_id(), "cand_optimize_seed");
    assert_eq!(result.frontier()[0].parent(), None);
    assert_eq!(result.frontier()[1].candidate_id(), "cand_optimize_child");

    assert_eq!(result.iterations(), 1);
    assert_eq!(result.metric_calls_used(), 12);
    assert_eq!(result.cost(), &json!({"usd_micro": 1500, "lm_calls": 6}));
    assert_eq!(result.run().run(), "run_optimize");
    assert_eq!(result.run().revision(), "rev_optimize_final");
    assert_eq!(result.applied_proposals(), &["wrec_optimize_batch_1"]);
}

#[test]
fn optimize_run_parses_all_four_objective_variants() {
    let package = package();

    for (wire, expected) in [
        ("instance", OptimizeObjective::Instance),
        ("objective", OptimizeObjective::Objective),
        ("hybrid", OptimizeObjective::Hybrid),
        ("cartesian", OptimizeObjective::Cartesian),
    ] {
        let mut request = optimize_run_request();
        request["optimizer"]["objective"] = json!(wire);
        let parsed = package
            .validate_optimize_run_request_document(&request)
            .unwrap();
        assert_eq!(parsed.optimizer().objective(), expected);
        assert_eq!(parsed.optimizer().objective().as_str(), wire);
    }
}

#[test]
fn optimize_run_parses_agentic_reflection() {
    let package = package();

    let mut request = optimize_run_request();
    request["reflection"] = json!({"kind": "agentic"});
    let parsed = package
        .validate_optimize_run_request_document(&request)
        .unwrap();
    assert_eq!(parsed.reflection(), &OptimizeReflection::Agentic);
}

#[test]
fn optimize_run_rejects_empty_case_manifest() {
    let package = package();

    let mut request = optimize_run_request();
    request["cases"] = json!([]);
    assert!(matches!(
        package
            .validate_optimize_run_request_document(&request)
            .unwrap_err(),
        PublicSeamError::InvalidOptimizeRun { .. } | PublicSeamError::ExampleValidation { .. }
    ));
}

#[test]
fn optimize_run_rejects_request_missing_message_and_wrong_objective() {
    let package = package();

    // A request without the message discriminator is refused.
    let mut no_message = optimize_run_request();
    no_message.as_object_mut().unwrap().remove("message");
    assert!(matches!(
        package
            .validate_optimize_run_request_document(&no_message)
            .unwrap_err(),
        PublicSeamError::InvalidOptimizeRun { .. } | PublicSeamError::ExampleValidation { .. }
    ));

    // An objective outside the locked enum is refused at the schema layer.
    let mut bad_objective = optimize_run_request();
    bad_objective["optimizer"]["objective"] = json!("pareto");
    assert!(matches!(
        package
            .validate_optimize_run_request_document(&bad_objective)
            .unwrap_err(),
        PublicSeamError::InvalidOptimizeRun { .. } | PublicSeamError::ExampleValidation { .. }
    ));
}

#[test]
fn optimize_run_rejects_zero_metric_calls_and_missing_target_field() {
    let package = package();

    // max_metric_calls must be at least 1.
    let mut zero_calls = optimize_run_request();
    zero_calls["optimizer"]["max_metric_calls"] = json!(0);
    assert!(matches!(
        package
            .validate_optimize_run_request_document(&zero_calls)
            .unwrap_err(),
        PublicSeamError::InvalidOptimizeRun { .. } | PublicSeamError::ExampleValidation { .. }
    ));

    // The case target field is required (it may be null, but it cannot be absent).
    let mut missing_target = optimize_run_request();
    missing_target["cases"][0]
        .as_object_mut()
        .unwrap()
        .remove("target");
    assert!(matches!(
        package
            .validate_optimize_run_request_document(&missing_target)
            .unwrap_err(),
        PublicSeamError::InvalidOptimizeRun { .. } | PublicSeamError::ExampleValidation { .. }
    ));
}

#[test]
fn optimize_run_rejects_best_not_in_frontier() {
    let package = package();

    // The best candidate id is not present in the frontier: the projection cannot
    // claim a best candidate the frontier never admitted.
    let mut result = optimize_run_result();
    result["best"]["candidate"] = json!("cand_optimize_phantom");
    assert!(matches!(
        package
            .validate_optimize_run_result_document(&result)
            .unwrap_err(),
        PublicSeamError::InvalidOptimizeRun { .. }
    ));
}

#[test]
fn optimize_run_rejects_empty_frontier() {
    let package = package();

    let mut result = optimize_run_result();
    result["frontier"] = json!([]);
    assert!(matches!(
        package
            .validate_optimize_run_result_document(&result)
            .unwrap_err(),
        PublicSeamError::InvalidOptimizeRun { .. } | PublicSeamError::ExampleValidation { .. }
    ));
}

#[test]
fn optimize_run_rejects_non_finite_and_non_numeric_scores() {
    let package = package();

    // A non-numeric score string cannot pass the typed score field.
    let mut string_score = optimize_run_result();
    string_score["best"]["score"] = json!("0.75");
    string_score["frontier"][1]["score"] = json!("0.75");
    assert!(matches!(
        package
            .validate_optimize_run_result_document(&string_score)
            .unwrap_err(),
        PublicSeamError::InvalidOptimizeRun { .. } | PublicSeamError::ExampleValidation { .. }
    ));

    // A null score is not a finite number.
    let mut null_score = optimize_run_result();
    null_score["best"]["score"] = Value::Null;
    null_score["frontier"][1]["score"] = Value::Null;
    assert!(matches!(
        package
            .validate_optimize_run_result_document(&null_score)
            .unwrap_err(),
        PublicSeamError::InvalidOptimizeRun { .. } | PublicSeamError::ExampleValidation { .. }
    ));
}

#[test]
fn optimize_run_rejects_malformed_applied_proposal_receipts() {
    let package = package();

    // applied_proposals receipts must match the wrec_ pattern.
    let mut bad_receipt = optimize_run_result();
    bad_receipt["applied_proposals"] = json!(["qrec_not_a_write_receipt"]);
    assert!(matches!(
        package
            .validate_optimize_run_result_document(&bad_receipt)
            .unwrap_err(),
        PublicSeamError::InvalidOptimizeRun { .. } | PublicSeamError::ExampleValidation { .. }
    ));
}

/// Asserts a parsed score number equals the exact wire value. JSON-decoded
/// `0.75` is bit-exact, so this catches an altered score without tolerating
/// drift.
#[track_caller]
fn assert_exact(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < f64::EPSILON,
        "expected `{expected}`, got `{actual}`"
    );
}

fn optimize_run_request() -> Value {
    json!({
        "schema_version": "leaven.optimize_run.v1",
        "message": "optimize_run_request",
        "run_id": "run_optimize",
        "seed": optimize_artifact(),
        "cases": [
            {
                "case": "case_optimize_1",
                "input": {"question": "2 + 2"},
                "target": {"answer": "4"},
                "metadata": {"source": "aime"},
                "split": "train"
            },
            {
                "case": "case_optimize_2",
                "input": {"question": "3 + 5"},
                "target": null
            }
        ],
        "optimizer": {
            "max_metric_calls": 30,
            "population_size": 4,
            "minibatch_size": 3,
            "objective": "instance"
        },
        "reflection": {"kind": "lm", "model": "gpt-5.4-mini"},
        "capability_fingerprint": "fp_cap_sha256_optimize"
    })
}

fn optimize_run_result() -> Value {
    json!({
        "schema_version": "leaven.optimize_run.v1",
        "message": "optimize_run_result",
        "best": {
            "candidate": "cand_optimize_child",
            "parent": "cand_optimize_seed",
            "score": 0.75,
            "artifact": optimize_artifact()
        },
        "frontier": [
            {
                "candidate": "cand_optimize_seed",
                "parent": null,
                "score": 0.5,
                "artifact": optimize_artifact()
            },
            {
                "candidate": "cand_optimize_child",
                "parent": "cand_optimize_seed",
                "score": 0.75,
                "artifact": optimize_artifact()
            }
        ],
        "iterations": 1,
        "metric_calls_used": 12,
        "cost": {"usd_micro": 1500, "lm_calls": 6},
        "run": {"run": "run_optimize", "revision": "rev_optimize_final"},
        "applied_proposals": ["wrec_optimize_batch_1"]
    })
}

fn optimize_artifact() -> Value {
    json!({
        "artifact_type": "prompt",
        "artifact_schema": "fp_schema_sha256_prompt",
        "artifact": {"template": "Answer the question: {{question}}"}
    })
}
