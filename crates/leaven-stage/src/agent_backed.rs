use std::marker::PhantomData;
use std::sync::Arc;
use std::time::Duration;

use futures::future::BoxFuture;
use leaven_agent::{AgentInstructions, AgentRunContext, AgentRunRequest, AgentRuntime};
use leaven_core::OptimizationProblem;
use leaven_engine::{Arity, ProposalContext, ProposalError, Proposer, StageEngineContext};
use leaven_kernel::{
    AgentSessionId, Cost, Metered, ProposerId, StageAttemptFailure, StageAttemptOutcome,
};
use leaven_workspace::{
    FactoryError, Workspace, WorkspaceConfig, WorkspaceFactory, WorkspacePath, WorkspaceView,
    fingerprint_file,
};

use crate::parser::ErasedStagePlan;
use crate::receipt_store::InlineReceiptStore;
use crate::{
    AgentStageBootstrap, AgentStageCallContext, OutputEntryReceipt, OutputEntryStatus,
    ParseReceipt, ParseStatus, ProposerSlot, StageAttemptReceiptBuilder, StageOutputParser,
    StageReadAuthority, WorkspaceSetupError, setup_stage_workspace,
};

type WorkspaceAllocator =
    dyn Fn(WorkspaceConfig) -> BoxFuture<'static, Result<Workspace, FactoryError>> + Send + Sync;

pub struct AgentBacked<Slot, Runtime, Bootstrap, Parser> {
    pub workspace_factory: Arc<WorkspaceAllocator>,
    pub runtime: Runtime,
    pub bootstrap: Bootstrap,
    pub parser: Parser,
    pub policy: AgentBackedPolicy,
    receipt_store: InlineReceiptStore,
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
            receipt_store: InlineReceiptStore::default(),
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

    #[must_use]
    pub fn receipt_store(&self) -> InlineReceiptStore {
        self.receipt_store.clone()
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

        let attempt = self
            .run_attempt(
                AttemptEnv {
                    ctx: &ctx,
                    engine_ctx: &engine_ctx,
                    call_ctx: &call_ctx,
                    role: plan.role.clone(),
                    plan: &erased,
                },
                &mut workspace,
                &mut receipt,
            )
            .await;
        let parsed = match attempt {
            Ok(parsed) => parsed,
            Err(source) => {
                workspace.cleanup().await.map_err(|cleanup| {
                    ProposalError::with_source("agent stage workspace cleanup failed", cleanup)
                })?;
                return Err(source);
            }
        };

        let stage_receipt = receipt.finish(StageAttemptOutcome::Completed);
        let receipt_ref = self
            .receipt_store
            .write_sync(stage_receipt)
            .map_err(|source| {
                ProposalError::with_source("agent stage receipt write failed", source)
            })?;
        ctx.record_stage_attempt(plan.role, receipt_ref, StageAttemptOutcome::Completed);
        workspace.cleanup().await.map_err(|source| {
            ProposalError::with_source("agent stage workspace cleanup failed", source)
        })?;
        Ok(parsed)
    }
}

struct AttemptEnv<'a, 'g, P: OptimizationProblem> {
    ctx: &'a ProposalContext<'g, P>,
    engine_ctx: &'a StageEngineContext<'g, P>,
    call_ctx: &'a AgentStageCallContext,
    role: leaven_kernel::StageRole,
    plan: &'a ErasedStagePlan,
}

