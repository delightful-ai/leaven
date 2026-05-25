use super::*;
use leaven_store::EvidenceStore;

#[test]
fn split_reports_group_custom_partitions() {
    futures::executor::block_on(async {
        let mut harness = report_harness();
        seed_split_reports(&mut harness).await;
        let reports =
            split_reports_for(&harness.engine.view(), &harness.store, &harness.splits).unwrap();

        assert_eq!(reports.len(), 2);
        assert!(reports.iter().any(|report| report.role == SplitRole::Train));
        assert!(
            reports
                .iter()
                .any(|report| report.role == SplitRole::Custom("audit".into()))
        );
    });
}

#[test]
fn split_reports_refuse_non_independent_partition_rows() {
    futures::executor::block_on(async {
        let harness = report_harness();
        let mut bad_engine =
            leaven_engine::Engine::<RunProblem<TestArtifact, &'static str>>::builder()
                .budget(leaven_kernel::Budget::unlimited())
                .evaluator(BadPartitionEvaluator)
                .build();
        let bad_first = bad_engine.insert_seed(TestArtifact, 0).unwrap();
        let bad_second = bad_engine.insert_seed(TestArtifact, 1).unwrap();
        let bad_store = InlineEvidenceStore::<CaseAssessmentEvidence>::new("bad-report-group");
        let malformed = bad_engine
            .evaluate(
                EvaluatorId::PRIMARY,
                EvaluationRequest::Independent {
                    candidates: vec![bad_first, bad_second],
                    set: EvaluationSet::Partition(PartitionId::from("TRAIN")),
                    granularity: AssessmentGranularity::PerCase,
                    purpose: EvaluationPurpose::Probe,
                },
                &harness.case_set,
                &bad_store,
            )
            .await
            .unwrap();
        assert!(!malformed.assessment_ids.is_empty());
        let error = split_reports_for(&bad_engine.view(), &bad_store, &harness.splits)
            .expect_err("split reports must reject non-independent partition rows");
        assert!(error.to_string().contains("independent assessment"));
    });
}

fn split_reports_for<A, I, T>(
    view: &leaven_engine::RunGraphView<'_, RunProblem<A, I, T>>,
    store: &dyn EvidenceStore<CaseAssessmentEvidence>,
    splits: &DatasetSplits,
) -> Result<Vec<SplitReport>, leaven_engine::OptimizerError>
where
    A: Artifact,
    I: Clone + Send + Sync + 'static,
    T: Clone + Send + Sync + 'static,
{
    let mut groups = BTreeMap::<
        (PartitionId, SplitRole, EvaluationRequestId, CandidateId),
        Vec<AssessmentId>,
    >::new();
    for assessment in view.all_assessments() {
        let Some((partition, role)) = assessment_split(view, assessment.id()) else {
            continue;
        };
        if splits.role(&partition).is_none() {
            continue;
        }
        let candidate = assessment.independent_candidate().ok_or_else(|| {
            leaven_engine::OptimizerError::Message(
                "report expected independent assessment".to_owned(),
            )
        })?;
        groups
            .entry((partition, role, assessment.request_id(), candidate))
            .or_default()
            .push(assessment.id());
    }

    let mut reports = BTreeMap::<PartitionId, SplitReport>::new();
    for ((partition, role, _, _), assessments) in groups {
        let summary = assessment_summary(view, store, &assessments)?;
        reports
            .entry(partition.clone())
            .or_insert_with(|| SplitReport {
                role,
                partition,
                candidates: Vec::new(),
            })
            .candidates
            .push(summary);
    }
    Ok(reports.into_values().collect())
}

fn assessment_split<A, I, T>(
    view: &leaven_engine::RunGraphView<'_, RunProblem<A, I, T>>,
    assessment: AssessmentId,
) -> Option<(PartitionId, SplitRole)>
where
    A: Artifact,
    I: Clone + Send + Sync + 'static,
    T: Clone + Send + Sync + 'static,
{
    let request_id = view.assessment(assessment)?.request_id();
    let evaluation_request = view.evaluation_request(request_id)?;
    let request = evaluation_request.request();
    let partition = match request {
        EvaluationRequest::Independent {
            set: EvaluationSet::Partition(partition),
            ..
        } => partition.clone(),
        _ => return None,
    };
    let role = match partition.0.as_str() {
        "TRAIN" => SplitRole::Train,
        "VALIDATION" => SplitRole::Validation,
        "TEST" => SplitRole::Test,
        other => SplitRole::Custom(other.to_owned().into()),
    };
    Some((partition, role))
}

async fn seed_split_reports(harness: &mut ReportHarness) {
    for partition in [
        PartitionId::from("TRAIN"),
        PartitionId::from("audit"),
        PartitionId::from("ignored"),
    ] {
        harness
            .engine
            .evaluate(
                EvaluatorId::PRIMARY,
                partition_request(harness.first, partition),
                &harness.case_set,
                &harness.store,
            )
            .await
            .unwrap();
    }
}

fn partition_request(candidate: CandidateId, partition: PartitionId) -> EvaluationRequest {
    EvaluationRequest::Independent {
        candidates: vec![candidate],
        set: EvaluationSet::Partition(partition),
        granularity: AssessmentGranularity::PerCase,
        purpose: EvaluationPurpose::Probe,
    }
}
