//! Agentic stage adapters.

mod artifact_reflector;
mod case;
mod case_evaluator;
mod case_record;
mod error;
mod evaluator;
mod inspection;
mod parser;
mod preflight;
mod proposer;
mod public_seam_stage;
mod repair;
mod repairing_proposer;

pub use artifact_reflector::{
    ArtifactReflector, ReadbackDiagnostic, ReadbackResult, ReflectionError, ReflectionLayoutConfig,
    ReflectionRunOutcome, ReflectionSessionAttachment, ReflectionWorkspace,
};
pub use case::{
    AgentCase, AgentWorkload, CaseFiles, CaseInput, CaseMessage, CasePartitionId, CasePartitions,
    CaseSuite, CaseTarget, SetupScript, WorkspaceRequirement,
};
pub use case_evaluator::{
    AgentCaseEvaluator, AgentCaseEvaluatorConfig, AgentCasePresentation,
    AgentCasePresentationInput, AgentCasePresenter, AgentCaseScoreInput, AgentCaseScorer,
};
pub use case_record::{
    AgentCaseLimits, AgentCaseRetryRecord, AgentCaseRunError, AgentCaseRunPolicy,
    AgentCaseRunRecord, CASE_RUN_RECORD_METADATA_KEY, CaseCheckpointPolicy, FailOnError,
    FailedAgentCaseRun, FiniteRatio, ScoredAgentCaseRun, ToolApprovalPolicy,
};
pub use error::{AgenticAdapterError, AgenticParseError, AgenticRepairError};
pub use evaluator::{AgenticEvaluator, AgenticEvaluatorConfig};
pub use inspection::{
    AgenticCostInspection, AgenticInspectionWarning, AgenticRunInspection, ProposalRepairInspection,
};
pub use parser::{
    AgentPromptTarget, AgenticRunInput, EvaluationInputBuilder, EvidenceParser, ProposalParser,
};
pub use preflight::{
    AgentRunPreflight, AgentRunPreflightReport, PreflightFinding, PreflightSeverity,
    PresenterDryRun, ScorerDryRun,
};
pub use proposer::{AgenticProposer, AgenticProposerConfig};
pub use public_seam_stage::{
    AdapterPayloadRole, AdapterRequestPayload, CallbackRequestPayload, JudgeContextPayload,
    JudgeContextPayloadFields, ProposeRequestPayload, PublicStagePayloadError,
    PublicStagePayloadIdentity, PublicStagePayloadIdentityFields, ReflectProposeHandoffPayload,
    ReflectRequestPayload, ReflectionResultPayload, RunnerRequestPayload, ScorerContextPayload,
    ScorerContextPayloadFields,
};
pub use repair::{
    PROPOSAL_REPAIR_ATTEMPTS_METADATA_KEY, ProposalRepairAttemptOutcome,
    ProposalRepairAttemptRecord,
};
pub use repair::{ProposalRepairFeedback, ProposalRepairPolicy, ProposalRepairPromptBuilder};
pub use repairing_proposer::{RepairingAgenticProposer, RepairingAgenticProposerConfig};

pub mod prelude {
    pub use crate::{
        AgentCase, AgentCaseEvaluator, AgentCaseEvaluatorConfig, AgentCaseLimits,
        AgentCasePresentation, AgentCasePresentationInput, AgentCasePresenter,
        AgentCaseRetryRecord, AgentCaseRunError, AgentCaseRunPolicy, AgentCaseRunRecord,
        AgentCaseScoreInput, AgentCaseScorer, AgentPromptTarget, AgentRunPreflight,
        AgentRunPreflightReport, AgentWorkload, AgenticAdapterError, AgenticCostInspection,
        AgenticEvaluator, AgenticEvaluatorConfig, AgenticInspectionWarning, AgenticParseError,
        AgenticProposer, AgenticProposerConfig, AgenticRepairError, AgenticRunInput,
        AgenticRunInspection, ArtifactReflector, CASE_RUN_RECORD_METADATA_KEY,
        CaseCheckpointPolicy, CaseFiles, CaseInput, CaseMessage, CasePartitionId, CasePartitions,
        CaseSuite, CaseTarget, EvaluationInputBuilder, EvidenceParser, FailOnError,
        FailedAgentCaseRun, FiniteRatio, PROPOSAL_REPAIR_ATTEMPTS_METADATA_KEY, PreflightFinding,
        PreflightSeverity, PresenterDryRun, ProposalParser, ProposalRepairAttemptOutcome,
        ProposalRepairAttemptRecord, ProposalRepairFeedback, ProposalRepairInspection,
        ProposalRepairPolicy, ProposalRepairPromptBuilder, ReadbackDiagnostic, ReadbackResult,
        ReflectionError, ReflectionLayoutConfig, ReflectionRunOutcome, ReflectionSessionAttachment,
        ReflectionWorkspace, RepairingAgenticProposer, RepairingAgenticProposerConfig,
        ScoredAgentCaseRun, ScorerDryRun, SetupScript, ToolApprovalPolicy, WorkspaceRequirement,
    };
}
