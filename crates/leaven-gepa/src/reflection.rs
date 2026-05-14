//! Shared GEPA reflection request and feedback vocabulary.

use std::fmt::Write as _;

use leaven_core::{InfoRef, OptimizationProblem, Proposal, ProposalBatch, ProposalBatchSemantics};
use leaven_engine::ProposalError;
use leaven_evidence::{CasewiseEvidence, ScalarEvidence, ScoredFeedbackEvidence};
use leaven_kernel::{AssessmentId, CandidateId, CaseId, MetadataBag};
use leaven_lm::{LmRequest, Messages};
use leaven_surface::EditSurface;

/// One proposer-readable feedback record selected for GEPA reflection.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct ReflectiveFeedbackRecord {
    pub case: Option<CaseId>,
    pub score: Option<f64>,
    pub feedback: String,
    pub trace: Vec<String>,
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
                feedback: String::new(),
                trace: Vec::new(),
                source_refs: Vec::new(),
            })
            .collect()
    }
}

impl GepaReflectionEvidence for CasewiseEvidence<ScoredFeedbackEvidence> {
    fn reflection_records(&self) -> Vec<ReflectiveFeedbackRecord> {
        self.outcomes()
            .iter()
            .map(|outcome| ReflectiveFeedbackRecord {
                case: Some(outcome.case()),
                score: Some(outcome.evidence().score().score()),
                feedback: outcome.evidence().feedback().to_owned(),
                trace: outcome.evidence().trace().to_vec(),
                source_refs: Vec::new(),
            })
            .collect()
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
    P::Artifact: std::fmt::Debug,
    S: EditSurface<P::Artifact>,
    S::PartId: std::fmt::Debug,
{
    fn render(&self, input: ReflectionRenderInput<'_, P, S>) -> Result<LmRequest, ProposalError> {
        let mut feedback = String::new();
        for record in &input.request.selected_feedback.records {
            feedback.push_str("- case: ");
            feedback.push_str(
                &record
                    .case
                    .map(|case| case.0.to_string())
                    .unwrap_or_else(|| "unknown".to_owned()),
            );
            if let Some(score) = record.score {
                let _ = write!(feedback, ", score: {score}");
            }
            if !record.feedback.is_empty() {
                feedback.push_str("\n  feedback: ");
                feedback.push_str(&record.feedback);
            }
            if !record.trace.is_empty() {
                feedback.push_str("\n  trace:\n");
                for line in &record.trace {
                    feedback.push_str("    ");
                    feedback.push_str(line);
                    feedback.push('\n');
                }
            }
            feedback.push('\n');
        }
        if feedback.trim().is_empty() {
            feedback.push_str("(no textual feedback records were selected)");
        }

        let system_prompt = "You are a GEPA reflection proposer. Use the selected evaluation feedback to propose one replacement edit for the selected artifact part. Return only the replacement text.";
        let user_prompt = format!(
            "Parent candidate: {parent}\nSelected part: {part_label}\nSelected part id: {part:?}\nCurrent artifact: {artifact:?}\n\nSelected feedback:\n{feedback}",
            parent = input.request.parent,
            part_label = input.request.part_label,
            part = input.request.part,
            artifact = input.artifact,
        );

        Ok(LmRequest::new(
            input.model,
            Messages::new()
                .with_system(system_prompt)
                .with_user(user_prompt),
        )
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
        let change = surface
            .change_part(artifact, request.part.clone(), assistant_text.to_owned())
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
}
