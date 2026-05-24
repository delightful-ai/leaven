//! Agent-backed evaluator adapter.

use std::marker::PhantomData;

use leaven_agent::{AgentInstructions, AgentRunContext, AgentRunRequest, AgentRuntime};
use leaven_core::{Assessment, OptimizationProblem, ResolvedEvaluationRequest};
use leaven_engine::{
    CachePolicy, EvaluationContext, EvaluationError, Evaluator, Materializer, Renderer,
};
use leaven_kernel::{AgentSessionId, Cost, EvaluatorId, Fingerprint, Metered};
use leaven_workspace::{WithWorkspaceError, WorkspaceConfig, WorkspaceFactory};

use crate::error::{checked_add_cost, map_workspace_error};
use crate::{AgentPromptTarget, AgenticAdapterError, EvaluationInputBuilder, EvidenceParser};

pub struct AgenticEvaluator<Factory, Runtime, Build, Materialize, Render, Parse, Input> {
    config: AgenticEvaluatorConfig,
    workspace_factory: Factory,
    runtime: Runtime,
    input_builder: Build,
    materializer: Materialize,
    renderer: Render,
    parser: Parse,
    marker: PhantomData<Input>,
}

impl<Factory, Runtime, Build, Materialize, Render, Parse, Input>
    AgenticEvaluator<Factory, Runtime, Build, Materialize, Render, Parse, Input>
{
    #[must_use]
    pub fn new(
        config: AgenticEvaluatorConfig,
        workspace_factory: Factory,
        runtime: Runtime,
        input_builder: Build,
        materializer: Materialize,
        renderer: Render,
        parser: Parse,
    ) -> Self {
        Self {
            config,
            workspace_factory,
            runtime,
            input_builder,
            materializer,
            renderer,
            parser,
            marker: PhantomData,
        }
    }
}

#[derive(Clone, Debug)]
pub struct AgenticEvaluatorConfig {
    pub id: EvaluatorId,
    pub fingerprint: Fingerprint,
    pub workspace: WorkspaceConfig,
    pub cache_policy: CachePolicy,
}

impl AgenticEvaluatorConfig {
    #[must_use]
    pub fn new(id: EvaluatorId, fingerprint: Fingerprint) -> Self {
        Self {
            id,
            fingerprint,
            workspace: WorkspaceConfig::default(),
            cache_policy: CachePolicy::Never,
        }
    }
}

impl<P, Factory, Runtime, Build, Materialize, Render, Parse, Input> Evaluator<P>
    for AgenticEvaluator<Factory, Runtime, Build, Materialize, Render, Parse, Input>
where
    P: OptimizationProblem,
    Factory: WorkspaceFactory,
    Runtime: AgentRuntime,
    Build: EvaluationInputBuilder<P, Input>,
    Materialize: Materializer<P, Input>,
    Render: Renderer<P, Input, AgentPromptTarget, View = AgentInstructions>,
    Parse: EvidenceParser<P, Input>,
    Input: Send + Sync,
{
    fn id(&self) -> EvaluatorId {
        self.config.id.clone()
    }

    fn fingerprint(&self) -> Fingerprint {
        self.config.fingerprint
    }

    fn cache_policy(&self, _request: &ResolvedEvaluationRequest) -> CachePolicy {
        self.config.cache_policy.clone()
    }

    async fn evaluate(
        &self,
        request: ResolvedEvaluationRequest,
        mut ctx: EvaluationContext<'_, P>,
    ) -> Result<Metered<Vec<Assessment<P>>>, EvaluationError> {
        let inputs = self
            .input_builder
            .build_inputs(&request, ctx.graph().clone())
            .map_err(|error| EvaluationError::with_source("agentic evaluator failed", error))?;
        let mut total = Cost::zero();
        let mut assessments = Vec::new();

        for input in inputs {
            let mut workspace = self
                .workspace_factory
                .allocate(self.config.workspace.clone())
                .await
                .map_err(|error| {
                    EvaluationError::with_source(
                        "agentic evaluator failed",
                        AgenticAdapterError::WorkspaceAllocate(error),
                    )
                })?;
            let stage_result = async {
                let mut view = workspace.view();
                let materialized = self
                    .materializer
                    .materialize_into(&input.value, &mut view, ctx.materialize_context())
                    .await
                    .map_err(AgenticAdapterError::Materialize)?;
                let rendered = self
                    .renderer
                    .render(&input.value, AgentPromptTarget, ctx.render_context())
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
                            cwd: input.cwd.clone(),
                            output_contract: input.output_contract.clone(),
                            env: input.env.clone(),
                            tool_policy: input.tool_policy.clone(),
                            limits: input.limits.clone(),
                        },
                        AgentRunContext::new(AgentSessionId::new(), &budget),
                    )
                    .await
                    .map_err(AgenticAdapterError::Runtime)?;
                let parsed = self
                    .parser
                    .parse_evidence(
                        &mut view,
                        &session.value,
                        &input.value,
                        &request,
                        ctx.graph().clone(),
                    )
                    .await
                    .map_err(AgenticAdapterError::Parse)?;
                let run_total = checked_add_cost(Cost::zero(), &materialized.cost)?;
                let run_total = checked_add_cost(run_total, &rendered.cost)?;
                let run_total = checked_add_cost(run_total, &session.cost)?;
                let run_total = checked_add_cost(run_total, &parsed.cost)?;
                drop(view);
                Ok(Metered::new(parsed.value, run_total))
            }
            .await;
            let cleanup_result = workspace.cleanup().await;
            let metered = match (stage_result, cleanup_result) {
                (Ok(metered), Ok(())) => Ok(metered),
                (Ok(_), Err(cleanup)) => {
                    Err(map_workspace_error(WithWorkspaceError::Cleanup(cleanup)))
                }
                (Err(stage), Ok(())) => Err(stage),
                (Err(stage), Err(cleanup)) => {
                    Err(map_workspace_error(WithWorkspaceError::StageAndCleanup {
                        stage,
                        cleanup,
                    }))
                }
            }
            .map_err(|error| EvaluationError::with_source("agentic evaluator failed", error))?;
            total = checked_add_cost(total, &metered.cost)
                .map_err(|error| EvaluationError::with_source("agentic evaluator failed", error))?;
            assessments.extend(metered.value);
        }

        Ok(Metered::new(assessments, total))
    }
}
