//! Reusable evidence value shapes for Leaven runs.
//!
//! This crate owns data that a stage or evaluator can produce and another
//! component can interpret: scalar scores, pairwise judgments, paired rollout
//! rewards, sparse casewise outcomes, command and trajectory records, output
//! records, skill-use telemetry, attribution, and case assessment records.
//!
pub mod attachment;
pub mod attribution;
pub mod casewise;
pub mod command;
pub mod feedback;
pub mod output;
pub mod pairwise;
pub mod rollout;
pub mod scalar;
pub mod skill_use;
pub use attachment::{Attachment, AttachmentKind};
pub use attribution::{AttributableEvidence, Attribution, AttributionKey};
pub use casewise::{CaseOutcome, CasewiseEvidence};
pub use command::{
    AgentAnalystCallError, AgentAnalystCallEvidence, AgentAnalystCallEvidenceInput,
    AgentAnalystCallStatus, AgentAnalystFanoutError, AgentAnalystFanoutEvidence, AgentAnalystRole,
    AgentPatchMergeDecision, AgentPatchMergeNode, AgentPatchMergeNodeInput,
    AgentPatchMergeTreeError, AgentPatchMergeTreeEvidence, AgentTrajectoryAnalysisKind,
    AgentTrajectoryAnalysisRecord, AgentTrajectoryCorpusError, AgentTrajectoryCorpusEvidence,
    AgentTrajectoryEvidence, AgentTrajectoryEvidenceInput, AgentTrajectoryOutcome, CommandEvidence,
    CommandRecord,
};
pub use feedback::{
    CandidateAssessmentOutput, CandidateAssessmentOutputError, CaseAssessmentEvidence,
    CaseDataReadEvidence,
};
pub use output::{
    DataClass, DataClassError, DataClassSet, OutputBlobAudit, OutputBlobAuditError, OutputMetadata,
    OutputRecord, OutputVisibility,
};
pub use pairwise::{PairwiseJudgment, PairwiseJudgmentEvidence};
pub use rollout::{PairedRolloutEvidence, PairedRolloutEvidenceError, RolloutGroupOutcome};
pub use scalar::{ScalarEvidence, ScalarEvidenceError};
pub use skill_use::{
    SkillTrajectoryUseEvidence, SkillTrajectoryUseEvidenceError, SkillUseConfidence, SkillUseEvent,
    SkillUseEvidence, SkillUseKind, SkillUseSource,
};
pub mod prelude {
    pub use crate::{
        AgentAnalystCallError, AgentAnalystCallEvidence, AgentAnalystCallEvidenceInput,
        AgentAnalystCallStatus, AgentAnalystFanoutError, AgentAnalystFanoutEvidence,
        AgentAnalystRole, AgentPatchMergeDecision, AgentPatchMergeNode, AgentPatchMergeNodeInput,
        AgentPatchMergeTreeError, AgentPatchMergeTreeEvidence, AgentTrajectoryAnalysisKind,
        AgentTrajectoryAnalysisRecord, AgentTrajectoryCorpusError, AgentTrajectoryCorpusEvidence,
        AgentTrajectoryEvidence, AgentTrajectoryEvidenceInput, AgentTrajectoryOutcome, Attachment,
        AttachmentKind, AttributableEvidence, CandidateAssessmentOutput,
        CandidateAssessmentOutputError, CaseAssessmentEvidence, CaseOutcome, CasewiseEvidence,
        CommandEvidence, CommandRecord, DataClass, DataClassSet, OutputBlobAudit,
        OutputBlobAuditError, OutputMetadata, OutputRecord, OutputVisibility,
        PairedRolloutEvidence, PairwiseJudgmentEvidence, RolloutGroupOutcome, ScalarEvidence,
        ScalarEvidenceError, SkillTrajectoryUseEvidence, SkillTrajectoryUseEvidenceError,
        SkillUseConfidence, SkillUseEvent, SkillUseEvidence, SkillUseKind, SkillUseSource,
    };
}
