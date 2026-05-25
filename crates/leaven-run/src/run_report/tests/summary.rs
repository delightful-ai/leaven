use std::path::Path;

use super::*;
use crate::{OptimizeError, RunNotResumableReason, RunResumability};

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
