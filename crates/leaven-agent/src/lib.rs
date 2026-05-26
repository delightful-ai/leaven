//! Provider-neutral agent runtime contracts.

mod error;
mod fake;
mod runtime;
mod session;
mod transcript;

pub use error::AgentRuntimeError;
pub use runtime::AgentRuntime;
pub use session::{
    AgentContextRef, AgentInstructions, AgentLimits, AgentRunContext, AgentRunRequest,
    AgentRuntimeCapabilities, AgentSession, AgentSessionArtifact, AgentSessionArtifactKind,
    AgentStatus, AgentToolPolicy, CancellationRef, JsonSchemaRef, OutputContract,
    WorkspaceAccessMode, validate_output_contract,
};
pub use transcript::{
    AgentTranscript, CommandRecord, RawProviderEvent, ToolCallRecord, TranscriptEvent,
    TranscriptRole, WorkspaceReadRecord,
};

pub mod prelude {
    pub use crate::{
        AgentContextRef, AgentInstructions, AgentLimits, AgentRunContext, AgentRunRequest,
        AgentRuntime, AgentRuntimeCapabilities, AgentRuntimeError, AgentSession,
        AgentSessionArtifact, AgentSessionArtifactKind, AgentStatus, AgentToolPolicy,
        AgentTranscript, CancellationRef, CommandRecord, JsonSchemaRef, OutputContract,
        RawProviderEvent, ToolCallRecord, TranscriptEvent, TranscriptRole, WorkspaceAccessMode,
        WorkspaceReadRecord, validate_output_contract,
    };
}

/// Explicit deterministic runtime support for tests, examples, and diagnostics.
///
/// These names are intentionally outside the crate root and prelude so fake
/// provider behavior cannot masquerade as an ordinary runtime route.
pub mod test_support {
    pub use crate::fake::{FakeAgentAction, FakeAgentRuntime};
}
