//! GEPA reflection proposers.

use leaven_core::{
    Artifact, InfoRef, OptimizationProblem, Proposal, ProposalBatch, ProposalBatchSemantics,
};
use leaven_engine::{OptimizerError, Proposer, RunContext};
use leaven_kernel::{AssessmentId, CandidateId, Cost, MetadataBag, StageId};
use leaven_stage::{AgentBacked, ProposerSlot};
use leaven_surface::{EditSurface, SurfaceError};

use crate::agent_stage::{ReflectRequest, SelectedFeedback};

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

/// Reflects on selected GEPA feedback and finalizes the resulting candidate
/// through the engine.
#[allow(async_fn_in_trait)]
pub trait GepaReflector<P, S>: Send + Sync
where
    P: OptimizationProblem,
    S: EditSurface<P::Artifact>,
{
    /// Produce and apply the next GEPA proposal for `parent`.
    async fn reflect_candidate(
        &mut self,
        ctx: &mut RunContext<'_, P>,
        surface: &S,
        parent: CandidateId,
        parent_assessment: AssessmentId,
        part: S::PartId,
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
        parent: CandidateId,
        parent_assessment: AssessmentId,
        part: S::PartId,
    ) -> Result<Option<CandidateId>, OptimizerError> {
        let artifact = ctx
            .graph()
            .artifact(parent)
            .ok_or_else(|| {
                OptimizerError::Message(format!("selected parent {parent} is missing from graph"))
            })?
            .clone();
        let edit = self
            .propose_edit(&artifact, surface, &part)
            .map_err(|source| OptimizerError::with_source("GEPA reflection failed", source))?;
        let change = surface
            .change_part(&artifact, part, edit)
            .map_err(|source| {
                OptimizerError::with_source("GEPA surface edit lowering failed", source)
            })?;
        let batch = ctx
            .record_proposal_batch(
                StageId::custom("gepa/fixed-surface-edit"),
                ProposalBatch {
                    proposals: vec![
                        Proposal::mutate(parent, change)
                            .informed_by([
                                InfoRef::Candidate(parent),
                                InfoRef::Assessment(parent_assessment),
                            ])
                            .build(),
                    ],
                    semantics: ProposalBatchSemantics::Alternatives,
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
        parent: CandidateId,
        parent_assessment: AssessmentId,
        part: S::PartId,
    ) -> Result<Option<CandidateId>, OptimizerError> {
        let selected_feedback = SelectedFeedback {
            assessment_refs: vec![parent_assessment],
            evidence_refs: Vec::new(),
            candidate_refs: vec![parent],
        };
        let request = ReflectRequest::new(parent, format!("{part:?}"))
            .with_selected_feedback(selected_feedback);
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
