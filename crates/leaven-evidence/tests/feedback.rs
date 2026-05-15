use leaven_evidence::{FeedbackAttachment, OutputRecord, ScalarEvidence, ScoredFeedbackEvidence};

#[test]
fn scored_feedback_preserves_trace_feedback_and_attachments() {
    let evidence = ScoredFeedbackEvidence::new(
        ScalarEvidence::new(0.75).unwrap(),
        "judge rationale",
        vec!["runner trace".to_owned()],
    )
    .with_attachments(vec![FeedbackAttachment::text(
        "judge-transcript",
        "full judge transcript",
    )]);

    assert!((evidence.score().score() - 0.75).abs() < f64::EPSILON);
    assert_eq!(evidence.feedback(), "judge rationale");
    assert_eq!(evidence.trace(), &["runner trace".to_owned()]);
    assert_eq!(evidence.attachments().len(), 1);
    let attachment = &evidence.attachments()[0];
    assert_eq!(attachment.name(), "judge-transcript");
    assert_eq!(attachment.media_type(), Some("text/plain"));
    assert_eq!(
        attachment.record(),
        &OutputRecord::inline("full judge transcript")
    );
}
