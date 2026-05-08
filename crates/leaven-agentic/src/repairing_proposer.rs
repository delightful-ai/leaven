//! Agent-backed proposer adapter with bounded proposal repair.

use std::marker::PhantomData;
use std::num::NonZeroUsize;

use leaven_agent::{AgentInstructions, AgentRunContext, AgentRunRequest, AgentRuntime};
use leaven_core::{OptimizationProblem, ProposalBatch};
use leaven_engine::{Arity, Materializer, ProposalContext, ProposalError, Proposer, Renderer};
use leaven_kernel::{AgentSessionId, Cost, MetadataValue, Metered, ProposerId};
use leaven_workspace::{WithWorkspaceError, WorkspaceConfig, WorkspaceFactory};

use crate::error::{checked_add_cost, map_workspace_error};
use crate::{
    AgentPromptTarget, AgenticAdapterError, AgenticRepairError, AgenticRunInput,
    PROPOSAL_REPAIR_ATTEMPTS_METADATA_KEY, ProposalParser, ProposalRepairAttemptOutcome,
    ProposalRepairAttemptRecord, ProposalRepairFeedback, ProposalRepairPolicy,
    ProposalRepairPromptBuilder,
};

pub struct RepairingAgenticProposer<Factory, Runtime, Materialize, Render, Repair, Parse, Input> {
    config: RepairingAgenticProposerConfig,
    workspace_factory: Factory,
    runtime: Runtime,
    materializer: Materialize,
    renderer: Render,
    repair: Repair,
    parser: Parse,
    marker: PhantomData<Input>,
}

impl<Factory, Runtime, Materialize, Render, Repair, Parse, Input>
    RepairingAgenticProposer<Factory, Runtime, Materialize, Render, Repair, Parse, Input>
{
    #[must_use]
    pub fn new(
        config: RepairingAgenticProposerConfig,
        workspace_factory: Factory,
        runtime: Runtime,
        materializer: Materialize,
        renderer: Render,
        repair: Repair,
        parser: Parse,
    ) -> Self {
        Self {
            config,
            workspace_factory,
            runtime,
            materializer,
            renderer,
            repair,
            parser,
            marker: PhantomData,
        }
    }
}

#[derive(Clone, Debug)]
pub struct RepairingAgenticProposerConfig {
    pub id: ProposerId,
    pub arity: Arity,
    pub workspace: WorkspaceConfig,
    pub repair: ProposalRepairPolicy,
}

impl RepairingAgenticProposerConfig {
    #[must_use]
    pub fn new(id: ProposerId, max_attempts: NonZeroUsize) -> Self {
        Self {
            id,
            arity: Arity::Single,
            workspace: WorkspaceConfig::default(),
            repair: ProposalRepairPolicy::new(max_attempts),
        }
    }
}

impl<P, Factory, Runtime, Materialize, Render, Repair, Parse, Input> Proposer<P>
    for RepairingAgenticProposer<Factory, Runtime, Materialize, Render, Repair, Parse, Input>
where
    P: OptimizationProblem,
    Factory: WorkspaceFactory,
    Runtime: AgentRuntime,
    Materialize: Materializer<P, Input>,
    Render: Renderer<P, Input, AgentPromptTarget, View = AgentInstructions>,
    Repair: ProposalRepairPromptBuilder<Input>,
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
            let mut total = checked_add_cost(Cost::zero(), &materialized.cost)?;
            total = checked_add_cost(total, &rendered.cost)?;
            let mut instructions = rendered.value;
            let max_attempts = self.config.repair.max_attempts.get();
            let mut records = Vec::new();

            for attempt in 1..=max_attempts {
                let budget = ctx.budget();
                let session = self
                    .runtime
                    .run_session(
                        &mut view,
                        AgentRunRequest {
                            instructions: instructions.clone(),
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
                total = checked_add_cost(total, &session.cost)?;
                match self
                    .parser
                    .parse_proposals(
                        &mut view,
                        &session.value,
                        &request.value,
                        ctx.graph().clone(),
                    )
                    .await
                {
                    Ok(parsed) => {
                        total = checked_add_cost(total, &parsed.cost)?;
                        records.push(ProposalRepairAttemptRecord {
                            attempt,
                            session: session.value.session_id,
                            outcome: ProposalRepairAttemptOutcome::Accepted,
                        });
                        let mut batch = parsed.value;
                        attach_repair_metadata(&mut batch, &records)?;
                        drop(view);
                        return Ok(Metered::new(batch, total));
                    }
                    Err(parse_error) if attempt < max_attempts => {
                        records.push(ProposalRepairAttemptRecord {
                            attempt,
                            session: session.value.session_id,
                            outcome: ProposalRepairAttemptOutcome::ParseFailed {
                                error: parse_error.to_string(),
                            },
                        });
                        let failed_attempt =
                            NonZeroUsize::new(attempt).expect("attempt starts at one");
                        instructions = self.repair.build_repair(
                            &request.value,
                            ProposalRepairFeedback {
                                failed_attempt,
                                max_attempts: self.config.repair.max_attempts,
                                parse_error: &parse_error,
                                previous_session: &session.value,
                            },
                        )?;
                    }
                    Err(parse_error) => {
                        records.push(ProposalRepairAttemptRecord {
                            attempt,
                            session: session.value.session_id,
                            outcome: ProposalRepairAttemptOutcome::ParseFailed {
                                error: parse_error.to_string(),
                            },
                        });
                        return Err(AgenticAdapterError::Repair(AgenticRepairError::Exhausted {
                            attempts: max_attempts,
                            records,
                            source: Box::new(parse_error),
                        }));
                    }
                }
            }

            unreachable!("repair policy always has at least one attempt")
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

fn attach_repair_metadata<P: OptimizationProblem>(
    batch: &mut ProposalBatch<P>,
    records: &[ProposalRepairAttemptRecord],
) -> Result<(), AgenticAdapterError> {
    let value = serde_json::to_value(records)
        .map_err(|error| AgenticAdapterError::Input(error.to_string()))?;
    batch.metadata.insert(
        PROPOSAL_REPAIR_ATTEMPTS_METADATA_KEY,
        MetadataValue::Json(value),
    );
    Ok(())
}
