//! Optimize builder lowering into the engine.

use std::{
    collections::{BTreeMap, BTreeSet},
    future::Future,
    marker::PhantomData,
    num::NonZeroUsize,
    sync::Arc,
};

use futures::{FutureExt, future::BoxFuture};
use leaven_core::{
    Artifact, AssessmentGranularity, EvaluationPurpose, EvaluationRequest, EvaluationSet,
    OptimizationProblem, PartitionId,
};
use leaven_engine::{Callback, Optimizer, TrustPolicy};
use leaven_eval::{
    CandidateEvaluationSummary, Dataset, DatasetSplits, EvaluationReport, ReportScore, SplitPolicy,
    SplitReport, SplitRole,
};
use leaven_evidence::{CaseAssessmentEvidence, CasewiseEvidence, OutputRecord};
use leaven_kernel::{Budget, BudgetSnapshot, CandidateId, CaseId, Cost, EvaluatorId};
use leaven_store::EvidenceStore;

use crate::{
    IntoOptimizeStore, OptimizeError, OptimizeStore, RunOutput, Score, ScoreContext, ScoreError,
    evaluator::{ScoringEvaluator, default_parallelism},
    result::{OptimizationReport, OptimizeResult, RunStorage, average},
};

type Runner<A, C> = Arc<dyn Fn(A, C) -> BoxFuture<'static, RunOutput> + Send + Sync>;
type Scorer<A, C> =
    Arc<dyn Fn(ScoreContext<A, C>) -> BoxFuture<'static, Result<Score, ScoreError>> + Send + Sync>;

struct FinalEvaluations {
    baseline_train: Option<CandidateEvaluationSummary>,
    train: Option<CandidateEvaluationSummary>,
    baseline_validation: Option<CandidateEvaluationSummary>,
    validation: Option<CandidateEvaluationSummary>,
    baseline_test: Option<CandidateEvaluationSummary>,
    test: Option<CandidateEvaluationSummary>,
    cost: Cost,
}

struct FinalEvaluationInputs {
    seed: CandidateId,
    best: CandidateId,
    has_train: bool,
    has_validation: bool,
    has_test: bool,
}

struct FinalPartitionEvaluation {
    partition: PartitionId,
    purpose: EvaluationPurpose,
}

struct FinalPartitionResults {
    baseline: CandidateEvaluationSummary,
    optimized: CandidateEvaluationSummary,
    cost: Cost,
}

struct ReportInputs<'a, C> {
    dataset: &'a Dataset<C>,
    splits: &'a DatasetSplits,
    best: CandidateId,
    final_evaluations: &'a FinalEvaluations,
    optimization_budget: BudgetSnapshot,
    stop_reason: leaven_engine::StopReason,
    storage: RunStorage,
}

/// Problem type used by the public run builder.
pub struct RunProblem<A, C> {
    _marker: PhantomData<(A, C)>,
}

impl<A, C> OptimizationProblem for RunProblem<A, C>
where
    A: Artifact,
    C: Send + Sync + 'static,
{
    type Artifact = A;
    type Case = C;
    type Evidence = CasewiseEvidence<CaseAssessmentEvidence>;
    type ProposalAnnotations = ();
}

/// Starts optimizing a seed artifact through the high-level product API.
#[must_use]
pub fn optimize<A>(seed: A) -> OptimizeBuilder<A, (), ()>
where
    A: Artifact,
{
    OptimizeBuilder {
        seed,
        train: Vec::new(),
        validation: Vec::new(),
        test: Vec::new(),
        runner: Arc::new(|_artifact, _case| async { RunOutput::default() }.boxed()),
        scorer: None,
        optimizer: (),
        budget: None,
        evaluation_parallelism: default_parallelism(),
        callbacks: Vec::new(),
        store: None,
    }
}

