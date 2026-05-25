//! Product-run report and summary construction.

use leaven_core::{Artifact, EvaluationPurpose, PartitionId};
use leaven_eval::{
    CandidateEvaluationSummary, Case, Dataset, DatasetSplits, EvaluationReport, SplitReport,
    SplitRole,
};
use leaven_kernel::{BudgetSnapshot, CandidateId, Cost};

use crate::{
    RunProblem,
    result::{BestCandidate, RunEventSummary, RunReportPaths, RunStorage, StandardRunSummary},
};

mod assessment;
mod events;
mod storage;

pub use assessment::final_eval;
use events::{event_summary, run_cache_summary, should_include_event_summary};
pub use storage::{report_paths_for, run_storage, write_summary_report};

#[cfg(test)]
use assessment::{assessment_summary, report_score};

pub struct FinalEvaluations {
    pub baseline_train: Option<CandidateEvaluationSummary>,
    pub train: Option<CandidateEvaluationSummary>,
    pub baseline_validation: Option<CandidateEvaluationSummary>,
    pub validation: Option<CandidateEvaluationSummary>,
    pub baseline_test: Option<CandidateEvaluationSummary>,
    pub test: Option<CandidateEvaluationSummary>,
    pub cost: Cost,
}

pub struct FinalEvaluationInputs {
    pub seed: CandidateId,
    pub best: Option<CandidateId>,
    pub has_train: bool,
    pub has_validation: bool,
    pub has_test: bool,
}

impl FinalEvaluationInputs {
    pub const fn has_any_split(&self) -> bool {
        self.has_train || self.has_validation || self.has_test
    }
}

pub struct FinalPartitionEvaluation {
    pub partition: PartitionId,
    pub purpose: EvaluationPurpose,
}

pub struct FinalPartitionResults {
    pub baseline: CandidateEvaluationSummary,
    pub optimized: Option<CandidateEvaluationSummary>,
    pub cost: Cost,
}

pub struct ReportInputs<'a, I, T> {
    pub dataset: &'a Dataset<Case<I, T>>,
    pub splits: &'a DatasetSplits,
    pub best: Option<CandidateId>,
    pub final_evaluations: &'a FinalEvaluations,
    pub optimization_budget: BudgetSnapshot,
    pub storage: RunStorage,
    pub reports: RunReportPaths,
    pub compatibility: Option<crate::result::RunCompatibilitySummary>,
    pub stop_reason: leaven_engine::StopReason,
}

type SummaryBuild<A> = (
    Option<BestCandidate<A>>,
    StandardRunSummary,
    Vec<RunEventSummary>,
);

pub fn build_summary<A, I, T>(
    engine: &leaven_engine::Engine<RunProblem<A, I, T>>,
    inputs: ReportInputs<'_, I, T>,
) -> SummaryBuild<A>
where
    A: Artifact,
    I: Clone + Send + Sync + 'static,
    T: Clone + Send + Sync + 'static,
{
    let view = engine.view();
    let best = inputs.best.map(|id| BestCandidate {
        id,
        artifact: view.artifact(id).expect("best exists").clone(),
    });
    let budget = engine.budget().snapshot();
    let cost = budget.spent.clone();
    let cache = run_cache_summary(view.events(), &inputs.storage);
    let summary = StandardRunSummary {
        storage: inputs.storage,
        reports: inputs.reports,
        compatibility: inputs.compatibility,
        optimization_budget: inputs.optimization_budget.clone(),
        budget,
        optimization_cost: inputs.optimization_budget.spent,
        final_report_cost: inputs.final_evaluations.cost.clone(),
        cost: cost.clone(),
        cache,
        baseline_train_score: inputs
            .final_evaluations
            .baseline_train
            .as_ref()
            .and_then(|summary| summary.average_score),
        optimized_train_score: inputs
            .final_evaluations
            .train
            .as_ref()
            .and_then(|summary| summary.average_score),
        baseline_validation_score: inputs
            .final_evaluations
            .baseline_validation
            .as_ref()
            .and_then(|summary| summary.average_score),
        validation_score: inputs
            .final_evaluations
            .validation
            .as_ref()
            .and_then(|summary| summary.average_score),
        baseline_test_score: inputs
            .final_evaluations
            .baseline_test
            .as_ref()
            .and_then(|summary| summary.average_score),
        test_score: inputs
            .final_evaluations
            .test
            .as_ref()
            .and_then(|summary| summary.average_score),
        evaluation: EvaluationReport {
            dataset: inputs.dataset.fingerprint(),
            splits: inputs.splits.fingerprint(),
            cost,
            splits_reported: final_evaluation_split_reports(inputs.final_evaluations),
        },
    };
    let events = view
        .events()
        .filter(|event| should_include_event_summary(event, inputs.stop_reason))
        .map(event_summary)
        .collect();
    (best, summary, events)
}

