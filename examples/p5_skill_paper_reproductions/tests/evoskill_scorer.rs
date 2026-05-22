use p5_skill_paper_reproductions::evoskill::{
    DEFAULT_FAILURE_THRESHOLD, DEFAULT_TOLERANCES, build_sealqa_judge_request,
    score_evoskill_answer, sealqa_judge_template_manifest,
};

fn assert_score(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= f64::EPSILON,
        "expected score {expected}, got {actual}"
    );
}

#[test]
fn multi_tolerance_score_records_weighted_failure_boundary() {
    let exact = score_evoskill_answer("100", "100");
    assert_score(exact.weighted_score, 1.0);
    assert!(!exact.is_failure);

    let close_but_not_exact = score_evoskill_answer("100", "100.5");
    assert_eq!(close_but_not_exact.tolerance_scores.len(), 5);
    assert_eq!(close_but_not_exact.tolerances(), DEFAULT_TOLERANCES);
    assert!(close_but_not_exact.weighted_score < DEFAULT_FAILURE_THRESHOLD);
    assert!(close_but_not_exact.is_failure);
}

#[test]
fn numeric_scoring_normalizes_units_and_filters_incidental_years() {
    let unit_equivalent = score_evoskill_answer("2.6 billion", "2,600 million");
    assert_score(unit_equivalent.weighted_score, 1.0);

    let answer_with_incidental_year = score_evoskill_answer("2,602", "reported in 2023 as 2,602");
    assert_score(answer_with_incidental_year.weighted_score, 1.0);

    let incidental_year_only = score_evoskill_answer("2,602", "reported in 2023");
    assert_score(incidental_year_only.weighted_score, 0.0);
    assert!(incidental_year_only.is_failure);
}

#[test]
fn hybrid_and_multi_number_answers_require_text_and_all_values() {
    let missing_text = score_evoskill_answer("March 1977", "1977");
    assert_score(missing_text.weighted_score, 0.0);

    let hybrid = score_evoskill_answer("March 1977", "The filing was in March of 1977.");
    assert_score(hybrid.weighted_score, 1.0);

    let full_list = score_evoskill_answer("2,602 and 1,500", "values were 1,500 then 2,602");
    assert_score(full_list.weighted_score, 1.0);

    let missing_value = score_evoskill_answer("2,602 and 1,500", "only 2,602 was reported");
    assert_score(missing_value.weighted_score, 0.0);
}

#[test]
fn textual_answers_use_normalized_substring_containment() {
    let report = score_evoskill_answer("\"Serban Ghenea\"", "The answer is serban ghenea.");
    assert_score(report.weighted_score, 1.0);
    assert!(!report.is_failure);
}

#[test]
fn sealqa_judge_request_preserves_paper_template_without_running_a_judge() {
    let manifest = sealqa_judge_template_manifest();
    assert_eq!(manifest.id, "sealqa-auto-grader-placeholder-v1");
    assert_eq!(manifest.dataset_id, "sealqa");
    assert_eq!(manifest.source_artifact_id, "paper_auto_grader_placeholder");
    assert_eq!(manifest.fingerprint.len(), 64);
    assert_eq!(manifest.runtime_status, "template_pinned_no_spend");

    let request = build_sealqa_judge_request(
        "Who holds the album of the year Grammy record?",
        "Serban Ghenea",
        "Serban Ghenea",
        0.01,
    );

    assert_eq!(request.template_id, manifest.id);
    assert_eq!(request.template_fingerprint, manifest.fingerprint);
    assert!(request.system.contains("Auto-Grader"));
    assert!(request.user.contains("question"));
    assert!(request.user.contains("Who holds the album"));
    assert!(request.user.contains("prediction"));
    assert!(request.user.contains("Serban Ghenea"));
    assert!(request.user.contains("reference"));
    assert!(request.user.contains("tolerance"));
    assert!(request.output_contract.contains("\"score\""));
    assert!(request.output_contract.contains("\"passed\""));
    assert!(request.output_contract.contains("\"error_breakdown\""));
}
