//! leaven-agentic crate skeleton.

mod evidence;
mod materializer;
mod proposer;
mod runtime;
mod transcript;

pub use evidence::{AgentEvidence, AgentTrajectoryEvidence};
pub use materializer::{HistoryMaterializer, TranscriptMaterializer};
pub use proposer::{AgenticProposer, AgenticProposerConfig};
pub use runtime::RunAgentInWorkspace;
pub use transcript::AgentTranscriptLoader;

pub mod prelude {
    pub use crate::{
        AgentEvidence, AgentTrajectoryEvidence, AgenticProposer, AgenticProposerConfig,
        HistoryMaterializer, TranscriptMaterializer,
    };
}