fn final_evaluation_split_reports(final_evaluations: &FinalEvaluations) -> Vec<SplitReport> {
    let mut reports = Vec::new();
    push_final_split_report(
        &mut reports,
        SplitRole::Train,
        PartitionId::from("TRAIN"),
        final_evaluations.baseline_train.clone(),
        final_evaluations.train.clone(),
    );
    push_final_split_report(
        &mut reports,
        SplitRole::Validation,
        PartitionId::from("VALIDATION"),
        final_evaluations.baseline_validation.clone(),
        final_evaluations.validation.clone(),
    );
    push_final_split_report(
        &mut reports,
        SplitRole::Test,
        PartitionId::from("TEST"),
        final_evaluations.baseline_test.clone(),
        final_evaluations.test.clone(),
    );
    reports
}

fn push_final_split_report(
    reports: &mut Vec<SplitReport>,
    role: SplitRole,
    partition: PartitionId,
    baseline: Option<CandidateEvaluationSummary>,
    optimized: Option<CandidateEvaluationSummary>,
) {
    let mut candidates = Vec::new();
    if let Some(baseline) = baseline {
        candidates.push(baseline);
    }
    if let Some(optimized) = optimized {
        candidates.push(optimized);
    }
    if !candidates.is_empty() {
        reports.push(SplitReport {
            role,
            partition,
            candidates,
        });
    }
}

#[cfg(test)]
mod tests {
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
    use leaven_store::EvidenceStore;
    use leaven_store_inline::InlineEvidenceStore;

    use crate::{
        EvaluationCacheBackend, EvaluationCacheBypassReason, EvaluationCacheBypassSummary,
        OptimizeError, RunNotResumableReason, RunResumability,
    };

