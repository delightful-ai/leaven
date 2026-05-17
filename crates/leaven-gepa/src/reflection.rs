//! Shared GEPA reflection request and reflective-dataset vocabulary.

use std::fmt::Write as _;
use std::future::Future;

use leaven_core::{
    AssessmentTarget, Evidence, InfoRef, OptimizationProblem, Proposal, ProposalBatch,
    ProposalBatchSemantics,
};
use leaven_engine::{ProposalError, RunContext, RunContextError};
use leaven_evidence::{CaseAssessmentEvidence, OutputRecord, ScalarEvidence};
use leaven_kernel::{AssessmentId, CandidateId, CaseId, MetadataBag};
use leaven_lm::{LmRequest, Messages};
use leaven_surface::EditSurface;
use thiserror::Error;

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
/// This is the typed unit a custom [`ReflectiveDatasetBuilder`] constructs. It
/// leads with `input` because reflecting without showing the model the input
/// the artifact ran on is materially weaker.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ReflectiveExample {
    /// Fully projected upstream-style side-info fields.
    ///
    /// When non-empty, the default renderer emits these fields verbatim as
    /// ordered markdown sections (`## name`) and skips the generic
    /// case/input/output/feedback projection. This is for adapters such as
    /// GEPA optimize-anything where the scorer returns a domain-specific
    /// side-info record.
    pub side_info: Vec<(String, String)>,
    /// Case the artifact ran on, when the projection knows it.
    pub case: Option<CaseId>,
    /// The input the artifact ran on, rendered for the reflector.
    pub input: String,
    /// The artifact's generated output for this case, when present.
    pub output: Option<String>,
    /// The case score, when a comparable score is present.
    pub score: Option<f64>,
    /// Feedback explaining how the response could be better.
    pub feedback: String,
    /// Provenance refs for this example.
    pub source_refs: Vec<InfoRef>,
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
    /// Reflective examples the reflector presents to the model.
    pub examples: Vec<ReflectiveExample>,
    /// Provenance refs lowered into the resulting proposal's `informed_by`.
    pub source_refs: Vec<InfoRef>,
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
        }
    }

    /// Attach the reflective examples for this request.
    #[must_use]
    pub fn with_examples(mut self, examples: impl IntoIterator<Item = ReflectiveExample>) -> Self {
        self.examples.extend(examples);
        self
    }

    /// Attach provenance refs lowered into the resulting proposal.
    #[must_use]
    pub fn with_source_refs(mut self, refs: impl IntoIterator<Item = InfoRef>) -> Self {
        self.source_refs.extend(refs);
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
            .chain(
                self.examples
                    .iter()
                    .flat_map(|example| example.source_refs.iter().cloned()),
            )
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
    /// Build the reflective examples for `parent` on `part`.
    async fn build(
        &self,
        ctx: &mut RunContext<'_, P>,
        parent: CandidateId,
        parent_assessments: &[AssessmentId],
        part: &S::PartId,
    ) -> Result<Vec<ReflectiveExample>, ReflectionError>;
}

