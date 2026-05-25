use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

use leaven_core::{
    Artifact, ArtifactIdentity, Assessment, AssessmentGranularity, AssessmentTarget,
    CaseSetVersion, CausalInputs, EvaluationRequest, EvaluationSet, ProposalEffectKind,
    ResolvedEvaluationRequest, ResolvedRequestKind,
};
use leaven_eval::SplitPolicy;
use leaven_evidence::{CaseAssessmentEvidence, OutputRecord, ScalarEvidence};
use leaven_kernel::{
    AssessmentId, BudgetSnapshot, CandidateId, CaseId, ContentId, Cost, ErrorKind, ErrorRecord,
    EvaluationRequestId, EvaluationSetId, EvaluatorId, Fingerprint, IterationId, MetadataBag,
    Metered, PopulationId, ProposalBatchId, ProposalId, RunId, StageAttemptFailure,
    StageAttemptOutcome, StageAttemptReceiptId, StageAttemptReceiptRef, StageCallId, StageId,
    StageRole,
};
use leaven_store_inline::InlineEvidenceStore;

use crate::{OptimizeError, RunNotResumableReason, RunResumability};

use super::assessment::assessment_summary;
use super::*;

mod assessment;
mod events;
mod splits;

fn report_summary(candidate: CandidateId, score: f64) -> CandidateEvaluationSummary {
    CandidateEvaluationSummary {
        candidate,
        request: EvaluationRequestId::new(),
        assessments: Vec::new(),
        average_score: Some(score),
        cases: Vec::new(),
    }
}

#[test]
fn summary_report_atomic_write_rejects_paths_without_file_names() {
    let error = storage::write_report_atomic(Path::new(""), b"{}", "write summary json")
        .expect_err("report atomic writes require a file path");

    assert!(matches!(error, OptimizeError::ReportStore { .. }));
    assert_eq!(
        error.to_string(),
        "run report failed during write summary json"
    );
}

#[test]
fn final_report_edges_refuse_empty_assessments_and_missing_checkpoints() {
    let no_splits = FinalEvaluationInputs {
        seed: CandidateId::new(),
        best: None,
        has_train: false,
        has_validation: false,
        has_test: false,
    };
    assert!(!no_splits.has_any_split());
    assert!(
        FinalEvaluationInputs {
            has_validation: true,
            ..no_splits
        }
        .has_any_split()
    );
    let explicit_store =
        crate::run_store::StoreConfig::<RunProblem<TestArtifact, (), ()>>::Explicit(
            crate::OptimizeStore::inline("explicit-source"),
        );
    assert!(matches!(
        explicit_store.into_source(),
        crate::run_store::StoreSource::DefaultDurable
    ));
    let run_case = crate::RunCase::from_case(&leaven_eval::Case::targeted(
        CaseId::from_index(99),
        "runner input",
        "hidden target",
    ));
    assert_eq!(run_case.id(), CaseId::from_index(99));
    assert_eq!(*run_case.input(), "runner input");
    assert_eq!(run_case.into_input(), "runner input");

    let engine = leaven_engine::Engine::<RunProblem<TestArtifact, (), ()>>::builder()
        .budget(leaven_kernel::Budget::unlimited())
        .build();
    let store = InlineEvidenceStore::<CaseAssessmentEvidence>::new("report-test");
    let error = assessment_summary(&engine.view(), &store, &[])
        .expect_err("empty assessment group must be rejected");
    assert!(error.to_string().contains("at least one assessment"));
    let error = assessment_summary(&engine.view(), &store, &[AssessmentId::new()])
        .expect_err("unknown assessment group member must be rejected");
    assert!(error.to_string().contains("assessment missing"));

    let run_id = RunId::new();
    let prepared = crate::run_store::PreparedStore::<RunProblem<TestArtifact, (), ()>> {
        store: crate::OptimizeStore::durable(
            InlineEvidenceStore::<CaseAssessmentEvidence>::new("durable-report-test"),
            NoopPersistence,
        ),
        run_dir: Some(".leaven/runs/report-test".into()),
        local_persistence: None,
        evaluation_cache: None,
        start: crate::run_store::StoreStart::Fresh { run_id },
    };
    let storage = run_storage(run_id, &prepared, None, true);
    assert!(matches!(
        storage,
        RunStorage::Stored {
            latest_checkpoint: None,
            resumability: RunResumability::NotResumable {
                reason: RunNotResumableReason::MissingLatestCheckpoint
            },
            ..
        }
    ));

    let storage = run_storage(
        run_id,
        &prepared,
        Some(leaven_kernel::CheckpointId::new()),
        false,
    );
    assert!(matches!(
        storage,
        RunStorage::Stored {
            resumability: RunResumability::NotResumable {
                reason: RunNotResumableReason::MissingCompatibilityManifest
            },
            ..
        }
    ));
}

