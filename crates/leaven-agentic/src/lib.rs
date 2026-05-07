//! leaven-agentic crate skeleton.

pub struct AgentEvidence;
pub struct AgentTrajectoryEvidence;
pub struct AgenticProposer;
pub struct AgenticProposerConfig;
pub struct HistoryMaterializer;
pub struct TranscriptMaterializer;
pub struct RunAgentInWorkspace;
pub struct AgentTranscriptLoader;
pub mod prelude {
    pub use crate::{
        AgentEvidence, AgentTrajectoryEvidence, AgenticProposer, AgenticProposerConfig,
        HistoryMaterializer, TranscriptMaterializer,
    };
}
