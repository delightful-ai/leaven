use std::fmt::Debug;
use std::marker::PhantomData;

use leaven_agent::{AgentRuntime, OutputContract};
use leaven_agentic::{AgenticProposer, AgenticProposerConfig, AgenticRunInput};
use leaven_agentic_git::{GitProgramMaterializer, GitProgramReadback};
use leaven_artifact_git::GitProgramArtifact;
use leaven_core::OptimizationProblem;
use leaven_engine::{OptimizerError, RunContext};
use leaven_gepa::{GepaReflector, ReflectRequest};
use leaven_kernel::CandidateId;
use leaven_surface::EditSurface;
use leaven_workspace::{WorkspaceFactory, WorkspacePath};

use crate::{
    GepaGitProgramReflectionRenderer, GitProgramGepaReflectionInput,
    GitProgramGepaReflectionMaterializer, GitProgramGepaReflectionParser,
};

/// GEPA reflector that runs Git-program reflection through the materializing
/// `AgenticProposer` path.
pub struct GepaGitProgramAgenticReflector<Factory, Runtime, Part> {
    proposer: AgenticProposer<
        Factory,
        Runtime,
        GitProgramGepaReflectionMaterializer,
        GepaGitProgramReflectionRenderer,
        GitProgramGepaReflectionParser,
        GitProgramGepaReflectionInput<Part>,
    >,
    marker: PhantomData<Part>,
}

impl<Factory, Runtime, Part> GepaGitProgramAgenticReflector<Factory, Runtime, Part>
where
    Factory: WorkspaceFactory,
    Runtime: AgentRuntime,
{
    /// Constructs the Git-program agentic GEPA reflector.
    #[must_use]
    pub fn new(
        config: AgenticProposerConfig,
        workspace_factory: Factory,
        runtime: Runtime,
        materializer: GitProgramMaterializer,
        readback: GitProgramReadback,
    ) -> Self {
        Self {
            proposer: AgenticProposer::new(
                config,
                workspace_factory,
                runtime,
                GitProgramGepaReflectionMaterializer::new(materializer),
                GepaGitProgramReflectionRenderer,
                GitProgramGepaReflectionParser::new(readback),
            ),
            marker: PhantomData,
        }
    }
}

impl<P, S, Factory, Runtime> GepaReflector<P, S>
    for GepaGitProgramAgenticReflector<Factory, Runtime, S::PartId>
where
    P: OptimizationProblem<Artifact = GitProgramArtifact>,
    P::ProposalAnnotations: Default,
    S: EditSurface<GitProgramArtifact> + Send + Sync,
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
        let input = GitProgramGepaReflectionInput::from_request(artifact, request);
        let batch = ctx
            .propose(
                &self.proposer,
                AgenticRunInput::new(
                    input,
                    OutputContract::WorkspaceDiff {
                        roots: vec![WorkspacePath::root()],
                    },
                ),
            )
            .await
            .map_err(|source| {
                OptimizerError::with_source("GEPA Git-program agentic reflection failed", source)
            })?;
        let applied = ctx.apply_batch(batch.batch_id).map_err(|source| {
            OptimizerError::with_source("GEPA Git-program proposal application failed", source)
        })?;
        Ok(applied.successful_candidates().next())
    }
}
