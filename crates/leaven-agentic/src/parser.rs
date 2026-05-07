//! Parser and input-builder traits for agentic stages.

use std::collections::BTreeMap;
use std::future::Future;

use leaven_agent::{AgentLimits, AgentSession, AgentToolPolicy, OutputContract};
use leaven_core::{Assessment, OptimizationProblem, ProposalBatch, ResolvedEvaluationRequest};
use leaven_engine::RunGraphView;
use leaven_kernel::Metered;
use leaven_workspace::{WorkspacePath, WorkspaceView};

use crate::{AgenticAdapterError, AgenticParseError};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgenticRunInput<I> {
    pub value: I,
    pub cwd: WorkspacePath,
    pub output_contract: OutputContract,
    pub env: BTreeMap<String, String>,
    pub tool_policy: AgentToolPolicy,
    pub limits: AgentLimits,
}

impl<I> AgenticRunInput<I> {
    #[must_use]
    pub fn new(value: I, output_contract: OutputContract) -> Self {
        Self {
            value,
            cwd: WorkspacePath::root(),
            output_contract,
            env: BTreeMap::new(),
            tool_policy: AgentToolPolicy::default(),
            limits: AgentLimits::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AgentPromptTarget;

pub trait ProposalParser<P, I>: Send + Sync
where
    P: OptimizationProblem,
{
    fn parse_proposals<'a>(
        &'a self,
        workspace: &'a mut WorkspaceView<'_>,
        session: &'a AgentSession,
        input: &'a I,
        graph: RunGraphView<'a, P>,
    ) -> impl Future<Output = Result<Metered<ProposalBatch<P>>, AgenticParseError>> + Send + 'a;
}

pub trait EvidenceParser<P, I>: Send + Sync
where
    P: OptimizationProblem,
{
    fn parse_evidence<'a>(
        &'a self,
        workspace: &'a mut WorkspaceView<'_>,
        session: &'a AgentSession,
        input: &'a I,
        request: &'a ResolvedEvaluationRequest,
        graph: RunGraphView<'a, P>,
    ) -> impl Future<Output = Result<Metered<Vec<Assessment<P>>>, AgenticParseError>> + Send + 'a;
}

pub trait EvaluationInputBuilder<P, I>: Send + Sync
where
    P: OptimizationProblem,
{
    fn build_inputs(
        &self,
        request: &ResolvedEvaluationRequest,
        graph: RunGraphView<'_, P>,
    ) -> Result<Vec<AgenticRunInput<I>>, AgenticAdapterError>;
}
