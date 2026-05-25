//! Scoring evaluator adapter.

use std::{future::Future, num::NonZeroUsize, sync::Arc};

use futures::{FutureExt, future::BoxFuture, stream, stream::StreamExt, stream::TryStreamExt};
use leaven_core::{
    Assessment, AssessmentGranularity, AssessmentTarget, ResolvedEvaluationRequest,
    ResolvedRequestKind,
};
use leaven_engine::{CachePolicy, EvaluationContext, EvaluationError, Evaluator};
use leaven_eval::{Case, NoTarget};
use leaven_evidence::{CaseAssessmentEvidence, ScalarEvidence};
use leaven_kernel::{BudgetSnapshot, Cost, EvaluationSetId, EvaluatorId, Fingerprint, Metered};

use crate::compatibility::ScoringEvaluatorIdentity;
use crate::evidence::{CaseDataReadLog, ReportableOutputScope};
use crate::{RunCase, RunError, RunOutput, RunProblem, Score, ScoreContext, ScoreError};

mod judging;

pub use judging::{JudgeCandidateOutput, JudgeScoreContext, JudgingEvaluator};

type Runner<A, I, Out> = Arc<
    dyn Fn(A, RunCase<I>) -> BoxFuture<'static, Result<RunOutput<Out>, RunError>> + Send + Sync,
>;
type Scorer<A, I, T, Out> = Arc<
    dyn Fn(ScoreContext<A, I, T, Out>) -> BoxFuture<'static, Result<Score, ScoreError>>
        + Send
        + Sync,
>;
/// Evaluator that runs a candidate on cases and emits one feedback row per case.
pub struct ScoringEvaluator<A, I, T = NoTarget, Out = ()> {
    cases: Arc<Vec<Case<I, T>>>,
    runner: Runner<A, I, Out>,
    scorer: Scorer<A, I, T, Out>,
    fingerprint: Fingerprint,
    cache_policy: CachePolicy,
    parallelism: NonZeroUsize,
}

impl<A, I, T, Out> ScoringEvaluator<A, I, T, Out> {
    /// Builds a scoring evaluator.
    #[must_use]
    pub fn new(
        cases: Arc<Vec<Case<I, T>>>,
        runner: Runner<A, I, Out>,
        scorer: Scorer<A, I, T, Out>,
        identity: &ScoringEvaluatorIdentity,
    ) -> Self {
        Self {
            cases,
            runner,
            scorer,
            fingerprint: identity.fingerprint(),
            cache_policy: identity.cache_policy.clone(),
            parallelism: default_parallelism(),
        }
    }

    /// Overrides the maximum number of runner/scorer jobs evaluated at once.
    #[must_use]
    pub const fn with_parallelism(mut self, parallelism: NonZeroUsize) -> Self {
        self.parallelism = parallelism;
        self
    }

    /// Overrides the evaluation cache policy declared to the engine.
    #[must_use]
    pub fn with_cache_policy(mut self, cache_policy: CachePolicy) -> Self {
        self.cache_policy = cache_policy;
        self
    }

    /// Returns the evaluator's maximum runner/scorer parallelism.
    #[must_use]
    pub const fn parallelism(&self) -> NonZeroUsize {
        self.parallelism
    }
}

