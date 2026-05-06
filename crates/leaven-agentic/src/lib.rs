//! leaven-agentic crate skeleton.

pub struct AgentEvidence;
pub struct AgentTrajectoryEvidence;
pub struct AgenticProposer;
pub struct AgenticProposerConfig;
pub struct HistoryWorkspaceRenderer;
pub struct TranscriptWorkspaceRenderer;
pub struct RunAgentInWorkspace;
pub struct AgentTranscriptLoader;
pub mod prelude {
    pub use crate::{
        AgentEvidence, AgentTrajectoryEvidence, AgenticProposer, AgenticProposerConfig,
        HistoryWorkspaceRenderer, TranscriptWorkspaceRenderer,
    };
}
