//! Pairwise and listwise judging evaluator adapter.

use std::{future::Future, num::NonZeroUsize, sync::Arc};

use futures::{FutureExt, future::BoxFuture, stream, stream::StreamExt, stream::TryStreamExt};
use leaven_core::{
    Assessment, AssessmentGranularity, AssessmentTarget, ResolvedEvaluationRequest,
    ResolvedRequestKind,
};
use leaven_engine::{CachePolicy, EvaluationContext, EvaluationError, Evaluator};
use leaven_eval::{Case, NoTarget};
use leaven_evidence::{
    CandidateAssessmentOutput, CandidateAssessmentOutputError, CaseAssessmentEvidence,
    OutputRecord, ScalarEvidence,
};
use leaven_kernel::{BudgetSnapshot, Cost, EvaluationSetId, EvaluatorId, Fingerprint, Metered};

use super::{MissingReportableOutput, Runner, default_parallelism};
use crate::compatibility::ScoringEvaluatorIdentity;
use crate::evidence::artifact_identity_output;
use crate::evidence::{CaseDataReadLog, ReportableOutputDeclaration, ReportableOutputScope};
use crate::{RunCase, RunOutput, RunProblem, Score, ScoreError};

type JudgeScorer<A, I, T, Out> = Arc<
    dyn Fn(JudgeScoreContext<A, I, T, Out>) -> BoxFuture<'static, Result<Score, ScoreError>>
        + Send
        + Sync,
>;

/// One candidate output visible to a pairwise/listwise judge.
#[derive(Clone, Debug)]
pub struct JudgeCandidateOutput<A, Out> {
    /// Candidate that produced the output.
    pub candidate: leaven_kernel::CandidateId,
    /// Artifact run for the candidate.
    pub artifact: A,
    /// Runner output for the candidate on the judge case.
    pub output: RunOutput<Out>,
}

/// Scoring context for pairwise and listwise judging.
#[derive(Clone, Debug)]
pub struct JudgeScoreContext<A, I, T = NoTarget, Out = ()> {
    /// Evaluation case visible to the judge.
    pub case: crate::ScoreCase<I, T>,
    /// Candidate outputs being judged together, in request order.
    pub outputs: Vec<JudgeCandidateOutput<A, Out>>,
    /// Point-in-time budget snapshot visible to the judge.
    pub budget: BudgetSnapshot,
    output_scope: ReportableOutputScope,
    expected_output: Option<ReportableOutputDeclaration>,
    case_data_reads: CaseDataReadLog,
}

impl<A, I, T, Out> JudgeScoreContext<A, I, T, Out> {
    fn new(
        case: crate::ScoreCase<I, T>,
        outputs: Vec<JudgeCandidateOutput<A, Out>>,
        budget: BudgetSnapshot,
        output_scope: ReportableOutputScope,
        expected_output: Option<ReportableOutputDeclaration>,
        case_data_reads: CaseDataReadLog,
    ) -> Self {
        Self {
            case,
            outputs,
            budget,
            output_scope,
            expected_output,
            case_data_reads,
        }
    }

    /// Loads the optional case target through the judge's audited case-data read path.
    #[must_use]
    pub fn load_target(&self) -> Option<&T> {
        let target = self.case.target_material();
        if target.is_some() {
            self.case_data_reads.record_target_read(self.case.id());
        }
        target
    }

    /// Wraps a judged output record for this exact candidate-group/case context.
    #[must_use]
    pub fn report_output(&self, output: leaven_evidence::OutputRecord) -> crate::ReportableOutput {
        crate::ReportableOutput::new(
            output,
            self.output_scope.clone(),
            self.expected_output.clone(),
        )
    }

    /// Wraps inline judged output text for this exact candidate-group/case context.
    #[must_use]
    pub fn report_text_output(&self, output: impl Into<String>) -> crate::ReportableOutput {
        self.report_output(leaven_evidence::OutputRecord::inline(output))
    }

