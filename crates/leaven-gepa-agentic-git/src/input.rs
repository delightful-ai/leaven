use leaven_artifact_git::GitProgramArtifact;
use leaven_core::InfoRef;
use leaven_gepa::{ReflectRequest, ReflectiveCase};
use leaven_kernel::CandidateId;

/// GEPA reflection input handed to the Git-program agentic proposer.
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct GitProgramGepaReflectionInput<Part = String> {
    /// Candidate being improved.
    pub parent: CandidateId,
    /// Parent artifact state materialized for the agent.
    pub artifact: GitProgramArtifact,
    /// Surface part selected by GEPA.
    pub part: Part,
    /// Human-readable selected-part label.
    pub part_label: String,
    /// Pre-built GEPA reflective examples.
    pub examples: Vec<ReflectiveCase>,
    /// Provenance refs lowered into the resulting proposal.
    pub source_refs: Vec<InfoRef>,
    /// GEPA proposal-attempt ordinal, when available.
    pub attempt_index: Option<usize>,
}

impl<Part> GitProgramGepaReflectionInput<Part> {
    /// Builds the agentic input from a pre-built GEPA reflection request and
    /// the parent artifact resolved from the run graph.
    #[must_use]
    pub fn from_request(artifact: GitProgramArtifact, request: ReflectRequest<Part>) -> Self {
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
            .chain(self.examples.iter().flat_map(|example| {
                example.source_refs.iter().cloned().chain(
                    example
                        .runs
                        .iter()
                        .flat_map(|run| run.source_refs.iter().cloned()),
                )
            }))
            .collect()
    }
}