impl<A, I, T, Out> Evaluator<RunProblem<A, I, T>> for ScoringEvaluator<A, I, T, Out>
where
    A: leaven_core::Artifact,
    I: Clone + Send + Sync + 'static,
    T: Clone + Send + Sync + 'static,
    Out: Clone + Send + Sync + 'static,
{
    fn id(&self) -> EvaluatorId {
        EvaluatorId::PRIMARY
    }

    fn fingerprint(&self) -> Fingerprint {
        self.fingerprint
    }

    fn cache_policy(&self, _request: &ResolvedEvaluationRequest) -> CachePolicy {
        self.cache_policy.clone()
    }

    async fn evaluate(
        &self,
        request: ResolvedEvaluationRequest,
        ctx: EvaluationContext<'_, RunProblem<A, I, T>>,
    ) -> Result<Metered<Vec<Assessment<RunProblem<A, I, T>>>>, EvaluationError> {
        if request.granularity != AssessmentGranularity::PerCase {
            return Err(EvaluationError::Message(
                "leaven-run scoring evaluator requires per-case granularity".to_owned(),
            ));
        }
        let ResolvedRequestKind::Independent { candidates } = request.kind else {
            return Err(EvaluationError::Message(
                "leaven-run scoring evaluator requires independent requests".to_owned(),
            ));
        };
        let case_count = request.set.case_ids.len();
        let mut jobs = Vec::with_capacity(candidates.len().saturating_mul(case_count));
        let budget = ctx.budget();
        let set = EvaluationSetId::from_uuid(request.set.id.as_uuid());
        for (candidate_index, candidate) in candidates.iter().copied().enumerate() {
            let artifact = ctx.graph().artifact(candidate).ok_or_else(|| {
                EvaluationError::Message(format!("candidate {candidate} is missing"))
            })?;
            for (case_index, case_id) in request.set.case_ids.iter().copied().enumerate() {
                let case = self
                    .cases
                    .iter()
                    .find(|case| case.id == case_id)
                    .ok_or_else(|| {
                        EvaluationError::Message(format!(
                            "case {case_id} is missing from evaluator cases"
                        ))
                    })?;
                jobs.push(EvaluationJob {
                    candidate_index,
                    case_index,
                    candidate,
                    artifact: artifact.clone(),
                    case: case.clone(),
                    budget: budget.clone(),
                });
            }
        }

        let outcomes = evaluate_jobs(jobs, &self.runner, &self.scorer, self.parallelism).await?;
        let mut by_candidate = (0..candidates.len())
            .map(|_| (0..case_count).map(|_| None).collect::<Vec<_>>())
            .collect::<Vec<_>>();
        let mut total_cost = Cost::zero();
        for outcome in outcomes {
            let EvaluationOutcome {
                candidate_index,
                case_index,
                case_id,
                evidence,
                cost,
            } = outcome;
            total_cost = total_cost.combine(&cost);
            by_candidate[candidate_index][case_index] = Some((case_id, evidence, cost));
        }

        let mut assessments = Vec::with_capacity(candidates.len().saturating_mul(case_count));
        for (candidate, outcomes) in candidates.into_iter().zip(by_candidate) {
            for outcome in outcomes {
                let (case, evidence, cost) = outcome.ok_or_else(|| {
                    EvaluationError::Message(
                        "parallel evaluator did not return every case outcome".to_owned(),
                    )
                })?;
                assessments.push(Assessment::Independent {
                    candidate,
                    target: AssessmentTarget::Case { set, case },
                    evidence,
                    cost,
                    metadata: leaven_kernel::MetadataBag::new(),
                });
            }
        }
        Ok(Metered::new(assessments, total_cost))
    }
}

struct EvaluationJob<A, I, T> {
    candidate_index: usize,
    case_index: usize,
    candidate: leaven_kernel::CandidateId,
    artifact: A,
    case: Case<I, T>,
    budget: BudgetSnapshot,
}

struct EvaluationOutcome {
    candidate_index: usize,
    case_index: usize,
    case_id: leaven_kernel::CaseId,
    evidence: CaseAssessmentEvidence,
    cost: Cost,
}

fn evaluate_jobs<A, I, T, Out>(
    jobs: Vec<EvaluationJob<A, I, T>>,
    runner: &Runner<A, I, Out>,
    scorer: &Scorer<A, I, T, Out>,
    parallelism: NonZeroUsize,
) -> impl Future<Output = Result<Vec<EvaluationOutcome>, EvaluationError>> + Send + 'static
where
    A: leaven_core::Artifact,
    I: Clone + Send + Sync + 'static,
    T: Clone + Send + Sync + 'static,
    Out: Clone + Send + Sync + 'static,
{
    let runner = Arc::clone(runner);
    let scorer = Arc::clone(scorer);
    evaluate_unordered_jobs(jobs, parallelism, move |job| {
        let runner = Arc::clone(&runner);
        let scorer = Arc::clone(&scorer);
        async move { evaluate_job(job, &runner, &scorer).await }
    })
}

