use leaven_evidence::{
    CandidateAssessmentOutput, CandidateAssessmentOutputError, CaseAssessmentEvidence, DataClass,
    DataClassSet, OutputMetadata, OutputRecord, OutputVisibility, ScalarEvidence,
};
use leaven_kernel::{CandidateId, CaseId};

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

#[test]
fn candidate_assessment_output_requires_assessed_candidate_data() {
    let candidate = CandidateId::new();
    let output =
        CandidateAssessmentOutput::new(candidate, OutputRecord::candidate_inline("answer"))
            .expect("candidate output is accepted");

    assert_eq!(output.candidate(), candidate);
    assert_eq!(output.output(), &OutputRecord::candidate_inline("answer"));

    let public_only =
        CandidateAssessmentOutput::new(candidate, OutputRecord::inline("summary")).unwrap_err();
    assert_eq!(
        public_only,
        CandidateAssessmentOutputError::MissingAssessedDataClass
    );

    let empty = CandidateAssessmentOutput::new(candidate, OutputRecord::candidate_inline(" \n\t "))
        .unwrap_err();
    assert_eq!(empty, CandidateAssessmentOutputError::EmptyInlineOutput);

    let artifact = OutputRecord::inline("artifact answer").with_metadata(OutputMetadata::new(
        OutputVisibility::Public,
        DataClassSet::new([DataClass::candidate_artifact(), DataClass::public()]),
    ));
    assert!(CandidateAssessmentOutput::new(candidate, artifact).is_ok());
}

#[test]
fn case_assessment_preserves_candidate_bound_outputs() {
    let candidate = CandidateId::new();
    let output =
        CandidateAssessmentOutput::new(candidate, OutputRecord::candidate_inline("answer"))
            .unwrap();
    let evidence = CaseAssessmentEvidence::new(
        ScalarEvidence::new(0.75).unwrap(),
        OutputRecord::candidate_inline("answer"),
        "judge explanation",
    )
    .with_candidate_outputs([output.clone()]);

    assert_eq!(evidence.candidate_outputs(), &[output]);
}

#[test]
fn case_data_read_evidence_preserves_read_values() {
    let read = leaven_evidence::CaseDataReadEvidence::new(
        "case_query.load",
        "qrec_case_1_target",
        CaseId::new(1),
        ["target"],
        ["case.target"],
    )
    .with_value("target", serde_json::json!({"answer": "42"}));

    assert_eq!(read.operation(), "case_query.load");
    assert_eq!(read.fields(), &["target"]);
    assert_eq!(read.values()["target"], serde_json::json!({"answer": "42"}));
}