impl<Req, Runtime, Bootstrap, Parser> AgentBacked<ProposerSlot<Req>, Runtime, Bootstrap, Parser>
where
    Req: serde::Serialize + Send + Sync + 'static,
{
    #[allow(clippy::future_not_send)]
    async fn run_attempt<P>(
        &self,
        env: AttemptEnv<'_, '_, P>,
        workspace: &mut Workspace,
        receipt: &mut StageAttemptReceiptBuilder,
    ) -> Result<Metered<leaven_core::ProposalBatch<P>>, ProposalError>
    where
        P: OptimizationProblem,
        Runtime: AgentRuntime,
        Parser: StageOutputParser<P, ProposerSlot<Req>>,
    {
        self.setup_and_prewarm(&env, workspace, receipt)?;
        let session = self.run_runtime(&env, workspace, receipt).await?;
        for output in output_receipts(&workspace.view(), env.plan) {
            receipt.push_output(output);
        }
        self.parse_outputs(&env, workspace, receipt, &session).await
    }

    fn setup_and_prewarm<P>(
        &self,
        env: &AttemptEnv<'_, '_, P>,
        workspace: &mut Workspace,
        receipt: &mut StageAttemptReceiptBuilder,
    ) -> Result<(), ProposalError>
    where
        P: OptimizationProblem,
    {
        let mut slot = workspace.slot(WorkspacePath::root()).map_err(|source| {
            ProposalError::with_source("agent stage workspace slot failed", source)
        })?;
        let setup = match setup_stage_workspace(&mut slot, env.plan) {
            Ok(setup) => setup,
            Err(source) => {
                drop(slot);
                let receipt = std::mem::replace(receipt, empty_receipt());
                self.record_failed_attempt(
                    env.ctx,
                    env.role.clone(),
                    receipt,
                    StageAttemptFailure::WorkspaceSetup,
                )?;
                return Err(map_setup_error(
                    "agent stage workspace setup failed",
                    source,
                ));
            }
        };
        receipt.set_setup(setup);

        let mut read_authority =
            StageReadAuthority::new(env.engine_ctx.clone(), env.plan.query.clone());
        let prewarm = match read_authority.prewarm(&mut slot) {
            Ok(prewarm) => prewarm,
            Err(source) => {
                drop(slot);
                let receipt = std::mem::replace(receipt, empty_receipt());
                self.record_failed_attempt(
                    env.ctx,
                    env.role.clone(),
                    receipt,
                    StageAttemptFailure::Query,
                )?;
                return Err(ProposalError::with_source(
                    "agent stage prewarm query failed",
                    source,
                ));
            }
        };
        for query in prewarm {
            receipt.push_query(query.into_record());
        }
        Ok(())
    }

    #[allow(clippy::future_not_send)]
    async fn run_runtime<P>(
        &self,
        env: &AttemptEnv<'_, '_, P>,
        workspace: &mut Workspace,
        receipt: &mut StageAttemptReceiptBuilder,
    ) -> Result<Metered<leaven_agent::AgentSession>, ProposalError>
    where
        P: OptimizationProblem,
        Runtime: AgentRuntime,
    {
        let agent_request = agent_run_request(env.plan, self.policy.runtime_timeout);
        let budget = env.engine_ctx.budget().clone();
        let session_result = {
            let mut view = workspace.view();
            self.runtime
                .run_session(
                    &mut view,
                    agent_request,
                    AgentRunContext::new(AgentSessionId::new(), &budget),
                )
                .await
        };
        match session_result {
            Ok(session) => {
                receipt.add_cost(&session.cost);
                Ok(session)
            }
            Err(source) => {
                let receipt = std::mem::replace(receipt, empty_receipt());
                self.record_failed_attempt(
                    env.ctx,
                    env.role.clone(),
                    receipt,
                    StageAttemptFailure::Runtime,
                )?;
                Err(ProposalError::with_source(
                    "agent stage runtime failed",
                    source,
                ))
            }
        }
    }

    #[allow(clippy::future_not_send)]
    async fn parse_outputs<P>(
        &self,
        env: &AttemptEnv<'_, '_, P>,
        workspace: &mut Workspace,
        receipt: &mut StageAttemptReceiptBuilder,
        session: &Metered<leaven_agent::AgentSession>,
    ) -> Result<Metered<leaven_core::ProposalBatch<P>>, ProposalError>
    where
        P: OptimizationProblem,
        Parser: StageOutputParser<P, ProposerSlot<Req>>,
    {
        let parsed_result = {
            let mut view = workspace.view();
            self.parser
                .parse(&mut view, &session.value, env.plan, env.call_ctx.clone())
                .await
        };
        match parsed_result {
            Ok(parsed) => {
                receipt.add_cost(&parsed.cost);
                receipt.set_parse(ParseReceipt {
                    status: ParseStatus::Succeeded,
                    diagnostics: Vec::new(),
                    files_read: output_paths(env.plan),
                    cost: parsed.cost.clone(),
                });
                Ok(parsed)
            }
            Err(source) => {
                receipt.set_parse(ParseReceipt {
                    status: ParseStatus::Failed,
                    diagnostics: Vec::new(),
                    files_read: output_paths(env.plan),
                    cost: Cost::zero(),
                });
                let receipt = std::mem::replace(receipt, empty_receipt());
                self.record_failed_attempt(
                    env.ctx,
                    env.role.clone(),
                    receipt,
                    StageAttemptFailure::OutputParse,
                )?;
                Err(ProposalError::with_source(
                    "agent stage output parse failed",
                    source,
                ))
            }
        }
    }

    fn record_failed_attempt<P>(
        &self,
        ctx: &ProposalContext<'_, P>,
        role: leaven_kernel::StageRole,
        receipt: StageAttemptReceiptBuilder,
        failure: StageAttemptFailure,
    ) -> Result<(), ProposalError>
    where
        P: OptimizationProblem,
    {
        let outcome = StageAttemptOutcome::Failed(failure);
        let stage_receipt = receipt.finish(outcome.clone());
        let receipt_ref = self
            .receipt_store
            .write_sync(stage_receipt)
            .map_err(|source| {
                ProposalError::with_source("agent stage receipt write failed", source)
            })?;
        ctx.record_stage_attempt(role, receipt_ref, outcome);
        Ok(())
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

fn output_receipts(view: &WorkspaceView<'_>, plan: &ErasedStagePlan) -> Vec<OutputEntryReceipt> {
    plan.output
        .all_entries()
        .map(|entry| match fingerprint_file(view, &entry.path) {
            Ok(file) => OutputEntryReceipt {
                id: entry.id.clone(),
                path: entry.path.clone(),
                role: entry.role.clone(),
                fingerprint: Some(file.fingerprint),
                bytes: Some(file.bytes),
                file: Some(file),
                status: OutputEntryStatus::Present,
            },
            Err(_) => OutputEntryReceipt {
                id: entry.id.clone(),
                path: entry.path.clone(),
                role: entry.role.clone(),
                fingerprint: None,
                bytes: None,
                file: None,
                status: OutputEntryStatus::Missing,
            },
        })
        .collect()
}

fn output_paths(plan: &ErasedStagePlan) -> Vec<WorkspacePath> {
    plan.output
        .all_entries()
        .map(|entry| entry.path.clone())
        .collect()
}

fn empty_receipt() -> StageAttemptReceiptBuilder {
    StageAttemptReceiptBuilder::new(
        leaven_kernel::WorkspaceId::new(),
        leaven_kernel::StageCallId::new(),
        leaven_kernel::StageRole::new_static("abandoned"),
        leaven_kernel::Fingerprint::from_bytes([0; 32]),
    )
}

fn map_setup_error(message: &'static str, source: WorkspaceSetupError) -> ProposalError {
    ProposalError::with_source(message, source)
}
