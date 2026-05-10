//! Optimize builder lowering into the engine.

use std::{
    collections::{BTreeMap, BTreeSet},
    marker::PhantomData,
    sync::Arc,
};

use leaven_core::{
    Artifact, AssessmentGranularity, EvaluationPurpose, EvaluationRequest, EvaluationSet,
    OptimizationProblem, PartitionId,
};
use leaven_engine::{Callback, Optimizer, TrustPolicy};
use leaven_eval::{
    CandidateEvaluationSummary, Dataset, DatasetSplits, EvaluationReport, ReportScore, SplitPolicy,
    SplitReport, SplitRole,
};
use leaven_evidence::{CasewiseEvidence, ScoredFeedbackEvidence};
use leaven_kernel::{Budget, CaseId, EvaluatorId};
use leaven_store::EvidenceStore;
use leaven_store_inline::InlineEvidenceStore;

use crate::{
    RunOutput, Score, ScoreContext,
    evaluator::ScoringEvaluator,
    result::{OptimizationReport, OptimizeResult, average},
};

type Runner<A, C> = Arc<dyn Fn(&A, &C) -> RunOutput + Send + Sync>;
type Scorer<A, C> = Arc<dyn for<'a> Fn(ScoreContext<'a, A, C>) -> Score + Send + Sync>;

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
    type Evidence = CasewiseEvidence<ScoredFeedbackEvidence>;
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
        runner: Arc::new(|_artifact, _case| RunOutput::default()),
        scorer: None,
        optimizer: (),
        budget: Budget::unlimited(),
        callbacks: Vec::new(),
    }
}

/// Public optimize/train/validation/test builder.
pub struct OptimizeBuilder<A, C, O> {
    seed: A,
    train: Vec<C>,
    validation: Vec<C>,
    test: Vec<C>,
    runner: Runner<A, C>,
    scorer: Option<Scorer<A, C>>,
    optimizer: O,
    budget: Budget,
    callbacks: Vec<Box<dyn Callback<RunProblem<A, C>>>>,
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
            runner: Arc::new(|_artifact, _case| RunOutput::default()),
            scorer: None,
            optimizer: (),
            budget: self.budget,
            callbacks: Vec::new(),
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
    pub fn runner<F>(mut self, runner: F) -> Self
    where
        F: Fn(&A, &C) -> RunOutput + Send + Sync + 'static,
    {
        self.runner = Arc::new(runner);
        self
    }

