//! Provider-neutral runtime trait.

use std::future::Future;

use leaven_kernel::{AgentRuntimeId, Fingerprint, Metered};
use leaven_workspace::WorkspaceView;

use crate::{
    AgentRunContext, AgentRunRequest, AgentRuntimeCapabilities, AgentRuntimeError, AgentSession,
};

/// Executes one agent session in an already materialized workspace.
///
/// `AgentRuntime` is intentionally below optimization vocabulary. It does not
/// know about candidates, proposals, assessments, evidence, populations, or
/// GEPA. Stage adapters decide why a session is being run and how to parse the
/// resulting files/transcript back into Leaven types.
pub trait AgentRuntime: Send + Sync {
    fn id(&self) -> AgentRuntimeId;

    fn fingerprint(&self) -> Fingerprint;

    fn capabilities(&self) -> AgentRuntimeCapabilities {
        AgentRuntimeCapabilities::default()
    }

    fn run_session<'a>(
        &'a self,
        workspace: &'a mut WorkspaceView<'_>,
        request: AgentRunRequest,
        ctx: AgentRunContext<'a>,
    ) -> impl Future<Output = Result<Metered<AgentSession>, AgentRuntimeError>> + Send + 'a;
}
