//! GEPA reflection proposers.

use leaven_core::{Artifact, OptimizationProblem, ProposalBatch};
use leaven_engine::{OptimizerError, ProposalContext, ProposalError, Proposer, RunContext};
use leaven_kernel::{CandidateId, Cost, MetadataBag, Metered, ProposerId, StageId};
use leaven_lm::{Lm, ModelName};
use leaven_stage::{AgentBacked, ProposerSlot};
use leaven_surface::{EditSurface, SurfaceError};

use crate::reflection::{
    DefaultReflectionRenderer, LmBackedReflectorConfig, PlainTextEditParser, ReflectRequest,
    ReflectionOutputParser, ReflectionRenderInput, ReflectionRenderer,
};

/// Hidden sentinel reflector used only so bare `Gepa<S>` has no runnable
/// reflection path.
#[doc(hidden)]
#[derive(Clone, Debug, Default)]
pub struct MissingReflector;

/// Produces a surface-native edit for one selected part.
pub trait SurfaceProposer<A, S>
where
    A: Artifact,
    S: EditSurface<A>,
{
    /// Propose an edit to `part`.
    fn propose_edit(
        &mut self,
        artifact: &A,
        surface: &S,
        part: &S::PartId,
    ) -> Result<S::Edit, SurfaceError>;
}

/// Deterministic fixed surface edit fixture.
#[derive(Clone, Debug)]
pub struct FixedSurfaceEdit<E> {
    edit: E,
}

impl<E> FixedSurfaceEdit<E> {
    /// Build a proposer that always returns the supplied edit.
    #[must_use]
    pub const fn new(edit: E) -> Self {
        Self { edit }
    }
}

impl<A, S> SurfaceProposer<A, S> for FixedSurfaceEdit<S::Edit>
where
    A: Artifact,
    S: EditSurface<A>,
{
    fn propose_edit(
        &mut self,
        _artifact: &A,
        _surface: &S,
        _part: &S::PartId,
    ) -> Result<S::Edit, SurfaceError> {
        Ok(self.edit.clone())
    }
}

/// Reflects on a pre-built [`ReflectRequest`] and finalizes the resulting
/// candidate through the engine.
///
/// The optimizer loop builds the request once via a
/// [`ReflectiveDatasetBuilder`](crate::ReflectiveDatasetBuilder) and passes the
/// same value to whichever reflector is configured. A reflector never builds
/// its own request, so the LM and agent backends provably see identical
/// reflective examples for identical inputs.
#[allow(async_fn_in_trait)]
pub trait GepaReflector<P, S>: Send + Sync
where
    P: OptimizationProblem,
    S: EditSurface<P::Artifact>,
{
    /// Produce and apply the next GEPA proposal for the request's parent.
    async fn reflect_candidate(
        &mut self,
        ctx: &mut RunContext<'_, P>,
        surface: &S,
        request: ReflectRequest<S::PartId>,
    ) -> Result<Option<CandidateId>, OptimizerError>;
}

impl<P, S> GepaReflector<P, S> for FixedSurfaceEdit<S::Edit>
where
    P: OptimizationProblem,
    P::ProposalAnnotations: Default,
    S: EditSurface<P::Artifact> + Send + Sync,
{
    async fn reflect_candidate(
        &mut self,
        ctx: &mut RunContext<'_, P>,
        surface: &S,
        request: ReflectRequest<S::PartId>,
    ) -> Result<Option<CandidateId>, OptimizerError> {
        let parent = request.parent;
        let artifact = ctx
            .graph()
            .artifact(parent)
            .ok_or_else(|| {
                OptimizerError::Message(format!("selected parent {parent} is missing from graph"))
            })?
            .clone();
        let edit = self
            .propose_edit(&artifact, surface, &request.part)
            .map_err(|source| OptimizerError::with_source("GEPA reflection failed", source))?;
        let change = surface
            .change_part(&artifact, request.part.clone(), edit)
            .map_err(|source| {
                OptimizerError::with_source("GEPA surface edit lowering failed", source)
            })?;
        let batch = ctx
            .record_proposal_batch(
                StageId::custom("gepa/fixed-surface-edit"),
                ProposalBatch {
                    proposals: vec![
                        leaven_core::Proposal::mutate(parent, change)
                            .informed_by(request.informed_by())
                            .build(),
                    ],
                    semantics: leaven_core::ProposalBatchSemantics::Alternatives,
                    metadata: MetadataBag::new(),
                },
                Cost::metric_calls(1),
            )
            .map_err(|source| {
                OptimizerError::with_source("GEPA proposal recording failed", source)
            })?;
        let applied = ctx.apply_batch(batch.batch_id).map_err(|source| {
            OptimizerError::with_source("GEPA proposal application failed", source)
        })?;
        Ok(applied.successful_candidates().next())
    }
}

impl<P, S, Runtime, Bootstrap, Parser> GepaReflector<P, S>
    for AgentBacked<ProposerSlot<ReflectRequest>, Runtime, Bootstrap, Parser>