    /// Reports the judged candidate artifact identities as the assessed output.
    #[must_use]
    pub fn report_artifact_identity_outputs(&self) -> crate::ReportableOutput
    where
        A: leaven_core::Artifact,
    {
        let output = grouped_artifact_identity_output(&self.outputs);
        crate::ReportableOutput::new(
            output.clone(),
            self.output_scope.clone(),
            Some(ReportableOutputDeclaration::derived(output)),
        )
    }
}

/// Evaluator that judges pairwise and listwise candidate outputs.
pub struct JudgingEvaluator<A, I, T = NoTarget, Out = ()> {
    cases: Arc<Vec<Case<I, T>>>,
    runner: Runner<A, I, Out>,
    scorer: JudgeScorer<A, I, T, Out>,
    fingerprint: Fingerprint,
    cache_policy: CachePolicy,
    parallelism: NonZeroUsize,
}

impl<A, I, T, Out> JudgingEvaluator<A, I, T, Out> {
    /// Builds a pairwise/listwise judging evaluator.
    #[must_use]
    pub fn new(
        cases: Arc<Vec<Case<I, T>>>,
        runner: Runner<A, I, Out>,
        scorer: JudgeScorer<A, I, T, Out>,
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

    /// Overrides the maximum number of judge jobs evaluated at once.
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

    /// Returns the evaluator's maximum judging parallelism.
    #[must_use]
    pub const fn parallelism(&self) -> NonZeroUsize {
        self.parallelism
    }
}

impl<A, I, T, Out> Evaluator<RunProblem<A, I, T>> for JudgingEvaluator<A, I, T, Out>
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
                "leaven-run judging evaluator requires per-case granularity".to_owned(),
            ));
        }
        let request_kind = JudgeRequestKind::from_resolved(request.kind)?;
        let candidate_ids = request_kind.candidates();
        if candidate_ids.is_empty() {
            return Err(EvaluationError::Message(
                "leaven-run judging evaluator requires at least one candidate".to_owned(),
            ));
        }
        let case_count = request.set.case_ids.len();
        let mut artifacts = Vec::with_capacity(candidate_ids.len());
        for candidate in &candidate_ids {
            let artifact = ctx.graph().artifact(*candidate).ok_or_else(|| {
                EvaluationError::Message(format!("candidate {candidate} is missing"))
            })?;
            artifacts.push((*candidate, (*artifact).clone()));
        }
        let budget = ctx.budget();
        let set = EvaluationSetId::from_uuid(request.set.id.as_uuid());
        let mut jobs = Vec::with_capacity(case_count);
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
            jobs.push(JudgeEvaluationJob {
                case_index,
                request_kind: request_kind.clone(),
                artifacts: artifacts.clone(),
                case: case.clone(),
                budget: budget.clone(),
            });
        }

        let outcomes =
            evaluate_judge_jobs(jobs, &self.runner, &self.scorer, self.parallelism).await?;
        let mut by_case = (0..case_count).map(|_| None).collect::<Vec<_>>();
        let mut total_cost = Cost::zero();
        for outcome in outcomes {
            let JudgeEvaluationOutcome {
                case_index,
                case_id,
                request_kind,
                evidence,
                cost,
            } = outcome;
            total_cost = total_cost.combine(&cost);
            by_case[case_index] = Some((case_id, request_kind, evidence, cost));
        }

        let mut assessments = Vec::with_capacity(case_count);
        for outcome in by_case {
            let (case, request_kind, evidence, cost) = outcome.ok_or_else(|| {
                EvaluationError::Message(
                    "parallel judging evaluator did not return every case outcome".to_owned(),
                )
            })?;
            let target = AssessmentTarget::Case { set, case };
            match request_kind {
                JudgeRequestKind::Pairwise { left, right } => {
                    assessments.push(Assessment::Pairwise {
                        left,
                        right,
                        target,
                        evidence,
                        cost,
                        metadata: leaven_kernel::MetadataBag::new(),
                    });
                }
                JudgeRequestKind::Listwise { candidates } => {
                    assessments.push(Assessment::Listwise {
                        candidates,
                        target,
                        evidence,
                        cost,
                        metadata: leaven_kernel::MetadataBag::new(),
                    });
                }
            }
        }
        Ok(Metered::new(assessments, total_cost))
    }
}

