use leaven_core::{
    Artifact, AssessmentGranularity, AssessmentTarget, EvaluationPurpose, EvaluationRequest,
    EvaluationSet, PartitionId,
};
use leaven_eval::{CandidateEvaluationSummary, ReportScore};
use leaven_evidence::CaseAssessmentEvidence;
use leaven_kernel::{AssessmentId, Cost, EvaluatorId};
use leaven_store::EvidenceStore;

use crate::RunProblem;

pub async fn final_eval<A, I, T>(
    engine: &mut leaven_engine::Engine<RunProblem<A, I, T>>,
    case_set: &leaven_engine::CaseSet<leaven_eval::Case<I, T>>,
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

pub(super) fn assessment_summary<A, I, T>(
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
        rows.push((
            *assessment,
            report_score(case, assessment_view.evidence_ref().clone(), &evidence),
        ));
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
        average_score: crate::result::average(&cases),
        cases,
    })
}

pub(super) fn report_score(
    case_id: leaven_kernel::CaseId,
    evidence_ref: leaven_kernel::EvidenceRef,
    evidence: &CaseAssessmentEvidence,
) -> ReportScore {
    ReportScore {
        case_id,
        score: evidence.score().score(),
        output_ref: Some(evidence_ref.clone()),
        feedback_ref: Some(evidence_ref.clone()),
        trace_refs: if evidence.trace().is_empty() {
            Vec::new()
        } else {
            vec![evidence_ref]
        },
        feedback: evidence.feedback().to_owned(),
        output: evidence.output().report_text(),
    }
}