#[test]
fn final_evaluation_split_reports_preserve_baseline_and_optimized_roles() {
    let candidate = CandidateId::new();
    let train = report_summary(candidate, 1.0);
    let validation = report_summary(candidate, 0.5);
    let test = report_summary(candidate, 0.0);
    let reports = final_evaluation_split_reports(&FinalEvaluations {
        baseline_train: Some(train.clone()),
        train: Some(train),
        baseline_validation: Some(validation.clone()),
        validation: Some(validation),
        baseline_test: Some(test.clone()),
        test: Some(test),
        cost: Cost::zero(),
    });

    assert_eq!(reports.len(), 3);
    for report in &reports {
        assert_eq!(report.candidates.len(), 2);
        assert_eq!(report.candidates[0].candidate, candidate);
        assert_eq!(report.candidates[1].candidate, candidate);
    }
    assert_eq!(reports[0].role, SplitRole::Train);
    assert_eq!(reports[1].role, SplitRole::Validation);
    assert_eq!(reports[2].role, SplitRole::Test);
}

#[test]
fn build_summary_projects_final_scores_storage_cache_and_events() {
    let harness = report_harness();
    let train = report_summary(harness.first, 1.0);
    let validation = report_summary(harness.first, 0.5);
    let test = report_summary(harness.first, 0.25);
    let final_evaluations = FinalEvaluations {
        baseline_train: Some(train.clone()),
        train: Some(train),
        baseline_validation: Some(validation.clone()),
        validation: Some(validation),
        baseline_test: Some(test.clone()),
        test: Some(test),
        cost: Cost::metric_calls(3),
    };
    let dataset = Dataset::from_cases(vec![
        Case::input(CaseId::from_index(0), "train"),
        Case::input(CaseId::from_index(1), "audit"),
    ])
    .unwrap();
    let storage = RunStorage::Stored {
        run_id: RunId::new(),
        run_dir: Some(".leaven/runs/report-test".into()),
        latest_checkpoint: Some(leaven_kernel::CheckpointId::new()),
        resumability: RunResumability::Resumable,
    };

    let (best, summary, events) = build_summary(
        &harness.engine,
        ReportInputs {
            dataset: &dataset,
            splits: &harness.splits,
            best: Some(harness.first),
            final_evaluations: &final_evaluations,
            optimization_budget: BudgetSnapshot {
                spent: Cost::metric_calls(2),
                ..BudgetSnapshot::default()
            },
            storage,
            reports: RunReportPaths {
                summary_json: Some(".leaven/runs/report-test/reports/summary.json".into()),
            },
            compatibility: None,
            stop_reason: leaven_engine::StopReason::OptimizerDone,
        },
    );

    assert_eq!(best.unwrap().id, harness.first);
    assert_eq!(summary.optimization_cost, Cost::metric_calls(2));
    assert_eq!(summary.final_report_cost, Cost::metric_calls(3));
    assert_eq!(summary.baseline_train_score, Some(1.0));
    assert_eq!(summary.optimized_train_score, Some(1.0));
    assert_eq!(summary.baseline_validation_score, Some(0.5));
    assert_eq!(summary.validation_score, Some(0.5));
    assert_eq!(summary.baseline_test_score, Some(0.25));
    assert_eq!(summary.test_score, Some(0.25));
    assert_eq!(summary.evaluation.splits_reported.len(), 3);
    assert!(summary.cache.evaluation.durable);
    assert!(events.is_empty());
}

struct ReportHarness {
    case_set: leaven_engine::CaseSet<leaven_eval::Case<&'static str>>,
    engine: leaven_engine::Engine<RunProblem<TestArtifact, &'static str>>,
    first: CandidateId,
    second: CandidateId,
    store: InlineEvidenceStore<CaseAssessmentEvidence>,
    splits: DatasetSplits,
}

fn report_harness() -> ReportHarness {
    let train = PartitionId::from("TRAIN");
    let audit = PartitionId::from("audit");
    let ignored = PartitionId::from("ignored");
    let train_case = CaseId::from_index(0);
    let audit_case = CaseId::from_index(1);
    let case_set = leaven_engine::CaseSet::from_entries([
        (train_case, leaven_eval::Case::input(train_case, "train")),
        (audit_case, leaven_eval::Case::input(audit_case, "audit")),
    ])
    .with_partition(train.clone(), vec![train_case])
    .with_partition(audit.clone(), vec![audit_case])
    .with_partition(ignored, vec![audit_case]);
    let mut engine = leaven_engine::Engine::<RunProblem<TestArtifact, &'static str>>::builder()
        .budget(leaven_kernel::Budget::unlimited())
        .evaluator(ReportEvaluator)
        .build();
    let first = engine.insert_seed(TestArtifact, 0).unwrap();
    let second = engine.insert_seed(TestArtifact, 1).unwrap();
    let splits = DatasetSplits::new(
        CaseSetVersion("report-v1".to_owned()),
        BTreeMap::from([
            (train.clone(), SplitRole::Train),
            (audit.clone(), SplitRole::Custom("audit".into())),
        ]),
        BTreeMap::from([(train, vec![train_case]), (audit, vec![audit_case])]),
        &BTreeSet::from([train_case, audit_case]),
        SplitPolicy::DisjointRequired,
    )
    .unwrap();
    ReportHarness {
        case_set,
        engine,
        first,
        second,
        store: InlineEvidenceStore::<CaseAssessmentEvidence>::new("report-groups"),
        splits,
    }
}

