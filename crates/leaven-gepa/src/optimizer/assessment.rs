use leaven_core::{
    AssessmentGranularity, AssessmentTarget, EvaluationPurpose, EvaluationRequest, EvaluationSet,
    OptimizationProblem,
};
use leaven_engine::{OptimizerError, RunContext};
use leaven_evidence::{CaseOutcome, CasewiseEvidence, ScalarEvidence};
use leaven_kernel::{AssessmentId, CandidateId, CaseId, EvaluatorId};

use crate::{GepaCaseEvidence, GepaEventSummary, ValidationPolicy};

use super::{Gepa, GepaCandidateHistoryEntry, GepaValidationBest};

impl<S, Pop, Reflect, CandidateSel, PartSel, GatePol, Batch, Validate, Dataset>
    Gepa<S, Pop, Reflect, CandidateSel, PartSel, GatePol, Batch, Validate, Dataset>
{
    pub(super) async fn validate_candidate<P>(
        &mut self,
        ctx: &mut RunContext<'_, P>,
        candidate: CandidateId,
        parents: Vec<crate::GepaCandidateIndex>,
        seed_validation: bool,
    ) -> Result<Option<crate::GepaCandidateIndex>, OptimizerError>
    where
        P: OptimizationProblem,
        P::Evidence: GepaCaseEvidence,
        Validate: ValidationPolicy + Sync,
        S: Sync,
        Pop: Sync,
        Reflect: Sync,
        CandidateSel: Sync,
        PartSel: Sync,
        GatePol: Sync,
        Batch: Sync,
        Dataset: Sync,
    {
        let Some(set) = self.validation_policy.validation_set(candidate) else {
            return Ok(None);
        };
        let assessment = self
            .evaluate_casewise(ctx, candidate, set, EvaluationPurpose::Validation)
            .await?;
        self.reference_state
            .add_metric_calls(assessment.metric_calls_new);
        self.reference_state.note_full_validation();
        let index = self.reference_state.add_validated_candidate(
            candidate,
            parents,
            self.reference_state.total_metric_calls(),
            assessment.average_score,
            assessment.assessments.clone(),
            &assessment.scalar_evidence,
        );
        if self
            .validation_best
            .as_ref()
            .is_none_or(|best| assessment.average_score > best.score)
        {
            self.validation_best = Some(GepaValidationBest {
                candidate,
                assessments: assessment.assessments.clone(),
                score: assessment.average_score,
            });
        }
        if seed_validation {
            self.record_event(GepaEventSummary::SeedValidationCompleted {
                candidate_index: index,
                metric_calls_delta: assessment.metric_calls_new,
                score: assessment.average_score.to_string(),
            });
        } else {
            self.record_event(GepaEventSummary::AcceptedValidationCompleted {
                candidate_index: index,
                metric_calls_delta: assessment.metric_calls_new,
                score: assessment.average_score.to_string(),
            });
            self.record_event(GepaEventSummary::CandidateAdmitted {
                candidate,
                candidate_index: index,
            });
        }
        self.record_event(GepaEventSummary::FrontierUpdated);
        Ok(Some(index))
    }

    pub(super) async fn evaluate_casewise<P>(
        &self,
        ctx: &mut RunContext<'_, P>,
        candidate: CandidateId,
        set: EvaluationSet,
        purpose: EvaluationPurpose,
    ) -> Result<GepaAssessment, OptimizerError>
    where
        P: OptimizationProblem,
        P::Evidence: GepaCaseEvidence,
        S: Sync,
        Pop: Sync,
        Reflect: Sync,
        CandidateSel: Sync,
        PartSel: Sync,
        GatePol: Sync,
        Batch: Sync,
        Validate: Sync,
        Dataset: Sync,
    {
        let expected_cases = Self::ensure_non_empty_casewise_set(ctx, candidate, &set, &purpose)?;
        let report = ctx
            .evaluate_independent_casewise_cached(
                EvaluatorId::PRIMARY,
                candidate,
                set,
                purpose.clone(),
            )
            .await
            .map_err(|source| OptimizerError::with_source("GEPA evaluation failed", source))?;
        let metric_calls_new = report.cost.metric_calls;
        let assessments = report.assessment_ids;
        if assessments.is_empty() {
            return Err(OptimizerError::Message(format!(
                "GEPA {purpose:?} expected at least one case assessment row"
            )));
        }
        let mut row_cases = Vec::with_capacity(assessments.len());
        for assessment in &assessments {
            let assessment_view = ctx.graph().assessment(*assessment).ok_or_else(|| {
                OptimizerError::Message(format!(
                    "GEPA assessment row `{assessment}` is missing from graph"
                ))
            })?;
            let row_candidate = assessment_view.independent_candidate().ok_or_else(|| {
                OptimizerError::Message("GEPA expected independent assessment rows".to_owned())
            })?;
            if row_candidate != candidate {
                return Err(OptimizerError::Message(
                    "GEPA evaluation returned a row for the wrong candidate".to_owned(),
                ));
            }
            let case = match assessment_view.target() {
                AssessmentTarget::Case { case, .. } => *case,
                AssessmentTarget::Unscoped | AssessmentTarget::EvaluationSet(_) => {
                    return Err(OptimizerError::Message(
                        "GEPA expected case-targeted assessment rows".to_owned(),
                    ));
                }
            };
            row_cases.push(case);
        }
        Self::ensure_exact_case_rows(&expected_cases, &row_cases)?;

        let mut outcomes = Vec::with_capacity(assessments.len());
        let mut row_scores = Vec::with_capacity(assessments.len());
        for (assessment, case) in assessments.iter().zip(&row_cases) {
            let evidence = ctx.assessment_evidence(*assessment).map_err(|source| {
                OptimizerError::with_source("GEPA evidence lookup failed", source)
            })?;
            let score = evidence.scalar_score().ok_or_else(|| {
                OptimizerError::Message("GEPA expected comparable case scores".to_owned())
            })?;
            row_scores.push(score.score());
            outcomes.push(CaseOutcome::new(*case, score));
        }
        // Average over assessment rows *with multiplicity*. Epoch-shuffled
        // minibatches intentionally pad with duplicate case ids; upstream GEPA
        // acceptance sums/averages that padded list. CasewiseEvidence
        // canonicalizes duplicates (last wins), so scoring must not go through
        // the deduped container.
        let average_score = average_row_scores(&row_scores).ok_or_else(|| {
            OptimizerError::Message("GEPA expected comparable case scores".to_owned())
        })?;
        let scalar_evidence = CasewiseEvidence::new(outcomes);
        Ok(GepaAssessment {
            assessments,
            scalar_evidence,
            average_score,
            row_cases,
            row_scores,
            metric_calls_new,
        })
    }

    fn ensure_non_empty_casewise_set<P>(
        ctx: &RunContext<'_, P>,
        candidate: CandidateId,
        set: &EvaluationSet,
        purpose: &EvaluationPurpose,
    ) -> Result<Vec<CaseId>, OptimizerError>
    where
        P: OptimizationProblem,
    {
        let request = EvaluationRequest::Independent {
            candidates: vec![candidate],
            set: set.clone(),
            granularity: AssessmentGranularity::PerCase,
            purpose: purpose.clone(),
        };
        let resolved = match ctx.resolve_evaluation_request(&request) {
            Ok(resolved) => resolved,
            Err(source) if matches!(purpose, EvaluationPurpose::Validation) => {
                return Err(OptimizerError::with_source(
                    reference_validation_required_message(),
                    source,
                ));
            }
            Err(source) => {
                return Err(OptimizerError::with_source(
                    "GEPA could not resolve casewise evaluation set",
                    source,
                ));
            }
        };
        if !resolved.case_ids.is_empty() {
            return Ok(resolved.case_ids);
        }
        let reason = match purpose {
            EvaluationPurpose::Validation => reference_validation_required_message(),
            _ => "GEPA casewise evaluation requires at least one visible case",
        };
        Err(OptimizerError::Message(reason.to_owned()))
    }

    fn ensure_exact_case_rows(
        expected_cases: &[CaseId],
        row_cases: &[CaseId],
    ) -> Result<(), OptimizerError> {
        let mut expected = expected_cases.to_vec();
        let mut returned = row_cases.to_vec();
        expected.sort_unstable();
        returned.sort_unstable();
        if expected == returned {
            return Ok(());
        }
        Err(OptimizerError::Message(
            "GEPA evaluation returned case rows that do not match requested set".to_owned(),
        ))
    }
}

