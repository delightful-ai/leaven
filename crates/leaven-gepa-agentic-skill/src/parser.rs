use leaven_agent::AgentSession;
use leaven_agentic::{AgenticParseError, ProposalParser};
use leaven_agentic_skill::{
    SkillBankProposalInput, SkillBankWorkspaceProposalParser, SkillWorkspaceLayout,
};
use leaven_artifact_skill::SkillBank;
use leaven_core::OptimizationProblem;
use leaven_engine::RunGraphView;
use leaven_kernel::Metered;
use leaven_workspace::WorkspaceView;

use crate::SkillBankGepaReflectionInput;

/// Parses a GEPA agent-edited skill workspace into a proposal batch.
#[derive(Clone, Debug, Default)]
pub struct SkillBankGepaReflectionParser {
    inner: SkillBankWorkspaceProposalParser,
}

impl SkillBankGepaReflectionParser {
    /// Constructs a parser with an explicit skill workspace layout.
    #[must_use]
    pub fn new(layout: SkillWorkspaceLayout) -> Self {
        Self {
            inner: SkillBankWorkspaceProposalParser::new(layout),
        }
    }

    /// Returns the layout used by this parser.
    #[must_use]
    pub const fn layout(&self) -> &SkillWorkspaceLayout {
        self.inner.layout()
    }
}

impl<P, Part> ProposalParser<P, SkillBankGepaReflectionInput<Part>>
    for SkillBankGepaReflectionParser
where
    P: OptimizationProblem<Artifact = SkillBank>,
    P::ProposalAnnotations: Default,
    Part: Send + Sync,
{
    async fn parse_proposals(
        &self,
        workspace: &mut WorkspaceView<'_>,
        session: &AgentSession,
        input: &SkillBankGepaReflectionInput<Part>,
        graph: RunGraphView<'_, P>,
    ) -> Result<Metered<leaven_core::ProposalBatch<P>>, AgenticParseError> {
        let skill_input = SkillBankProposalInput::new(input.parent);
        let mut parsed = self
            .inner
            .parse_proposals(workspace, session, &skill_input, graph)
            .await?;
        let informed_by = input.informed_by();
        for proposal in &mut parsed.value.proposals {
            proposal.provenance = proposal.provenance.clone().informed_by(informed_by.clone());
        }
        Ok(parsed)
    }
}