async fn evaluate_job<A, I, T, Out>(
    job: EvaluationJob<A, I, T>,
    runner: &Runner<A, I, Out>,
    scorer: &Scorer<A, I, T, Out>,
) -> Result<EvaluationOutcome, EvaluationError>
where
    A: leaven_core::Artifact,
    I: Clone + Send + Sync + 'static,
    T: Clone + Send + Sync + 'static,
    Out: Clone + Send + Sync + 'static,
{
    let run_case = RunCase::from_case(&job.case);
    let score_case = crate::ScoreCase::from_case(&job.case);
    let case_id = job.case.id;
    let output = runner(job.artifact.clone(), run_case)
        .await
        .map_err(|source| {
            let cost = source.cost().clone();
            EvaluationError::with_cost_source("runner function failed", cost, source)
        })?;
    let output_scope = ReportableOutputScope::new(job.candidate, case_id);
    let case_data_reads = CaseDataReadLog::default();
    let mut score = scorer(ScoreContext::new(
        job.artifact.clone(),
        score_case,
        output.clone(),
        job.budget.clone(),
        output_scope.clone(),
        case_data_reads.clone(),
    ))
    .await
    .map_err(|source| {
        let cost = Cost::metric_calls(1)
            .combine(&output.cost)
            .combine(source.cost());
        EvaluationError::with_cost_source("scoring function failed", cost, source)
    })?;
    let cost = Cost::metric_calls(1)
        .combine(&output.cost)
        .combine(&score.cost);
    let scalar = ScalarEvidence::new(score.value).map_err(|source| {
        EvaluationError::with_cost_source("score was not finite", cost.clone(), source)
    })?;
    let generated_output = score.output.take().ok_or_else(|| {
        EvaluationError::with_cost_source(
            "score did not provide reportable output",
            cost.clone(),
            MissingReportableOutput,
        )
    })?;
    let generated_output = generated_output
        .into_record(&output_scope)
        .map_err(|source| {
            let message = source.to_string();
            EvaluationError::with_cost_source(message, cost.clone(), source)
        })?;
    let case_data_reads = case_data_reads.snapshot();
    let trace = output
        .trace
        .into_iter()
        .chain(score.trace)
        .collect::<Vec<_>>();
    Ok(EvaluationOutcome {
        candidate_index: job.candidate_index,
        case_index: job.case_index,
        case_id,
        evidence: CaseAssessmentEvidence::new(scalar, generated_output, score.feedback)
            .with_trace(trace)
            .with_case_data_reads(case_data_reads),
        cost,
    })
}

#[derive(Debug, thiserror::Error)]
#[error(
    "scorer returned `Score` without supplying reportable output; \
     every successful score must call `Score::with_output(...)` with a \
     `ScoreContext`-minted reportable output so reports, evidence stores, and \
     GEPA reflection see a durable rendering of the runner's typed output"
)]
struct MissingReportableOutput;

fn evaluate_unordered_jobs<Job, Outcome, Fut, Evaluate>(
    jobs: Vec<Job>,
    parallelism: NonZeroUsize,
    evaluate: Evaluate,
) -> impl Future<Output = Result<Vec<Outcome>, EvaluationError>> + Send + 'static
where
    Job: Send + 'static,
    Outcome: Send + 'static,
    Fut: Future<Output = Result<Outcome, EvaluationError>> + Send + 'static,
    Evaluate: Fn(Job) -> Fut + Send + Sync + 'static,
{
    let parallelism = parallelism.get().min(jobs.len().max(1));
    async move {
        stream::iter(jobs)
            .map(evaluate)
            .buffer_unordered(parallelism)
            .try_collect::<Vec<_>>()
            .await
    }
    .boxed()
}

pub fn default_parallelism() -> NonZeroUsize {
    std::thread::available_parallelism()
        .unwrap_or_else(|_| NonZeroUsize::new(32).expect("32 is non-zero"))
}
