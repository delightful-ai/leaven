use std::marker::PhantomData;

use leaven_core::{OptimizationProblem, ProposalBatch};
use leaven_kernel::StageRole;

pub trait SlotMarker<P>: Send + Sync + 'static
where
    P: OptimizationProblem,
{
    type Request: serde::Serialize + Send + Sync + 'static;
    type Output: Send + Sync + 'static;

    fn role() -> StageRole;
}

pub struct ProposerSlot<Req>(PhantomData<Req>);

impl<P, Req> SlotMarker<P> for ProposerSlot<Req>
where
    P: OptimizationProblem,
    Req: serde::Serialize + Send + Sync + 'static,
{
    type Request = Req;
    type Output = ProposalBatch<P>;

    fn role() -> StageRole {
        StageRole::reflect()
    }
}
