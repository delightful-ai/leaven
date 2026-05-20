use std::fmt::Debug;
use std::marker::PhantomData;

use leaven_agent::AgentRuntime;
use leaven_agentic::{
    AgenticProposerConfig, ReadbackDiagnostic, ReadbackResult, ReflectionWorkspace,
};
use leaven_agentic_skill::SkillWorkspaceLayout;
use leaven_artifact_skill::SkillBank;
use leaven_core::{OptimizationProblem, Proposal, ProposalBatch, ProposalBatchSemantics};
use leaven_engine::{Arity, OptimizerError, ProposalContext, ProposalError, Proposer, RunContext};
use leaven_gepa::{GepaReflector, ReflectRequest};
use leaven_kernel::{CandidateId, Cost, MetadataBag, Metered, ProposerId, StageId};
use leaven_surface::EditSurface;
use leaven_workspace::WorkspaceFactory;

use crate::{SkillBankReflectionInput, SkillBankReflector, SkillPartScope};

/// GEPA reflector that runs skill-bank reflection through the generic
/// materialized-workspace contract.
pub struct GepaSkillBankAgenticReflector<Factory, Runtime, Part> {
    id: ProposerId,
    arity: Arity,
    workspace: ReflectionWorkspace,
    skill_reflector: SkillBankReflector<Part>,
    workspace_factory: Factory,
    runtime: Runtime,
    layout: SkillWorkspaceLayout,
    last_invalid_readback_diagnostics: Vec<ReadbackDiagnostic>,
    marker: PhantomData<Part>,
}

impl<Factory, Runtime, Part> GepaSkillBankAgenticReflector<Factory, Runtime, Part>
where
    Factory: WorkspaceFactory,
    Runtime: AgentRuntime,
{
    #[must_use]
    pub fn new(
        config: AgenticProposerConfig,
        workspace_factory: Factory,
        runtime: Runtime,
        layout: SkillWorkspaceLayout,
    ) -> Self {
        let workspace =
            ReflectionWorkspace::default().with_workspace_config(config.workspace.clone());
        Self {
            id: config.id,
            arity: config.arity,
            workspace,
            skill_reflector: SkillBankReflector::new(layout.clone()),
            workspace_factory,
            runtime,
            layout,
            last_invalid_readback_diagnostics: Vec::new(),
            marker: PhantomData,
        }
    }

    #[must_use]
    pub const fn layout(&self) -> &SkillWorkspaceLayout {
        &self.layout
    }

    #[must_use]
    pub fn last_invalid_readback_diagnostics(&self) -> &[ReadbackDiagnostic] {
        &self.last_invalid_readback_diagnostics
    }
}

impl<P, S, Factory, Runtime> GepaReflector<P, S>
    for GepaSkillBankAgenticReflector<Factory, Runtime, S::PartId>
where
    P: OptimizationProblem<Artifact = SkillBank>,
    P::ProposalAnnotations: Default,
    S: EditSurface<SkillBank> + Send + Sync,
    S::PartId: SkillPartScope + Clone + Debug + Send + Sync,
    Factory: WorkspaceFactory,
    Runtime: AgentRuntime,
    Self: Send + Sync,
{
    async fn reflect_candidate(
        &mut self,
        ctx: &mut RunContext<'_, P>,
        _surface: &S,
        request: ReflectRequest<S::PartId>,
    ) -> Result<Option<CandidateId>, OptimizerError> {
        let artifact = ctx
            .graph()
            .artifact(request.parent)
            .ok_or_else(|| {
                OptimizerError::Message(format!(
                    "selected parent {} is missing from graph",
                    request.parent
                ))
            })?
            .clone();
        let source_refs = request.informed_by();
        let input = SkillBankReflectionInput::from_request(artifact, request);
        let outcome = self
            .workspace
            .run(
                &self.skill_reflector,
                &input,
                &input.examples,
                &source_refs,
                &self.workspace_factory,
                &self.runtime,
                &ctx.budget(),
            )
            .await
            .map_err(|source| {
                OptimizerError::with_source("GEPA skill-bank agentic reflection failed", source)
            })?;

        let change = match outcome.readback {
            ReadbackResult::Valid(change) => {
                self.last_invalid_readback_diagnostics.clear();
                change
            }
            ReadbackResult::Empty => {
                self.last_invalid_readback_diagnostics.clear();
                ctx.charge(StageId::from_proposer(self.id.clone()), outcome.cost)
                    .map_err(|source| {
                        OptimizerError::with_source(
                            "GEPA skill-bank empty reflection cost charge failed",
                            source,
                        )
                    })?;
                return Ok(None);
            }
            ReadbackResult::Invalid { diagnostics } => {
                self.last_invalid_readback_diagnostics = diagnostics;
                ctx.charge(StageId::from_proposer(self.id.clone()), outcome.cost)
                    .map_err(|source| {
                        OptimizerError::with_source(
                            "GEPA skill-bank invalid reflection cost charge failed",
                            source,
                        )
                    })?;
                return Ok(None);
            }
        };

        let proposer = PrebuiltSkillProposal {
            id: self.id.clone(),
            parent: input.parent,
            change,
            informed_by: source_refs,
            cost: outcome.cost,
            arity: self.arity,
            marker: PhantomData,
        };
        let batch = ctx.propose(&proposer, ()).await.map_err(|source| {
            OptimizerError::with_source("GEPA skill-bank proposal recording failed", source)
        })?;
        let applied = ctx.apply_batch(batch.batch_id).map_err(|source| {
            OptimizerError::with_source("GEPA skill-bank proposal application failed", source)
        })?;
        Ok(applied.successful_candidates().next())
    }
}

struct PrebuiltSkillProposal<P>
where
    P: OptimizationProblem<Artifact = SkillBank>,
{
    id: ProposerId,
    parent: CandidateId,
    change: leaven_artifact_skill::SkillBankChange,
    informed_by: Vec<leaven_core::InfoRef>,
    cost: Cost,
    arity: Arity,
    marker: PhantomData<P>,
}

impl<P> Proposer<P> for PrebuiltSkillProposal<P>
where
    P: OptimizationProblem<Artifact = SkillBank>,
    P::ProposalAnnotations: Default,
{
    type Request = ();

    fn id(&self) -> ProposerId {
        self.id.clone()
    }

    fn arity(&self) -> Arity {
        self.arity
    }

    async fn propose(
        &self,
        _request: Self::Request,
        _ctx: ProposalContext<'_, P>,
    ) -> Result<Metered<ProposalBatch<P>>, ProposalError> {
        Ok(Metered::new(
            ProposalBatch {
                proposals: vec![
                    Proposal::mutate(self.parent, self.change.clone())
                        .informed_by(self.informed_by.clone())
                        .build(),
                ],
                semantics: ProposalBatchSemantics::Alternatives,
                metadata: MetadataBag::new(),
            },
            self.cost.clone(),
        ))
    }
}