    use super::*;

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
    fn cache_summary_groups_hits_misses_and_bypasses_by_storage_status() {
        let request_id = EvaluationRequestId::new();
        let evaluator = EvaluatorId::PRIMARY;
        let events = [
            leaven_engine::RunEvent::OptimizationStarted {
                run_id: leaven_kernel::RunId::new(),
            },
            completed(
                request_id,
                evaluator.clone(),
                leaven_engine::CacheStatus::Hit,
                Cost::zero(),
            ),
            completed(
                EvaluationRequestId::new(),
                evaluator.clone(),
                leaven_engine::CacheStatus::Hit,
                Cost::metric_calls(1),
            ),
            completed(
                EvaluationRequestId::new(),
                evaluator.clone(),
                leaven_engine::CacheStatus::Miss,
                Cost::zero(),
            ),
            completed(
                EvaluationRequestId::new(),
                evaluator.clone(),
                leaven_engine::CacheStatus::Bypassed(
                    leaven_engine::CacheBypassReason::DisabledByPolicy,
                ),
                Cost::zero(),
            ),
            completed(
                EvaluationRequestId::new(),
                evaluator.clone(),
                leaven_engine::CacheStatus::Bypassed(
                    leaven_engine::CacheBypassReason::DisabledByPolicy,
                ),
                Cost::zero(),
            ),
            completed(
                EvaluationRequestId::new(),
                evaluator.clone(),
                leaven_engine::CacheStatus::Bypassed(
                    leaven_engine::CacheBypassReason::CacheUnavailable,
                ),
                Cost::zero(),
            ),
            completed(
                EvaluationRequestId::new(),
                evaluator,
                leaven_engine::CacheStatus::Bypassed(
                    leaven_engine::CacheBypassReason::MissingCandidateIdentity {
                        candidate: CandidateId::new(),
                    },
                ),
                Cost::zero(),
            ),
        ];

        let storage = RunStorage::Stored {
            run_id: leaven_kernel::RunId::new(),
            run_dir: Some(".leaven/runs/test".into()),
            latest_checkpoint: Some(leaven_kernel::CheckpointId::new()),
            resumability: RunResumability::Resumable,
        };
        let durable = run_cache_summary(events.iter(), &storage).evaluation;

        assert!(durable.durable);
        assert_eq!(durable.backend, EvaluationCacheBackend::SqliteRunStore);
        assert_eq!(durable.hits, 2);
        assert_eq!(durable.misses, 1);
        assert_eq!(durable.write_errors, 0);
        assert!(!durable.hit_cost_zero);
        assert_eq!(
            durable.bypasses,
            vec![
                EvaluationCacheBypassSummary {
                    reason: EvaluationCacheBypassReason::DisabledByPolicy,
                    count: 2,
                },
                EvaluationCacheBypassSummary {
                    reason: EvaluationCacheBypassReason::CacheUnavailable,
                    count: 1,
                },
                EvaluationCacheBypassSummary {
                    reason: EvaluationCacheBypassReason::MissingCandidateIdentity,
                    count: 1,
                },
            ]
        );

        let ephemeral = run_cache_summary(
            events.iter(),
            &RunStorage::Ephemeral {
                run_id: leaven_kernel::RunId::new(),
            },
        )
        .evaluation;
        assert!(!ephemeral.durable);
        assert_eq!(ephemeral.backend, EvaluationCacheBackend::InMemory);
    }

    #[test]
    fn event_summary_maps_lifecycle_and_budget_events() {
        assert_event_summary(
            leaven_engine::RunEvent::OptimizationStarted {
                run_id: leaven_kernel::RunId::new(),
            },
            RunEventSummary::OptimizationStarted,
        );
        assert_event_summary(
            leaven_engine::RunEvent::IterationStarted {
                iteration: IterationId::new(),
            },
            RunEventSummary::IterationStarted,
        );
        assert_event_summary(
            leaven_engine::RunEvent::BudgetCharged {
                stage: StageId::custom("test"),
                cost: Cost::metric_calls(1),
                remaining: BudgetSnapshot::default(),
            },
            RunEventSummary::BudgetCharged,
        );
        assert_event_summary(
            leaven_engine::RunEvent::IterationEnded {
                iteration: IterationId::new(),
            },
            RunEventSummary::IterationEnded,
        );
        assert_event_summary(
            leaven_engine::RunEvent::OptimizationStopping {
                reason: leaven_engine::StopReason::OptimizerDone,
            },
            RunEventSummary::OptimizationStopping,
        );
        assert_event_summary(
            leaven_engine::RunEvent::OptimizationEnded {
                run_id: leaven_kernel::RunId::new(),
                best: None,
                budget: BudgetSnapshot::default(),
            },
            RunEventSummary::OptimizationEnded,
        );
    }

