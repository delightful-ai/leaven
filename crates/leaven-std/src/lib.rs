//! Curated standard library for Leaven.

pub mod evidence {
    //! Standard evidence shapes.

    pub use leaven_evidence::{
        AgentAnalystCallError, AgentAnalystCallEvidence, AgentAnalystCallEvidenceInput,
        AgentAnalystCallStatus, AgentAnalystFanoutError, AgentAnalystFanoutEvidence,
        AgentAnalystRole, AgentPatchMergeDecision, AgentPatchMergeNode, AgentPatchMergeNodeInput,
        AgentPatchMergeTreeError, AgentPatchMergeTreeEvidence, AgentTrajectoryAnalysisKind,
        AgentTrajectoryAnalysisRecord, AgentTrajectoryCorpusError, AgentTrajectoryCorpusEvidence,
        AgentTrajectoryEvidence, AgentTrajectoryEvidenceInput, AgentTrajectoryOutcome, Attachment,
        AttachmentKind, AttributableEvidence, Attribution, AttributionKey,
        CandidateAssessmentOutput, CandidateAssessmentOutputError, CaseAssessmentEvidence,
        CaseDataReadEvidence, CaseOutcome, CasewiseEvidence, CommandEvidence, CommandRecord,
        DataClass, DataClassError, DataClassSet, OutputBlobAudit, OutputBlobAuditError,
        OutputMetadata, OutputRecord, OutputVisibility, PairedRolloutEvidence,
        PairedRolloutEvidenceError, PairwiseJudgment, PairwiseJudgmentEvidence,
        RolloutGroupOutcome, ScalarEvidence, ScalarEvidenceError, SkillTrajectoryUseEvidence,
        SkillTrajectoryUseEvidenceError, SkillUseConfidence, SkillUseEvent, SkillUseEvidence,
        SkillUseKind, SkillUseSource,
    };
}

pub mod preferences {
    //! Stateless preference relations.

    pub use leaven_preference::{HigherScoreIsBetter, LowerScoreIsBetter};
}

pub mod populations {
    //! Standard populations.

    pub use leaven_population::{
        BradleyTerryFit, KeepBest, ParetoFrontier, ParetoFrontierBuilder, PartitionFilter,
        SkillPairedRolloutUtilityInput, SkillPairedRolloutUtilityInputError,
        SkillPairedRolloutUtilityUpdates, SkillPruningCandidate, SkillRetrievalCandidate,
        SkillSimilarityCandidate, SkillSimilarityCandidateError, SkillSimilarityRank,
        SkillStepTrajectoryOutcome, SkillStepTrajectoryOutcomeError, SkillTwoStageRetrievalConfig,
        SkillTwoStageRetrievalConfigError, SkillTwoStageRetrievalError, SkillTwoStageRetrievalPlan,
        SkillTwoStageRetriever, SkillUseStats, SkillUtilityCredit, SkillUtilityPrunePlan,
        SkillUtilityPruner, SkillUtilityPruningConfig, SkillUtilityPruningError,
        SkillUtilityPruningRank, SkillUtilityRank, SkillUtilityRanker, SkillUtilityRankingWeights,
        SkillUtilityRankingWeightsError, SkillUtilitySmoothing, SkillUtilitySmoothingError,
        SkillUtilityState, SkillUtilityTransfer, SkillUtilityUpdate, TopKFrontier,
        TopKParentSelectionPolicy, TopKParentSelector, TournamentPopulation,
    };
}

pub mod surfaces {
    //! Standard surface exports.

    pub use leaven_surface::{
        EditSurface, Part, PartAddress, PartSelection, PartView, PathAddress, PathPartId,
        PathSurfaceConfig, SurfaceError, SurfaceFingerprint,
    };
}

pub mod prelude {
    //! Common standard-library imports.

    pub use leaven_evidence::{
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
    pub use leaven_population::{
        BradleyTerryFit, KeepBest, ParetoFrontier, ParetoFrontierBuilder, PartitionFilter,
        SkillPairedRolloutUtilityInput, SkillPairedRolloutUtilityInputError,
        SkillPairedRolloutUtilityUpdates, SkillPruningCandidate, SkillRetrievalCandidate,
        SkillSimilarityCandidate, SkillSimilarityCandidateError, SkillSimilarityRank,
        SkillStepTrajectoryOutcome, SkillStepTrajectoryOutcomeError, SkillTwoStageRetrievalConfig,
        SkillTwoStageRetrievalConfigError, SkillTwoStageRetrievalError, SkillTwoStageRetrievalPlan,
        SkillTwoStageRetriever, SkillUseStats, SkillUtilityCredit, SkillUtilityPrunePlan,
        SkillUtilityPruner, SkillUtilityPruningConfig, SkillUtilityPruningError,
        SkillUtilityPruningRank, SkillUtilityRank, SkillUtilityRanker, SkillUtilityRankingWeights,
        SkillUtilityRankingWeightsError, SkillUtilitySmoothing, SkillUtilitySmoothingError,
        SkillUtilityState, SkillUtilityTransfer, SkillUtilityUpdate, TopKFrontier,
        TopKParentSelectionPolicy, TopKParentSelector, TournamentPopulation,
    };
    pub use leaven_preference::{HigherScoreIsBetter, LowerScoreIsBetter};
    pub use leaven_surface::{
        EditSurface, Part, PartAddress, PartSelection, PartView, SurfaceError, SurfaceFingerprint,
    };

    #[cfg(feature = "git")]
    pub use leaven_artifact_git::{
        GitArtifact, GitArtifactError, GitArtifactIdentityMode, GitChange, GitDiff, GitDiffSummary,
        GitFsOp, GitLineage, GitObjectId, GitPath, GitProgramArtifact, GitProgramChange,
        GitProgramLayout, GitRef, GitRefKey, GitRefKind, GitRefName, GitRefTarget, GitRepoArtifact,
        GitRepoChange, GitRevision, GitRevisionKind, RemoteRef, RepoKey, RepoRef, RepoStoreRef,
    };

    #[cfg(feature = "jj")]
    pub use leaven_artifact_jj::{JjArtifact, JjChange};

    #[cfg(feature = "skill")]
    pub use leaven_artifact_skill::{
        ParsedSkillMd, SkillBank, SkillBankChange, SkillBankError, SkillBody, SkillBodyEdit,
        SkillBodyPartId, SkillBodySurface, SkillCard, SkillDescription, SkillDescriptionError,
        SkillFile, SkillFileEdit, SkillFilePartId, SkillFilePermissions, SkillFileSurface,
        SkillFolder, SkillFolderEdit, SkillFolderSurface, SkillManifest, SkillManifestEdit,
        SkillManifestPartId, SkillManifestSurface, SkillMetadata, SkillMetadataValue, SkillName,
        SkillNameError, SkillParseError, SkillPath, SkillPathError, SkillReferenceEdit,
        SkillReferencePartId, SkillReferenceSurface, SkillRouteEntry, SkillRouteKey,
        SkillRouteKeyError, SkillRoutePool, SkillRoutePoolError, SkillRouteRegistry,
        SkillRouteRegistryError, SkillRouteSpec, SkillTokenBreakdown, SkillTokenProfile,
        SkillTokenProfileComparison, SkillTokenProfileError, SkillTokenizer,
    };
}
