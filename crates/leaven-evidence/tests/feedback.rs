use leaven_evidence::{CaseAssessmentEvidence, OutputRecord, ScalarEvidence};

#[test]
fn case_assessment_preserves_output_score_and_feedback() {
    let evidence = CaseAssessmentEvidence::new(
        ScalarEvidence::new(0.75).unwrap(),
        OutputRecord::inline("generated answer"),
        "judge explanation",
    );

    assert!((evidence.score().score() - 0.75).abs() < f64::EPSILON);
    assert_eq!(evidence.output(), &OutputRecord::inline("generated answer"));
    assert_eq!(evidence.feedback(), "judge explanation");
}
