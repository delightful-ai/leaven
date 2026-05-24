//! Agent-backed proposer adapter.

use std::marker::PhantomData;

use leaven_agent::{AgentInstructions, AgentRunContext, AgentRunRequest, AgentRuntime};
use leaven_core::{OptimizationProblem, ProposalBatch};
use leaven_engine::{Arity, Materializer, ProposalContext, ProposalError, Proposer, Renderer};
use leaven_kernel::{AgentSessionId, Cost, Metered, ProposerId};
use leaven_workspace::{WithWorkspaceError, WorkspaceConfig, WorkspaceFactory};

use crate::error::{checked_add_cost, map_workspace_error};
use crate::{AgentPromptTarget, AgenticAdapterError, AgenticRunInput, ProposalParser};

pub struct AgenticProposer<Factory, Runtime, Materialize, Render, Parse, Input> {
    config: AgenticProposerConfig,
    workspace_factory: Factory,
    runtime: Runtime,
    materializer: Materialize,
    renderer: Render,
    parser: Parse,
    marker: PhantomData<Input>,
}

impl<Factory, Runtime, Materialize, Render, Parse, Input>
    AgenticProposer<Factory, Runtime, Materialize, Render, Parse, Input>
{
    #[must_use]
    pub fn new(
        config: AgenticProposerConfig,
        workspace_factory: Factory,
        runtime: Runtime,
        materializer: Materialize,
        renderer: Render,
        parser: Parse,
    ) -> Self {
        Self {
            config,
            workspace_factory,
            runtime,
            materializer,
            renderer,
            parser,
            marker: PhantomData,
        }
    }
}

#[derive(Clone, Debug)]
pub struct AgenticProposerConfig {
    pub id: ProposerId,
    pub arity: Arity,
    pub workspace: WorkspaceConfig,
}

impl AgenticProposerConfig {
    #[must_use]
    pub fn new(id: ProposerId) -> Self {
        Self {
            id,
            arity: Arity::Single,
            workspace: WorkspaceConfig::default(),
        }
    }
}

impl<P, Factory, Runtime, Materialize, Render, Parse, Input> Proposer<P>
    for AgenticProposer<Factory, Runtime, Materialize, Render, Parse, Input>
where
    P: OptimizationProblem,
    Factory: WorkspaceFactory,
    Runtime: AgentRuntime,
    Materialize: Materializer<P, Input>,
    Render: Renderer<P, Input, AgentPromptTarget, View = AgentInstructions>,
    Parse: ProposalParser<P, Input>,
    Input: Send + Sync,
{
    type Request = AgenticRunInput<Input>;

    fn id(&self) -> ProposerId {
        self.config.id.clone()
    }

    fn arity(&self) -> Arity {
        self.config.arity
    }

    async fn propose(
        &self,
        request: Self::Request,
        mut ctx: ProposalContext<'_, P>,
    ) -> Result<Metered<ProposalBatch<P>>, ProposalError> {
        let mut workspace = self
            .workspace_factory
            .allocate(self.config.workspace.clone())
            .await
            .map_err(|error| {
                ProposalError::with_source(
                    "agentic proposer failed",
                    AgenticAdapterError::WorkspaceAllocate(error),
                )
            })?;
        let stage_result = async {
            let mut view = workspace.view();
            let materialized = self
                .materializer
                .materialize_into(&request.value, &mut view, ctx.materialize_context())
                .await
                .map_err(AgenticAdapterError::Materialize)?;
            let rendered = self
                .renderer
                .render(&request.value, AgentPromptTarget, ctx.render_context())
                .await
                .map_err(AgenticAdapterError::Render)?;
            let budget = ctx.budget();
            let session = self
                .runtime
                .run_session(
                    &mut view,
                    AgentRunRequest {
                        runtime: None,
                        runtime_fingerprint: None,
                        instructions: rendered.value,
                        cwd: request.cwd.clone(),
                        output_contract: request.output_contract.clone(),
                        env: request.env.clone(),
                        tool_policy: request.tool_policy.clone(),
                        limits: request.limits.clone(),
                    },
                    AgentRunContext::new(AgentSessionId::new(), &budget),
                )
                .await
                .map_err(AgenticAdapterError::Runtime)?;
            let parsed = self
                .parser
                .parse_proposals(
                    &mut view,
                    &session.value,
                    &request.value,
                    ctx.graph().clone(),
                )
                .await
                .map_err(AgenticAdapterError::Parse)?;
            let total = checked_add_cost(Cost::zero(), &materialized.cost)?;
            let total = checked_add_cost(total, &rendered.cost)?;
            let total = checked_add_cost(total, &session.cost)?;
            let total = checked_add_cost(total, &parsed.cost)?;
            drop(view);
            Ok(Metered::new(parsed.value, total))
        }
        .await;
        let cleanup_result = workspace.cleanup().await;
        match (stage_result, cleanup_result) {
            (Ok(metered), Ok(())) => Ok(metered),
            (Ok(_), Err(cleanup)) => Err(map_workspace_error(WithWorkspaceError::Cleanup(cleanup))),
            (Err(stage), Ok(())) => Err(stage),
            (Err(stage), Err(cleanup)) => {
                Err(map_workspace_error(WithWorkspaceError::StageAndCleanup {
                    stage,
                    cleanup,
                }))
            }
        }
        .map_err(|error| ProposalError::with_source("agentic proposer failed", error))
    }
}
