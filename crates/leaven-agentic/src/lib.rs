//! Agentic stage adapters.

mod error;
mod evaluator;
mod parser;
mod proposer;

pub use error::{AgenticAdapterError, AgenticParseError};
pub use evaluator::{AgenticEvaluator, AgenticEvaluatorConfig};
pub use parser::{
    AgentPromptTarget, AgenticRunInput, EvaluationInputBuilder, EvidenceParser, ProposalParser,
};
pub use proposer::{AgenticProposer, AgenticProposerConfig};

pub mod prelude {
    pub use crate::{
        AgentPromptTarget, AgenticAdapterError, AgenticEvaluator, AgenticEvaluatorConfig,
        AgenticParseError, AgenticProposer, AgenticProposerConfig, AgenticRunInput,
        EvaluationInputBuilder, EvidenceParser, ProposalParser,
    };
}
