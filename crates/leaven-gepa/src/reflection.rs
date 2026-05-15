//! Shared GEPA reflection request and feedback vocabulary.

use std::fmt::Write as _;

use leaven_core::{InfoRef, OptimizationProblem, Proposal, ProposalBatch, ProposalBatchSemantics};
use leaven_engine::ProposalError;
use leaven_evidence::{CaseAssessmentEvidence, CasewiseEvidence, OutputRecord, ScalarEvidence};
use leaven_kernel::{AssessmentId, CandidateId, CaseId, MetadataBag};
use leaven_lm::{LmRequest, Messages};
use leaven_surface::EditSurface;

/// GEPA's default instruction-improvement prompt template.
///
/// The template follows upstream GEPA's ordinary instruction reflection shape:
/// present the current editable text plus selected feedback records, then ask
/// the reflection LM to return the replacement text inside triple backticks.
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

/// One proposer-readable feedback record selected for GEPA reflection.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct ReflectiveFeedbackRecord {
    pub case: Option<CaseId>,
    pub score: Option<f64>,
    pub output: Option<String>,
    pub feedback: String,
    pub source_refs: Vec<InfoRef>,
}

impl ReflectiveFeedbackRecord {
    /// Attach additional source refs to this record.
    #[must_use]
    pub fn with_source_refs(mut self, refs: impl IntoIterator<Item = InfoRef>) -> Self {
        self.source_refs.extend(refs);
        self
    }
}

/// Selected feedback and provenance refs for one GEPA reflection call.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct SelectedFeedback {
    pub assessment_refs: Vec<AssessmentId>,
    pub evidence_refs: Vec<InfoRef>,
    pub candidate_refs: Vec<CandidateId>,
    pub records: Vec<ReflectiveFeedbackRecord>,
}

impl SelectedFeedback {
    #[must_use]
    pub fn with_assessments(mut self, feedback: impl IntoIterator<Item = AssessmentId>) -> Self {
        self.assessment_refs.extend(feedback);
        self
    }

    #[must_use]
    pub fn with_records(
        mut self,
        records: impl IntoIterator<Item = ReflectiveFeedbackRecord>,
    ) -> Self {
        self.records.extend(records);
        self
    }

    #[must_use]
    pub fn source_refs(&self) -> Vec<InfoRef> {
        self.candidate_refs
            .iter()
            .copied()
            .map(InfoRef::Candidate)
            .chain(
                self.assessment_refs
                    .iter()
                    .copied()
                    .map(InfoRef::Assessment),
            )
            .chain(self.evidence_refs.iter().cloned())
            .chain(
                self.records
                    .iter()
                    .flat_map(|record| record.source_refs.iter().cloned()),
            )
            .collect()
    }
}

/// Shared GEPA reflection request.
///
/// The default `String` part keeps agent-stage JSON requests small. Typed
/// reflectors can use their surface's native `PartId`.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ReflectRequest<Part = String> {
    pub parent: CandidateId,
    pub part: Part,
    pub part_label: String,
    pub selected_feedback: SelectedFeedback,
}

impl ReflectRequest<String> {
    #[must_use]
    pub fn new(parent: CandidateId, part_label: impl Into<String>) -> Self {
        let part_label = part_label.into();
        Self {
            parent,
            part: part_label.clone(),
            part_label,
            selected_feedback: SelectedFeedback::default(),
        }
    }
}

impl<Part> ReflectRequest<Part> {
    #[must_use]
    pub fn for_part(parent: CandidateId, part: Part, part_label: impl Into<String>) -> Self {
        Self {
            parent,
            part,
            part_label: part_label.into(),
            selected_feedback: SelectedFeedback::default(),
        }
    }

    #[must_use]
    pub fn with_feedback(mut self, feedback: impl IntoIterator<Item = AssessmentId>) -> Self {
        self.selected_feedback = self.selected_feedback.with_assessments(feedback);
        self
    }

    #[must_use]
    pub fn with_selected_feedback(mut self, selected_feedback: SelectedFeedback) -> Self {
        self.selected_feedback = selected_feedback;
        self
    }
}

/// Evidence that can be projected into GEPA reflection records.
pub trait GepaReflectionEvidence: leaven_core::Evidence {
    /// Project casewise records suitable for reflection prompts.
    fn reflection_records(&self) -> Vec<ReflectiveFeedbackRecord>;
}

impl GepaReflectionEvidence for CasewiseEvidence<ScalarEvidence> {
    fn reflection_records(&self) -> Vec<ReflectiveFeedbackRecord> {
        self.outcomes()
            .iter()
            .map(|outcome| ReflectiveFeedbackRecord {
                case: Some(outcome.case()),
                score: Some(outcome.evidence().score()),
                output: None,
                feedback: String::new(),
                source_refs: Vec::new(),
            })
            .collect()
    }
}

impl GepaReflectionEvidence for CasewiseEvidence<CaseAssessmentEvidence> {
    fn reflection_records(&self) -> Vec<ReflectiveFeedbackRecord> {
        self.outcomes()
            .iter()
            .map(|outcome| ReflectiveFeedbackRecord {
                case: Some(outcome.case()),
                score: Some(outcome.evidence().score().score()),
                output: Some(output_record_text(outcome.evidence().output())),
                feedback: outcome.evidence().feedback().to_owned(),
                source_refs: Vec::new(),
            })
            .collect()
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
    fn render(&self, input: ReflectionRenderInput<'_, P, S>) -> Result<LmRequest, ProposalError>;
}

/// Inputs available to a reflection renderer.
pub struct ReflectionRenderInput<'a, P, S>
where
    P: OptimizationProblem,
    S: EditSurface<P::Artifact>,
{
    pub request: &'a ReflectRequest<S::PartId>,
    pub artifact: &'a P::Artifact,
    pub surface: &'a S,
    pub model: leaven_lm::ModelName,
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
        let feedback = render_feedback_records(&input.request.selected_feedback.records);
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
                    .informed_by(request.selected_feedback.source_refs())
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
    pub sampling: leaven_lm::SamplingOptions,
    pub output: leaven_lm::OutputMode,
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

fn render_feedback_records(records: &[ReflectiveFeedbackRecord]) -> String {
    if records.is_empty() {
        return "(no textual feedback records were selected)".to_owned();
    }

    let mut feedback = String::new();
    for (index, record) in records.iter().enumerate() {
        let _ = writeln!(feedback, "# Example {}", index + 1);
        if let Some(case) = record.case {
            let _ = writeln!(feedback, "## Case\n{case}");
        }
        if let Some(score) = record.score {
            let _ = writeln!(feedback, "## Score\n{score}");
        }
        if let Some(output) = &record.output {
            let _ = writeln!(feedback, "## Output\n{}", output.trim());
        }
        if !record.feedback.is_empty() {
            let _ = writeln!(feedback, "## Feedback\n{}", record.feedback.trim());
        }
        feedback.push('\n');
    }
    feedback
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