    /// Supplies the scoring function.
    #[must_use]
    pub fn score<F>(mut self, scorer: F) -> Self
    where
        F: for<'a> Fn(ScoreContext<'a, A, C>) -> Score + Send + Sync + 'static,
    {
        self.scorer = Some(Arc::new(scorer));
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
            callbacks: self.callbacks,
        }
    }

    /// Sets the budget.
    #[must_use]
    pub fn budget(mut self, budget: Budget) -> Self {
        self.budget = budget;
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
    pub async fn run(mut self) -> Result<OptimizeResult<A>, leaven_engine::OptimizerError> {
        let scorer = self.scorer.take().ok_or_else(|| {
            leaven_engine::OptimizerError::Message("score function is required".to_owned())
        })?;
        let all_cases = all_cases(&self.train, &self.validation, &self.test);
        let dataset = Dataset::from_ordered(all_cases.clone());
        let splits = dataset_splits(self.train.len(), self.validation.len(), self.test.len());
        let case_set = case_set(
            all_cases,
            self.train.len(),
            self.validation.len(),
            self.test.len(),
        );
        let store =
            InlineEvidenceStore::<CasewiseEvidence<ScoredFeedbackEvidence>>::new("leaven-run");
        let evaluator = ScoringEvaluator::new(
            Arc::new(case_set_cases(&self.train, &self.validation, &self.test)),
            self.runner.clone(),
            scorer,
            "leaven-run/score",
        );
        let mut engine =
            leaven_engine::Engine::<RunProblem<A, C>>::builder()
                .budget(self.budget)
                .trust_policy(TrustPolicy::default().hide_from_proposers([
                    PartitionId::from("VALIDATION"),
                    PartitionId::from("TEST"),
                ]))
                .evaluator(evaluator)
                .build();
        let seed = engine.insert_seed(self.seed.clone(), 0).map_err(|source| {
            leaven_engine::OptimizerError::with_source("seed insertion failed", source)
        })?;
        let run = engine.run(&mut self.optimizer, &case_set, &store).await?;
        let best = run.best.ok_or_else(|| {
            leaven_engine::OptimizerError::Message(
                "optimizer finished without a best candidate".to_owned(),
            )
        })?;

        let baseline_validation_score = if self.validation.is_empty() {
            None
        } else {
            Some(
                final_eval(
                    &mut engine,
                    &case_set,
                    &store,
                    seed,
                    PartitionId::from("VALIDATION"),
                    EvaluationPurpose::Validation,
                )
                .await?,
            )
        };
        let validation_score = if self.validation.is_empty() {
            None
        } else {
            Some(
                final_eval(
                    &mut engine,
                    &case_set,
                    &store,
                    best,
                    PartitionId::from("VALIDATION"),
                    EvaluationPurpose::Validation,
                )
                .await?,
            )
        };
        let baseline_test_score = if self.test.is_empty() {
            None
        } else {
            Some(
                final_eval(
                    &mut engine,
                    &case_set,
                    &store,
                    seed,
                    PartitionId::from("TEST"),
                    EvaluationPurpose::FinalTest,
                )
                .await?,
            )
        };
        let test_score = if self.test.is_empty() {
            None
        } else {
            Some(
                final_eval(
                    &mut engine,
                    &case_set,
                    &store,
                    best,
                    PartitionId::from("TEST"),
                    EvaluationPurpose::FinalTest,
                )
                .await?,
            )
        };

        let view = engine.view();
        let train_partition = PartitionId::from("TRAIN");
        let baseline_train =
            latest_average_for_partition(&view, &store, seed, &train_partition)?.unwrap_or(0.0);
        let optimized_train =
            latest_average_for_partition(&view, &store, best, &train_partition)?.unwrap_or(0.0);
        let events = view.events().map(event_name).collect::<Vec<_>>();
        let best_artifact = view.artifact(best).expect("best exists").clone();
        let split_reports = split_reports_for(&view, &store, &splits)?;
        let cost = engine.budget().snapshot().spent;
        let report = OptimizationReport {
            dataset: dataset.fingerprint(),
            splits: splits.fingerprint(),
            budget: engine.budget().snapshot(),
            cost: cost.clone(),
            baseline_train_score: baseline_train,
            optimized_train_score: optimized_train,
            baseline_validation_score: baseline_validation_score
                .as_ref()
                .map(|summary| summary.average_score),
            validation_score: validation_score
                .as_ref()
                .map(|summary| summary.average_score),
            baseline_test_score: baseline_test_score
                .as_ref()
                .map(|summary| summary.average_score),
            test_score: test_score.as_ref().map(|summary| summary.average_score),
            evaluation: EvaluationReport {
                dataset: dataset.fingerprint(),
                splits: splits.fingerprint(),
                cost,
                splits_reported: split_reports,
            },
            events,
        };
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

async fn final_eval<A, C>(
    engine: &mut leaven_engine::Engine<RunProblem<A, C>>,
    case_set: &leaven_engine::CaseSet<C>,
    store: &InlineEvidenceStore<CasewiseEvidence<ScoredFeedbackEvidence>>,
    candidate: leaven_kernel::CandidateId,
    partition: PartitionId,
    purpose: EvaluationPurpose,
) -> Result<CandidateEvaluationSummary, leaven_engine::OptimizerError>
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
    assessment_summary(&view, store, report.assessment_ids[0])
}

fn latest_average_for_partition<A, C>(
    view: &leaven_engine::RunGraphView<'_, RunProblem<A, C>>,
    store: &InlineEvidenceStore<CasewiseEvidence<ScoredFeedbackEvidence>>,
    candidate: leaven_kernel::CandidateId,
    partition: &PartitionId,
) -> Result<Option<f64>, leaven_engine::OptimizerError>
where
    A: Artifact,
    C: Clone + Send + Sync + 'static,
{
    let mut latest = None;
    for assessment in view.assessments(candidate).iter() {
        let Some((assessment_partition, _role)) = assessment_split(view, assessment.id()) else {
            continue;
        };
        if &assessment_partition != partition {
            continue;
        }
        latest = Some(assessment_summary(view, store, assessment.id())?.average_score);
    }
    Ok(latest)
}

fn split_reports_for<A, C>(
    view: &leaven_engine::RunGraphView<'_, RunProblem<A, C>>,
    store: &InlineEvidenceStore<CasewiseEvidence<ScoredFeedbackEvidence>>,
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
    store: &InlineEvidenceStore<CasewiseEvidence<ScoredFeedbackEvidence>>,
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

fn report_scores(evidence: &CasewiseEvidence<ScoredFeedbackEvidence>) -> Vec<ReportScore> {
    evidence
        .outcomes()
        .iter()
        .map(|outcome| ReportScore {
            score: outcome.evidence().score().score(),
            feedback: outcome.evidence().feedback().to_owned(),
            trace: outcome.evidence().trace().to_vec(),
        })
        .collect()
}

fn event_name(event: &leaven_engine::RunEvent) -> String {
    match event {
        leaven_engine::RunEvent::OptimizationStarted { .. } => "optimization_started",
        leaven_engine::RunEvent::IterationStarted { .. } => "iteration_started",
        leaven_engine::RunEvent::BudgetCharged { .. } => "budget_charged",
        leaven_engine::RunEvent::ProposalBatchProduced { .. } => "proposal_batch_produced",
        leaven_engine::RunEvent::ProposalRecorded { .. } => "proposal_recorded",
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
