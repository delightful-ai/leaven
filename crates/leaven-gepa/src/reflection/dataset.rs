use std::future::Future;

use leaven_core::{AssessmentTarget, Evidence, InfoRef, OptimizationProblem};
use leaven_engine::RunContext;
use leaven_evidence::{CaseAssessmentEvidence, ScalarEvidence};
use leaven_kernel::{AssessmentId, CandidateId, CaseId};
use leaven_surface::EditSurface;

use super::identity::refresh_default_run_id;
use super::{ReflectionError, ReflectiveCase, ReflectiveValue};

/// Builds the reflective examples for one parent candidate and selected part.
///
/// This is the swappable "what data does reflection see" seam. The default
/// implementation is a GEPA-parity projection: one example per evaluated case,
/// carrying the case input, generated output, score, and feedback.
///
/// The builder receives `&mut RunContext` so a custom builder can reach run
/// history, sibling candidates, or diffs, not only the latest evidence.
#[allow(async_fn_in_trait)]
pub trait ReflectiveDatasetBuilder<P, S>: Send + Sync
where
    P: OptimizationProblem,
    S: EditSurface<P::Artifact>,
{
    /// Build the reflective cases for `parent` on `part`.
    async fn build(
        &self,
        ctx: &mut RunContext<'_, P>,
        parent: CandidateId,
        parent_assessments: &[AssessmentId],
        part: &S::PartId,
    ) -> Result<Vec<ReflectiveCase>, ReflectionError>;
}

