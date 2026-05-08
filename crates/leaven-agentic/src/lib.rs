//! Agentic stage adapters.

mod error;
mod evaluator;
mod parser;
mod proposer;
mod repair;
mod repairing_proposer;

pub use error::{AgenticAdapterError, AgenticParseError, AgenticRepairError};
pub use evaluator::{AgenticEvaluator, AgenticEvaluatorConfig};
pub use parser::{
    AgentPromptTarget, AgenticRunInput, EvaluationInputBuilder, EvidenceParser, ProposalParser,
};
pub use proposer::{AgenticProposer, AgenticProposerConfig};
pub use repair::{ProposalRepairFeedback, ProposalRepairPolicy, ProposalRepairPromptBuilder};
pub use repairing_proposer::{RepairingAgenticProposer, RepairingAgenticProposerConfig};

pub mod prelude {
    pub use crate::{
        AgentPromptTarget, AgenticAdapterError, AgenticEvaluator, AgenticEvaluatorConfig,
        AgenticParseError, AgenticProposer, AgenticProposerConfig, AgenticRepairError,
        AgenticRunInput, EvaluationInputBuilder, EvidenceParser, ProposalParser,
        ProposalRepairFeedback, ProposalRepairPolicy, ProposalRepairPromptBuilder,
        RepairingAgenticProposer, RepairingAgenticProposerConfig,
    };
}