fn reference_validation_required_message() -> &'static str {
    "GEPA reference profile requires a non-empty validation set; supply `.validation(...)` or choose an explicit non-reference fallback profile"
}

pub(super) struct GepaAssessment {
    pub(super) assessments: Vec<AssessmentId>,
    pub(super) scalar_evidence: CasewiseEvidence<ScalarEvidence>,
    pub(super) average_score: f64,
    pub(super) row_cases: Vec<CaseId>,
    pub(super) row_scores: Vec<f64>,
    pub(super) metric_calls_new: u64,
}

impl GepaAssessment {
    pub(super) fn history_entry(&self, candidate: CandidateId) -> GepaCandidateHistoryEntry {
        GepaCandidateHistoryEntry {
            candidate,
            assessments: self.assessments.clone(),
            score: self.average_score,
        }
    }

    pub(super) fn all_scores_at_least(&self, threshold: f64) -> bool {
        !self.row_scores.is_empty() && self.row_scores.iter().all(|score| *score >= threshold)
    }

    pub(super) fn cases(&self) -> Vec<CaseId> {
        self.row_cases.clone()
    }
}

fn average_row_scores(scores: &[f64]) -> Option<f64> {
    if scores.is_empty() {
        return None;
    }
    let total: f64 = scores.iter().sum();
    let count = u32::try_from(scores.len()).expect("case count fits into u32");
    Some(total / f64::from(count))
}