impl<P, S, F, Fut> ReflectiveDatasetBuilder<P, S> for F
where
    P: OptimizationProblem,
    S: EditSurface<P::Artifact>,
    F: Fn(&mut RunContext<'_, P>, CandidateId, &[AssessmentId], &S::PartId) -> Fut + Send + Sync,
    Fut: Future<Output = Result<Vec<ReflectiveCase>, ReflectionError>> + Send,
{
    async fn build(
        &self,
        ctx: &mut RunContext<'_, P>,
        parent: CandidateId,
        parent_assessments: &[AssessmentId],
        part: &S::PartId,
    ) -> Result<Vec<ReflectiveCase>, ReflectionError> {
        self(ctx, parent, parent_assessments, part).await
    }
}

/// Target-safe case input projection for GEPA's default reflective dataset.
///
/// This trait is deliberately narrower than [`std::fmt::Display`]. Implement it
/// only for the runner-visible case input that reflection is allowed to see. If
/// a case envelope also stores targets or metadata, use
/// [`GepaReflectiveDataset::with_case_input`] or implement this trait by
/// projecting only the allowed input field.
pub trait ReflectiveCaseInput {
    /// Render the runner-visible case input for reflection.
    fn reflective_input(&self) -> String;
}

impl ReflectiveCaseInput for () {
    fn reflective_input(&self) -> String {
        String::new()
    }
}

impl ReflectiveCaseInput for str {
    fn reflective_input(&self) -> String {
        self.to_owned()
    }
}

impl ReflectiveCaseInput for &str {
    fn reflective_input(&self) -> String {
        (*self).to_owned()
    }
}

impl ReflectiveCaseInput for String {
    fn reflective_input(&self) -> String {
        self.clone()
    }
}

/// GEPA-parity reflective-dataset builder.
///
/// Projects one [`ReflectiveCase`] per evaluated case from the parent's
/// assessment evidence: case input (read from the installed case set via
/// [`RunContext::case`]), generated output, score, and feedback. Assessment
/// provenance is attached to every example.
///
/// The default builder requires [`ReflectiveCaseInput`] rather than
/// [`std::fmt::Display`] so target-bearing case envelopes do not accidentally
/// teach reflection hidden answers or metadata.
#[derive(Clone, Copy, Debug, Default)]
pub struct GepaReflectiveDataset;

impl GepaReflectiveDataset {
    /// Use an explicit target-safe projection while preserving GEPA's default
    /// evidence-to-example projection.
    #[must_use]
    pub fn with_case_input<F>(project_case_input: F) -> CaseInputProjectedDataset<F> {
        CaseInputProjectedDataset { project_case_input }
    }
}

/// GEPA-parity reflective dataset with caller-supplied case input projection.
#[derive(Clone, Copy, Debug)]
pub struct CaseInputProjectedDataset<F> {
    project_case_input: F,
}

impl<P, S> ReflectiveDatasetBuilder<P, S> for GepaReflectiveDataset
where
    P: OptimizationProblem,
    P::Case: ReflectiveCaseInput,
    P::Evidence: ReflectionProjection,
    S: EditSurface<P::Artifact>,
{
    async fn build(
        &self,
        ctx: &mut RunContext<'_, P>,
        _parent: CandidateId,
        parent_assessments: &[AssessmentId],
        _part: &S::PartId,
    ) -> Result<Vec<ReflectiveCase>, ReflectionError> {
        build_gepa_reflective_cases(
            ctx,
            parent_assessments,
            ReflectiveCaseInput::reflective_input,
        )
    }
}

impl<P, S, F> ReflectiveDatasetBuilder<P, S> for CaseInputProjectedDataset<F>
where
    P: OptimizationProblem,
    P::Evidence: ReflectionProjection,
    S: EditSurface<P::Artifact>,
    F: Fn(&P::Case) -> String + Send + Sync,
{
    async fn build(
        &self,
        ctx: &mut RunContext<'_, P>,
        _parent: CandidateId,
        parent_assessments: &[AssessmentId],
        _part: &S::PartId,
    ) -> Result<Vec<ReflectiveCase>, ReflectionError> {
        build_gepa_reflective_cases(ctx, parent_assessments, &self.project_case_input)
    }
}

/// Projects an evidence value into GEPA reflective cases.
///
/// This is a crate-private projection seam: it converts the concrete
/// `P::Evidence` shape into per-case records without an `input`, which the
/// default builder fills from the case set.
trait ReflectionProjection: Evidence {
    /// Project one case record, leaving `input` empty.
    fn reflection_case(&self, case: CaseId) -> ReflectiveCase;
}

fn build_gepa_reflective_cases<P>(
    ctx: &RunContext<'_, P>,
    parent_assessments: &[AssessmentId],
    project_case_input: impl Fn(&P::Case) -> String,
) -> Result<Vec<ReflectiveCase>, ReflectionError>
where
    P: OptimizationProblem,
    P::Evidence: ReflectionProjection,
{
    let mut cases = Vec::with_capacity(parent_assessments.len());
    for parent_assessment in parent_assessments {
        let assessment = ctx.graph().assessment(*parent_assessment).ok_or_else(|| {
            ReflectionError::builder(format!(
                "parent assessment row `{parent_assessment}` is missing from graph"
            ))
        })?;
        let case = match assessment.target() {
            AssessmentTarget::Case { case, .. } => *case,
            AssessmentTarget::Unscoped | AssessmentTarget::EvaluationSet(_) => {
                return Err(ReflectionError::builder(
                    "GEPA reflective dataset expected case-targeted assessment rows",
                ));
            }
        };
        let evidence = ctx.assessment_evidence(*parent_assessment)?;
        let mut reflective_case = evidence.reflection_case(case);
        reflective_case
            .source_refs
            .push(InfoRef::Assessment(*parent_assessment));
        if let Some(case) = reflective_case
            .case_id
            .and_then(|case_id| ctx.case(case_id))
        {
            reflective_case.input = ReflectiveValue::Text(project_case_input(case));
            refresh_default_run_id(&mut reflective_case);
        }
        cases.push(reflective_case);
    }
    Ok(cases)
}

impl ReflectionProjection for ScalarEvidence {
    fn reflection_case(&self, case: CaseId) -> ReflectiveCase {
        let mut reflective_case = ReflectiveCase::from_example(
            ReflectiveValue::default(),
            None,
            None,
            Some(self.score()),
            String::new(),
        );
        reflective_case.case_id = Some(case);
        reflective_case.runs[0].attempt_index = None;
        reflective_case
    }
}

impl ReflectionProjection for CaseAssessmentEvidence {
    fn reflection_case(&self, case: CaseId) -> ReflectiveCase {
        let mut reflective_case = ReflectiveCase::from_example(
            ReflectiveValue::default(),
            None,
            Some(ReflectiveValue::Text(self.output().report_text())),
            Some(self.score().score()),
            self.feedback().to_owned(),
        );
        reflective_case.case_id = Some(case);
        reflective_case.runs[0].attempt_index = None;
        reflective_case
    }
}