/// Public optimize/train/validation/test builder.
pub struct OptimizeBuilder<A, C, O>
where
    A: Artifact,
    C: Send + Sync + 'static,
{
    seed: A,
    train: Vec<C>,
    validation: Vec<C>,
    test: Vec<C>,
    runner: Runner<A, C>,
    scorer: Option<Scorer<A, C>>,
    optimizer: O,
    budget: Option<Budget>,
    evaluation_parallelism: NonZeroUsize,
    callbacks: Vec<Box<dyn Callback<RunProblem<A, C>>>>,
    store: Option<OptimizeStore<RunProblem<A, C>>>,
}

impl<A> OptimizeBuilder<A, (), ()>
where
    A: Artifact,
{
    /// Supplies training cases and fixes the run case type.
    #[must_use]
    pub fn train<C>(self, train: Vec<C>) -> OptimizeBuilder<A, C, ()>
    where
        C: Clone + Send + Sync + 'static,
    {
        OptimizeBuilder {
            seed: self.seed,
            train,
            validation: Vec::new(),
            test: Vec::new(),
            runner: Arc::new(|_artifact, _case| async { RunOutput::default() }.boxed()),
            scorer: None,
            optimizer: (),
            budget: self.budget,
            evaluation_parallelism: self.evaluation_parallelism,
            callbacks: Vec::new(),
            store: None,
        }
    }
}