#[derive(Clone, Debug)]
enum JudgeRequestKind {
    Pairwise {
        left: leaven_kernel::CandidateId,
        right: leaven_kernel::CandidateId,
    },
    Listwise {
        candidates: Vec<leaven_kernel::CandidateId>,
    },
}

impl JudgeRequestKind {
    fn from_resolved(kind: ResolvedRequestKind) -> Result<Self, EvaluationError> {
        match kind {
            ResolvedRequestKind::Pairwise { left, right, .. } => Ok(Self::Pairwise { left, right }),
            ResolvedRequestKind::Listwise { candidates } => Ok(Self::Listwise { candidates }),
            ResolvedRequestKind::Independent { .. } => Err(EvaluationError::Message(
                "leaven-run judging evaluator requires pairwise or listwise requests".to_owned(),
            )),
        }
    }

    fn candidates(&self) -> Vec<leaven_kernel::CandidateId> {
        match self {
            Self::Pairwise { left, right } => vec![*left, *right],
            Self::Listwise { candidates } => candidates.clone(),
        }
    }
}

struct JudgeEvaluationJob<A, I, T> {
    case_index: usize,
    request_kind: JudgeRequestKind,
    artifacts: Vec<(leaven_kernel::CandidateId, A)>,
    case: Case<I, T>,
    budget: BudgetSnapshot,
}

struct JudgeEvaluationOutcome {
    case_index: usize,
    case_id: leaven_kernel::CaseId,
    request_kind: JudgeRequestKind,
    evidence: CaseAssessmentEvidence,
    cost: Cost,
}

fn evaluate_judge_jobs<A, I, T, Out>(
    jobs: Vec<JudgeEvaluationJob<A, I, T>>,
    runner: &Runner<A, I, Out>,
    scorer: &JudgeScorer<A, I, T, Out>,
    parallelism: NonZeroUsize,
) -> impl Future<Output = Result<Vec<JudgeEvaluationOutcome>, EvaluationError>> + Send + 'static
where
    A: leaven_core::Artifact,
    I: Clone + Send + Sync + 'static,
    T: Clone + Send + Sync + 'static,
    Out: Clone + Send + Sync + 'static,
{
    let parallelism = parallelism.get().min(jobs.len().max(1));
    let runner = Arc::clone(runner);
    let scorer = Arc::clone(scorer);
    async move {
        stream::iter(jobs)
            .map(move |job| {
                let runner = Arc::clone(&runner);
                let scorer = Arc::clone(&scorer);
                async move { evaluate_judge_job(job, &runner, &scorer).await }
            })
            .buffer_unordered(parallelism)
            .try_collect::<Vec<_>>()
            .await
    }
    .boxed()
}

