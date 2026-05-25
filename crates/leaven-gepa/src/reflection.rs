//! Shared GEPA reflection request and reflective-dataset vocabulary.

use std::future::Future;

use leaven_core::{
    AssessmentTarget, Evidence, InfoRef, OptimizationProblem, Proposal, ProposalBatch,
    ProposalBatchSemantics,
};
use leaven_engine::{ProposalError, RunContext, RunContextError};
pub use leaven_evidence::{Attachment, AttachmentKind};
use leaven_evidence::{CaseAssessmentEvidence, ScalarEvidence};
use leaven_kernel::{AgentId, AssessmentId, CandidateId, CaseId, CaseRunId, MetadataBag, TraceRef};
use leaven_lm::{LmRequest, Messages};
use leaven_surface::EditSurface;
use thiserror::Error;

mod identity;
mod render;

use identity::{deterministic_example_run_id, refresh_default_run_id};
use render::{
    extract_replacement_text, render_prompt_template, render_reflective_cases, selected_part_view,
};

/// GEPA's default instruction-improvement prompt template.
///
/// The template follows upstream GEPA's ordinary instruction reflection shape:
/// present the current editable text plus selected reflective examples, then
/// ask the reflection LM to return the replacement text inside triple backticks.
pub const DEFAULT_REFLECTION_PROMPT_TEMPLATE: &str = r"I provided an assistant with the following instructions to perform a task for me:
```
<curr_param>
```

The following are examples of different task inputs provided to the assistant along with the assistant's response for each of them, and some feedback on how the assistant's response could be better:
```
<side_info>
```

Your task is to write a new instruction for the assistant.

Read the inputs carefully and identify the input format and infer detailed task description about the task I wish to solve with the assistant.

Read all the assistant responses and the corresponding feedback. Identify all niche and domain specific factual information about the task and include it in the instruction, as a lot of it may not be available to the assistant in the future. The assistant may have utilized a generalizable strategy to solve the task, if so, include that in the instruction as well.

