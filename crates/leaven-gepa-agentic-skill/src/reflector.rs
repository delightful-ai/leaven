use std::fmt::Debug;
use std::marker::PhantomData;

use leaven_agent::AgentRuntime;
use leaven_agentic::{
    AgenticProposerConfig, ReadbackResult, ReflectionWorkspace,
};
use leaven_agentic_skill::SkillWorkspaceLayout;
use leaven_artifact_skill::SkillBank;
use leaven_core::{OptimizationProblem, Proposal, ProposalBatch, ProposalBatchSemantics};
use leaven_engine::{OptimizerError, Proposer, ProposalContext, ProposalError, RunContext};
use leaven_gepa::{GepaReflector, ReflectRequest};
use leaven_kernel::{CandidateId, Cost, MetadataBag, Metered, ProposerId};
use leaven_surface::EditSurface;
use leaven_workspace::WorkspaceFactory;

use crate::{SkillBankReflectionInput, SkillBankReflector};

/// GEPA reflector that runs skill-bank reflection through the generic
/// materialized-workspace contract.
pub struct GepaSkillBankAgenticReflector<Factory, Runtime, Part> {
    id: ProposerId,
    workspace: ReflectionWorkspace,
    skill_reflector: SkillBankReflector<Part>,
    workspace_factory: Factory,
    runtime: Runtime,
    layout: SkillWorkspaceLayout,
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
        Self {
            id: config.id,
            workspace: ReflectionWorkspace::default(),
            skill_reflector: SkillBankReflector::new(layout.clone()),
            workspace_factory,
            runtime,
            layout,
            marker: PhantomData,
        }
    }

    #[must_use]
    pub const fn layout(&self) -> &SkillWorkspaceLayout {
        &self.layout
    }
}

impl<P, S, Factory, Runtime> GepaReflector<P, S>
    for GepaSkillBankAgenticReflector<Factory, Runtime, S::PartId>
where
    P: OptimizationProblem<Artifact = SkillBank>,
    P::ProposalAnnotations: Default,
    S: EditSurface<SkillBank> + Send + Sync,
    S::PartId: Clone + Debug + Send + Sync,
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
        let cases = request.examples.clone();
        let source_refs = request.informed_by();
        let input = SkillBankReflectionInput::from_request(artifact, request);
        let outcome = self
            .workspace
            .run(
                &self.skill_reflector,
                &input,
                &cases,
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
            ReadbackResult::Valid(change) => change,
            ReadbackResult::Empty => return Ok(None),
            ReadbackResult::Invalid { diagnostics } => {
                return Err(OptimizerError::Message(format!(
                    "GEPA skill-bank readback was invalid: {diagnostics:?}"
                )));
            }
        };

        let proposer = PrebuiltSkillProposal {
            id: self.id.clone(),
            parent: input.parent,
            change,
            informed_by: input.informed_by(),
            cost: outcome.cost,
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