impl<A, C, O> OptimizeBuilder<A, C, O>
where
    A: Artifact,
    C: Clone + Send + Sync + 'static,
{
    /// Supplies validation/dev cases.
    #[must_use]
    pub fn validation(mut self, validation: Vec<C>) -> Self {
        self.validation = validation;
        self
    }

    /// Supplies held-out final test cases.
    #[must_use]
    pub fn test(mut self, test: Vec<C>) -> Self {
        self.test = test;
        self
    }

    /// Supplies the runner/executor.
    #[must_use]
    pub fn runner<F, Fut>(mut self, runner: F) -> Self
    where
        F: Fn(A, C) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = RunOutput> + Send + 'static,
    {
        self.runner = Arc::new(move |artifact, case| runner(artifact, case).boxed());
        self
    }

    /// Supplies the async scoring function.
    #[must_use]
    pub fn score<F, Fut>(mut self, scorer: F) -> Self
    where
        F: Fn(ScoreContext<A, C>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Score, ScoreError>> + Send + 'static,
    {
        self.scorer = Some(Arc::new(move |ctx| scorer(ctx).boxed()));
        self
    }

    /// Supplies the optimizer.
    #[must_use]
    pub fn using<Next>(self, optimizer: Next) -> OptimizeBuilder<A, C, Next> {
        OptimizeBuilder {
            seed: self.seed,
            train: self.train,
            validation: self.validation,
            test: self.test,
            runner: self.runner,
            scorer: self.scorer,
            optimizer,
            budget: self.budget,
            evaluation_parallelism: self.evaluation_parallelism,
            callbacks: self.callbacks,
            store: self.store,
        }
    }

    /// Sets the budget.
    #[must_use]
    pub fn budget(mut self, budget: Budget) -> Self {
        self.budget = Some(budget);
        self
    }

    /// Sets the maximum number of runner/scorer jobs evaluated at once.
    #[must_use]
    pub const fn evaluation_parallelism(mut self, parallelism: NonZeroUsize) -> Self {
        self.evaluation_parallelism = parallelism;
        self
    }

    /// Registers a callback for public run events.
    #[must_use]
    pub fn on_event<Cb>(mut self, callback: Cb) -> Self
    where
        Cb: Callback<RunProblem<A, C>> + 'static,
    {
        self.callbacks.push(Box::new(callback));
        self
    }

    /// Supplies evidence and optional checkpoint persistence for the run.
    #[must_use]
    pub fn store<S>(mut self, store: S) -> Self
    where
        S: IntoOptimizeStore<RunProblem<A, C>>,
    {
        self.store = Some(store.into_optimize_store());
        self
    }
}

impl<A, C, O> OptimizeBuilder<A, C, O>
where
    A: Artifact,
    C: Clone + Send + Sync + 'static,
    O: Optimizer<RunProblem<A, C>>,
{
    /// Runs the optimization.
    pub async fn run(mut self) -> Result<OptimizeResult<A>, OptimizeError> {
        let scorer = self.scorer.take().ok_or(OptimizeError::MissingScore)?;
        let budget = self.budget.take().ok_or(OptimizeError::MissingBudget)?;
        let metric_call_limit = budget.metric_calls;
        if self.train.is_empty() && (!self.validation.is_empty() || !self.test.is_empty()) {
            return Err(OptimizeError::HeldOutWithoutTrain);
        }
        let all_cases = all_cases(&self.train, &self.validation, &self.test);
        let dataset = Dataset::from_ordered(all_cases.clone());
        let splits = dataset_splits(self.train.len(), self.validation.len(), self.test.len());
        let case_set = case_set(
            all_cases,
            self.train.len(),
            self.validation.len(),
            self.test.len(),
        );
        let store = self
            .store
            .take()
            .unwrap_or_else(|| OptimizeStore::inline("leaven-run"));
        let has_persistence = store.persistence().is_some();
        let evaluator = ScoringEvaluator::new(
            Arc::new(case_set_cases(&self.train, &self.validation, &self.test)),
            self.runner.clone(),
            scorer,
            "leaven-run/score",
        )
        .with_parallelism(self.evaluation_parallelism);
        let mut engine_builder =
            leaven_engine::Engine::<RunProblem<A, C>>::builder()
                .budget(budget)
                .trust_policy(TrustPolicy::default().hide_from_proposers([
                    PartitionId::from("VALIDATION"),
                    PartitionId::from("TEST"),
                ]))
                .evaluator(evaluator);
        if let Some(limit) = metric_call_limit {
            engine_builder = engine_builder.metric_call_budget_stopper(limit);
        }
        if let Some(persistence) = store.persistence() {
            engine_builder = engine_builder.persistence(persistence);
        }
        for callback in self.callbacks {
            engine_builder = engine_builder.callback(callback);
        }
        let mut engine = engine_builder.build();
        let seed = engine
            .insert_seed(self.seed.clone(), 0)
            .map_err(|source| OptimizeError::SeedInsertion { source })?;
        let run = engine
            .run(&mut self.optimizer, &case_set, store.evidence_store())
            .await?;
        let optimization_budget = engine.budget().snapshot();
        let stop_reason = stop_reason_from_events(&engine.view())?;
        let storage = run_storage(run.run_id, has_persistence);
        let best = run.best.ok_or_else(|| {
            leaven_engine::OptimizerError::Message(
                "optimizer finished without a best candidate".to_owned(),
            )
        })?;
        let has_train = !self.train.is_empty();
        let has_validation = !self.validation.is_empty();
        let has_test = !self.test.is_empty();
        if has_train || has_validation || has_test {
            engine.set_budget_limit(Budget::unlimited());
        }

        let final_evaluations = run_final_evaluations(
            &mut engine,
            &case_set,
            store.evidence_store(),
            FinalEvaluationInputs {
                seed,
                best,
                has_train,
                has_validation,
                has_test,
            },
        )
        .await?;
        let (best_artifact, report) = build_report(
            &engine,
            store.evidence_store(),
            ReportInputs {
                dataset: &dataset,
                splits: &splits,
                best,
                final_evaluations: &final_evaluations,
                optimization_budget,
                stop_reason,
                storage,
            },
        )?;
        Ok(OptimizeResult {
            run_id: run.run_id,
            best,
            best_artifact,
            seed_artifact: self.seed,
            report,
        })
    }
}

fn all_cases<C: Clone>(train: &[C], validation: &[C], test: &[C]) -> Vec<C> {
    train
        .iter()
        .chain(validation)
        .chain(test)
        .cloned()
        .collect()
}

fn case_set_cases<C: Clone>(train: &[C], validation: &[C], test: &[C]) -> Vec<C> {
    all_cases(train, validation, test)
}

fn case_set<C: Clone>(
    cases: Vec<C>,
    train: usize,
    validation: usize,
    test: usize,
) -> leaven_engine::CaseSet<C> {
    let train_ids = (0..train).map(CaseId::from_index).collect::<Vec<_>>();
    let validation_ids = (train..train + validation)
        .map(CaseId::from_index)
        .collect::<Vec<_>>();
    let test_ids = (train + validation..train + validation + test)
        .map(CaseId::from_index)
        .collect::<Vec<_>>();
    leaven_engine::CaseSet::new(cases)
        .with_partition(PartitionId::from("TRAIN"), train_ids)
        .with_partition(PartitionId::from("VALIDATION"), validation_ids)
        .with_partition(PartitionId::from("TEST"), test_ids)
}

fn dataset_splits(train: usize, validation: usize, test: usize) -> DatasetSplits {
    let known = (0..train + validation + test)
        .map(CaseId::from_index)
        .collect::<BTreeSet<_>>();
    let roles = BTreeMap::from([
        (PartitionId::from("TRAIN"), SplitRole::Train),
        (PartitionId::from("VALIDATION"), SplitRole::Validation),
        (PartitionId::from("TEST"), SplitRole::Test),
    ]);
    let cases = BTreeMap::from([
        (
            PartitionId::from("TRAIN"),
            (0..train).map(CaseId::from_index).collect(),
        ),
        (
            PartitionId::from("VALIDATION"),
            (train..train + validation)
                .map(CaseId::from_index)
                .collect(),
        ),
        (
            PartitionId::from("TEST"),
            (train + validation..train + validation + test)
                .map(CaseId::from_index)
                .collect(),
        ),
    ]);
    DatasetSplits::new(
        leaven_core::CaseSetVersion("0".to_owned()),
        roles,
        cases,
        &known,
        SplitPolicy::DisjointRequired,
    )
    .expect("builder constructs disjoint split ids")
}

async fn run_final_evaluations<A, C>(
    engine: &mut leaven_engine::Engine<RunProblem<A, C>>,
    case_set: &leaven_engine::CaseSet<C>,
    store: &dyn EvidenceStore<CasewiseEvidence<CaseAssessmentEvidence>>,
    inputs: FinalEvaluationInputs,
) -> Result<FinalEvaluations, leaven_engine::OptimizerError>
where
    A: Artifact,
    C: Clone + Send + Sync + 'static,
{
    let mut cost = Cost::zero();
    let train = if inputs.has_train {
        let results = final_eval_partition(
            engine,
            case_set,
            store,
            &inputs,
            FinalPartitionEvaluation {
                partition: PartitionId::from("TRAIN"),
                purpose: EvaluationPurpose::Custom("final-train-report".into()),
            },
        )
        .await?;
        cost = cost.combine(&results.cost);
        Some((results.baseline, results.optimized))
    } else {
        None
    };
    let validation = if inputs.has_validation {
        let results = final_eval_partition(
            engine,
            case_set,
            store,
            &inputs,
            FinalPartitionEvaluation {
                partition: PartitionId::from("VALIDATION"),
                purpose: EvaluationPurpose::Validation,
            },
        )
        .await?;
        cost = cost.combine(&results.cost);
        Some((results.baseline, results.optimized))
    } else {
        None
    };
    let test = if inputs.has_test {
        let results = final_eval_partition(
            engine,
            case_set,
            store,
            &inputs,
            FinalPartitionEvaluation {
                partition: PartitionId::from("TEST"),
                purpose: EvaluationPurpose::FinalTest,
            },
        )
        .await?;
        cost = cost.combine(&results.cost);
        Some((results.baseline, results.optimized))
    } else {
        None
    };
    Ok(FinalEvaluations {
        baseline_train: train.as_ref().map(|(baseline, _)| baseline.clone()),
        train: train.map(|(_, optimized)| optimized),
        baseline_validation: validation.as_ref().map(|(baseline, _)| baseline.clone()),
        validation: validation.map(|(_, optimized)| optimized),
        baseline_test: test.as_ref().map(|(baseline, _)| baseline.clone()),
        test: test.map(|(_, optimized)| optimized),
        cost,
    })
}

async fn final_eval_partition<A, C>(
    engine: &mut leaven_engine::Engine<RunProblem<A, C>>,
    case_set: &leaven_engine::CaseSet<C>,
    store: &dyn EvidenceStore<CasewiseEvidence<CaseAssessmentEvidence>>,
    inputs: &FinalEvaluationInputs,
    evaluation: FinalPartitionEvaluation,
) -> Result<FinalPartitionResults, leaven_engine::OptimizerError>
where
    A: Artifact,
    C: Clone + Send + Sync + 'static,
{
    let (baseline, baseline_cost) = final_eval(
        engine,
        case_set,
        store,
        inputs.seed,
        evaluation.partition.clone(),
        evaluation.purpose.clone(),
    )
    .await?;
    let (optimized, optimized_cost) = final_eval(
        engine,
        case_set,
        store,
        inputs.best,
        evaluation.partition,
        evaluation.purpose,
    )
    .await?;
    Ok(FinalPartitionResults {
        baseline,
        optimized,
        cost: baseline_cost.combine(&optimized_cost),
    })
}

fn build_report<A, C>(
    engine: &leaven_engine::Engine<RunProblem<A, C>>,
    store: &dyn EvidenceStore<CasewiseEvidence<CaseAssessmentEvidence>>,
    inputs: ReportInputs<'_, C>,
) -> Result<(A, OptimizationReport), leaven_engine::OptimizerError>
where
    A: Artifact,
    C: Clone + Send + Sync + 'static,
{
    let view = engine.view();
    let best_artifact = view.artifact(inputs.best).expect("best exists").clone();
    let budget = engine.budget().snapshot();
    let cost = budget.spent.clone();
    let report = OptimizationReport {
        stop_reason: inputs.stop_reason.into(),
        storage: inputs.storage,
        optimization_budget: inputs.optimization_budget.clone(),
        budget,
        optimization_cost: inputs.optimization_budget.spent,
        final_report_cost: inputs.final_evaluations.cost.clone(),
        cost: cost.clone(),
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
        events: view.events().map(event_name).collect(),
    };
    Ok((best_artifact, report))
}

async fn final_eval<A, C>(
    engine: &mut leaven_engine::Engine<RunProblem<A, C>>,
    case_set: &leaven_engine::CaseSet<C>,
    store: &dyn EvidenceStore<CasewiseEvidence<CaseAssessmentEvidence>>,
    candidate: leaven_kernel::CandidateId,
    partition: PartitionId,
    purpose: EvaluationPurpose,
) -> Result<(CandidateEvaluationSummary, Cost), leaven_engine::OptimizerError>
where
    A: Artifact,
    C: Clone + Send + Sync + 'static,
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
        assessment_summary(&view, store, report.assessment_ids[0])?,
        report.cost,
    ))
}

