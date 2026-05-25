use super::super::assessment::report_score;
use super::*;

#[test]
fn report_scores_preserve_inline_and_blob_outputs() {
    let inline = report_score(
        leaven_kernel::CaseId::new(1),
        leaven_kernel::EvidenceRef {
            store: "test".to_owned(),
            key: "inline".to_owned(),
        },
        &CaseAssessmentEvidence::new(
            ScalarEvidence::new(1.0).unwrap(),
            OutputRecord::inline("inline answer"),
            "inline feedback",
        ),
    );
    let blob = report_score(
        leaven_kernel::CaseId::new(2),
        leaven_kernel::EvidenceRef {
            store: "test".to_owned(),
            key: "blob".to_owned(),
        },
        &CaseAssessmentEvidence::new(
            ScalarEvidence::new(0.25).unwrap(),
            OutputRecord::blob(leaven_kernel::BlobRef {
                store: "blob-store".to_owned(),
                key: "answer.txt".to_owned(),
            }),
            "blob feedback",
        )
        .with_trace(["provider transcript"]),
    );

    assert_eq!(inline.output, "inline answer");
    assert_eq!(inline.feedback, "inline feedback");
    assert_eq!(inline.output_ref.as_ref().unwrap().key, "inline");
    assert_eq!(blob.feedback_ref.as_ref().unwrap().key, "blob");
    assert!(inline.trace_refs.is_empty());
    assert_eq!(
        blob.trace_refs,
        blob.feedback_ref.iter().cloned().collect::<Vec<_>>()
    );
    assert_eq!(blob.output, "blob:blob-store:answer.txt");
    assert!((blob.score - 0.25).abs() < f64::EPSILON);
}

#[test]
fn assessment_summary_refuses_bad_assessment_groups() {
    futures::executor::block_on(async {
        let mut harness = report_harness();
        let mixed_candidates = harness
            .engine
            .evaluate(
                EvaluatorId::PRIMARY,
                EvaluationRequest::Independent {
                    candidates: vec![harness.first, harness.second],
                    set: EvaluationSet::All,
                    granularity: AssessmentGranularity::PerCase,
                    purpose: EvaluationPurpose::Probe,
                },
                &harness.case_set,
                &harness.store,
            )
            .await
            .unwrap();
        let error = assessment_summary(
            &harness.engine.view(),
            &harness.store,
            &mixed_candidates.assessment_ids,
        )
        .expect_err("mixed candidate assessment group must be rejected");
        assert!(error.to_string().contains("mixed candidates"));

        let first_request = harness
            .engine
            .evaluate(
                EvaluatorId::PRIMARY,
                all_cases_request(harness.first, AssessmentGranularity::PerCase),
                &harness.case_set,
                &harness.store,
            )
            .await
            .unwrap();
        let second_request = harness
            .engine
            .evaluate(
                EvaluatorId::PRIMARY,
                all_cases_request(harness.first, AssessmentGranularity::PerCase),
                &harness.case_set,
                &harness.store,
            )
            .await
            .unwrap();
        let error = assessment_summary(
            &harness.engine.view(),
            &harness.store,
            &[
                first_request.assessment_ids[0],
                second_request.assessment_ids[0],
            ],
        )
        .expect_err("mixed request assessment group must be rejected");
        assert!(error.to_string().contains("mixed requests"));

        let aggregate = harness
            .engine
            .evaluate(
                EvaluatorId::PRIMARY,
                all_cases_request(harness.first, AssessmentGranularity::Aggregate),
                &harness.case_set,
                &harness.store,
            )
            .await
            .unwrap();
        let error = assessment_summary(
            &harness.engine.view(),
            &harness.store,
            &aggregate.assessment_ids,
        )
        .expect_err("aggregate assessment group must be rejected");
        assert!(error.to_string().contains("case-targeted"));

        let pairwise = harness
            .engine
            .evaluate(
                EvaluatorId::PRIMARY,
                EvaluationRequest::Pairwise {
                    left: harness.first,
                    right: harness.second,
                    set: EvaluationSet::All,
                    granularity: AssessmentGranularity::PerCase,
                    purpose: EvaluationPurpose::Probe,
                    order: leaven_core::PairOrder::Ordered,
                },
                &harness.case_set,
                &harness.store,
            )
            .await
            .unwrap();
        let error = assessment_summary(
            &harness.engine.view(),
            &harness.store,
            &pairwise.assessment_ids,
        )
        .expect_err("non-independent assessment group must be rejected");
        assert!(error.to_string().contains("independent assessment"));
    });
}

fn all_cases_request(
    candidate: CandidateId,
    granularity: AssessmentGranularity,
) -> EvaluationRequest {
    EvaluationRequest::Independent {
        candidates: vec![candidate],
        set: EvaluationSet::All,
        granularity,
        purpose: EvaluationPurpose::Probe,
    }
}
