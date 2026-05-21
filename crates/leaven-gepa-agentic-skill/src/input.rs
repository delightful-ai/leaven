use leaven_artifact_skill::SkillBank;
use leaven_core::InfoRef;
use leaven_gepa::{ReflectRequest, ReflectiveExample};
use leaven_kernel::CandidateId;

/// GEPA reflection input handed to the skill-bank agentic proposer.
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct SkillBankGepaReflectionInput<Part = String> {
    /// Candidate being improved.
    pub parent: CandidateId,
    /// Parent artifact state materialized for the agent.
    pub artifact: SkillBank,
    /// Surface part selected by GEPA.
    pub part: Part,
    /// Human-readable selected-part label.
    pub part_label: String,
    /// Pre-built GEPA reflective examples.
    pub examples: Vec<ReflectiveExample>,
    /// Provenance refs lowered into the resulting proposal.
    pub source_refs: Vec<InfoRef>,
    /// GEPA proposal-attempt ordinal, when available.
    pub attempt_index: Option<usize>,
}

impl<Part> SkillBankGepaReflectionInput<Part> {
    /// Builds the agentic input from a pre-built GEPA reflection request and
    /// the parent artifact resolved from the run graph.
    #[must_use]
    pub fn from_request(artifact: SkillBank, request: ReflectRequest<Part>) -> Self {
        Self {
            parent: request.parent,
            artifact,
            part: request.part,
            part_label: request.part_label,
            examples: request.examples,
            source_refs: request.source_refs,
            attempt_index: request.attempt_index,
        }
    }

    /// Returns all provenance refs for the resulting proposal.
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