fn split_reports_for<A, C>(
    view: &leaven_engine::RunGraphView<'_, RunProblem<A, C>>,
    store: &dyn EvidenceStore<CasewiseEvidence<CaseAssessmentEvidence>>,
    splits: &DatasetSplits,
) -> Result<Vec<SplitReport>, leaven_engine::OptimizerError>
where
    A: Artifact,
    C: Clone + Send + Sync + 'static,
{
    let mut reports = BTreeMap::<PartitionId, SplitReport>::new();
    for assessment in view.all_assessments() {
        let Some((partition, role)) = assessment_split(view, assessment.id()) else {
            continue;
        };
        if splits.role(&partition).is_none() {
            continue;
        }
        let summary = assessment_summary(view, store, assessment.id())?;
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

fn assessment_split<A, C>(
    view: &leaven_engine::RunGraphView<'_, RunProblem<A, C>>,
    assessment: leaven_kernel::AssessmentId,
) -> Option<(PartitionId, SplitRole)>
where
    A: Artifact,
    C: Clone + Send + Sync + 'static,
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

fn assessment_summary<A, C>(
    view: &leaven_engine::RunGraphView<'_, RunProblem<A, C>>,
    store: &dyn EvidenceStore<CasewiseEvidence<CaseAssessmentEvidence>>,
    assessment: leaven_kernel::AssessmentId,
) -> Result<CandidateEvaluationSummary, leaven_engine::OptimizerError>
where
    A: Artifact,
    C: Clone + Send + Sync + 'static,
{
    let assessment_view = view.assessment(assessment).ok_or_else(|| {
        leaven_engine::OptimizerError::Message("assessment missing from graph".to_owned())
    })?;
    let candidate = assessment_view.independent_candidate().ok_or_else(|| {
        leaven_engine::OptimizerError::Message("report expected independent assessment".to_owned())
    })?;
    let evidence = store
        .get(assessment_view.evidence_ref())
        .map_err(|source| {
            leaven_engine::OptimizerError::with_source("report evidence lookup failed", source)
        })?;
    let cases = report_scores(&evidence);
    Ok(CandidateEvaluationSummary {
        candidate,
        request: assessment_view.request_id(),
        assessment,
        average_score: average(&cases),
        cases,
    })
}

fn report_scores(evidence: &CasewiseEvidence<CaseAssessmentEvidence>) -> Vec<ReportScore> {
    evidence
        .outcomes()
        .iter()
        .map(|outcome| ReportScore {
            case_id: outcome.case(),
            score: outcome.evidence().score().score(),
            feedback: outcome.evidence().feedback().to_owned(),
            output: output_record_text(outcome.evidence().output()),
        })
        .collect()
}

fn output_record_text(output: &OutputRecord) -> String {
    match output {
        OutputRecord::Inline { text, .. } => text.clone(),
        OutputRecord::BlobRef(reference) => format!("blob:{}:{}", reference.store, reference.key),
    }
}

fn stop_reason_from_events<A, C>(
    view: &leaven_engine::RunGraphView<'_, RunProblem<A, C>>,
) -> Result<leaven_engine::StopReason, leaven_engine::OptimizerError>
where
    A: Artifact,
    C: Clone + Send + Sync + 'static,
{
    let mut stop_reason = None;
    for event in view.events() {
        if let leaven_engine::RunEvent::OptimizationStopping { reason } = event {
            stop_reason = Some(reason);
        }
    }
    stop_reason.copied().ok_or_else(|| {
        leaven_engine::OptimizerError::Message(
            "optimizer finished without a stop reason".to_owned(),
        )
    })
}

fn run_storage(run_id: leaven_kernel::RunId, has_persistence: bool) -> RunStorage {
    if has_persistence {
        RunStorage::Stored {
            run_id,
            resumable: false,
        }
    } else {
        RunStorage::Ephemeral { run_id }
    }
}

fn event_name(event: &leaven_engine::RunEvent) -> String {
    match event {
        leaven_engine::RunEvent::OptimizationStarted { .. } => "optimization_started",
        leaven_engine::RunEvent::IterationStarted { .. } => "iteration_started",
        leaven_engine::RunEvent::BudgetCharged { .. } => "budget_charged",
        leaven_engine::RunEvent::ProposalBatchProduced { .. } => "proposal_batch_produced",
        leaven_engine::RunEvent::ProposalRecorded { .. } => "proposal_recorded",
        leaven_engine::RunEvent::StageAttemptRecorded { .. } => "stage_attempt_recorded",
        leaven_engine::RunEvent::ApplySucceeded { .. } => "apply_succeeded",
        leaven_engine::RunEvent::ApplyFailed { .. } => "apply_failed",
        leaven_engine::RunEvent::EvaluationRequested { .. } => "evaluation_requested",
        leaven_engine::RunEvent::EvaluationCompleted { .. } => "evaluation_completed",
        leaven_engine::RunEvent::PopulationUpdated { .. } => "population_updated",
        leaven_engine::RunEvent::IterationEnded { .. } => "iteration_ended",
        leaven_engine::RunEvent::OptimizationStopping { .. } => "optimization_stopping",
        leaven_engine::RunEvent::OptimizationEnded { .. } => "optimization_ended",
        leaven_engine::RunEvent::Error { .. } => "error",
    }
    .to_owned()
}
