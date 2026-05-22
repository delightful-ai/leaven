use p5_skill_paper_reproductions::evoskill::{
    DEFAULT_FAILURE_THRESHOLD, EvoSkillAnswerAttempt, extract_evoskill_failure_feedback,
    score_evoskill_attempt,
};

#[test]
fn failure_feedback_extracts_only_failed_attempts_in_order() {
    let attempts = vec![
        EvoSkillAnswerAttempt {
            source_id: "UID0001".to_owned(),
            ground_truth: "2,602".to_owned(),
            prediction: "2,602".to_owned(),
        },
        EvoSkillAnswerAttempt {
            source_id: "UID0002".to_owned(),
            ground_truth: "507".to_owned(),
            prediction: "2023".to_owned(),
        },
        EvoSkillAnswerAttempt {
            source_id: "UID0003".to_owned(),
            ground_truth: "44,463".to_owned(),
            prediction: "44".to_owned(),
        },
    ];

    let failures = extract_evoskill_failure_feedback(attempts);

    assert_eq!(failures.len(), 2);
    assert_eq!(failures[0].source_id, "UID0002");
    assert_eq!(failures[1].source_id, "UID0003");
    assert!(failures[0].weighted_score < DEFAULT_FAILURE_THRESHOLD);
    assert!(failures[0].feedback.contains("UID0002"));
    assert!(failures[0].feedback.contains("507"));
    assert!(failures[0].feedback.contains("2023"));
}

#[test]
fn scored_attempt_preserves_tolerance_scores_for_reporting() {
    let scored = score_evoskill_attempt(EvoSkillAnswerAttempt {
        source_id: "UID0004".to_owned(),
        ground_truth: "100".to_owned(),
        prediction: "100.5".to_owned(),
    });

    assert_eq!(scored.source_id, "UID0004");
    assert!(scored.score.is_failure);
    assert_eq!(scored.score.tolerance_scores.len(), 5);
    assert_eq!(scored.ground_truth, "100");
    assert_eq!(scored.prediction, "100.5");
}
