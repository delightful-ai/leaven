use leaven_agent::AgentSession;
use leaven_agentic::{AgenticParseError, ProposalParser};
use leaven_agentic_git::GitProgramReadback;
use leaven_artifact_git::GitProgramArtifact;
use leaven_core::{Artifact, OptimizationProblem, Proposal, ProposalBatch, ProposalBatchSemantics};
use leaven_engine::RunGraphView;
use leaven_kernel::{Cost, MetadataBag, Metered};
use leaven_workspace::WorkspaceView;

use crate::GitProgramGepaReflectionInput;

/// Parses a GEPA agent-edited Git program workspace into a proposal batch.
#[derive(Clone, Debug)]
pub struct GitProgramGepaReflectionParser {
    readback: GitProgramReadback,
}

impl GitProgramGepaReflectionParser {
    /// Constructs a parser from the Git program readback adapter.
    #[must_use]
    pub const fn new(readback: GitProgramReadback) -> Self {
        Self { readback }
    }

    /// Returns the underlying Git program readback adapter.
    #[must_use]
    pub const fn readback(&self) -> &GitProgramReadback {
        &self.readback
    }

    /// Parses a materialized Git-program reflection workspace into proposals.
    pub fn parse_workspace<P, Part>(
        &self,
        workspace: &mut WorkspaceView<'_>,
        input: &GitProgramGepaReflectionInput<Part>,
        graph: &RunGraphView<'_, P>,
    ) -> Result<Metered<ProposalBatch<P>>, AgenticParseError>
    where
        P: OptimizationProblem<Artifact = GitProgramArtifact>,
        P::ProposalAnnotations: Default,
        Part: Send + Sync,
    {
        let parent = graph
            .artifact(input.parent)
            .ok_or_else(|| AgenticParseError::Message("parent Git program not found".to_owned()))?;
        let change = self
            .readback
            .read_back_change(parent, workspace)
            .map_err(|source| {
                AgenticParseError::with_source("Git program reflection readback failed", source)
            })?
            .ok_or_else(|| {
                AgenticParseError::Message("Git program workspace had no changes".to_owned())
            })?;
        let _verified_child = parent.apply_change(&change).map_err(|source| {
            AgenticParseError::with_source("Git program readback change did not apply", source)
        })?;
        let proposal = Proposal::mutate(input.parent, change)
            .informed_by(input.informed_by())
            .build();
        Ok(Metered::new(
            ProposalBatch {
                proposals: vec![proposal],
                semantics: ProposalBatchSemantics::Alternatives,
                metadata: MetadataBag::new(),
            },
            Cost::zero(),
        ))
    }
}

impl<P, Part> ProposalParser<P, GitProgramGepaReflectionInput<Part>>
    for GitProgramGepaReflectionParser
where
    P: OptimizationProblem<Artifact = GitProgramArtifact>,
    P::ProposalAnnotations: Default,
    Part: Send + Sync,
{
    async fn parse_proposals(
        &self,
        workspace: &mut WorkspaceView<'_>,
        _session: &AgentSession,
        input: &GitProgramGepaReflectionInput<Part>,
        graph: RunGraphView<'_, P>,
    ) -> Result<Metered<ProposalBatch<P>>, AgenticParseError> {
        self.parse_workspace(workspace, input, &graph)
    }
}
