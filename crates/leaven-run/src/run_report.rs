//! Product-run report and summary construction.

use std::{collections::BTreeMap, fs};

use leaven_core::{
    Artifact, AssessmentGranularity, AssessmentTarget, EvaluationPurpose, EvaluationRequest,
    EvaluationSet, PartitionId,
};
use leaven_eval::{
    CandidateEvaluationSummary, Case, Dataset, DatasetSplits, EvaluationReport, ReportScore,
    SplitReport, SplitRole,
};
use leaven_evidence::{CaseAssessmentEvidence, OutputRecord};
use leaven_kernel::{
    AssessmentId, BudgetSnapshot, CandidateId, Cost, EvaluationRequestId, EvaluatorId,
};
use leaven_store::EvidenceStore;

use crate::{
    OptimizeError, RunProblem,
    result::{
        BestCandidate, EvaluationCacheBackend, EvaluationCacheBypassReason,
        EvaluationCacheBypassSummary, EvaluationCacheSummary, RunCacheSummary, RunEventSummary,
        RunNotResumableReason, RunReportPaths, RunResumability, RunStorage, StandardRunSummary,
        average,
    },
    run_store::PreparedStore,
};

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
}

type SummaryBuild<A> = (
    Option<BestCandidate<A>>,
    StandardRunSummary,
    Vec<RunEventSummary>,
);

pub fn build_summary<A, I, T>(
    engine: &leaven_engine::Engine<RunProblem<A, I, T>>,
    store: &dyn EvidenceStore<CaseAssessmentEvidence>,
    inputs: ReportInputs<'_, I, T>,
) -> Result<SummaryBuild<A>, leaven_engine::OptimizerError>
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
            splits_reported: split_reports_for(&view, store, inputs.splits)?,
        },
    };
    let events = view.events().map(event_summary).collect();
    Ok((best, summary, events))
}

