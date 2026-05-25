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
mod tests;
