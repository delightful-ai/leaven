use std::marker::PhantomData;
use std::sync::Arc;
use std::time::Duration;

use futures::future::BoxFuture;
use leaven_agent::{AgentInstructions, AgentRunContext, AgentRunRequest, AgentRuntime};
use leaven_core::OptimizationProblem;
use leaven_engine::{Arity, ProposalContext, ProposalError, Proposer};
use leaven_kernel::{
    AgentSessionId, FingerprintBuilder, Metered, ProposerId, StageAttemptOutcome,
    StageAttemptReceiptRef,
};
use leaven_workspace::{FactoryError, Workspace, WorkspaceConfig, WorkspaceFactory, WorkspacePath};

use crate::parser::ErasedStagePlan;
use crate::{
    AgentStageBootstrap, AgentStageCallContext, ProposerSlot, StageAttemptReceipt,
    StageAttemptReceiptBuilder, StageOutputParser, StageReadAuthority, WorkspaceSetupError,
    setup_stage_workspace,
};

type WorkspaceAllocator =
    dyn Fn(WorkspaceConfig) -> BoxFuture<'static, Result<Workspace, FactoryError>> + Send + Sync;

pub struct AgentBacked<Slot, Runtime, Bootstrap, Parser> {
    pub workspace_factory: Arc<WorkspaceAllocator>,
    pub runtime: Runtime,
    pub bootstrap: Bootstrap,
    pub parser: Parser,
    pub policy: AgentBackedPolicy,
    _marker: PhantomData<Slot>,
}

impl<Slot, Runtime, Bootstrap, Parser> AgentBacked<Slot, Runtime, Bootstrap, Parser> {
    #[must_use]
    pub fn new(
        workspace_factory: Arc<WorkspaceAllocator>,
        runtime: Runtime,
        bootstrap: Bootstrap,
        parser: Parser,
        policy: AgentBackedPolicy,
    ) -> Self {
        Self {
            workspace_factory,
            runtime,
            bootstrap,
            parser,
            policy,
            _marker: PhantomData,
        }
    }

    #[must_use]
    pub fn from_factory<Factory>(
        workspace_factory: Factory,
        runtime: Runtime,
        bootstrap: Bootstrap,
        parser: Parser,
        policy: AgentBackedPolicy,
    ) -> Self
    where
        Factory: WorkspaceFactory + Send + Sync + 'static,
    {
        let workspace_factory = Arc::new(workspace_factory);
        Self::new(
            Arc::new(move |config| {
                let workspace_factory = Arc::clone(&workspace_factory);
                Box::pin(async move { workspace_factory.allocate(config).await })
            }),
            runtime,
            bootstrap,
            parser,
            policy,
        )
    }
}

impl<P, Req, Runtime, Bootstrap, Parser> Proposer<P>
    for AgentBacked<ProposerSlot<Req>, Runtime, Bootstrap, Parser>