pub async fn final_eval<A, I, T>(
    engine: &mut leaven_engine::Engine<RunProblem<A, I, T>>,
    case_set: &leaven_engine::CaseSet<Case<I, T>>,
    store: &dyn EvidenceStore<CaseAssessmentEvidence>,
    candidate: leaven_kernel::CandidateId,
    partition: PartitionId,
    purpose: EvaluationPurpose,
) -> Result<(CandidateEvaluationSummary, Cost), leaven_engine::OptimizerError>
where
    A: Artifact,
    I: Clone + Send + Sync + 'static,
    T: Clone + Send + Sync + 'static,
{
    let report = engine
        .evaluate(
            EvaluatorId::PRIMARY,
            EvaluationRequest::Independent {
                candidates: vec![candidate],
                set: EvaluationSet::Partition(partition),
                granularity: AssessmentGranularity::PerCase,
                purpose,
            },
            case_set,
            store,
        )
        .await
        .map_err(|source| {
            leaven_engine::OptimizerError::with_source("final evaluation failed", source)
        })?;
    let view = engine.view();
    Ok((
        assessment_summary(&view, store, &report.assessment_ids)?,
        report.cost,
    ))
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

fn assessment_summary<A, I, T>(
    view: &leaven_engine::RunGraphView<'_, RunProblem<A, I, T>>,
    store: &dyn EvidenceStore<CaseAssessmentEvidence>,
    assessments: &[AssessmentId],
) -> Result<CandidateEvaluationSummary, leaven_engine::OptimizerError>
where
    A: Artifact,
    I: Clone + Send + Sync + 'static,
    T: Clone + Send + Sync + 'static,
{
    let mut candidate = None;
    let mut request = None;
    let mut rows = Vec::with_capacity(assessments.len());
    for assessment in assessments {
        let assessment_view = view.assessment(*assessment).ok_or_else(|| {
            leaven_engine::OptimizerError::Message("assessment missing from graph".to_owned())
        })?;
        let row_candidate = assessment_view.independent_candidate().ok_or_else(|| {
            leaven_engine::OptimizerError::Message(
                "report expected independent assessment".to_owned(),
            )
        })?;
        let row_request = assessment_view.request_id();
        if candidate.is_some_and(|candidate| candidate != row_candidate) {
            return Err(leaven_engine::OptimizerError::Message(
                "report assessment group mixed candidates".to_owned(),
            ));
        }
        if request.is_some_and(|request| request != row_request) {
            return Err(leaven_engine::OptimizerError::Message(
                "report assessment group mixed requests".to_owned(),
            ));
        }
        let case = match assessment_view.target() {
            AssessmentTarget::Case { case, .. } => *case,
            AssessmentTarget::Unscoped | AssessmentTarget::EvaluationSet(_) => {
                return Err(leaven_engine::OptimizerError::Message(
                    "report expected case-targeted assessment".to_owned(),
                ));
            }
        };
        let evidence = store
            .get(assessment_view.evidence_ref())
            .map_err(|source| {
                leaven_engine::OptimizerError::with_source("report evidence lookup failed", source)
            })?;
        candidate = Some(row_candidate);
        request = Some(row_request);
        rows.push((*assessment, report_score(case, &evidence)));
    }
    rows.sort_by_key(|(_, score)| score.case_id);
    let assessments = rows.iter().map(|(assessment, _)| *assessment).collect();
    let cases = rows.into_iter().map(|(_, score)| score).collect::<Vec<_>>();
    Ok(CandidateEvaluationSummary {
        candidate: candidate.ok_or_else(|| {
            leaven_engine::OptimizerError::Message(
                "report expected at least one assessment".to_owned(),
            )
        })?,
        request: request.ok_or_else(|| {
            leaven_engine::OptimizerError::Message(
                "report expected at least one assessment".to_owned(),
            )
        })?,
        assessments,
        average_score: average(&cases),
        cases,
    })
}

fn report_score(case_id: leaven_kernel::CaseId, evidence: &CaseAssessmentEvidence) -> ReportScore {
    ReportScore {
        case_id,
        score: evidence.score().score(),
        feedback: evidence.feedback().to_owned(),
        output: output_record_text(evidence.output()),
    }
}

fn output_record_text(output: &OutputRecord) -> String {
    match output {
        OutputRecord::Inline { text, .. } => text.clone(),
        OutputRecord::BlobRef(reference) => format!("blob:{}:{}", reference.store, reference.key),
    }
}

pub fn run_storage<P>(
    run_id: leaven_kernel::RunId,
    store: &PreparedStore<P>,
    latest_checkpoint: Option<leaven_kernel::CheckpointId>,
) -> RunStorage
where
    P: leaven_core::OptimizationProblem,
{
    if store.store.persistence().is_some() {
        RunStorage::Stored {
            run_id,
            run_dir: store.run_dir.clone(),
            latest_checkpoint,
            resumability: if store.run_dir.is_none() {
                RunResumability::NotResumable {
                    reason: RunNotResumableReason::ExplicitStoreWithoutLocalRunDir,
                }
            } else if latest_checkpoint.is_none() {
                RunResumability::NotResumable {
                    reason: RunNotResumableReason::MissingLatestCheckpoint,
                }
            } else {
                RunResumability::Resumable
            },
        }
    } else {
        RunStorage::Ephemeral { run_id }
    }
}

pub fn report_paths_for(storage: &RunStorage) -> RunReportPaths {
    match storage {
        RunStorage::Stored {
            run_dir: Some(run_dir),
            ..
        } => RunReportPaths {
            summary_json: Some(run_dir.join("reports").join("summary.json")),
        },
        RunStorage::Stored { .. } | RunStorage::Ephemeral { .. } => RunReportPaths::default(),
    }
}

pub fn write_summary_report(summary: &StandardRunSummary) -> Result<(), OptimizeError> {
    let Some(path) = &summary.reports.summary_json else {
        return Ok(());
    };
    let parent = path
        .parent()
        .expect("summary report path has parent directory");
    fs::create_dir_all(parent).map_err(|source| OptimizeError::ReportStore {
        operation: "create report directory",
        source,
    })?;
    let bytes =
        serde_json::to_vec_pretty(summary).expect("standard run summary is JSON-serializable");
    fs::write(path, bytes).map_err(|source| OptimizeError::ReportStore {
        operation: "write summary json",
        source,
    })
}

fn event_summary(event: &leaven_engine::RunEvent) -> RunEventSummary {
    match event {
        leaven_engine::RunEvent::OptimizationStarted { .. } => RunEventSummary::OptimizationStarted,
        leaven_engine::RunEvent::IterationStarted { .. } => RunEventSummary::IterationStarted,
        leaven_engine::RunEvent::BudgetCharged { .. } => RunEventSummary::BudgetCharged,
        leaven_engine::RunEvent::ProposalBatchProduced { .. } => {
            RunEventSummary::ProposalBatchProduced
        }
        leaven_engine::RunEvent::ProposalRecorded { .. } => RunEventSummary::ProposalRecorded,
        leaven_engine::RunEvent::StageAttemptRecorded { .. } => {
            RunEventSummary::StageAttemptRecorded
        }
        leaven_engine::RunEvent::ApplySucceeded { .. } => RunEventSummary::ApplySucceeded,
        leaven_engine::RunEvent::ApplyFailed { .. } => RunEventSummary::ApplyFailed,
        leaven_engine::RunEvent::EvaluationRequested { .. } => RunEventSummary::EvaluationRequested,
        leaven_engine::RunEvent::EvaluationCompleted { .. } => RunEventSummary::EvaluationCompleted,
        leaven_engine::RunEvent::PopulationUpdated { .. } => RunEventSummary::PopulationUpdated,
        leaven_engine::RunEvent::IterationEnded { .. } => RunEventSummary::IterationEnded,
        leaven_engine::RunEvent::OptimizationStopping { .. } => {
            RunEventSummary::OptimizationStopping
        }
        leaven_engine::RunEvent::OptimizationEnded { .. } => RunEventSummary::OptimizationEnded,
        leaven_engine::RunEvent::Error { .. } => RunEventSummary::Error,
    }
}

fn run_cache_summary<'a>(
    events: impl Iterator<Item = &'a leaven_engine::RunEvent>,
    storage: &RunStorage,
) -> RunCacheSummary {
    let durable = matches!(
        storage,
        RunStorage::Stored {
            resumability: RunResumability::Resumable,
            ..
        }
    );
    let backend = if durable {
        EvaluationCacheBackend::SqliteRunStore
    } else {
        EvaluationCacheBackend::InMemory
    };
    let mut evaluation = EvaluationCacheSummary {
        durable,
        backend,
        hits: 0,
        misses: 0,
        bypasses: Vec::new(),
        write_errors: 0,
        hit_cost_zero: true,
    };

    for event in events {
        let leaven_engine::RunEvent::EvaluationCompleted { cache, cost, .. } = event else {
            continue;
        };
        match cache {
            leaven_engine::CacheStatus::Hit => {
                evaluation.hits += 1;
                evaluation.hit_cost_zero &= cost.is_zero();
            }
            leaven_engine::CacheStatus::Miss => {
                evaluation.misses += 1;
            }
            leaven_engine::CacheStatus::Bypassed(reason) => {
                increment_bypass(&mut evaluation.bypasses, cache_bypass_reason(*reason));
            }
        }
    }

    RunCacheSummary { evaluation }
}

