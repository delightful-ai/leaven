//! Reusable evidence value shapes for Leaven runs.
//!
//! This crate owns data that a stage or evaluator can produce and another
//! component can interpret: scalar scores, pairwise judgments, paired rollout
//! rewards, sparse casewise outcomes, command and trajectory records, output
//! records, skill-use telemetry, attribution, and case assessment records.
//!
pub mod attribution {
    use leaven_core::Evidence;
    use leaven_kernel::FiniteF64;

    /// Evidence that attributes behavior to arbitrary keys.
    ///
    /// Keys may be surface part IDs, paths, agents, changesets, tools,
    /// modules, conflict regions, or any user-defined key.
    pub trait AttributableEvidence<K>: Evidence {
        /// Returns all attribution records carried by this evidence.
        fn attributions(&self) -> Vec<Attribution<K>>;

        /// Returns human-readable evidence for one key, when available.
        fn evidence_for(&self, key: &K) -> Option<String>;
    }

    /// One attribution from an evidence item to a caller-defined key.
    #[derive(Clone, Debug)]
    pub struct Attribution<K> {
        /// Key this evidence refers to.
        pub key: K,
        /// Optional signed finite weight. Normalization is domain-specific.
        pub weight: Option<FiniteF64>,
        /// Optional human-readable note about the attribution.
        pub note: Option<String>,
    }

    /// Marker bound for values usable as attribution keys.
    pub trait AttributionKey: Eq + std::hash::Hash + Clone + Send + Sync + 'static {}
    impl<T> AttributionKey for T where T: Eq + std::hash::Hash + Clone + Send + Sync + 'static {}
}
pub mod attachment;
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