where
    P: OptimizationProblem,
    S: EditSurface<P::Artifact> + Send + Sync,
    S::PartId: std::fmt::Debug + Send + Sync,
    Self: Proposer<P, Request = ReflectRequest>,
{
    #[allow(clippy::future_not_send)]
    async fn reflect_candidate(
        &mut self,
        ctx: &mut RunContext<'_, P>,
        _surface: &S,
        request: ReflectRequest<S::PartId>,
    ) -> Result<Option<CandidateId>, OptimizerError> {
        let request = ReflectRequest {
            parent: request.parent,
            part: format!("{:?}", request.part),
            part_label: request.part_label,
            examples: request.examples,
            source_refs: request.source_refs,
            attempt_index: request.attempt_index,
        };
        let batch = ctx
            .propose(self, request)
            .await
            .map_err(|source| OptimizerError::with_source("GEPA reflection failed", source))?;
        let applied = ctx.apply_batch(batch.batch_id).map_err(|source| {
            OptimizerError::with_source("GEPA proposal application failed", source)
        })?;
        Ok(applied.successful_candidates().next())
    }
}

/// GEPA reflector that uses a provider-neutral LM to propose surface edits.
#[derive(Clone, Debug)]
pub struct LmBackedReflector<L, Renderer, Parser> {
    lm: L,
    model: ModelName,
    renderer: Renderer,
    parser: Parser,
    config: LmBackedReflectorConfig,
    id: ProposerId,
}

impl<L, Renderer, Parser> LmBackedReflector<L, Renderer, Parser> {
    /// Build an LM-backed reflector with explicit renderer and parser.
    #[must_use]
    pub fn new(lm: L, model: impl Into<ModelName>, renderer: Renderer, parser: Parser) -> Self {
        Self {
            lm,
            model: model.into(),
            renderer,
            parser,
            config: LmBackedReflectorConfig::default(),
            id: ProposerId::from("gepa/lm-backed-reflector"),
        }
    }

    /// Override LM request controls.
    #[must_use]
    pub fn with_config(mut self, config: LmBackedReflectorConfig) -> Self {
        self.config = config;
        self
    }

    /// Override proposer identity used for events and budget accounting.
    #[must_use]
    pub fn with_id(mut self, id: impl Into<ProposerId>) -> Self {
        self.id = id.into();
        self
    }
}

impl<L> LmBackedReflector<L, DefaultReflectionRenderer, PlainTextEditParser> {
    /// Build an LM-backed reflector with the standard text renderer/parser.
    #[must_use]
    pub fn with_default_renderer(lm: L, model: impl Into<ModelName>) -> Self {
        Self::new(lm, model, DefaultReflectionRenderer, PlainTextEditParser)
    }
}

impl<P, S, L, Renderer, Parser> GepaReflector<P, S> for LmBackedReflector<L, Renderer, Parser>
where
    P: OptimizationProblem,
    S: EditSurface<P::Artifact> + Send + Sync,
    S::PartId: std::fmt::Debug + Send + Sync,
    L: Lm,
    Renderer: ReflectionRenderer<P, S>,
    Parser: ReflectionOutputParser<P, S>,
{
    async fn reflect_candidate(
        &mut self,
        ctx: &mut RunContext<'_, P>,
        surface: &S,
        request: ReflectRequest<S::PartId>,
    ) -> Result<Option<CandidateId>, OptimizerError> {
        let call = LmBackedProposalCall {
            reflector: self,
            surface,
        };
        let batch = ctx
            .propose(&call, request)
            .await
            .map_err(|source| OptimizerError::with_source("GEPA reflection failed", source))?;
        let applied = ctx.apply_batch(batch.batch_id).map_err(|source| {
            OptimizerError::with_source("GEPA proposal application failed", source)
        })?;
        Ok(applied.successful_candidates().next())
    }
}

struct LmBackedProposalCall<'a, L, Renderer, Parser, S> {
    reflector: &'a LmBackedReflector<L, Renderer, Parser>,
    surface: &'a S,
}

impl<P, S, L, Renderer, Parser> Proposer<P> for LmBackedProposalCall<'_, L, Renderer, Parser, S>
where
    P: OptimizationProblem,
    S: EditSurface<P::Artifact> + Send + Sync,
    L: Lm,
    Renderer: ReflectionRenderer<P, S>,
    Parser: ReflectionOutputParser<P, S>,
{
    type Request = ReflectRequest<S::PartId>;

    fn id(&self) -> ProposerId {
        self.reflector.id.clone()
    }

    async fn propose(
        &self,
        request: Self::Request,
        ctx: ProposalContext<'_, P>,
    ) -> Result<Metered<ProposalBatch<P>>, ProposalError> {
        let artifact = ctx
            .graph()
            .artifact(request.parent)
            .ok_or_else(|| {
                ProposalError::Message(format!(
                    "selected parent {} is missing from graph",
                    request.parent
                ))
            })?
            .clone();
        let lm_request = self.reflector.renderer.render(ReflectionRenderInput {
            request: &request,
            artifact: &artifact,
            surface: self.surface,
            model: self.reflector.model.clone(),
            config: &self.reflector.config,
        })?;
        let metered = self
            .reflector
            .lm
            .complete(lm_request)
            .await
            .map_err(|source| {
                ProposalError::with_source("LM-backed GEPA reflection failed", source)
            })?;
        let batch = self.reflector.parser.parse(
            metered.value.assistant.content(),
            &request,
            &artifact,
            self.surface,
        )?;
        Ok(Metered::new(batch, metered.cost))
    }
}