Provide the new instructions within ``` blocks.";

/// One evaluated case, projected for GEPA reflection.
///
/// The case owns runner-visible input/expected material and one or more runs
/// over that input. LM-parity datasets emit exactly one run per case; agentic
/// datasets can attach transcripts, checks, and multiple attempts without
/// flattening them into prose.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ReflectiveCase {
    /// Case the artifact ran on, when the projection knows it.
    pub case_id: Option<CaseId>,
    /// Runner-visible case input.
    pub input: ReflectiveValue,
    /// Optional expected output, reference, rubric, or oracle.
    pub expected: Option<ReflectiveValue>,
    /// Runs observed for this case.
    pub runs: Vec<ReflectiveRun>,
    /// Provenance refs for this case-level projection.
    pub source_refs: Vec<InfoRef>,
}

impl ReflectiveCase {
    /// Flat constructor for the common single-run reflective row.
    #[must_use]
    pub fn from_example(
        input: ReflectiveValue,
        expected: Option<ReflectiveValue>,
        produced: Option<ReflectiveValue>,
        score: Option<f64>,
        feedback: impl Into<String>,
    ) -> Self {
        let feedback = feedback.into();
        let run_id = deterministic_example_run_id(
            &input,
            expected.as_ref(),
            produced.as_ref(),
            score,
            &feedback,
        );
        Self {
            case_id: None,
            input,
            expected,
            runs: vec![ReflectiveRun {
                run_id,
                agent_id: None,
                attempt_index: Some(0),
                produced,
                score,
                max_score: None,
                passed: None,
                feedback,
                checks: None,
                side_info: Vec::new(),
                attachments: Vec::new(),
                source_refs: Vec::new(),
            }],
            source_refs: Vec::new(),
        }
    }
}

/// One observed artifact run for a reflective case.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ReflectiveRun {
    pub run_id: CaseRunId,
    pub agent_id: Option<AgentId>,
    pub attempt_index: Option<usize>,
    pub produced: Option<ReflectiveValue>,
    pub score: Option<f64>,
    pub max_score: Option<f64>,
    pub passed: Option<bool>,
    pub feedback: String,
    pub checks: Option<Checks>,
    /// LM-paradigm flat field rendering. Empty for agent-paradigm cases.
    pub side_info: Vec<(String, ReflectiveSideInfoValue)>,
    /// Agent-paradigm typed evidence. Empty for LM-only cases.
    pub attachments: Vec<Attachment>,
    pub source_refs: Vec<InfoRef>,
}

/// Reflective value material that can be rendered or materialized.
///
/// `serde_json::Value` carries `f64`, so this enum stays `PartialEq` only.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum ReflectiveValue {
    Text(String),
    Json(serde_json::Value),
    File(TraceRef),
    Mapping(Vec<(String, Self)>),
}

impl Default for ReflectiveValue {
    fn default() -> Self {
        Self::Text(String::new())
    }
}

/// Structured pass/fail checks for a reflective run.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Checks {
    pub passes: Vec<Check>,
    pub fails: Vec<Check>,
}

/// One structured pass/fail check.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Check {
    pub id: String,
    pub requirement: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Upstream GEPA side-info value rendered into markdown for reflection.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum ReflectiveSideInfoValue {
    /// Scalar text rendered as-is after trimming surrounding whitespace.
    Text(String),
    /// Ordered mapping rendered as nested markdown headings.
    Mapping(Vec<(String, Self)>),
    /// Ordered list rendered as `Item N` nested markdown headings.
    List(Vec<Self>),
}

impl ReflectiveSideInfoValue {
    /// Build an ordered mapping value.
    pub fn mapping(fields: impl IntoIterator<Item = (String, Self)>) -> Self {
        Self::Mapping(fields.into_iter().collect())
    }

    /// Build an ordered list value.
    pub fn list(items: impl IntoIterator<Item = Self>) -> Self {
        Self::List(items.into_iter().collect())
    }
}

impl From<String> for ReflectiveSideInfoValue {
    fn from(value: String) -> Self {
        Self::Text(value)
    }
}

impl From<&str> for ReflectiveSideInfoValue {
    fn from(value: &str) -> Self {
        Self::Text(value.to_owned())
    }
}

/// Shared GEPA reflection request.
///
/// The optimizer loop builds this exactly once per reflection step, via the
/// configured [`ReflectiveDatasetBuilder`], and passes the same value to
/// whichever reflector is configured. There is no place for a backend to
/// project the reflective data differently.
///
/// The default `String` part keeps agent-stage JSON requests small. Typed
/// reflectors can use their surface's native `PartId`.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ReflectRequest<Part = String> {
    /// Candidate the reflection improves.
    pub parent: CandidateId,
    /// Surface part the reflection edits.
    pub part: Part,
    /// Human-readable label for the selected part.
    pub part_label: String,
    /// Reflective cases the reflector presents to the model.
    pub examples: Vec<ReflectiveCase>,
    /// Provenance refs lowered into the resulting proposal's `informed_by`.
    pub source_refs: Vec<InfoRef>,
    /// Stable GEPA proposal-attempt ordinal, when the request comes from the
    /// reference loop.
    pub attempt_index: Option<usize>,
}

impl ReflectRequest<String> {
    /// Build a request with a `String` part equal to its label.
    #[must_use]
    pub fn new(parent: CandidateId, part_label: impl Into<String>) -> Self {
        let part_label = part_label.into();
        Self {
            parent,
            part: part_label.clone(),
            part_label,
            examples: Vec::new(),
            source_refs: Vec::new(),
            attempt_index: None,
        }
    }
}

impl<Part> ReflectRequest<Part> {
    /// Build a request for an explicit surface-native part.
    #[must_use]
    pub fn for_part(parent: CandidateId, part: Part, part_label: impl Into<String>) -> Self {
        Self {
            parent,
            part,
            part_label: part_label.into(),
            examples: Vec::new(),
            source_refs: Vec::new(),
            attempt_index: None,
        }
    }

    /// Attach the reflective cases for this request.
    #[must_use]
    pub fn with_examples(mut self, examples: impl IntoIterator<Item = ReflectiveCase>) -> Self {
        self.examples.extend(examples);
        self
    }

    /// Attach provenance refs lowered into the resulting proposal.
    #[must_use]
    pub fn with_source_refs(mut self, refs: impl IntoIterator<Item = InfoRef>) -> Self {
        self.source_refs.extend(refs);
        self
    }

    /// Attach the GEPA proposal-attempt ordinal for cache-stable reflection
    /// diversity.
    #[must_use]
    pub const fn with_attempt_index(mut self, attempt_index: usize) -> Self {
        self.attempt_index = Some(attempt_index);
        self
    }

    /// All provenance refs for the resulting proposal's `informed_by`.
    ///
    /// This is the union of the request-level `source_refs` and every
    /// example's own `source_refs`.
    #[must_use]
    pub fn informed_by(&self) -> Vec<InfoRef> {
        self.source_refs
            .iter()
            .cloned()
            .chain(self.examples.iter().flat_map(|case| {
                case.source_refs.iter().cloned().chain(
                    case.runs
                        .iter()
                        .flat_map(|run| run.source_refs.iter().cloned()),
                )
            }))
            .collect()
    }
}

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
pub(crate) trait ReflectionProjection: Evidence {
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

/// Renders one LM request for an LM-backed GEPA reflection call.
pub trait ReflectionRenderer<P, S>: Send + Sync
where
    P: OptimizationProblem,
    S: EditSurface<P::Artifact>,
{
    /// Render the reflection request into an LM request.
    fn render(&self, input: ReflectionRenderInput<'_, P, S>) -> Result<LmRequest, ProposalError>;
}

/// Inputs available to a reflection renderer.
pub struct ReflectionRenderInput<'a, P, S>
where
    P: OptimizationProblem,
    S: EditSurface<P::Artifact>,
{
    /// The pre-built reflection request.
    pub request: &'a ReflectRequest<S::PartId>,
    /// The parent artifact.
    pub artifact: &'a P::Artifact,
    /// The edit surface.
    pub surface: &'a S,
    /// Reflection model name.
    pub model: leaven_lm::ModelName,
    /// LM request controls.
    pub config: &'a LmBackedReflectorConfig,
}

/// Default text renderer for GEPA reflection.
#[derive(Clone, Debug, Default)]
pub struct DefaultReflectionRenderer;

impl<P, S> ReflectionRenderer<P, S> for DefaultReflectionRenderer
where
    P: OptimizationProblem,
    S: EditSurface<P::Artifact>,
    S::PartId: std::fmt::Debug,
    for<'a> S::View<'a>: std::fmt::Display,
{
    fn render(&self, input: ReflectionRenderInput<'_, P, S>) -> Result<LmRequest, ProposalError> {
        let current_instruction = selected_part_view(&input)?;
        let feedback = render_reflective_cases(&input.request.examples);
        let template = input
            .config
            .prompt_template
            .as_deref()
            .unwrap_or(DEFAULT_REFLECTION_PROMPT_TEMPLATE);
        let prompt = render_prompt_template(template, &current_instruction, &feedback)?;

        let mut provider_hints = leaven_lm::ProviderHints::default();
        if let Some(attempt_index) = input.request.attempt_index {
            provider_hints
                .metadata
                .insert("gepa_attempt_index".to_owned(), attempt_index.to_string());
        }
        Ok(LmRequest::new(input.model, Messages::from_user(prompt))
            .with_sampling(input.config.sampling.clone())
            .with_output(input.config.output.clone())
            .with_provider_hints(provider_hints))
    }
}

/// Parses LM output into a GEPA proposal batch.
pub trait ReflectionOutputParser<P, S>: Send + Sync
where
    P: OptimizationProblem,
    S: EditSurface<P::Artifact>,
{
    /// Parse the assistant text into a proposal batch.
    fn parse(
        &self,
        assistant_text: &str,
        request: &ReflectRequest<S::PartId>,
        artifact: &P::Artifact,
        surface: &S,
    ) -> Result<ProposalBatch<P>, ProposalError>;
}

/// Parser that treats the LM response as a replacement text edit.
#[derive(Clone, Debug, Default)]
pub struct PlainTextEditParser;

impl<P, S> ReflectionOutputParser<P, S> for PlainTextEditParser
where
    P: OptimizationProblem,
    P::ProposalAnnotations: Default,
    S: EditSurface<P::Artifact, Edit = String>,
{
    fn parse(
        &self,
        assistant_text: &str,
        request: &ReflectRequest<S::PartId>,
        artifact: &P::Artifact,
        surface: &S,
    ) -> Result<ProposalBatch<P>, ProposalError> {
        let replacement = extract_replacement_text(assistant_text);
        let change = surface
            .change_part(artifact, request.part.clone(), replacement)
            .map_err(|source| {
                ProposalError::with_source("GEPA surface edit lowering failed", source)
            })?;
        Ok(ProposalBatch {
            proposals: vec![
                Proposal::mutate(request.parent, change)
                    .informed_by(request.informed_by())
                    .build(),
            ],
            semantics: ProposalBatchSemantics::Alternatives,
            metadata: MetadataBag::new(),
        })
    }
}

/// LM request controls for LM-backed reflection.
#[derive(Clone, Debug, Default)]
pub struct LmBackedReflectorConfig {
    /// Sampling options for the reflection LM call.
    pub sampling: leaven_lm::SamplingOptions,
    /// Output mode for the reflection LM call.
    pub output: leaven_lm::OutputMode,
    /// Optional override of the default GEPA prompt template.
    pub prompt_template: Option<String>,
}

impl LmBackedReflectorConfig {
    /// Override the default GEPA instruction-reflection prompt template.
    ///
    /// Templates must contain `<curr_param>` and `<side_info>` placeholders.
    #[must_use]
    pub fn with_prompt_template(mut self, template: impl Into<String>) -> Self {
        self.prompt_template = Some(template.into());
        self
    }
}

/// Failure from the reflective-dataset builder seam.
#[derive(Debug, Error)]
pub enum ReflectionError {
    /// The parent assessment evidence could not be read from the run.
    #[error("GEPA reflective-dataset projection failed")]
    Evidence {
        /// The run-context refusal that produced the failure.
        #[source]
        source: RunContextError,
    },
    /// A custom builder declined to project a reflective dataset.
    #[error("GEPA reflective-dataset builder failed: {reason}")]
    Builder {
        /// Human-readable reason from the custom builder.
        reason: String,
    },
}

impl ReflectionError {
    /// Build a builder-side failure with a human-readable reason.
    #[must_use]
    pub fn builder(reason: impl Into<String>) -> Self {
        Self::Builder {
            reason: reason.into(),
        }
    }
}

impl From<RunContextError> for ReflectionError {
    fn from(source: RunContextError) -> Self {
        Self::Evidence { source }
    }
}

#[cfg(test)]
mod tests {
    use leaven_evidence::OutputRecord;
    use leaven_kernel::{AgentId, BlobRef};

    use super::identity::refresh_default_run_id;
    use super::render::strip_optional_language;
    use super::{
        ReflectiveCase, ReflectiveCaseInput, ReflectiveRun, ReflectiveSideInfoValue,
        ReflectiveValue, extract_replacement_text, render_prompt_template, render_reflective_cases,
    };

    fn example_case(
        case_id: Option<leaven_kernel::CaseId>,
        input: &str,
        output: Option<&str>,
        score: Option<f64>,
        feedback: &str,
    ) -> ReflectiveCase {
        let mut case = ReflectiveCase::from_example(
            ReflectiveValue::Text(input.to_owned()),
            None,
            output.map(|value| ReflectiveValue::Text(value.to_owned())),
            score,
            feedback.to_owned(),
        );
        case.case_id = case_id;
        case
    }

    fn side_info_case(side_info: Vec<(String, ReflectiveSideInfoValue)>) -> ReflectiveCase {
        let mut case = ReflectiveCase::from_example(
            ReflectiveValue::default(),
            None,
            None,
            None,
            String::new(),
        );
        case.runs[0].side_info = side_info;
        case
    }

    #[test]
    fn reflective_case_input_impls_project_target_safe_text() {
        assert_eq!(().reflective_input(), "");
        assert_eq!("borrowed".reflective_input(), "borrowed");
        assert_eq!(String::from("owned").reflective_input(), "owned");
        let borrowed: &str = "explicit-ref";
        assert_eq!(borrowed.reflective_input(), "explicit-ref");
    }

    #[test]
    fn projected_case_input_refreshes_default_run_id() {
        let produced = Some(ReflectiveValue::Text("answer".to_owned()));
        let mut projected = ReflectiveCase::from_example(
            ReflectiveValue::default(),
            None,
            produced.clone(),
            Some(0.5),
            "feedback",
        );
        let placeholder_run_id = projected.runs[0].run_id;
        projected.input = ReflectiveValue::Text("actual input".to_owned());
        refresh_default_run_id(&mut projected);

        let direct = ReflectiveCase::from_example(
            ReflectiveValue::Text("actual input".to_owned()),
            None,
            produced,
            Some(0.5),
            "feedback",
        );

        assert_ne!(placeholder_run_id, projected.runs[0].run_id);
        assert_eq!(direct.runs[0].run_id, projected.runs[0].run_id);
    }

    #[test]
    fn projected_case_input_refreshes_all_run_ids_with_attempt_metadata() {
        let mut projected = ReflectiveCase::from_example(
            ReflectiveValue::default(),
            None,
            Some(ReflectiveValue::Text("answer".to_owned())),
            Some(0.5),
            "feedback",
        );
        projected.runs.push(ReflectiveRun {
            run_id: projected.runs[0].run_id,
            agent_id: Some(AgentId::from("worker")),
            attempt_index: Some(1),
            produced: Some(ReflectiveValue::Text("answer".to_owned())),
            score: Some(0.5),
            max_score: Some(1.0),
            passed: Some(false),
            feedback: "feedback".to_owned(),
            checks: None,
            side_info: Vec::new(),
            attachments: Vec::new(),
            source_refs: Vec::new(),
        });
        let placeholder_ids = projected
            .runs
            .iter()
            .map(|run| run.run_id)
            .collect::<Vec<_>>();

        projected.input = ReflectiveValue::Text("actual input".to_owned());
        refresh_default_run_id(&mut projected);

        assert_ne!(placeholder_ids[0], projected.runs[0].run_id);
        assert_ne!(placeholder_ids[1], projected.runs[1].run_id);
        assert_ne!(projected.runs[0].run_id, projected.runs[1].run_id);
    }

    #[test]
    fn reflection_helpers_cover_prompt_and_output_variants() {
        let prompt = render_prompt_template(
            "current=<curr_param>\nexamples=<side_info>",
            "seed instruction",
            "case feedback",
        )
        .expect("valid template");
        assert!(prompt.contains("seed instruction"));
        assert!(prompt.contains("case feedback"));

        let missing = render_prompt_template("<curr_param>", "seed", "side")
            .expect_err("missing side-info placeholder");
        assert!(missing.to_string().contains("<side_info>"));

        let blob = OutputRecord::blob(BlobRef {
            store: "file".to_owned(),
            key: "outputs/1".to_owned(),
        });
        assert_eq!(blob.report_text(), "blob:file:outputs/1");
        assert_eq!(
            OutputRecord::inline("inline answer").report_text(),
            "inline answer"
        );

        assert!(render_reflective_cases(&[]).contains("no reflective examples"));

        let rendered_examples = render_reflective_cases(&[example_case(
            Some(leaven_kernel::CaseId::new(7)),
            "  question  ",
            Some("  answer  "),
            Some(0.5),
            "  improve arithmetic  ",
        )]);
        assert!(rendered_examples.contains("## Case"));
        assert!(rendered_examples.contains("## Input"));
        assert!(rendered_examples.contains("## Score"));
        assert!(rendered_examples.contains("## Output"));
        assert!(rendered_examples.contains("## Feedback"));

        let sparse_example = render_reflective_cases(&[example_case(None, "", None, None, "")]);
        assert!(sparse_example.contains("# Example 1"));
        assert!(!sparse_example.contains("## Case"));
        assert!(!sparse_example.contains("## Input"));
        assert!(!sparse_example.contains("## Score"));
        assert!(!sparse_example.contains("## Output"));
        assert!(!sparse_example.contains("## Feedback"));
    }

    #[test]
    fn reflective_examples_render_case_input_once_for_multiple_runs() {
        let mut case = example_case(None, "shared input", Some("first output"), Some(0.0), "bad");
        let mut second_run = case.runs[0].clone();
        second_run.attempt_index = Some(1);
        second_run.produced = Some(ReflectiveValue::Text("second output".to_owned()));
        second_run.feedback = "better".to_owned();
        case.runs.push(second_run);

        let rendered = render_reflective_cases(&[case]);

        assert_eq!(rendered.matches("## Input").count(), 1);
        assert!(rendered.contains("## Run 1"));
        assert!(rendered.contains("## Run 2"));
        assert!(rendered.contains("first output"));
        assert!(rendered.contains("second output"));
    }

    #[test]
    fn reflective_examples_can_render_upstream_side_info_records() {
        let rendered = render_reflective_cases(&[side_info_case(vec![
            ("score".to_owned(), "0".into()),
            ("input".to_owned(), "What is 19 + 23?".into()),
            ("prompt".to_owned(), "Solve carefully.".into()),
            ("output".to_owned(), "44".into()),
            ("reasoning".to_owned(), "I added incorrectly.".into()),
            (
                "execution_feedback".to_owned(),
                "Your answer is incorrect. The correct answer is '42'.".into(),
            ),
        ])]);

        assert_eq!(
            rendered,
            "# Example 1\n## score\n0\n\n## input\nWhat is 19 + 23?\n\n## prompt\nSolve carefully.\n\n## output\n44\n\n## reasoning\nI added incorrectly.\n\n## execution_feedback\nYour answer is incorrect. The correct answer is '42'.\n\n"
        );
        assert!(!rendered.contains("## Input"));
        assert!(!rendered.contains("Inline {"));
    }

    #[test]
    fn reflective_examples_render_nested_side_info_like_upstream_gepa() {
        let rendered = render_reflective_cases(&[side_info_case(vec![(
            "scores".to_owned(),
            ReflectiveSideInfoValue::mapping([
                ("exact".to_owned(), "0.0".into()),
                (
                    "attempts".to_owned(),
                    ReflectiveSideInfoValue::list([
                        ReflectiveSideInfoValue::mapping([("answer".to_owned(), "44".into())]),
                        ReflectiveSideInfoValue::mapping([("answer".to_owned(), "42".into())]),
                    ]),
                ),
            ]),
        )])]);

        assert_eq!(
            rendered,
            "# Example 1\n## scores\n### exact\n0.0\n\n### attempts\n#### Item 1\n##### answer\n44\n\n#### Item 2\n##### answer\n42\n\n"
        );
    }

    #[test]
    fn reflective_examples_join_multiple_side_info_records_like_upstream_gepa() {
        let rendered = render_reflective_cases(&[
            side_info_case(vec![("score".to_owned(), "0.0".into())]),
            side_info_case(vec![("score".to_owned(), "1.0".into())]),
        ]);

        assert_eq!(
            rendered,
            "# Example 1\n## score\n0.0\n\n\n\n# Example 2\n## score\n1.0\n\n"
        );
    }

    #[test]
    fn plain_text_parser_extracts_fenced_and_unfenced_replacements() {
        assert_eq!(
            extract_replacement_text("  use this directly  "),
            "use this directly"
        );
        assert_eq!(
            extract_replacement_text("before\n```text\nreplacement\n```\nafter"),
            "replacement"
        );
        assert_eq!(
            extract_replacement_text("```rust\nreplacement without close"),
            "replacement without close"
        );
        assert_eq!(extract_replacement_text("```\nopen only"), "open only");
        assert_eq!(extract_replacement_text("```"), "");
        assert_eq!(extract_replacement_text("``````"), "");
        assert_eq!(
            strip_optional_language("not a language line\nbody"),
            "not a language line\nbody"
        );
        assert_eq!(strip_optional_language("\nbody"), "body");
        assert_eq!(
            strip_optional_language("json\n{\"ok\":true}"),
            "{\"ok\":true}"
        );
    }
}
