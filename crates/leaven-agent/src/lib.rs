//! Provider-neutral agent runtime contracts.

mod error;
mod fake;
mod runtime;
mod session;
mod transcript;

pub use error::AgentRuntimeError;
pub use fake::{FakeAgentAction, FakeAgentRuntime};
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
        AgentTranscript, CancellationRef, CommandRecord, FakeAgentAction, FakeAgentRuntime,
        JsonSchemaRef, OutputContract, RawProviderEvent, ToolCallRecord, TranscriptEvent,
        TranscriptRole, WorkspaceAccessMode, WorkspaceReadRecord, validate_output_contract,
    };
}