async fn evaluate_judge_job<A, I, T, Out>(
    job: JudgeEvaluationJob<A, I, T>,
    runner: &Runner<A, I, Out>,
    scorer: &JudgeScorer<A, I, T, Out>,
) -> Result<JudgeEvaluationOutcome, EvaluationError>
where
    A: leaven_core::Artifact,
    I: Clone + Send + Sync + 'static,
    T: Clone + Send + Sync + 'static,
    Out: Clone + Send + Sync + 'static,
{
    let case_id = job.case.id;
    let mut outputs = Vec::with_capacity(job.artifacts.len());
    let mut runner_cost = Cost::zero();
    let mut trace = Vec::new();
    for (candidate, artifact) in job.artifacts {
        let output = runner(artifact.clone(), RunCase::from_case(&job.case))
            .await
            .map_err(|source| {
                let cost = source.cost().clone();
                EvaluationError::with_cost_source("runner function failed", cost, source)
            })?;
        runner_cost = runner_cost.combine(&output.cost);
        trace.extend(output.trace.iter().cloned());
        outputs.push(JudgeCandidateOutput {
            candidate,
            artifact,
            output,
        });
    }
    let output_scope = ReportableOutputScope::group(job.request_kind.candidates(), case_id);
    let candidate_outputs = assessed_candidate_outputs(&outputs)?;
    let expected_output = assessed_group_output(&outputs);
    let case_data_reads = CaseDataReadLog::default();
    let mut score = scorer(JudgeScoreContext::new(
        crate::ScoreCase::from_case(&job.case),
        outputs,
        job.budget,
        output_scope.clone(),
        expected_output,
        case_data_reads.clone(),
    ))
    .await
    .map_err(|source| {
        let cost = Cost::metric_calls(1)
            .combine(&runner_cost)
            .combine(source.cost());
        EvaluationError::with_cost_source("scoring function failed", cost, source)
    })?;
    let cost = Cost::metric_calls(1)
        .combine(&runner_cost)
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
    trace.extend(score.trace);
    Ok(JudgeEvaluationOutcome {
        case_index: job.case_index,
        case_id,
        request_kind: job.request_kind,
        evidence: CaseAssessmentEvidence::new(scalar, generated_output, score.feedback)
            .with_candidate_outputs(candidate_outputs)
            .with_trace(trace)
            .with_case_data_reads(case_data_reads),
        cost,
    })
}

fn assessed_candidate_outputs<A, Out>(
    outputs: &[JudgeCandidateOutput<A, Out>],
) -> Result<Vec<CandidateAssessmentOutput>, EvaluationError> {
    outputs
        .iter()
        .map(|output| {
            let reportable = output.output.reportable_output().ok_or_else(|| {
                EvaluationError::Message(
                    "runner output did not declare reportable assessed output".to_owned(),
                )
            })?;
            CandidateAssessmentOutput::new(output.candidate, reportable.record().clone()).map_err(
                |error| match error {
                    CandidateAssessmentOutputError::MissingAssessedDataClass => {
                        EvaluationError::Message(
                            "runner output did not declare candidate or artifact assessed output"
                                .to_owned(),
                        )
                    }
                    CandidateAssessmentOutputError::EmptyInlineOutput => {
                        EvaluationError::Message(error.to_string())
                    }
                },
            )
        })
        .collect()
}

fn assessed_group_output<A, Out>(
    outputs: &[JudgeCandidateOutput<A, Out>],
) -> Option<ReportableOutputDeclaration> {
    let mut texts = Vec::with_capacity(outputs.len());
    let mut truncated = false;
    let mut metadata = None;
    let mut unbound_explicit_assessed_output = false;
    for output in outputs {
        let reportable = output.output.reportable_output()?;
        unbound_explicit_assessed_output |= reportable.is_unbound_explicit_candidate_output()
            || reportable.is_unbound_explicit_candidate_artifact();
        let OutputRecord::Inline {
            text,
            truncated: output_truncated,
            metadata: output_metadata,
        } = reportable.record()
        else {
            return None;
        };
        if let Some(metadata) = &metadata {
            if metadata != output_metadata {
                return None;
            }
        } else {
            metadata = Some(output_metadata.clone());
        }
        truncated |= *output_truncated;
        texts.push(text.clone());
    }
    let record = OutputRecord::Inline {
        text: texts.join("|"),
        truncated,
        metadata: metadata.unwrap_or_else(leaven_evidence::OutputMetadata::public),
    };
    if unbound_explicit_assessed_output {
        Some(ReportableOutputDeclaration::explicit(record))
    } else {
        Some(ReportableOutputDeclaration::derived(record))
    }
}