where
    P: OptimizationProblem,
    Req: serde::Serialize + Send + Sync + 'static,
    Runtime: AgentRuntime,
    Bootstrap: AgentStageBootstrap<P, ProposerSlot<Req>>,
    Parser: StageOutputParser<P, ProposerSlot<Req>>,
{
    type Request = Req;

    fn id(&self) -> ProposerId {
        ProposerId::from("agent-backed")
    }

    fn arity(&self) -> Arity {
        Arity::Single
    }

    async fn propose(
        &self,
        request: Self::Request,
        ctx: ProposalContext<'_, P>,
    ) -> Result<Metered<leaven_core::ProposalBatch<P>>, ProposalError> {
        let engine_ctx = ctx.stage_engine_context();
        let call_ctx = AgentStageCallContext::from_engine(&engine_ctx);
        let plan = self
            .bootstrap
            .plan(request, call_ctx.clone())
            .await
            .map_err(|source| ProposalError::with_source("agent stage bootstrap failed", source))?;
        plan.output.validate().map_err(|source| {
            ProposalError::with_source("agent stage output contract invalid", source)
        })?;
        let erased = ErasedStagePlan::from_plan(&plan).map_err(|source| {
            ProposalError::with_source("agent stage plan serialization failed", source)
        })?;
        let mut workspace = (self.workspace_factory)(self.policy.workspace.clone())
            .await
            .map_err(|source| {
                ProposalError::with_source("agent stage workspace allocation failed", source)
            })?;
        let mut receipt = StageAttemptReceiptBuilder::new(
            workspace.id(),
            engine_ctx.stage_call_id(),
            plan.role.clone(),
            erased.fingerprint,
        );

        let parsed = {
            let mut slot = workspace.slot(WorkspacePath::root()).map_err(|source| {
                ProposalError::with_source("agent stage workspace slot failed", source)
            })?;
            let setup = setup_stage_workspace(&mut slot, &erased)
                .map_err(|source| map_setup_error("agent stage workspace setup failed", source))?;
            receipt.set_setup(setup);
            let mut read_authority =
                StageReadAuthority::new(engine_ctx.clone(), plan.query.clone());
            for query in read_authority.prewarm(&mut slot).map_err(|source| {
                ProposalError::with_source("agent stage prewarm query failed", source)
            })? {
                receipt.push_query(query.into_record());
            }
            drop(slot);

            let agent_request = agent_run_request(&erased, self.policy.runtime_timeout);
            let budget = engine_ctx.budget().clone();
            let session = self
                .runtime
                .run_session(
                    &mut workspace.view(),
                    agent_request,
                    AgentRunContext::new(AgentSessionId::new(), &budget),
                )
                .await
                .map_err(|source| {
                    ProposalError::with_source("agent stage runtime failed", source)
                })?;
            receipt.add_cost(&session.cost);
            let parsed = self
                .parser
                .parse(
                    &mut workspace.view(),
                    &session.value,
                    &erased,
                    call_ctx.clone(),
                )
                .await
                .map_err(|source| {
                    ProposalError::with_source("agent stage output parse failed", source)
                })?;
            receipt.add_cost(&parsed.cost);
            parsed
        };

        let stage_receipt = receipt.finish(StageAttemptOutcome::Completed);
        ctx.record_stage_attempt(
            plan.role,
            receipt_ref(&stage_receipt),
            StageAttemptOutcome::Completed,
        );
        workspace.cleanup().await.map_err(|source| {
            ProposalError::with_source("agent stage workspace cleanup failed", source)
        })?;
        Ok(parsed)
    }
}

#[derive(Clone, Debug)]
pub struct AgentBackedPolicy {
    pub workspace: WorkspaceConfig,
    pub runtime_timeout: Option<Duration>,
    pub on_parse_failure: ParseFailurePolicy,
    pub receipt_sink: ReceiptSinkPolicy,
}

impl Default for AgentBackedPolicy {
    fn default() -> Self {
        Self {
            workspace: WorkspaceConfig::default(),
            runtime_timeout: None,
            on_parse_failure: ParseFailurePolicy::Strict,
            receipt_sink: ReceiptSinkPolicy::Inline,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParseFailurePolicy {
    Strict,
    RecordAttempt,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReceiptSinkPolicy {
    Inline,
    External { sink: String },
}

fn agent_run_request(plan: &ErasedStagePlan, timeout: Option<Duration>) -> AgentRunRequest {
    let mut request = AgentRunRequest::new(
        AgentInstructions::task(render_task(plan)),
        plan.output.to_agent_output_contract(),
    );
    request.limits.timeout = timeout;
    request
}

fn render_task(plan: &ErasedStagePlan) -> String {
    let mut task = format!(
        "{}\n\n{}\n\nWrite required outputs under output/.",
        plan.directive.title, plan.directive.instructions
    );
    if !plan.directive.success_criteria.is_empty() {
        task.push_str("\n\nSuccess criteria:\n");
        for criterion in &plan.directive.success_criteria {
            task.push_str("- ");
            task.push_str(criterion);
            task.push('\n');
        }
    }
    task
}

fn receipt_ref(receipt: &StageAttemptReceipt) -> StageAttemptReceiptRef {
    let bytes = serde_json::to_vec(receipt).expect("stage attempt receipts are serializable");
    let mut fingerprint = FingerprintBuilder::new();
    fingerprint
        .update(b"leaven.stage.attempt-receipt.v1")
        .update(bytes);
    StageAttemptReceiptRef {
        id: receipt.receipt_id,
        fingerprint: Some(fingerprint.finish()),
    }
}

fn map_setup_error(message: &'static str, source: WorkspaceSetupError) -> ProposalError {
    ProposalError::with_source(message, source)
}