#[cfg(test)]
mod tests {
    use leaven_evidence::{CaseOutcome, CasewiseEvidence, ScalarEvidence};
    use leaven_kernel::{AssessmentId, CandidateId, CaseId};

    use super::{Gepa, GepaAssessment, average_row_scores};
    use crate::GepaCandidateHistoryEntry;

    #[test]
    fn assessment_rejects_case_rows_that_do_not_exactly_match_request() {
        let expected = [CaseId::new(0), CaseId::new(1)];

        Gepa::<(), (), ()>::ensure_exact_case_rows(&expected, &[CaseId::new(1), CaseId::new(0)])
            .unwrap();
        let err = Gepa::<(), (), ()>::ensure_exact_case_rows(
            &expected,
            &[CaseId::new(0), CaseId::new(0)],
        )
        .unwrap_err();

        assert!(
            err.to_string()
                .contains("case rows that do not match requested set")
        );
    }

    #[test]
    fn assessment_helpers_preserve_casewise_average_and_history_rows() {
        let evidence = CasewiseEvidence::new(vec![
            CaseOutcome::new(CaseId::new(0), ScalarEvidence::new(0.25).unwrap()),
            CaseOutcome::new(CaseId::new(1), ScalarEvidence::new(0.75).unwrap()),
        ]);
        assert_eq!(average_row_scores(&[0.25, 0.75]), Some(0.5));
        assert_eq!(average_row_scores(&[]), None);
        assert!(
            (average_row_scores(&[1.0, 0.0, 1.0]).expect("padded average") - (2.0 / 3.0)).abs()
                < f64::EPSILON,
            "padded minibatch duplicates must keep multiplicity in the screening average"
        );
        let deduped = CasewiseEvidence::new(vec![
            CaseOutcome::new(CaseId::new(0), ScalarEvidence::new(1.0).unwrap()),
            CaseOutcome::new(CaseId::new(1), ScalarEvidence::new(0.0).unwrap()),
            CaseOutcome::new(CaseId::new(0), ScalarEvidence::new(1.0).unwrap()),
        ]);
        assert_eq!(
            deduped.outcomes().len(),
            2,
            "CasewiseEvidence still canonicalizes duplicates for sparse frontier observation"
        );

        let candidate = CandidateId::new();
        let rows = vec![AssessmentId::new(), AssessmentId::new()];
        let assessment = GepaAssessment {
            assessments: rows.clone(),
            scalar_evidence: evidence,
            average_score: 0.5,
            row_cases: vec![CaseId::new(0), CaseId::new(1)],
            row_scores: vec![0.25, 0.75],
            metric_calls_new: 2,
        };
        let entry: GepaCandidateHistoryEntry = assessment.history_entry(candidate);

        assert_eq!(entry.candidate(), candidate);
        assert_eq!(entry.assessments(), rows.as_slice());
        assert!((entry.score() - 0.5).abs() < f64::EPSILON);
        assert_eq!(assessment.metric_calls_new, 2);
        assert_eq!(assessment.scalar_evidence.outcomes().len(), 2);
        assert!(assessment.all_scores_at_least(0.25));
        assert!(!assessment.all_scores_at_least(0.5));
    }
}