#[derive(Clone, Debug)]
struct TestArtifact;

#[derive(Debug)]
struct TestArtifactError;

impl std::fmt::Display for TestArtifactError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("test artifact error")
    }
}

impl std::error::Error for TestArtifactError {}

impl Artifact for TestArtifact {
    type Change = ();
    type ApplyError = TestArtifactError;

    fn identity(&self) -> ArtifactIdentity {
        ArtifactIdentity::Content(ContentId::from_bytes([7; ContentId::BYTES]))
    }

    fn apply_change(&self, _change: &Self::Change) -> Result<Self, Self::ApplyError> {
        Ok(Self)
    }
}

struct NoopPersistence;

impl leaven_engine::RunPersistence<RunProblem<TestArtifact, (), ()>> for NoopPersistence {
    fn checkpoint(
        &self,
        _request: leaven_engine::RunCheckpointRequest<'_, RunProblem<TestArtifact, (), ()>>,
    ) -> Result<(), leaven_engine::RunPersistenceError> {
        Ok(())
    }
}

struct ReportEvaluator;

impl leaven_engine::Evaluator<RunProblem<TestArtifact, &'static str>> for ReportEvaluator {
    fn id(&self) -> EvaluatorId {
        EvaluatorId::PRIMARY
    }

    fn fingerprint(&self) -> Fingerprint {
        Fingerprint::from_bytes([9; 32])
    }

    async fn evaluate(
        &self,
        request: ResolvedEvaluationRequest,
        _ctx: leaven_engine::EvaluationContext<'_, RunProblem<TestArtifact, &'static str>>,
    ) -> Result<
        Metered<Vec<Assessment<RunProblem<TestArtifact, &'static str>>>>,
        leaven_engine::EvaluationError,
    > {
        let mut assessments = Vec::new();
        match request.kind {
            ResolvedRequestKind::Independent { candidates } => {
                for candidate in candidates {
                    if matches!(request.granularity, AssessmentGranularity::Aggregate) {
                        assessments.push(Assessment::Independent {
                            candidate,
                            target: AssessmentTarget::EvaluationSet(EvaluationSetId::new()),
                            evidence: report_evidence("aggregate"),
                            cost: Cost::metric_calls(1),
                            metadata: MetadataBag::new(),
                        });
                        continue;
                    }
                    for case in &request.set.case_ids {
                        assessments.push(Assessment::Independent {
                            candidate,
                            target: AssessmentTarget::Case {
                                set: EvaluationSetId::new(),
                                case: *case,
                            },
                            evidence: report_evidence("case"),
                            cost: Cost::metric_calls(1),
                            metadata: MetadataBag::new(),
                        });
                    }
                }
            }
            ResolvedRequestKind::Pairwise { left, right, .. } => {
                for case in &request.set.case_ids {
                    assessments.push(Assessment::Pairwise {
                        left,
                        right,
                        target: AssessmentTarget::Case {
                            set: EvaluationSetId::new(),
                            case: *case,
                        },
                        evidence: report_evidence("pairwise"),
                        cost: Cost::metric_calls(1),
                        metadata: MetadataBag::new(),
                    });
                }
            }
            ResolvedRequestKind::Listwise { .. } => {}
        }
        Ok(Metered::new(
            assessments,
            Cost::metric_calls(request.set.case_ids.len() as u64),
        ))
    }
}

fn report_evidence(label: &'static str) -> CaseAssessmentEvidence {
    CaseAssessmentEvidence::new(
        ScalarEvidence::new(1.0).unwrap(),
        OutputRecord::inline(label),
        format!("{label} feedback"),
    )
}

struct BadPartitionEvaluator;

impl leaven_engine::Evaluator<RunProblem<TestArtifact, &'static str>> for BadPartitionEvaluator {
    fn id(&self) -> EvaluatorId {
        EvaluatorId::PRIMARY
    }

    fn fingerprint(&self) -> Fingerprint {
        Fingerprint::from_bytes([8; 32])
    }

    async fn evaluate(
        &self,
        request: ResolvedEvaluationRequest,
        _ctx: leaven_engine::EvaluationContext<'_, RunProblem<TestArtifact, &'static str>>,
    ) -> Result<
        Metered<Vec<Assessment<RunProblem<TestArtifact, &'static str>>>>,
        leaven_engine::EvaluationError,
    > {
        let ResolvedRequestKind::Independent { candidates } = request.kind else {
            return Ok(Metered::new(Vec::new(), Cost::zero()));
        };
        let [left, right, ..] = candidates.as_slice() else {
            return Ok(Metered::new(Vec::new(), Cost::zero()));
        };
        let case = request.set.case_ids[0];
        Ok(Metered::new(
            vec![Assessment::Pairwise {
                left: *left,
                right: *right,
                target: AssessmentTarget::Case {
                    set: EvaluationSetId::new(),
                    case,
                },
                evidence: report_evidence("bad pairwise"),
                cost: Cost::metric_calls(1),
                metadata: MetadataBag::new(),
            }],
            Cost::metric_calls(1),
        ))
    }
}