impl<P, S, F, Fut> ReflectiveDatasetBuilder<P, S> for F
where
    P: OptimizationProblem,
    S: EditSurface<P::Artifact>,
    F: Fn(&mut RunContext<'_, P>, CandidateId, &[AssessmentId], &S::PartId) -> Fut + Send + Sync,
    Fut: Future<Output = Result<Vec<ReflectiveExample>, ReflectionError>> + Send,
{
    async fn build(
        &self,
        ctx: &mut RunContext<'_, P>,
        parent: CandidateId,
        parent_assessments: &[AssessmentId],
        part: &S::PartId,
    ) -> Result<Vec<ReflectiveExample>, ReflectionError> {
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
/// Projects one [`ReflectiveExample`] per evaluated case from the parent's
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
    ) -> Result<Vec<ReflectiveExample>, ReflectionError> {
        build_gepa_reflective_examples(
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
    ) -> Result<Vec<ReflectiveExample>, ReflectionError> {
        build_gepa_reflective_examples(ctx, parent_assessments, &self.project_case_input)
    }
}

/// Projects an evidence value into GEPA reflective examples.
///
/// This is a crate-private projection seam: it converts the concrete
/// `P::Evidence` shape into per-case examples without an `input`, which the
/// default builder fills from the case set.
pub(crate) trait ReflectionProjection: Evidence {
    /// Project one case-row example, leaving `input` empty.
    fn reflection_example(&self, case: CaseId) -> ReflectiveExample;
}

fn build_gepa_reflective_examples<P>(
    ctx: &RunContext<'_, P>,
    parent_assessments: &[AssessmentId],
    project_case_input: impl Fn(&P::Case) -> String,
) -> Result<Vec<ReflectiveExample>, ReflectionError>
where
    P: OptimizationProblem,
    P::Evidence: ReflectionProjection,
{
    let mut examples = Vec::with_capacity(parent_assessments.len());
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
        let mut example = evidence.reflection_example(case);
        example
            .source_refs
            .push(InfoRef::Assessment(*parent_assessment));
        if let Some(case) = example.case.and_then(|case_id| ctx.case(case_id)) {
            example.input = project_case_input(case);
        }
        examples.push(example);
    }
    Ok(examples)
}

impl ReflectionProjection for ScalarEvidence {
    fn reflection_example(&self, case: CaseId) -> ReflectiveExample {
        ReflectiveExample {
            side_info: Vec::new(),
            case: Some(case),
            input: String::new(),
            output: None,
            score: Some(self.score()),
            feedback: String::new(),
            source_refs: Vec::new(),
        }
    }
}

impl ReflectionProjection for CaseAssessmentEvidence {
    fn reflection_example(&self, case: CaseId) -> ReflectiveExample {
        ReflectiveExample {
            side_info: Vec::new(),
            case: Some(case),
            input: String::new(),
            output: Some(output_record_text(self.output())),
            score: Some(self.score().score()),
            feedback: self.feedback().to_owned(),
            source_refs: Vec::new(),
        }
    }
}

fn output_record_text(output: &OutputRecord) -> String {
    match output {
        OutputRecord::Inline { text, .. } => text.clone(),
        OutputRecord::BlobRef(reference) => format!("blob:{}:{}", reference.store, reference.key),
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
        let feedback = render_reflective_examples(&input.request.examples);
        let template = input
            .config
            .prompt_template
            .as_deref()
            .unwrap_or(DEFAULT_REFLECTION_PROMPT_TEMPLATE);
        let prompt = render_prompt_template(template, &current_instruction, &feedback)?;

        Ok(LmRequest::new(input.model, Messages::from_user(prompt))
            .with_sampling(input.config.sampling.clone())
            .with_output(input.config.output.clone()))
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

fn selected_part_view<P, S>(
    input: &ReflectionRenderInput<'_, P, S>,
) -> Result<String, ProposalError>
where
    P: OptimizationProblem,
    S: EditSurface<P::Artifact>,
    S::PartId: std::fmt::Debug,
    for<'a> S::View<'a>: std::fmt::Display,
{
    let parts = input.surface.parts(input.artifact).map_err(|source| {
        ProposalError::with_source("GEPA reflection surface projection failed", source)
    })?;
    let part = parts
        .into_iter()
        .find(|part| part.id == input.request.part)
        .ok_or_else(|| {
            ProposalError::Message(format!(
                "selected GEPA reflection part {:?} is missing from surface",
                input.request.part
            ))
        })?;
    Ok(part.view.to_string())
}

fn render_reflective_examples(examples: &[ReflectiveExample]) -> String {
    if examples.is_empty() {
        return "(no reflective examples were selected)".to_owned();
    }

    examples
        .iter()
        .enumerate()
        .map(|(index, example)| {
            let mut rendered = String::new();
            let _ = writeln!(rendered, "# Example {}", index + 1);
            render_reflective_example_sections(&mut rendered, example);
            rendered
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn render_reflective_example_sections(rendered: &mut String, example: &ReflectiveExample) {
    if !example.side_info.is_empty() {
        for (name, value) in &example.side_info {
            let _ = writeln!(rendered, "## {}\n{}\n", name.trim(), value.trim());
        }
        return;
    }
    if let Some(case) = example.case {
        let _ = writeln!(rendered, "## Case\n{case}");
    }
    if !example.input.is_empty() {
        let _ = writeln!(rendered, "## Input\n{}", example.input.trim());
    }
    if let Some(score) = example.score {
        let _ = writeln!(rendered, "## Score\n{score}");
    }
    if let Some(output) = &example.output {
        let _ = writeln!(rendered, "## Output\n{}", output.trim());
    }
    if !example.feedback.is_empty() {
        let _ = writeln!(rendered, "## Feedback\n{}", example.feedback.trim());
    }
    rendered.push('\n');
}

fn render_prompt_template(
    template: &str,
    current_instruction: &str,
    side_info: &str,
) -> Result<String, ProposalError> {
    let missing = ["<curr_param>", "<side_info>"]
        .into_iter()
        .filter(|placeholder| !template.contains(placeholder))
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(ProposalError::Message(format!(
            "GEPA reflection prompt template is missing placeholder(s): {}",
            missing.join(", ")
        )));
    }

    Ok(template
        .replace("<curr_param>", current_instruction)
        .replace("<side_info>", side_info))
}

fn extract_replacement_text(assistant_text: &str) -> String {
    let text = assistant_text.trim();
    let Some(start) = text.find("```") else {
        return text.to_owned();
    };
    let content_start = start + 3;
    let Some(end) = text.rfind("```").filter(|end| *end >= content_start) else {
        return strip_opening_fence(text);
    };
    if end == start {
        return strip_opening_fence(text);
    }

    let fenced = &text[content_start..end];
    strip_optional_language(fenced).trim().to_owned()
}

fn strip_opening_fence(text: &str) -> String {
    text.strip_prefix("```")
        .map(strip_optional_language)
        .unwrap_or(text)
        .trim()
        .trim_end_matches("```")
        .trim()
        .to_owned()
}

fn strip_optional_language(text: &str) -> &str {
    let trimmed = text.trim_start();
    match trimmed.find('\n') {
        Some(newline) => {
            let first_line = &trimmed[..newline];
            if !first_line.is_empty() && !first_line.contains(char::is_whitespace) {
                &trimmed[newline + 1..]
            } else {
                trimmed
            }
        }
        None => trimmed,
    }
}

#[cfg(test)]
mod tests {
    use leaven_evidence::OutputRecord;
    use leaven_kernel::BlobRef;

    use super::{
        ReflectiveCaseInput, ReflectiveExample, extract_replacement_text, output_record_text,
        render_prompt_template, render_reflective_examples, strip_optional_language,
    };

    #[test]
    fn reflective_case_input_impls_project_target_safe_text() {
        assert_eq!(().reflective_input(), "");
        assert_eq!("borrowed".reflective_input(), "borrowed");
        assert_eq!(String::from("owned").reflective_input(), "owned");
        let borrowed: &str = "explicit-ref";
        assert_eq!(borrowed.reflective_input(), "explicit-ref");
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

        let blob = OutputRecord::BlobRef(BlobRef {
            store: "file".to_owned(),
            key: "outputs/1".to_owned(),
        });
        assert_eq!(output_record_text(&blob), "blob:file:outputs/1");
        assert_eq!(
            output_record_text(&OutputRecord::inline("inline answer")),
            "inline answer"
        );

        assert!(render_reflective_examples(&[]).contains("no reflective examples"));

        let rendered_examples = render_reflective_examples(&[ReflectiveExample {
            side_info: Vec::new(),
            case: Some(leaven_kernel::CaseId::new(7)),
            input: "  question  ".to_owned(),
            output: Some("  answer  ".to_owned()),
            score: Some(0.5),
            feedback: "  improve arithmetic  ".to_owned(),
            source_refs: Vec::new(),
        }]);
        assert!(rendered_examples.contains("## Case"));
        assert!(rendered_examples.contains("## Input"));
        assert!(rendered_examples.contains("## Score"));
        assert!(rendered_examples.contains("## Output"));
        assert!(rendered_examples.contains("## Feedback"));

        let sparse_example = render_reflective_examples(&[ReflectiveExample {
            side_info: Vec::new(),
            case: None,
            input: String::new(),
            output: None,
            score: None,
            feedback: String::new(),
            source_refs: Vec::new(),
        }]);
        assert!(sparse_example.contains("# Example 1"));
        assert!(!sparse_example.contains("## Case"));
        assert!(!sparse_example.contains("## Input"));
        assert!(!sparse_example.contains("## Score"));
        assert!(!sparse_example.contains("## Output"));
        assert!(!sparse_example.contains("## Feedback"));
    }

    #[test]
    fn reflective_examples_can_render_upstream_side_info_records() {
        let rendered = render_reflective_examples(&[ReflectiveExample {
            side_info: vec![
                ("score".to_owned(), "0".to_owned()),
                ("input".to_owned(), "What is 19 + 23?".to_owned()),
                ("prompt".to_owned(), "Solve carefully.".to_owned()),
                ("output".to_owned(), "44".to_owned()),
                ("reasoning".to_owned(), "I added incorrectly.".to_owned()),
                (
                    "execution_feedback".to_owned(),
                    "Your answer is incorrect. The correct answer is '42'.".to_owned(),
                ),
            ],
            source_refs: Vec::new(),
            ..ReflectiveExample::default()
        }]);

        assert_eq!(
            rendered,
            "# Example 1\n## score\n0\n\n## input\nWhat is 19 + 23?\n\n## prompt\nSolve carefully.\n\n## output\n44\n\n## reasoning\nI added incorrectly.\n\n## execution_feedback\nYour answer is incorrect. The correct answer is '42'.\n\n"
        );
        assert!(!rendered.contains("## Input"));
        assert!(!rendered.contains("Inline {"));
    }

    #[test]
    fn reflective_examples_join_multiple_side_info_records_like_upstream_gepa() {
        let rendered = render_reflective_examples(&[
            ReflectiveExample {
                side_info: vec![("score".to_owned(), "0.0".to_owned())],
                ..ReflectiveExample::default()
            },
            ReflectiveExample {
                side_info: vec![("score".to_owned(), "1.0".to_owned())],
                ..ReflectiveExample::default()
            },
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
