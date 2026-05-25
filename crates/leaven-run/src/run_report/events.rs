use leaven_kernel::ErrorKind;

use crate::result::{
    EvaluationCacheBackend, EvaluationCacheBypassReason, EvaluationCacheBypassSummary,
    EvaluationCacheSummary, RunCacheSummary, RunEventSummary, RunResumability, RunStorage,
};

pub(super) fn event_summary(event: &leaven_engine::RunEvent) -> RunEventSummary {
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

pub(super) fn should_include_event_summary(
    event: &leaven_engine::RunEvent,
    stop_reason: leaven_engine::StopReason,
) -> bool {
    !matches!(
        (stop_reason, event),
        (
            leaven_engine::StopReason::BudgetReached,
            leaven_engine::RunEvent::Error {
                error,
                policy: leaven_engine::ErrorPolicy::StoppedRun,
                ..
            }
        ) if error.kind == ErrorKind::Budget
    )
}

pub(super) fn run_cache_summary<'a>(
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
