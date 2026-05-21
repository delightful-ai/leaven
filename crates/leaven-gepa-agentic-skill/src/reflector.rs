use std::fmt::Debug;
use std::marker::PhantomData;

use leaven_agent::{AgentRuntime, OutputContract};
use leaven_agentic::{AgenticProposer, AgenticProposerConfig, AgenticRunInput};
use leaven_agentic_skill::SkillWorkspaceLayout;
use leaven_artifact_skill::SkillBank;
use leaven_core::OptimizationProblem;
use leaven_engine::{OptimizerError, RunContext};
use leaven_gepa::{GepaReflector, ReflectRequest};
use leaven_kernel::CandidateId;
use leaven_surface::EditSurface;
use leaven_workspace::{WorkspaceFactory, WorkspacePath};

use crate::{
    GepaSkillBankReflectionRenderer, SkillBankGepaReflectionInput,
    SkillBankGepaReflectionMaterializer, SkillBankGepaReflectionParser,
};

/// GEPA reflector that runs the skill-bank reflection proposal stage through
/// the materializing `AgenticProposer` path.
pub struct GepaSkillBankAgenticReflector<Factory, Runtime, Part> {
    proposer: AgenticProposer<
        Factory,
        Runtime,
        SkillBankGepaReflectionMaterializer,
        GepaSkillBankReflectionRenderer,
        SkillBankGepaReflectionParser,
        SkillBankGepaReflectionInput<Part>,
    >,
    layout: SkillWorkspaceLayout,
    marker: PhantomData<Part>,
}

impl<Factory, Runtime, Part> GepaSkillBankAgenticReflector<Factory, Runtime, Part>
where
    Factory: WorkspaceFactory,
    Runtime: AgentRuntime,
{
    /// Constructs the skill-bank agentic GEPA reflector.
    #[must_use]
    pub fn new(
        config: AgenticProposerConfig,
        workspace_factory: Factory,
        runtime: Runtime,
        layout: SkillWorkspaceLayout,
    ) -> Self {
        Self {
            proposer: AgenticProposer::new(
                config,
                workspace_factory,
                runtime,
                SkillBankGepaReflectionMaterializer::new(layout.clone()),
                GepaSkillBankReflectionRenderer::new(layout.clone()),
                SkillBankGepaReflectionParser::new(layout.clone()),
            ),
            layout,
            marker: PhantomData,
        }
    }

    /// Returns the skill workspace layout used by the reflector.
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
        let input = SkillBankGepaReflectionInput::from_request(artifact, request);
        let batch = ctx
            .propose(
                &self.proposer,
                AgenticRunInput::new(
                    input,
                    OutputContract::WorkspaceDiff {
                        roots: vec![output_root(&self.layout)],
                    },
                ),
            )
            .await
            .map_err(|source| {
                OptimizerError::with_source("GEPA skill-bank agentic reflection failed", source)
            })?;
        let applied = ctx.apply_batch(batch.batch_id).map_err(|source| {
            OptimizerError::with_source("GEPA skill-bank proposal application failed", source)
        })?;
        Ok(applied.successful_candidates().next())
    }
}

fn output_root(layout: &SkillWorkspaceLayout) -> WorkspacePath {
    if layout.skills_root.as_str().is_empty() {
        WorkspacePath::root()
    } else {
        layout.skills_root.clone()
    }
}
