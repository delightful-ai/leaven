//! Agentic lowering helpers for the public-seam reflection/proposal split.

mod adapter;
mod evaluation;
mod identity;
mod reflection;
mod util;

pub use adapter::{AdapterPayloadRole, AdapterRequestPayload, CallbackRequestPayload};
pub use evaluation::{
    JudgeContextPayload, JudgeContextPayloadFields, RunnerRequestPayload, ScorerContextPayload,
    ScorerContextPayloadFields,
};
pub use identity::{
    PublicStagePayloadError, PublicStagePayloadIdentity, PublicStagePayloadIdentityFields,
};
pub use reflection::{
    ProposeRequestPayload, ReflectProposeHandoffPayload, ReflectRequestPayload,
    ReflectionResultPayload,
};