fn grouped_artifact_identity_output<A, Out>(
    outputs: &[JudgeCandidateOutput<A, Out>],
) -> OutputRecord
where
    A: leaven_core::Artifact,
{
    let text = outputs
        .iter()
        .map(|output| match artifact_identity_output(&output.artifact) {
            OutputRecord::Inline { text, .. } => text,
            OutputRecord::BlobRef { .. } => unreachable!("artifact identity output is inline"),
        })
        .collect::<Vec<_>>()
        .join("|");
    OutputRecord::candidate_artifact_inline(text)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use futures::{FutureExt, executor::block_on};
    use leaven_core::{Artifact, ArtifactIdentity};
    use leaven_eval::Case;
    use leaven_evidence::OutputRecord;
    use leaven_kernel::{BudgetSnapshot, CandidateId, CaseId, ContentId, Cost};

    use super::{JudgeEvaluationJob, JudgeRequestKind, JudgeScoreContext, evaluate_judge_job};
    use crate::{RunCase, RunOutput, Score};

    #[test]
    fn judge_job_preserves_group_scoped_reportable_output() {
        block_on(async {
            let left = CandidateId::new();
            let right = CandidateId::new();
            let runner: super::Runner<TestArtifact, i32, String> =
                Arc::new(|artifact: TestArtifact, case: RunCase<i32>| {
                    async move {
                        Ok(RunOutput::new(format!("{}:{}", artifact.0, case.input()))
                            .with_cost(Cost::metric_calls(2))
                            .with_trace(format!("ran {}", artifact.0)))
                    }
                    .boxed()
                });
            let scorer: super::JudgeScorer<TestArtifact, i32, leaven_eval::NoTarget, String> =
                Arc::new(
                    |ctx: JudgeScoreContext<TestArtifact, i32, leaven_eval::NoTarget, String>| {
                        async move {
                            assert_eq!(ctx.outputs.len(), 2);
                            let report = ctx.report_text_output(format!(
                                "{}|{}",
                                ctx.outputs[0].output.output, ctx.outputs[1].output.output
                            ));
                            Ok(Score::new(1.0, "left wins")
                                .with_output(report)
                                .with_trace("judged pair"))
                        }
                        .boxed()
                    },
                );
            let outcome = evaluate_judge_job(
                JudgeEvaluationJob {
                    case_index: 0,
                    request_kind: JudgeRequestKind::Pairwise { left, right },
                    artifacts: vec![(left, TestArtifact(40)), (right, TestArtifact(41))],
                    case: Case::input(CaseId::new(0), 2),
                    budget: BudgetSnapshot::default(),
                },
                &runner,
                &scorer,
            )
            .await
            .unwrap();

            assert_eq!(outcome.case_id, CaseId::new(0));
            assert_eq!(outcome.cost.metric_calls, 5);
            assert_eq!(outcome.evidence.feedback(), "left wins");
            assert_eq!(
                outcome.evidence.trace(),
                &[
                    "ran 40".to_owned(),
                    "ran 41".to_owned(),
                    "judged pair".to_owned()
                ]
            );
            assert_eq!(
                outcome.evidence.output(),
                &candidate_output_record("40:2|41:2")
            );
        });
    }

    fn candidate_output_record(output: impl Into<String>) -> OutputRecord {
        OutputRecord::candidate_inline(output)
    }

    #[derive(Clone, Debug)]
    struct TestArtifact(i32);

    #[derive(Debug)]
    struct TestArtifactError;

    impl std::fmt::Display for TestArtifactError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("test artifact error")
        }
    }

    impl std::error::Error for TestArtifactError {}

    impl Artifact for TestArtifact {
        type Change = i32;
        type ApplyError = TestArtifactError;

        fn identity(&self) -> ArtifactIdentity {
            let mut bytes = [0; ContentId::BYTES];
            bytes[..std::mem::size_of::<i32>()].copy_from_slice(&self.0.to_le_bytes());
            ArtifactIdentity::Content(ContentId::from_bytes(bytes))
        }

        fn apply_change(&self, change: &Self::Change) -> Result<Self, Self::ApplyError> {
            Ok(Self(*change))
        }
    }
}
