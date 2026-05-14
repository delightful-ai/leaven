use leaven_agent::AgentSession;
use leaven_core::OptimizationProblem;
use leaven_kernel::{Fingerprint, MetadataBag, Metered, StageRole};
use leaven_workspace::WorkspaceView;

use crate::{
    AgentStageCallContext, SlotMarker, StageDirective, StageOutputContract, StageOutputParseError,
    StageQueryPolicy,
};

#[allow(async_fn_in_trait)]
pub trait StageOutputParser<P, Slot>: Send + Sync
where
    P: OptimizationProblem,
    Slot: SlotMarker<P>,
{
    async fn parse(
        &self,
        workspace: &mut WorkspaceView<'_>,
        session: &AgentSession,
        plan: &ErasedStagePlan,
        ctx: AgentStageCallContext,
    ) -> Result<Metered<Slot::Output>, StageOutputParseError>;
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ErasedStagePlan {
    pub role: StageRole,
    pub request_json: serde_json::Value,
    pub directive: StageDirective,
    pub query: StageQueryPolicy,
    pub output: StageOutputContract,
    pub metadata: MetadataBag,
    pub fingerprint: Fingerprint,
}
