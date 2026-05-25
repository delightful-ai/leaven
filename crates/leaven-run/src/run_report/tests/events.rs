use super::*;
use crate::{
    EvaluationCacheBackend, EvaluationCacheBypassReason, EvaluationCacheBypassSummary,
    RunResumability,
};

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
