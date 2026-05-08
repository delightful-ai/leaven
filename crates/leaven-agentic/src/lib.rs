//! Agentic stage adapters.

mod case;
mod case_evaluator;
mod error;
mod evaluator;
mod parser;
mod preflight;
mod proposer;
mod repair;
mod repairing_proposer;

pub use case::{
    AgentCase, AgentWorkload, CaseFiles, CaseInput, CaseMessage, CasePartitionId, CasePartitions,
    CaseSuite, CaseTarget, SetupScript, WorkspaceRequirement,
};
pub use case_evaluator::{
    AgentCaseEvaluator, AgentCaseEvaluatorConfig, AgentCasePresentation,
    AgentCasePresentationInput, AgentCasePresenter, AgentCaseScoreInput, AgentCaseScorer,
};
pub use error::{AgenticAdapterError, AgenticParseError, AgenticRepairError};
pub use evaluator::{AgenticEvaluator, AgenticEvaluatorConfig};
pub use parser::{
    AgentPromptTarget, AgenticRunInput, EvaluationInputBuilder, EvidenceParser, ProposalParser,
};
pub use preflight::{
    AgentRunPreflight, AgentRunPreflightReport, PreflightFinding, PreflightSeverity,
};
pub use proposer::{AgenticProposer, AgenticProposerConfig};
pub use repair::{ProposalRepairFeedback, ProposalRepairPolicy, ProposalRepairPromptBuilder};
pub use repairing_proposer::{RepairingAgenticProposer, RepairingAgenticProposerConfig};

pub mod prelude {
    pub use crate::{
        AgentCase, AgentCaseEvaluator, AgentCaseEvaluatorConfig, AgentCasePresentation,
        AgentCasePresentationInput, AgentCasePresenter, AgentCaseScoreInput, AgentCaseScorer,
        AgentPromptTarget, AgentRunPreflight, AgentRunPreflightReport, AgentWorkload,
        AgenticAdapterError, AgenticEvaluator, AgenticEvaluatorConfig, AgenticParseError,
        AgenticProposer, AgenticProposerConfig, AgenticRepairError, AgenticRunInput, CaseFiles,
        CaseInput, CaseMessage, CasePartitionId, CasePartitions, CaseSuite, CaseTarget,
        EvaluationInputBuilder, EvidenceParser, PreflightFinding, PreflightSeverity,
        ProposalParser, ProposalRepairFeedback, ProposalRepairPolicy, ProposalRepairPromptBuilder,
        RepairingAgenticProposer, RepairingAgenticProposerConfig, SetupScript,
        WorkspaceRequirement,
    };
}
