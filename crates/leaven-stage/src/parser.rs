use leaven_agent::AgentSession;
use leaven_core::OptimizationProblem;
use leaven_kernel::{Fingerprint, FingerprintBuilder, MetadataBag, Metered, StageRole};
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

impl ErasedStagePlan {
    pub fn from_plan<Req: serde::Serialize>(
        plan: &crate::AgentStagePlan<Req>,
    ) -> Result<Self, serde_json::Error> {
        let request_json = serde_json::to_value(&plan.request)?;
        let mut fingerprint = FingerprintBuilder::new();
        fingerprint
            .update(b"leaven.stage.plan.v1")
            .update(serde_json::to_vec(&plan.role)?)
            .update(serde_json::to_vec(&request_json)?)
            .update(serde_json::to_vec(&plan.directive)?)
            .update(serde_json::to_vec(&plan.query)?)
            .update(serde_json::to_vec(&plan.output)?);
        Ok(Self {
            role: plan.role.clone(),
            request_json,
            directive: plan.directive.clone(),
            query: plan.query.clone(),
            output: plan.output.clone(),
            metadata: plan.metadata.clone(),
            fingerprint: fingerprint.finish(),
        })
    }
}
