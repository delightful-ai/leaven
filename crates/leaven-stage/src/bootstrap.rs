use leaven_core::OptimizationProblem;

use crate::{AgentStageCallContext, AgentStagePlan, SlotMarker, StageBootstrapError};

#[allow(async_fn_in_trait)]
pub trait AgentStageBootstrap<P, Slot>: Send + Sync
where
    P: OptimizationProblem,
    Slot: SlotMarker<P>,
{
    async fn plan(
        &self,
        request: Slot::Request,
        ctx: AgentStageCallContext,
    ) -> Result<AgentStagePlan<Slot::Request>, StageBootstrapError>;
}