    #[test]
    fn event_summary_maps_graph_stage_eval_population_and_error_events() {
        let error = ErrorRecord::new(ErrorKind::Internal, "bad input");
        let receipt = StageAttemptReceiptRef {
            id: StageAttemptReceiptId::new(),
            fingerprint: None,
        };
        assert_event_summary(
            leaven_engine::RunEvent::ProposalBatchProduced {
                iteration: Some(IterationId::new()),
                batch_id: ProposalBatchId::new(),
                proposer: StageId::custom("proposer"),
                proposal_count: 1,
            },
            RunEventSummary::ProposalBatchProduced,
        );
        assert_event_summary(
            leaven_engine::RunEvent::ProposalRecorded {
                proposal_id: ProposalId::new(),
                batch_id: ProposalBatchId::new(),
                effect: ProposalEffectKind::Create,
                causal: CausalInputs::None,
                informed_by_count: 0,
            },
            RunEventSummary::ProposalRecorded,
        );
        assert_event_summary(
            leaven_engine::RunEvent::StageAttemptRecorded {
                stage_call_id: StageCallId::new(),
                role: StageRole::reflect(),
                receipt,
                outcome: StageAttemptOutcome::Failed(StageAttemptFailure::OutputParse),
            },
            RunEventSummary::StageAttemptRecorded,
        );
        assert_event_summary(
            leaven_engine::RunEvent::ApplySucceeded {
                proposal_id: ProposalId::new(),
                candidate_id: CandidateId::new(),
            },
            RunEventSummary::ApplySucceeded,
        );
        assert_event_summary(
            leaven_engine::RunEvent::ApplyFailed {
                proposal_id: ProposalId::new(),
                error: error.clone(),
            },
            RunEventSummary::ApplyFailed,
        );
        assert_event_summary(
            leaven_engine::RunEvent::EvaluationRequested {
                request_id: EvaluationRequestId::new(),
                evaluator: EvaluatorId::PRIMARY,
                request: leaven_engine::EvaluationRequestSummary { candidate_count: 1 },
            },
            RunEventSummary::EvaluationRequested,
        );
        assert_event_summary(
            completed(
                EvaluationRequestId::new(),
                EvaluatorId::PRIMARY,
                leaven_engine::CacheStatus::Miss,
                Cost::zero(),
            ),
            RunEventSummary::EvaluationCompleted,
        );
        assert_event_summary(
            leaven_engine::RunEvent::PopulationUpdated {
                population_id: PopulationId::new(),
                events: Vec::new(),
            },
            RunEventSummary::PopulationUpdated,
        );
        assert_event_summary(
            leaven_engine::RunEvent::Error {
                stage: None,
                error,
                policy: leaven_engine::ErrorPolicy::StoppedRun,
            },
            RunEventSummary::Error,
        );
    }

    #[test]
    fn budget_stop_summary_suppresses_only_the_synthetic_budget_error() {
        let budget_error = leaven_engine::RunEvent::Error {
            stage: None,
            error: ErrorRecord::new(ErrorKind::Budget, "budget exhausted"),
            policy: leaven_engine::ErrorPolicy::StoppedRun,
        };
        assert!(!should_include_event_summary(
            &budget_error,
            leaven_engine::StopReason::BudgetReached
        ));
        assert!(should_include_event_summary(
            &budget_error,
            leaven_engine::StopReason::BudgetExceeded
        ));

        let continued_budget_error = leaven_engine::RunEvent::Error {
            stage: None,
            error: ErrorRecord::new(ErrorKind::Budget, "budget warning"),
            policy: leaven_engine::ErrorPolicy::Continued,
        };
        assert!(should_include_event_summary(
            &continued_budget_error,
            leaven_engine::StopReason::BudgetReached
        ));

        let internal_error = leaven_engine::RunEvent::Error {
            stage: None,
            error: ErrorRecord::new(ErrorKind::Internal, "runner failed"),
            policy: leaven_engine::ErrorPolicy::StoppedRun,
        };
        assert!(should_include_event_summary(
            &internal_error,
            leaven_engine::StopReason::BudgetReached
        ));
    }

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

    fn assert_event_summary(event: leaven_engine::RunEvent, expected: RunEventSummary) {
        assert_eq!(event_summary(&event), expected);
        drop(event);
    }

    fn completed(
        request_id: EvaluationRequestId,
        evaluator: EvaluatorId,
        cache: leaven_engine::CacheStatus,
        cost: Cost,
    ) -> leaven_engine::RunEvent {
        leaven_engine::RunEvent::EvaluationCompleted {
            request_id,
            evaluator,
            assessment_ids: vec![AssessmentId::new()],
            cost,
            cache,
        }
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
}