fn cache_bypass_reason(reason: leaven_engine::CacheBypassReason) -> EvaluationCacheBypassReason {
    match reason {
        leaven_engine::CacheBypassReason::DisabledByPolicy => {
            EvaluationCacheBypassReason::DisabledByPolicy
        }
        leaven_engine::CacheBypassReason::CacheUnavailable => {
            EvaluationCacheBypassReason::CacheUnavailable
        }
        leaven_engine::CacheBypassReason::MissingCandidateIdentity { .. } => {
            EvaluationCacheBypassReason::MissingCandidateIdentity
        }
    }
}

fn increment_bypass(
    bypasses: &mut Vec<EvaluationCacheBypassSummary>,
    reason: EvaluationCacheBypassReason,
) {
    if let Some(summary) = bypasses.iter_mut().find(|summary| summary.reason == reason) {
        summary.count += 1;
        return;
    }
    bypasses.push(EvaluationCacheBypassSummary { reason, count: 1 });
}

#[cfg(test)]
mod tests {
    use leaven_core::{CausalInputs, ProposalEffectKind};
    use leaven_evidence::ScalarEvidence;
    use leaven_kernel::{
        AssessmentId, BudgetSnapshot, CandidateId, Cost, ErrorKind, ErrorRecord,
        EvaluationRequestId, EvaluatorId, IterationId, PopulationId, ProposalBatchId, ProposalId,
        StageAttemptFailure, StageAttemptOutcome, StageAttemptReceiptId, StageAttemptReceiptRef,
        StageCallId, StageId, StageRole,
    };

    use super::*;

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
    fn report_scores_preserve_inline_and_blob_outputs() {
        let inline = report_score(
            leaven_kernel::CaseId::new(1),
            &CaseAssessmentEvidence::new(
                ScalarEvidence::new(1.0).unwrap(),
                OutputRecord::inline("inline answer"),
                "inline feedback",
            ),
        );
        let blob = report_score(
            leaven_kernel::CaseId::new(2),
            &CaseAssessmentEvidence::new(
                ScalarEvidence::new(0.25).unwrap(),
                OutputRecord::BlobRef(leaven_kernel::BlobRef {
                    store: "blob-store".to_owned(),
                    key: "answer.txt".to_owned(),
                }),
                "blob feedback",
            ),
        );

        assert_eq!(inline.output, "inline answer");
        assert_eq!(inline.feedback, "inline feedback");
        assert_eq!(blob.output, "blob:blob-store:answer.txt");
        assert!((blob.score - 0.25).abs() < f64::EPSILON);
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
}
