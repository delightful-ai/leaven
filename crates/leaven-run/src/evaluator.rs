//! Scoring evaluator adapter.

use std::sync::Arc;

use leaven_core::{
    Assessment, AssessmentGranularity, AssessmentTarget, ResolvedEvaluationRequest,
    ResolvedRequestKind,
};
use leaven_engine::{CachePolicy, EvaluationContext, EvaluationError, Evaluator};
use leaven_evidence::{CaseOutcome, CasewiseEvidence, ScalarEvidence, ScoredFeedbackEvidence};
use leaven_kernel::{Cost, EvaluationSetId, EvaluatorId, Fingerprint, FingerprintBuilder, Metered};

use crate::{RunOutput, RunProblem, Score, ScoreContext};

type Runner<A, C> = Arc<dyn Fn(&A, &C) -> RunOutput + Send + Sync>;
type Scorer<A, C> = Arc<dyn for<'a> Fn(ScoreContext<'a, A, C>) -> Score + Send + Sync>;

/// Evaluator that runs a candidate on cases and normalizes scores into
/// casewise feedback evidence.
pub struct ScoringEvaluator<A, C> {
    cases: Arc<Vec<C>>,
    runner: Runner<A, C>,
    scorer: Scorer<A, C>,
    fingerprint: Fingerprint,
}

impl<A, C> ScoringEvaluator<A, C> {
    /// Builds a scoring evaluator.
    #[must_use]
    pub fn new(
        cases: Arc<Vec<C>>,
        runner: Runner<A, C>,
        scorer: Scorer<A, C>,
        label: &str,
    ) -> Self {
        let mut fingerprint = FingerprintBuilder::new();
        fingerprint.update(label.as_bytes());
        fingerprint.update(cases.len().to_le_bytes());
        Self {
            cases,
            runner,
            scorer,
            fingerprint: fingerprint.finish(),
        }
    }
}

impl<A, C> Evaluator<RunProblem<A, C>> for ScoringEvaluator<A, C>
where
    A: leaven_core::Artifact,
    C: Clone + Send + Sync + 'static,
{
    fn id(&self) -> EvaluatorId {
        EvaluatorId::PRIMARY
    }

    fn fingerprint(&self) -> Fingerprint {
        self.fingerprint
    }

    fn cache_policy(&self, _request: &ResolvedEvaluationRequest) -> CachePolicy {
        CachePolicy::Never
    }

    async fn evaluate(
        &self,
        request: ResolvedEvaluationRequest,
        ctx: EvaluationContext<'_, RunProblem<A, C>>,
    ) -> Result<Metered<Vec<Assessment<RunProblem<A, C>>>>, EvaluationError> {
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
        let mut assessments = Vec::new();
        let mut metric_calls = 0_u64;
        for candidate in candidates {
            let artifact = ctx.graph().artifact(candidate).ok_or_else(|| {
                EvaluationError::Message(format!("candidate {candidate} is missing"))
            })?;
            let mut outcomes = Vec::new();
            for case_id in &request.set.case_ids {
                let index = usize::try_from(case_id.0).map_err(|_| {
                    EvaluationError::Message(format!("case id {case_id} does not fit usize"))
                })?;
                let case = self.cases.get(index).ok_or_else(|| {
                    EvaluationError::Message(format!(
                        "case {case_id} is missing from evaluator cases"
                    ))
                })?;
                let output = (self.runner)(artifact, case);
                let score = (self.scorer)(ScoreContext {
                    artifact,
                    case,
                    output: &output,
                });
                let scalar = ScalarEvidence::new(score.value).map_err(|source| {
                    EvaluationError::with_source("score was not finite", source)
                })?;
                let mut trace = output.trace.clone();
                trace.extend(
                    score
                        .structured
                        .iter()
                        .map(|(key, value)| format!("{key}: {value}")),
                );
                outcomes.push(CaseOutcome::new(
                    *case_id,
                    ScoredFeedbackEvidence::new(scalar, score.feedback, trace),
                ));
                metric_calls += 1;
            }
            assessments.push(Assessment::Independent {
                candidate,
                target: AssessmentTarget::EvaluationSet(EvaluationSetId::new()),
                evidence: CasewiseEvidence::new(outcomes),
                cost: Cost::metric_calls(metric_calls),
                metadata: leaven_kernel::MetadataBag::new(),
            });
        }
        Ok(Metered::new(assessments, Cost::metric_calls(metric_calls)))
    }
}
