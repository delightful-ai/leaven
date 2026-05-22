use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::Read;
use std::num::NonZeroUsize;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use futures::executor::block_on;
use leaven_agentic_git::{
    GitAgenticGitError, GitProgramMaterializer, GitProgramReadback, GitProgramStores,
};
use leaven_artifact_git::{
    GitArtifactIdentityMode, GitObjectId, GitPath, GitProgramArtifact, GitProgramChange,
    GitProgramLayout, GitRepoArtifact, GitRevision, RepoKey, RepoRef,
};
use leaven_core::{Artifact, CaseSetVersion};
use leaven_eval::{
    CategoryRoundRobinSampler, ExplicitSplitBuilder, RowOrderSplitBuilder, SourceRow,
    SourceRowManifest, SplitRole, StratifiedSplitBuilder,
};
use leaven_eval_parquet::{ParquetSourceRowError, read_parquet_source_rows};
use leaven_evidence::ScalarEvidence;
use leaven_kernel::{AssessmentId, CandidateId, CaseId, Fingerprint};
use leaven_population::{TopKFrontier, TopKParentSelector};
use leaven_workspace::{
    Command as WorkspaceCommand, WorkspaceConfig, WorkspaceFactory, WorkspacePath,
};
use leaven_workspace_local::LocalWorkspaceFactory;
use sha2::{Digest, Sha256};
use smol_str::SmolStr;

pub const DEFAULT_TOLERANCES: [f64; 5] = [0.0, 0.01, 0.025, 0.05, 0.10];
pub const DEFAULT_FAILURE_THRESHOLD: f64 = 0.8;
const YEAR_START: f64 = 1900.0;
const YEAR_END: f64 = 2100.0;
const OFFICEQA_VALIDATION_ROWS: usize = 17;
const OFFICEQA_TRAIN_SIZES: [usize; 3] = [12, 24, 36];
const SEALQA_ROWS: usize = 111;
const SEALQA_TRAIN_ROWS: usize = 11;
const OFFICEQA_EXACT_SPLIT_MANIFEST_PATH: &str =
    "tmp/replication/evoskill/officeqa/paper_split_manifest.json";
const SEALQA_EXACT_SPLIT_MANIFEST_PATH: &str =
    "tmp/replication/evoskill/sealqa/paper_split_manifest.json";
const BROWSECOMP_TRANSFER_SAMPLE_PATH: &str =
    "tmp/replication/evoskill/browsecomp/transfer_sample.jsonl";
const BROWSECOMP_PUBLIC_CSV_PATH: &str =
    "tmp/replication/evoskill/browsecomp/public_browsecomp_test_set.csv";
const SOURCE_PIN_MANIFEST_PATH: &str = "tmp/replication/evoskill/source_pin_manifest.json";
const SPLIT_POLICY_MANIFEST_PATH: &str = "tmp/replication/evoskill/split_policy_manifest.json";
const SCORE_RESULT_MANIFEST_PATH: &str = "tmp/replication/evoskill/score_result_manifest.json";
const PAPER_DECLARED_SOURCE_ID_MANIFEST_METHOD: &str = "paper_declared_source_id_manifest";
const BROWSECOMP_TRANSFER_ROWS: usize = 128;
const BROWSECOMP_TRANSFER_ROWS_U64: u64 = 128;
const SEALQA_JUDGE_TEMPLATE_ID: &str = "sealqa-auto-grader-placeholder-v1";
const SEALQA_JUDGE_SOURCE_ARTIFACT_ID: &str = "paper_auto_grader_placeholder";
const SEALQA_JUDGE_RUNTIME_STATUS: &str = "template_pinned_no_spend";
const SEALQA_JUDGE_SYSTEM_PROMPT: &str = "You are the **Auto-Grader** agent. Evaluate a model output against a reference answer and scoring rubric.";
const SEALQA_JUDGE_OUTPUT_CONTRACT: &str = r#"Return a JSON object with:
- "score": <FLOAT_0_TO_1>
- "passed": <TRUE_FALSE>
- "reason": <SHORT_EXPLANATION>
- "error_breakdown": [{"type": <TYPE>, "value": <VALUE>}, ...]"#;
const SEALQA_JUDGE_NOTES: &str = "Prefer deterministic checks when possible. If numeric, compute relative error using the paper placeholder formula.";
const EVOSKILL_REPLICA_FRONTIER_SIZE: usize = 3;
const EVOSKILL_REPLICA_TRAIN_ROWS: usize = 12;
const EVOSKILL_REPLICA_RESUME_AFTER_ITERATION: u64 = 2;
const REPLICA_VALIDATION_SCORES: [f64; 4] = [0.60, 0.20, 0.10, 0.80];
const SOURCE_REVISION_SPECS: [(&str, &str); 2] = [
    ("evoskill_repo", "tmp/repros/evoskill"),
    ("officeqa_repo", "tmp/repros/officeqa"),
];

#[derive(Clone, Debug)]
pub struct ManifestBuildInput {
    root: PathBuf,
}

impl ManifestBuildInput {
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct EvoSkillReplicaManifest {
    pub schema_version: u32,
    pub paper: PaperTarget,
    pub exactness: ExactnessClass,
    pub source_revisions: Vec<SourceRevision>,
    pub artifacts: Vec<SourceArtifact>,
    pub source_blockers: Vec<SourceBlockerReport>,
    pub datasets: Vec<DatasetRequirement>,
    pub source_universe: Vec<SourceUniverseEntry>,
    pub source_materializations: Vec<DatasetMaterializationReport>,
    pub scorer: ScorerManifest,
    pub frontier: FrontierManifest,
    pub schedule: ScheduleManifest,
    pub model_pins: Vec<ModelPin>,
    pub paper_result_targets: Vec<PaperResultTarget>,
    pub blockers: Vec<ReplicationBlocker>,
    pub proxy_rejections: Vec<String>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct PaperTarget {
    pub id: String,
    pub arxiv_id: String,
    pub title: String,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExactnessClass {
    BlockedBeforePaperClose,
    PaperCloseCandidate,
    PaperClose,
    PaperExact,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct SourceRevision {
    pub id: String,
    pub relative_path: String,
    pub head: Option<String>,
    pub branch: Option<String>,
    pub remote_url: Option<String>,
    pub remote_head: Option<String>,
    pub remote_probe_status: SourceRemoteProbeStatus,
    pub paper_release_ref: Option<String>,
    pub paper_release_head: Option<String>,
    pub paper_release_status: SourcePaperReleaseStatus,
    pub status: SourceRevisionStatus,
    pub blocker_ids: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceRevisionStatus {
    MissingPath,
    NotGitCheckout,
    Present,
    ProbeFailed,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceRemoteProbeStatus {
    MissingPath,
    NotGitCheckout,
    MissingRemote,
    NotProbedNoNetworkDefault,
    ProbeFailed,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourcePaperReleaseStatus {
    MissingPath,
    NotGitCheckout,
    Unresolved,
    PinnedLocalCheckout,
    ProbeFailed,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct SourceArtifact {
    pub id: String,
    pub role: String,
    pub relative_path: String,
    pub exists: bool,
    pub bytes: Option<u64>,
    pub sha256: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SourceBlockerReport {
    pub blocker_id: String,
    pub dataset_id: String,
    pub status: SourceBlockerStatus,
    pub required_for: Vec<String>,
    pub local_path_candidates: Vec<SourceBlockerCandidate>,
    pub note: String,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SourceBlockerCandidate {
    pub relative_path: String,
    pub exists: bool,
    pub is_file: bool,
    pub is_dir: bool,
    pub bytes: Option<u64>,
    pub sha256: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceBlockerStatus {
    UnresolvedSourcePolicy,
    MissingLocalArtifact,
    MissingExactSplitManifest,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct DatasetRequirement {
    pub id: String,
    pub paper_rows: Option<u64>,
    pub train_sizes: Vec<u64>,
    pub validation_rows: Option<u64>,
    pub held_out: String,
    pub split_status: SplitManifestStatus,
    pub blocker_ids: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SourceUniverseEntry {
    pub dataset_id: String,
    pub source_revision_ids: Vec<String>,
    pub source_artifact_ids: Vec<String>,
    pub paper_rows: Option<u64>,
    pub materialized_rows: Option<u64>,
    pub source_row_fingerprint: Option<String>,
    pub split_ids: Vec<String>,
    pub split_exactness: Vec<MaterializationExactness>,
    pub blocker_ids: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SplitManifestStatus {
    ExactPublished,
    PaperCloseSubstituteAccepted,
    PaperCloseSubstituteRequired,
    BlockedMissingCategoryManifest,
    BlockedMissingSplitManifest,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceMaterializationStatus {
    MissingArtifact,
    Materialized,
    BlockedMissingRowReader,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MaterializationExactness {
    PaperExact,
    PaperCloseSubstitute,
    Blocked,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SplitAcceptanceStatus {
    NotRequired,
    PendingPaperClosePolicy,
    AcceptedPaperClosePolicy,
    Blocked,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct StratumMaterializationReport {
    pub name: String,
    pub rows: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SplitMaterializationReport {
    pub id: String,
    pub method: String,
    pub exactness: MaterializationExactness,
    pub train_rows: u64,
    pub validation_rows: Option<u64>,
    pub test_rows: Option<u64>,
    pub split_fingerprint: Option<String>,
    pub role_manifests: Vec<SplitRoleMaterializationReport>,
    pub blocker_ids: Vec<String>,
    pub acceptance_status: SplitAcceptanceStatus,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SplitRoleMaterializationReport {
    pub role: String,
    pub rows: u64,
    pub source_ids: Vec<String>,
    pub source_id_fingerprint: String,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DatasetMaterializationReport {
    pub dataset_id: String,
    pub source_artifact_id: String,
    pub source_status: SourceMaterializationStatus,
    pub source_rows: Option<u64>,
    pub case_rows: Option<u64>,
    pub source_row_fingerprint: Option<String>,
    pub source_artifact_sha256: Option<String>,
    pub target_policy: String,
    pub strata: Vec<StratumMaterializationReport>,
    pub split_materializations: Vec<SplitMaterializationReport>,
    pub blocker_ids: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OfficeQaInput {
    pub question: String,
    pub source_docs: String,
    pub source_files: String,
    pub difficulty: String,
}

#[derive(Clone, Debug)]
pub struct OfficeQaSourceMaterialization {
    pub rows: SourceRowManifest<OfficeQaInput, String>,
    pub report: DatasetMaterializationReport,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SealQaInput {
    pub question: String,
    pub urls: Vec<String>,
    pub topic: Option<String>,
}

#[derive(Clone, Debug)]
pub struct SealQaSourceMaterialization {
    pub rows: SourceRowManifest<SealQaInput, String>,
    pub report: DatasetMaterializationReport,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BrowseCompTransferInput {
    pub question: String,
    pub stratum: Option<String>,
}

#[derive(Clone, Debug)]
pub struct BrowseCompTransferSourceMaterialization {
    pub rows: SourceRowManifest<BrowseCompTransferInput, String>,
    pub report: DatasetMaterializationReport,
}

#[derive(Clone, Debug)]
struct EvoSkillSourceMaterializations {
    officeqa: Option<OfficeQaSourceMaterialization>,
    sealqa: Option<SealQaSourceMaterialization>,
    browsecomp_transfer: Option<BrowseCompTransferSourceMaterialization>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ScorerManifest {
    pub id: String,
    pub tolerances: Vec<f64>,
    pub failure_threshold: f64,
    pub implementation_status: String,
    pub judge_templates: Vec<JudgeTemplateManifest>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct JudgeTemplateManifest {
    pub id: String,
    pub dataset_id: String,
    pub source_artifact_id: String,
    pub source_artifact_exists: bool,
    pub source_artifact_bytes: Option<u64>,
    pub source_artifact_sha256: Option<String>,
    pub runtime_status: String,
    pub fingerprint: String,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SealQaJudgeRequest {
    pub template_id: String,
    pub template_fingerprint: String,
    pub system: String,
    pub user: String,
    pub output_contract: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct FrontierManifest {
    pub capacity: u64,
    pub parent_selection: String,
    pub admission: String,
    pub eviction: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ScheduleManifest {
    pub epochs: f64,
    pub train_batch_policy: String,
    pub feedback_history: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ModelPin {
    pub role: String,
    pub paper_model: String,
    pub leaven_candidate_model: Option<String>,
    pub status: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PaperResultTarget {
    pub id: String,
    pub dataset_id: String,
    pub candidate_role: String,
    pub metric: String,
    pub tolerance: f64,
    pub value_percent: f64,
    pub source: String,
    pub status: PaperResultTargetStatus,
    pub ambiguity_group: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaperResultTargetStatus {
    Reported,
    AmbiguousCandidate,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ReplicationBlocker {
    pub id: String,
    pub description: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct EvoSkillFinalReport {
    pub schema_version: u32,
    pub exactness: ExactnessClass,
    pub exactness_gaps: Vec<ExactnessGapReport>,
    pub manifest: EvoSkillReplicaManifest,
    pub loop_report: Option<EvoSkillReplicaLoopReport>,
    pub live_run_gate: LiveRunGateReport,
    pub paper_close_gates: Vec<PaperCloseGateReport>,
    pub manifest_fingerprint: ManifestFingerprintReport,
    pub scorer_fingerprint: ScorerFingerprintReport,
    pub score_result_manifest: Option<ScoreResultManifestReport>,
    pub score_slots: Vec<FinalScoreSlot>,
    pub cost: FinalReportCost,
    pub errors: Vec<FinalReportError>,
    pub ablations: Vec<AblationStatusReport>,
    pub proxy_rejection_gates: Vec<ProxyRejectionGate>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ExactnessGapReport {
    pub id: String,
    pub dataset_id: Option<String>,
    pub status: ExactnessGapStatus,
    pub observed: String,
    pub required_for_paper_exact: String,
    pub paper_close_policy: String,
    pub evidence: Vec<String>,
    pub blocker_ids: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExactnessGapStatus {
    PaperReleaseUnverified,
    AcceptedPaperCloseSubstitute,
    BlockedBeforePaperClose,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PaperCloseGateReport {
    pub id: String,
    pub status: PaperCloseGateStatus,
    pub blocker_ids: Vec<String>,
    pub note: String,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaperCloseGateStatus {
    Proven,
    SourceBlocked,
    ApprovalBlocked,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ProxyRejectionGate {
    pub id: String,
    pub status: ProxyRejectionStatus,
    pub proxy: String,
    pub why_not: String,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProxyRejectionStatus {
    RejectedAsCompletionEvidence,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ManifestFingerprintReport {
    pub schema_version: u32,
    pub fingerprint: String,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ScorerFingerprintReport {
    pub scorer_id: String,
    pub fingerprint: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScoreEvidenceKind {
    RustScorerReplay,
    ExactAnswerReplay,
    ExternalJudgeRun,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ScoreResultManifestReport {
    pub relative_path: String,
    pub schema_version: u32,
    pub entries: u64,
    pub manifest_fingerprint: String,
    pub scorer_fingerprint: String,
    pub cost: FinalReportCost,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ScoreEvidenceArtifact {
    pub relative_path: String,
    pub sha256: String,
    pub bytes: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinalScoreStatus {
    Reported,
    NotRun,
    Blocked,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FinalScoreSlot {
    pub dataset_id: String,
    pub split_id: String,
    pub split_role: String,
    pub split_exactness: MaterializationExactness,
    pub split_fingerprint: Option<String>,
    pub role_source_id_fingerprint: Option<String>,
    pub candidate_role: String,
    pub paper_result_target_ids: Vec<String>,
    pub expected_rows: Option<u64>,
    pub score: Option<f64>,
    pub score_evidence_id: Option<String>,
    pub score_evidence_kind: Option<ScoreEvidenceKind>,
    pub score_evidence_approval_id: Option<String>,
    pub score_evidence_artifact: Option<ScoreEvidenceArtifact>,
    pub status: FinalScoreStatus,
    pub blocker_ids: Vec<String>,
}

struct FinalScoreSlotAudit {
    split_exactness: MaterializationExactness,
    split_fingerprint: Option<String>,
    role_source_id_fingerprint: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FinalReportCost {
    pub llm_calls: u64,
    pub metric_calls: u64,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FinalReportError {
    pub blocker_id: String,
    pub message: String,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AblationStatusReport {
    pub id: String,
    pub status: String,
    pub blocker_ids: Vec<String>,
    pub note: String,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LiveRunGateStatus {
    BlockedNoSpendApproval,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct LiveRunGateReport {
    pub status: LiveRunGateStatus,
    pub runtime_role: String,
    pub candidate_model: Option<String>,
    pub credential_probe_status: String,
    pub spend_approval_status: String,
    pub blocker_ids: Vec<String>,
    pub note: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct EvoSkillScoreReport {
    pub weighted_score: f64,
    pub is_failure: bool,
    pub tolerance_scores: Vec<ToleranceScore>,
}

impl EvoSkillScoreReport {
    #[must_use]
    pub fn tolerances(&self) -> Vec<f64> {
        self.tolerance_scores
            .iter()
            .map(|score| score.tolerance)
            .collect()
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ToleranceScore {
    pub tolerance: f64,
    pub weight: f64,
    pub score: f64,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct EvoSkillAnswerAttempt {
    pub source_id: String,
    pub ground_truth: String,
    pub prediction: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct EvoSkillScoredAttempt {
    pub source_id: String,
    pub ground_truth: String,
    pub prediction: String,
    pub score: EvoSkillScoreReport,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct EvoSkillFailureFeedbackRow {
    pub source_id: String,
    pub ground_truth: String,
    pub prediction: String,
    pub weighted_score: f64,
    pub feedback: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct EvoSkillReplicaLoopReport {
    pub exactness: MaterializationExactness,
    pub run_manifest: EvoSkillReplicaLoopRunManifest,
    pub frontier_capacity: u64,
    pub iterations: Vec<EvoSkillReplicaIterationReport>,
    pub feedback_history_rows: u64,
    pub final_frontier_members: Vec<CandidateId>,
    pub final_best_score: Option<f64>,
    pub checkpoint_resume: EvoSkillReplicaCheckpointResumeReport,
    pub proxy_rejection: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct EvoSkillReplicaLoopRunManifest {
    pub manifest_schema_version: u32,
    pub manifest_fingerprint: String,
    pub scorer_id: String,
    pub scorer_fingerprint: String,
    pub source_dataset_id: String,
    pub source_artifact_id: String,
    pub source_row_fingerprint: String,
    pub train_split_id: String,
    pub train_split_exactness: MaterializationExactness,
    pub train_split_fingerprint: String,
    pub train_role_source_id_fingerprint: String,
    pub train_rows: u64,
    pub validation_split_id: String,
    pub validation_split_fingerprint: String,
    pub validation_role_source_id_fingerprint: String,
    pub validation_rows: u64,
    pub validation_policy: String,
    pub sampler_policy: String,
    pub frontier_capacity: u64,
    pub parent_selection: String,
    pub admission_policy: String,
    pub eviction_policy: String,
    pub planned_iterations: u64,
    pub checkpoint_resume_after_iteration: u64,
    pub schedule_epochs: f64,
    pub schedule_train_batch_policy: String,
    pub schedule_feedback_history: String,
    pub git_identity_mode: String,
    pub runtime: String,
    pub validation_score_source: String,
    pub proof_limit: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct EvoSkillReplicaIterationReport {
    pub iteration: u64,
    pub selected_parent: CandidateId,
    pub train_sample_rows: u64,
    pub feedback_rows_seen: u64,
    pub new_feedback_rows: u64,
    pub child: CandidateId,
    pub parent_revision: String,
    pub change_expected_parent: String,
    pub child_revision: String,
    pub validation_rows_evaluated: u64,
    pub validation_score: f64,
    pub admitted: bool,
    pub frontier_members_after: Vec<CandidateId>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct EvoSkillReplicaCheckpointResumeReport {
    pub after_iteration: u64,
    pub frontier_before: Vec<CandidateId>,
    pub frontier_after: Vec<CandidateId>,
    pub parent_selector_cursor_before: usize,
    pub parent_selector_cursor_after: usize,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct EvoSkillReplicaLoopState {
    frontier: TopKFrontier,
    parent_selector: TopKParentSelector,
    train_sampler: CategoryRoundRobinSampler,
    candidates: Vec<EvoSkillReplicaCandidateState>,
    feedback_history: Vec<EvoSkillFailureFeedbackRow>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct EvoSkillReplicaCandidateState {
    id: CandidateId,
    program: GitProgramArtifact,
    validation_score: f64,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ScoreResultManifestFile {
    schema_version: u32,
    manifest_fingerprint: String,
    scorer_fingerprint: String,
    cost: FinalReportCost,
    entries: Vec<ScoreResultManifestEntry>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ScoreResultManifestEntry {
    dataset_id: String,
    split_id: String,
    split_role: String,
    candidate_role: String,
    split_fingerprint: Option<String>,
    role_source_id_fingerprint: Option<String>,
    expected_rows: Option<u64>,
    scored_rows: u64,
    score: f64,
    resolved_blocker_ids: Vec<String>,
    score_evidence_kind: ScoreEvidenceKind,
    score_evidence_approval_id: Option<String>,
    evidence_id: String,
    evidence_artifact: ScoreEvidenceArtifact,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ScoreEvidenceRow {
    source_id: String,
    prediction: String,
    score: f64,
    judge_template_fingerprint: Option<String>,
}

#[derive(Clone, Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct OfficeQaPredictionRow {
    dataset_id: String,
    split_id: String,
    split_role: String,
    candidate_role: String,
    source_id: String,
    prediction: String,
}

struct ValidatedScoreResultManifest {
    report: ScoreResultManifestReport,
    entries: Vec<ScoreResultManifestEntry>,
}

struct EvoSkillAgenticGitHarness {
    stores: GitProgramStores,
    seed_program: GitProgramArtifact,
    _temp: tempfile::TempDir,
}

#[derive(Debug, thiserror::Error)]
pub enum ManifestError {
    #[error("failed to read `{path}`: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to write `{path}`: {source}")]
    Write {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse CSV `{path}`: {source}")]
    Csv {
        path: PathBuf,
        #[source]
        source: csv::Error,
    },
    #[error("failed to build Leaven source rows: {source}")]
    Dataset {
        #[source]
        source: leaven_eval::DatasetError,
    },
    #[error("failed to build Leaven split materialization: {source}")]
    Split {
        #[source]
        source: leaven_eval::DatasetSplitsError,
    },
    #[error("failed to parse split manifest `{path}`: {source}")]
    SplitManifestJson {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("invalid split manifest `{path}`: {message}")]
    SplitManifest { path: PathBuf, message: String },
    #[error("failed to parse source pin manifest `{path}`: {source}")]
    SourcePinManifestJson {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("failed to serialize source pin manifest `{path}`: {source}")]
    SourcePinManifestSerialize {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("invalid source pin manifest `{path}`: {message}")]
    SourcePinManifest { path: PathBuf, message: String },
    #[error("failed to parse split policy manifest `{path}`: {source}")]
    SplitPolicyManifestJson {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("failed to serialize split policy manifest `{path}`: {source}")]
    SplitPolicyManifestSerialize {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("invalid split policy manifest `{path}`: {message}")]
    SplitPolicyManifest { path: PathBuf, message: String },
    #[error("failed to parse score result manifest `{path}`: {source}")]
    ScoreResultManifestJson {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("failed to serialize score result manifest `{path}`: {source}")]
    ScoreResultManifestSerialize {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("invalid score result manifest `{path}`: {message}")]
    ScoreResultManifest { path: PathBuf, message: String },
    #[error("failed to parse OfficeQA prediction rows `{path}` line {line}: {source}")]
    OfficeQaPredictionJson {
        path: PathBuf,
        line: usize,
        #[source]
        source: serde_json::Error,
    },
    #[error("invalid OfficeQA prediction rows `{path}`: {message}")]
    OfficeQaPrediction { path: PathBuf, message: String },
    #[error("failed to parse BrowseComp transfer sample `{path}` line {line}: {source}")]
    BrowseCompSampleJson {
        path: PathBuf,
        line: usize,
        #[source]
        source: serde_json::Error,
    },
    #[error("invalid BrowseComp transfer sample `{path}`: {message}")]
    BrowseCompSample { path: PathBuf, message: String },
    #[error("failed to materialize Parquet source rows: {source}")]
    Parquet {
        #[source]
        source: ParquetSourceRowError,
    },
    #[error("failed to build Leaven sampler: {source}")]
    Sampler {
        #[source]
        source: leaven_eval::SamplerError,
    },
    #[error("failed to build scalar evidence: {source}")]
    Scalar {
        #[source]
        source: leaven_evidence::ScalarEvidenceError,
    },
    #[error("failed to update Git program artifact: {source}")]
    Git {
        #[source]
        source: leaven_artifact_git::GitArtifactError,
    },
    #[error("failed in agentic Git adapter: {source}")]
    AgenticGit {
        #[source]
        source: GitAgenticGitError,
    },
    #[error("failed to allocate workspace for agentic Git readback: {source}")]
    WorkspaceFactory {
        #[source]
        source: leaven_workspace::FactoryError,
    },
    #[error("failed to use workspace for agentic Git readback: {source}")]
    Workspace {
        #[source]
        source: leaven_workspace::WorkspaceError,
    },
    #[error("failed to {action} `{path}`: {source}")]
    Io {
        action: &'static str,
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("git command `{args}` failed in `{cwd}`: {stderr}")]
    GitCommand {
        cwd: PathBuf,
        args: String,
        stderr: String,
    },
    #[error("failed to round-trip EvoSkill replica checkpoint: {source}")]
    Checkpoint {
        #[source]
        source: serde_json::Error,
    },
    #[error("failed to serialize EvoSkill replica manifest for fingerprinting: {source}")]
    ManifestSerialize {
        #[source]
        source: serde_json::Error,
    },
    #[error("EvoSkill replica loop is blocked: {reason}")]
    LoopBlocked { reason: String },
}

pub fn build_evoskill_replica_manifest(
    input: &ManifestBuildInput,
) -> Result<EvoSkillReplicaManifest, ManifestError> {
    let sources = materialize_evoskill_sources(input)?;
    build_evoskill_replica_manifest_from_sources(input, &sources)
}

pub fn write_evoskill_local_source_pin_manifest(
    input: &ManifestBuildInput,
) -> Result<PathBuf, ManifestError> {
    let path = input.root.join(SOURCE_PIN_MANIFEST_PATH);
    let manifest = local_source_pin_manifest(&input.root, &path)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| ManifestError::Write {
            path: parent.to_owned(),
            source,
        })?;
    }
    let bytes = serde_json::to_vec_pretty(&manifest).map_err(|source| {
        ManifestError::SourcePinManifestSerialize {
            path: path.clone(),
            source,
        }
    })?;
    fs::write(&path, bytes).map_err(|source| ManifestError::Write {
        path: path.clone(),
        source,
    })?;
    let _ = read_source_pin_manifest(&input.root)?;
    Ok(path)
}

pub fn write_evoskill_paper_close_split_policy_manifest(
    input: &ManifestBuildInput,
) -> Result<PathBuf, ManifestError> {
    let path = input.root.join(SPLIT_POLICY_MANIFEST_PATH);
    let sources = materialize_evoskill_sources(input)?;
    let reports = raw_source_materialization_reports(&sources);
    let manifest = paper_close_split_policy_manifest(&reports, &path)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| ManifestError::Write {
            path: parent.to_owned(),
            source,
        })?;
    }
    let bytes = serde_json::to_vec_pretty(&manifest).map_err(|source| {
        ManifestError::SplitPolicyManifestSerialize {
            path: path.clone(),
            source,
        }
    })?;
    fs::write(&path, bytes).map_err(|source| ManifestError::Write {
        path: path.clone(),
        source,
    })?;
    let _ = read_split_policy_manifest(&input.root, &sources)?;
    Ok(path)
}

pub fn write_evoskill_browsecomp_public_transfer_sample(
    input: &ManifestBuildInput,
    public_csv_path: impl AsRef<Path>,
) -> Result<PathBuf, ManifestError> {
    let public_csv_path = public_csv_path.as_ref();
    let public_csv_path = if public_csv_path.is_absolute() {
        public_csv_path.to_owned()
    } else {
        input.root.join(public_csv_path)
    };
    let path = input.root.join(BROWSECOMP_TRANSFER_SAMPLE_PATH);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| ManifestError::Write {
            path: parent.to_owned(),
            source,
        })?;
    }
    let records = browsecomp_public_transfer_sample_records(&public_csv_path)?;
    let mut jsonl = String::new();
    for record in records {
        let line = serde_json::to_string(&record).map_err(|source| {
            ManifestError::BrowseCompSampleJson {
                path: path.clone(),
                line: 0,
                source,
            }
        })?;
        jsonl.push_str(&line);
        jsonl.push('\n');
    }
    fs::write(&path, jsonl).map_err(|source| ManifestError::Write {
        path: path.clone(),
        source,
    })?;
    let rows = read_browsecomp_transfer_rows(&path)?;
    if rows.len() != BROWSECOMP_TRANSFER_ROWS {
        return Err(ManifestError::BrowseCompSample {
            path,
            message: format!(
                "generated public BrowseComp substitute has {} rows, expected {BROWSECOMP_TRANSFER_ROWS}",
                rows.len()
            ),
        });
    }
    Ok(path)
}

pub fn write_evoskill_officeqa_score_result_manifest(
    input: &ManifestBuildInput,
    predictions_path: impl AsRef<Path>,
) -> Result<PathBuf, ManifestError> {
    let predictions_path = root_relative_input_path(&input.root, predictions_path.as_ref());
    let predictions = read_officeqa_prediction_rows(&predictions_path)?;
    let sources = materialize_evoskill_sources(input)?;
    let manifest = build_evoskill_replica_manifest_from_sources(input, &sources)?;
    let manifest_fingerprint = manifest_fingerprint_report(&manifest)?;
    let scorer_fingerprint = scorer_fingerprint_report(&manifest.scorer);
    let mut slots = final_score_slots(&manifest);
    let path = input.root.join(SCORE_RESULT_MANIFEST_PATH);
    let score_manifest = officeqa_score_result_manifest_from_predictions(
        &input.root,
        &path,
        &sources,
        &slots,
        &manifest_fingerprint,
        &scorer_fingerprint,
        predictions,
    )?;
    write_score_result_manifest_file(&path, &score_manifest)?;
    let validated =
        read_score_result_manifest(&input.root, &manifest_fingerprint, &scorer_fingerprint)?
            .expect("score result manifest was just written");
    apply_score_result_manifest(
        &input.root,
        &sources,
        &manifest.scorer,
        &mut slots,
        &validated,
    )?;
    Ok(path)
}

fn root_relative_input_path(root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_owned()
    } else {
        root.join(path)
    }
}

fn build_evoskill_replica_manifest_from_sources(
    input: &ManifestBuildInput,
    sources: &EvoSkillSourceMaterializations,
) -> Result<EvoSkillReplicaManifest, ManifestError> {
    let source_pin_manifest = read_source_pin_manifest(&input.root)?;
    let split_policy_manifest = read_split_policy_manifest(&input.root, sources)?;
    let source_revisions = source_revisions(&input.root, source_pin_manifest.as_ref());
    let artifacts = source_artifacts(&input.root)?;
    let source_materializations =
        source_materialization_reports(sources, split_policy_manifest.as_ref());
    let datasets = dataset_requirements(&source_materializations);
    let source_universe = source_universe(&datasets, &source_materializations);
    let scorer = scorer_manifest(&artifacts);
    let source_blockers = source_blockers(
        &input.root,
        &source_materializations,
        source_pin_manifest.is_some(),
    )?;
    let exactness = replica_manifest_exactness(&source_blockers);
    let blockers = blockers(&source_blockers);

    Ok(EvoSkillReplicaManifest {
        schema_version: 13,
        paper: PaperTarget {
            id: "evoskill".to_owned(),
            arxiv_id: "2603.02766".to_owned(),
            title: "EvoSkill".to_owned(),
        },
        exactness,
        source_revisions,
        artifacts,
        source_blockers,
        datasets,
        source_universe,
        source_materializations,
        scorer,
        frontier: frontier_manifest(),
        schedule: schedule_manifest(),
        model_pins: model_pins(),
        paper_result_targets: paper_result_targets(),
        blockers,
        proxy_rejections: proxy_rejections(),
    })
}

fn replica_manifest_exactness(source_blockers: &[SourceBlockerReport]) -> ExactnessClass {
    if source_blockers.is_empty() {
        ExactnessClass::PaperCloseCandidate
    } else {
        ExactnessClass::BlockedBeforePaperClose
    }
}

pub fn build_evoskill_final_report(
    input: &ManifestBuildInput,
) -> Result<EvoSkillFinalReport, ManifestError> {
    let sources = materialize_evoskill_sources(input)?;
    let manifest = build_evoskill_replica_manifest_from_sources(input, &sources)?;
    let manifest_fingerprint = manifest_fingerprint_report(&manifest)?;
    let scorer_fingerprint = scorer_fingerprint_report(&manifest.scorer);
    let loop_report = if let Some(officeqa) = &sources.officeqa {
        Some(run_evoskill_replica_mechanics_with_manifest(
            &manifest,
            &manifest_fingerprint,
            &scorer_fingerprint,
            officeqa,
        )?)
    } else {
        None
    };
    let mut score_slots = final_score_slots(&manifest);
    let score_result_manifest =
        read_score_result_manifest(&input.root, &manifest_fingerprint, &scorer_fingerprint)?;
    let cost = if let Some(score_result_manifest) = &score_result_manifest {
        apply_score_result_manifest(
            &input.root,
            &sources,
            &manifest.scorer,
            &mut score_slots,
            score_result_manifest,
        )?;
        score_result_manifest.report.cost.clone()
    } else {
        FinalReportCost::default()
    };
    let live_run_gate = final_report_live_run_gate(&manifest);
    let report_blockers = final_report_blockers(&manifest, &score_slots);
    let paper_close_gates = final_report_paper_close_gates(
        &manifest,
        loop_report.as_ref(),
        &live_run_gate,
        &score_slots,
    );
    let errors = final_report_errors(&report_blockers);
    let ablations = final_report_ablations(&manifest, &score_slots);
    let exactness = manifest.exactness.clone();
    let exactness_gaps = final_report_exactness_gaps(&manifest);
    let proxy_rejection_gates = proxy_rejection_gates();

    Ok(EvoSkillFinalReport {
        schema_version: 19,
        exactness,
        exactness_gaps,
        manifest,
        loop_report,
        live_run_gate,
        paper_close_gates,
        manifest_fingerprint,
        scorer_fingerprint,
        score_result_manifest: score_result_manifest.map(|manifest| manifest.report),
        score_slots,
        cost,
        errors,
        ablations,
        proxy_rejection_gates,
    })
}

#[derive(Debug, serde::Deserialize)]
struct OfficeQaCsvRecord {
    uid: String,
    question: String,
    answer: String,
    source_docs: String,
    source_files: String,
    difficulty: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct BrowseCompTransferJsonlRecord {
    source_id: String,
    question: String,
    answer: String,
    #[serde(default)]
    stratum: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct BrowseCompPublicCsvRecord {
    problem: String,
    answer: String,
    problem_topic: String,
    canary: String,
}

#[derive(Clone, Debug)]
struct BrowseCompPublicPlainRow {
    source_id: String,
    question: String,
    answer: String,
    stratum: String,
}

#[derive(Debug, serde::Deserialize)]
struct PaperSplitManifestFile {
    schema_version: u32,
    dataset_id: String,
    split_id: String,
    #[serde(default)]
    roles: PaperSplitRoles,
}

#[derive(Debug)]
struct ValidatedSourcePinManifest {
    policy: SourcePinPolicy,
    sources: BTreeMap<String, SourcePinManifestEntry>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum SourcePinPolicy {
    LocalCheckoutPinned,
}

impl SourcePinPolicy {
    const fn as_str(self) -> &'static str {
        match self {
            Self::LocalCheckoutPinned => "local_checkout_pinned",
        }
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct SourcePinManifestFile {
    schema_version: u32,
    policy: SourcePinPolicy,
    #[serde(default)]
    sources: Vec<SourcePinManifestEntry>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct SourcePinManifestEntry {
    id: String,
    relative_path: String,
    head: String,
    branch: String,
    remote_url: String,
}

#[derive(Debug)]
struct ValidatedSplitPolicyManifest {
    policy: SplitPolicy,
    splits: BTreeMap<(String, String), SplitPolicyManifestEntry>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum SplitPolicy {
    AcceptDocumentedPaperCloseSubstitutes,
}

impl SplitPolicy {
    const fn acceptance_status(self) -> SplitAcceptanceStatus {
        match self {
            Self::AcceptDocumentedPaperCloseSubstitutes => {
                SplitAcceptanceStatus::AcceptedPaperClosePolicy
            }
        }
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct SplitPolicyManifestFile {
    schema_version: u32,
    policy: SplitPolicy,
    #[serde(default)]
    splits: Vec<SplitPolicyManifestEntry>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct SplitPolicyManifestEntry {
    dataset_id: String,
    split_id: String,
    source_row_fingerprint: String,
    split_fingerprint: String,
    #[serde(default)]
    roles: Vec<SplitPolicyRoleEntry>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct SplitPolicyRoleEntry {
    role: String,
    rows: u64,
    source_id_fingerprint: String,
}

#[derive(Default, Debug, serde::Deserialize)]
struct PaperSplitRoles {
    #[serde(default)]
    train: Vec<String>,
    #[serde(default)]
    validation: Vec<String>,
    #[serde(default, alias = "test")]
    held_out_test: Vec<String>,
}

pub fn materialize_officeqa_source(
    input: &ManifestBuildInput,
) -> Result<Option<OfficeQaSourceMaterialization>, ManifestError> {
    let path = input.root.join("tmp/repros/officeqa/officeqa_full.csv");
    if !path.exists() {
        return Ok(None);
    }

    let mut reader = csv::ReaderBuilder::new()
        .trim(csv::Trim::All)
        .from_path(&path)
        .map_err(|source| ManifestError::Csv {
            path: path.clone(),
            source,
        })?;
    let mut rows = Vec::new();
    for record in reader.deserialize::<OfficeQaCsvRecord>() {
        let record = record.map_err(|source| ManifestError::Csv {
            path: path.clone(),
            source,
        })?;
        if record.uid.trim().is_empty() {
            continue;
        }
        rows.push(SourceRow::targeted(
            record.uid,
            OfficeQaInput {
                question: record.question,
                source_docs: record.source_docs,
                source_files: record.source_files,
                difficulty: record.difficulty,
            },
            record.answer,
        ));
    }

    let rows = SourceRowManifest::new(rows).map_err(|source| ManifestError::Dataset { source })?;
    let dataset = rows
        .clone()
        .into_dataset()
        .map_err(|source| ManifestError::Dataset { source })?;
    let strata = officeqa_difficulty_strata(&rows);
    let split_manifest_path = input.root.join(OFFICEQA_EXACT_SPLIT_MANIFEST_PATH);
    let split_materializations =
        if let Some(manifest) = read_paper_split_manifest(&split_manifest_path, "officeqa")? {
            vec![officeqa_paper_declared_split_report(
                &rows,
                &manifest,
                &split_manifest_path,
            )?]
        } else {
            officeqa_difficulty_split_reports(&rows, &strata)?
        };
    let blocker_ids = materialization_blocker_ids(&split_materializations);
    let report = DatasetMaterializationReport {
        dataset_id: "officeqa".to_owned(),
        source_artifact_id: "officeqa_full_csv".to_owned(),
        source_status: SourceMaterializationStatus::Materialized,
        source_rows: Some(u64::try_from(rows.len()).expect("row count fits in u64")),
        case_rows: Some(u64::try_from(dataset.cases().len()).expect("case count fits in u64")),
        source_row_fingerprint: Some(fingerprint_hex(rows.fingerprint())),
        source_artifact_sha256: Some(sha256_file(&path)?),
        target_policy: "answers are scorer targets only; runner inputs exclude answers".to_owned(),
        strata: strata_reports(&strata),
        split_materializations,
        blocker_ids,
    };
    Ok(Some(OfficeQaSourceMaterialization { rows, report }))
}

fn materialize_evoskill_sources(
    input: &ManifestBuildInput,
) -> Result<EvoSkillSourceMaterializations, ManifestError> {
    Ok(EvoSkillSourceMaterializations {
        officeqa: materialize_officeqa_source(input)?,
        sealqa: materialize_sealqa_source(input)?,
        browsecomp_transfer: materialize_browsecomp_transfer_source(input)?,
    })
}

fn source_materialization_reports(
    sources: &EvoSkillSourceMaterializations,
    split_policy_manifest: Option<&ValidatedSplitPolicyManifest>,
) -> Vec<DatasetMaterializationReport> {
    raw_source_materialization_reports(sources)
        .into_iter()
        .map(|report| apply_split_policy_manifest(report, split_policy_manifest))
        .collect()
}

fn raw_source_materialization_reports(
    sources: &EvoSkillSourceMaterializations,
) -> Vec<DatasetMaterializationReport> {
    let officeqa = sources.officeqa.as_ref().map_or_else(
        || {
            missing_materialization_report(
                "officeqa",
                "officeqa_full_csv",
                "officeqa_full_csv_missing",
            )
        },
        |materialization| materialization.report.clone(),
    );
    let sealqa = sources.sealqa.as_ref().map_or_else(
        || missing_materialization_report("sealqa", "sealqa_parquet", "sealqa_parquet_missing"),
        |materialization| materialization.report.clone(),
    );
    let browsecomp_transfer = sources.browsecomp_transfer.as_ref().map_or_else(
        || {
            missing_materialization_report(
                "browsecomp_transfer",
                "browsecomp_transfer_sample",
                "browsecomp_transfer_sample",
            )
        },
        |materialization| materialization.report.clone(),
    );
    vec![officeqa, sealqa, browsecomp_transfer]
}

fn apply_split_policy_manifest(
    mut report: DatasetMaterializationReport,
    split_policy_manifest: Option<&ValidatedSplitPolicyManifest>,
) -> DatasetMaterializationReport {
    let Some(split_policy_manifest) = split_policy_manifest else {
        return report;
    };
    match split_policy_manifest.policy {
        SplitPolicy::AcceptDocumentedPaperCloseSubstitutes => {}
    }
    let mut accepted_substitute = false;
    for split in &mut report.split_materializations {
        if split.exactness == MaterializationExactness::PaperCloseSubstitute
            && split_policy_manifest
                .splits
                .contains_key(&(report.dataset_id.clone(), split.id.clone()))
        {
            split.blocker_ids.clear();
            split.acceptance_status = split_policy_manifest.policy.acceptance_status();
            accepted_substitute = true;
        }
    }
    if accepted_substitute {
        report.blocker_ids = materialization_blocker_ids(&report.split_materializations);
    }
    report
}

fn source_universe(
    datasets: &[DatasetRequirement],
    materializations: &[DatasetMaterializationReport],
) -> Vec<SourceUniverseEntry> {
    datasets
        .iter()
        .map(|dataset| {
            let materialization = materializations
                .iter()
                .find(|materialization| materialization.dataset_id == dataset.id);
            let mut blocker_ids = if materialization.is_some_and(has_paper_exact_split) {
                Vec::new()
            } else {
                dataset.blocker_ids.clone()
            };
            if let Some(materialization) = materialization {
                extend_unique(&mut blocker_ids, &materialization.blocker_ids);
            }
            SourceUniverseEntry {
                dataset_id: dataset.id.clone(),
                source_revision_ids: source_revision_ids_for_dataset(&dataset.id),
                source_artifact_ids: source_artifact_ids_for_dataset(&dataset.id, materialization),
                paper_rows: dataset.paper_rows,
                materialized_rows: materialization.and_then(|report| report.source_rows),
                source_row_fingerprint: materialization
                    .and_then(|report| report.source_row_fingerprint.clone()),
                split_ids: materialization
                    .map(|report| {
                        report
                            .split_materializations
                            .iter()
                            .map(|split| split.id.clone())
                            .collect()
                    })
                    .unwrap_or_default(),
                split_exactness: materialization
                    .map(|report| {
                        report
                            .split_materializations
                            .iter()
                            .map(|split| split.exactness.clone())
                            .collect()
                    })
                    .unwrap_or_default(),
                blocker_ids,
            }
        })
        .collect()
}

fn source_revision_ids_for_dataset(dataset_id: &str) -> Vec<String> {
    match dataset_id {
        "officeqa" => vec!["officeqa_repo".to_owned()],
        _ => Vec::new(),
    }
}

fn source_artifact_ids_for_dataset(
    dataset_id: &str,
    materialization: Option<&DatasetMaterializationReport>,
) -> Vec<String> {
    if let Some(materialization) = materialization {
        let mut artifact_ids = vec![materialization.source_artifact_id.clone()];
        if has_paper_exact_split(materialization) {
            match dataset_id {
                "officeqa" => artifact_ids.push("officeqa_exact_split_manifest".to_owned()),
                "sealqa" => artifact_ids.push("sealqa_exact_split_manifest".to_owned()),
                _ => {}
            }
        } else if has_accepted_or_exact_split(materialization) {
            artifact_ids.push("split_policy_manifest".to_owned());
        }
        return artifact_ids;
    }
    match dataset_id {
        "officeqa" => vec!["officeqa_full_csv".to_owned()],
        "sealqa" => vec!["sealqa_parquet".to_owned()],
        "browsecomp_transfer" => vec!["browsecomp_transfer_sample".to_owned()],
        _ => Vec::new(),
    }
}

fn has_paper_exact_split(materialization: &DatasetMaterializationReport) -> bool {
    materialization
        .split_materializations
        .iter()
        .any(|split| split.exactness == MaterializationExactness::PaperExact)
}

fn has_accepted_or_exact_split(materialization: &DatasetMaterializationReport) -> bool {
    materialization.split_materializations.iter().any(|split| {
        split.exactness == MaterializationExactness::PaperExact
            || split.acceptance_status == SplitAcceptanceStatus::AcceptedPaperClosePolicy
    })
}

fn extend_unique(values: &mut Vec<String>, additions: &[String]) {
    for addition in additions {
        if !values.contains(addition) {
            values.push(addition.clone());
        }
    }
}

fn officeqa_difficulty_strata(
    rows: &SourceRowManifest<OfficeQaInput, String>,
) -> BTreeMap<SmolStr, Vec<CaseId>> {
    let mut strata = BTreeMap::<SmolStr, Vec<CaseId>>::new();
    for (row_index, row) in rows.rows().iter().enumerate() {
        strata
            .entry(SmolStr::new(row.input().difficulty.as_str()))
            .or_default()
            .push(CaseId::from_index(row_index));
    }
    strata
}

fn officeqa_difficulty_split_reports(
    rows: &SourceRowManifest<OfficeQaInput, String>,
    strata: &BTreeMap<SmolStr, Vec<CaseId>>,
) -> Result<Vec<SplitMaterializationReport>, ManifestError> {
    let mut reports = Vec::new();
    for train_rows in OFFICEQA_TRAIN_SIZES {
        reports.push(officeqa_difficulty_split_report(rows, strata, train_rows)?);
    }
    Ok(reports)
}

fn officeqa_paper_declared_split_report(
    rows: &SourceRowManifest<OfficeQaInput, String>,
    manifest: &PaperSplitManifestFile,
    path: &Path,
) -> Result<SplitMaterializationReport, ManifestError> {
    paper_declared_split_report(
        rows,
        manifest,
        path,
        &[
            (SplitRole::Train, "train", manifest.roles.train.as_slice()),
            (
                SplitRole::Validation,
                "validation",
                manifest.roles.validation.as_slice(),
            ),
            (
                SplitRole::Test,
                "held_out_test",
                manifest.roles.held_out_test.as_slice(),
            ),
        ],
    )
}

fn read_paper_split_manifest(
    path: &Path,
    expected_dataset_id: &str,
) -> Result<Option<PaperSplitManifestFile>, ManifestError> {
    if !path.exists() {
        return Ok(None);
    }
    let bytes = fs::read(path).map_err(|source| ManifestError::Read {
        path: path.to_owned(),
        source,
    })?;
    let manifest = serde_json::from_slice::<PaperSplitManifestFile>(&bytes).map_err(|source| {
        ManifestError::SplitManifestJson {
            path: path.to_owned(),
            source,
        }
    })?;
    if manifest.schema_version != 1 {
        return Err(ManifestError::SplitManifest {
            path: path.to_owned(),
            message: format!(
                "expected schema_version 1, found {}",
                manifest.schema_version
            ),
        });
    }
    if manifest.dataset_id != expected_dataset_id {
        return Err(ManifestError::SplitManifest {
            path: path.to_owned(),
            message: format!(
                "expected dataset_id `{expected_dataset_id}`, found `{}`",
                manifest.dataset_id
            ),
        });
    }
    if manifest.split_id.trim().is_empty() {
        return Err(ManifestError::SplitManifest {
            path: path.to_owned(),
            message: "split_id must not be empty".to_owned(),
        });
    }
    Ok(Some(manifest))
}

fn read_source_pin_manifest(
    root: &Path,
) -> Result<Option<ValidatedSourcePinManifest>, ManifestError> {
    let path = root.join(SOURCE_PIN_MANIFEST_PATH);
    if !path.exists() {
        return Ok(None);
    }
    let bytes = fs::read(&path).map_err(|source| ManifestError::Read {
        path: path.clone(),
        source,
    })?;
    let manifest = serde_json::from_slice::<SourcePinManifestFile>(&bytes).map_err(|source| {
        ManifestError::SourcePinManifestJson {
            path: path.clone(),
            source,
        }
    })?;
    validate_source_pin_manifest(root, &path, manifest).map(Some)
}

fn local_source_pin_manifest(
    root: &Path,
    manifest_path: &Path,
) -> Result<SourcePinManifestFile, ManifestError> {
    let sources = SOURCE_REVISION_SPECS
        .iter()
        .map(|(id, relative_path)| local_source_pin_entry(root, manifest_path, id, relative_path))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(SourcePinManifestFile {
        schema_version: 1,
        policy: SourcePinPolicy::LocalCheckoutPinned,
        sources,
    })
}

fn local_source_pin_entry(
    root: &Path,
    manifest_path: &Path,
    id: &str,
    relative_path: &str,
) -> Result<SourcePinManifestEntry, ManifestError> {
    let checkout = root.join(relative_path);
    if !checkout.exists() {
        return Err(ManifestError::SourcePinManifest {
            path: manifest_path.to_owned(),
            message: format!("source `{id}` path is missing"),
        });
    }
    if !checkout.join(".git").exists() {
        return Err(ManifestError::SourcePinManifest {
            path: manifest_path.to_owned(),
            message: format!("source `{id}` is not a git checkout"),
        });
    }
    Ok(SourcePinManifestEntry {
        id: id.to_owned(),
        relative_path: relative_path.to_owned(),
        head: required_git_stdout(manifest_path, &checkout, id, &["rev-parse", "HEAD"])?,
        branch: required_git_stdout(manifest_path, &checkout, id, &["branch", "--show-current"])?,
        remote_url: required_git_stdout(
            manifest_path,
            &checkout,
            id,
            &["config", "--get", "remote.origin.url"],
        )?,
    })
}

fn validate_source_pin_manifest(
    root: &Path,
    path: &Path,
    manifest: SourcePinManifestFile,
) -> Result<ValidatedSourcePinManifest, ManifestError> {
    if manifest.schema_version != 1 {
        return Err(ManifestError::SourcePinManifest {
            path: path.to_owned(),
            message: format!(
                "expected schema_version 1, found {}",
                manifest.schema_version
            ),
        });
    }

    let mut sources = BTreeMap::new();
    for entry in manifest.sources {
        let expected_relative_path = source_revision_relative_path(&entry.id).ok_or_else(|| {
            ManifestError::SourcePinManifest {
                path: path.to_owned(),
                message: format!("unknown source id `{}`", entry.id),
            }
        })?;
        if entry.relative_path != expected_relative_path {
            return Err(ManifestError::SourcePinManifest {
                path: path.to_owned(),
                message: format!(
                    "source `{}` expected relative_path `{expected_relative_path}`, found `{}`",
                    entry.id, entry.relative_path
                ),
            });
        }
        validate_source_pin_entry(root, path, &entry)?;
        if sources.insert(entry.id.clone(), entry).is_some() {
            return Err(ManifestError::SourcePinManifest {
                path: path.to_owned(),
                message: "source ids must be unique".to_owned(),
            });
        }
    }

    for (id, _) in SOURCE_REVISION_SPECS {
        if !sources.contains_key(id) {
            return Err(ManifestError::SourcePinManifest {
                path: path.to_owned(),
                message: format!("missing source pin for `{id}`"),
            });
        }
    }

    Ok(ValidatedSourcePinManifest {
        policy: manifest.policy,
        sources,
    })
}

fn validate_source_pin_entry(
    root: &Path,
    path: &Path,
    entry: &SourcePinManifestEntry,
) -> Result<(), ManifestError> {
    let checkout = root.join(&entry.relative_path);
    if !checkout.exists() {
        return Err(ManifestError::SourcePinManifest {
            path: path.to_owned(),
            message: format!("source `{}` path is missing", entry.id),
        });
    }
    if !checkout.join(".git").exists() {
        return Err(ManifestError::SourcePinManifest {
            path: path.to_owned(),
            message: format!("source `{}` is not a git checkout", entry.id),
        });
    }
    let head = required_git_stdout(path, &checkout, &entry.id, &["rev-parse", "HEAD"])?;
    if head != entry.head {
        return Err(ManifestError::SourcePinManifest {
            path: path.to_owned(),
            message: format!(
                "source `{}` head mismatch: expected `{}`, found `{head}`",
                entry.id, entry.head
            ),
        });
    }
    let branch = required_git_stdout(path, &checkout, &entry.id, &["branch", "--show-current"])?;
    if branch != entry.branch {
        return Err(ManifestError::SourcePinManifest {
            path: path.to_owned(),
            message: format!(
                "source `{}` branch mismatch: expected `{}`, found `{branch}`",
                entry.id, entry.branch
            ),
        });
    }
    let remote_url = required_git_stdout(
        path,
        &checkout,
        &entry.id,
        &["config", "--get", "remote.origin.url"],
    )?;
    if remote_url != entry.remote_url {
        return Err(ManifestError::SourcePinManifest {
            path: path.to_owned(),
            message: format!(
                "source `{}` remote_url mismatch: expected `{}`, found `{remote_url}`",
                entry.id, entry.remote_url
            ),
        });
    }
    Ok(())
}

fn required_git_stdout(
    manifest_path: &Path,
    checkout: &Path,
    source_id: &str,
    args: &[&str],
) -> Result<String, ManifestError> {
    git_stdout(checkout, args).ok_or_else(|| ManifestError::SourcePinManifest {
        path: manifest_path.to_owned(),
        message: format!(
            "source `{source_id}` git command `{}` failed",
            args.join(" ")
        ),
    })
}

fn source_revision_relative_path(id: &str) -> Option<&'static str> {
    SOURCE_REVISION_SPECS
        .iter()
        .find_map(|(source_id, relative_path)| (*source_id == id).then_some(*relative_path))
}

fn read_split_policy_manifest(
    root: &Path,
    sources: &EvoSkillSourceMaterializations,
) -> Result<Option<ValidatedSplitPolicyManifest>, ManifestError> {
    let path = root.join(SPLIT_POLICY_MANIFEST_PATH);
    if !path.exists() {
        return Ok(None);
    }
    let bytes = fs::read(&path).map_err(|source| ManifestError::Read {
        path: path.clone(),
        source,
    })?;
    let manifest = serde_json::from_slice::<SplitPolicyManifestFile>(&bytes).map_err(|source| {
        ManifestError::SplitPolicyManifestJson {
            path: path.clone(),
            source,
        }
    })?;
    let reports = raw_source_materialization_reports(sources);
    validate_split_policy_manifest(&path, manifest, &reports).map(Some)
}

fn paper_close_split_policy_manifest(
    reports: &[DatasetMaterializationReport],
    path: &Path,
) -> Result<SplitPolicyManifestFile, ManifestError> {
    let splits = paper_close_substitute_split_entries(reports)?;
    if splits.is_empty() {
        return Err(ManifestError::SplitPolicyManifest {
            path: path.to_owned(),
            message: "no materialized paper-close substitute splits are available".to_owned(),
        });
    }
    Ok(SplitPolicyManifestFile {
        schema_version: 1,
        policy: SplitPolicy::AcceptDocumentedPaperCloseSubstitutes,
        splits,
    })
}

fn validate_split_policy_manifest(
    path: &Path,
    manifest: SplitPolicyManifestFile,
    reports: &[DatasetMaterializationReport],
) -> Result<ValidatedSplitPolicyManifest, ManifestError> {
    if manifest.schema_version != 1 {
        return Err(ManifestError::SplitPolicyManifest {
            path: path.to_owned(),
            message: format!(
                "expected schema_version 1, found {}",
                manifest.schema_version
            ),
        });
    }
    let expected = paper_close_substitute_split_entries(reports)?
        .into_iter()
        .map(|entry| ((entry.dataset_id.clone(), entry.split_id.clone()), entry))
        .collect::<BTreeMap<_, _>>();
    if expected.is_empty() {
        return Err(ManifestError::SplitPolicyManifest {
            path: path.to_owned(),
            message: "no materialized paper-close substitute splits are available".to_owned(),
        });
    }

    let mut splits = BTreeMap::new();
    for entry in manifest.splits {
        let key = (entry.dataset_id.clone(), entry.split_id.clone());
        let expected_entry = expected.get(&key).ok_or_else(|| {
            ManifestError::SplitPolicyManifest {
                path: path.to_owned(),
                message: format!(
                    "split policy references unknown or non-substitute split `{}` for dataset `{}`",
                    entry.split_id, entry.dataset_id
                ),
            }
        })?;
        validate_split_policy_entry(path, &entry, expected_entry)?;
        if splits.insert(key, entry).is_some() {
            return Err(ManifestError::SplitPolicyManifest {
                path: path.to_owned(),
                message: "split policy entries must be unique by dataset_id and split_id"
                    .to_owned(),
            });
        }
    }

    for key in expected.keys() {
        if !splits.contains_key(key) {
            return Err(ManifestError::SplitPolicyManifest {
                path: path.to_owned(),
                message: format!(
                    "missing split policy entry for dataset `{}` split `{}`",
                    key.0, key.1
                ),
            });
        }
    }

    Ok(ValidatedSplitPolicyManifest {
        policy: manifest.policy,
        splits,
    })
}

fn validate_split_policy_entry(
    path: &Path,
    actual: &SplitPolicyManifestEntry,
    expected: &SplitPolicyManifestEntry,
) -> Result<(), ManifestError> {
    if actual.source_row_fingerprint != expected.source_row_fingerprint {
        return Err(split_policy_mismatch(
            path,
            actual,
            "source_row_fingerprint",
            &expected.source_row_fingerprint,
            &actual.source_row_fingerprint,
        ));
    }
    if actual.split_fingerprint != expected.split_fingerprint {
        return Err(split_policy_mismatch(
            path,
            actual,
            "split_fingerprint",
            &expected.split_fingerprint,
            &actual.split_fingerprint,
        ));
    }
    if actual.roles.len() != expected.roles.len() {
        return Err(ManifestError::SplitPolicyManifest {
            path: path.to_owned(),
            message: format!(
                "split `{}` for dataset `{}` expected {} roles, found {}",
                actual.split_id,
                actual.dataset_id,
                expected.roles.len(),
                actual.roles.len()
            ),
        });
    }
    let expected_roles = expected
        .roles
        .iter()
        .map(|role| (role.role.as_str(), role))
        .collect::<BTreeMap<_, _>>();
    for actual_role in &actual.roles {
        let expected_role = expected_roles
            .get(actual_role.role.as_str())
            .ok_or_else(|| ManifestError::SplitPolicyManifest {
                path: path.to_owned(),
                message: format!(
                    "split `{}` for dataset `{}` references unknown role `{}`",
                    actual.split_id, actual.dataset_id, actual_role.role
                ),
            })?;
        if actual_role.rows != expected_role.rows {
            return Err(ManifestError::SplitPolicyManifest {
                path: path.to_owned(),
                message: format!(
                    "split `{}` for dataset `{}` role `{}` rows mismatch: expected {}, found {}",
                    actual.split_id,
                    actual.dataset_id,
                    actual_role.role,
                    expected_role.rows,
                    actual_role.rows
                ),
            });
        }
        if actual_role.source_id_fingerprint != expected_role.source_id_fingerprint {
            return Err(ManifestError::SplitPolicyManifest {
                path: path.to_owned(),
                message: format!(
                    "split `{}` for dataset `{}` role `{}` source_id_fingerprint mismatch",
                    actual.split_id, actual.dataset_id, actual_role.role
                ),
            });
        }
    }
    Ok(())
}

fn split_policy_mismatch(
    path: &Path,
    entry: &SplitPolicyManifestEntry,
    field: &str,
    expected: &str,
    actual: &str,
) -> ManifestError {
    ManifestError::SplitPolicyManifest {
        path: path.to_owned(),
        message: format!(
            "split `{}` for dataset `{}` {field} mismatch: expected `{expected}`, found `{actual}`",
            entry.split_id, entry.dataset_id
        ),
    }
}

fn paper_close_substitute_split_entries(
    reports: &[DatasetMaterializationReport],
) -> Result<Vec<SplitPolicyManifestEntry>, ManifestError> {
    let mut entries = Vec::new();
    for report in reports {
        let Some(source_row_fingerprint) = &report.source_row_fingerprint else {
            continue;
        };
        for split in &report.split_materializations {
            if split.exactness != MaterializationExactness::PaperCloseSubstitute {
                continue;
            }
            let split_fingerprint = split.split_fingerprint.clone().ok_or_else(|| {
                ManifestError::SplitPolicyManifest {
                    path: PathBuf::from(SPLIT_POLICY_MANIFEST_PATH),
                    message: format!(
                        "split `{}` for dataset `{}` has no split fingerprint",
                        split.id, report.dataset_id
                    ),
                }
            })?;
            entries.push(SplitPolicyManifestEntry {
                dataset_id: report.dataset_id.clone(),
                split_id: split.id.clone(),
                source_row_fingerprint: source_row_fingerprint.clone(),
                split_fingerprint,
                roles: split
                    .role_manifests
                    .iter()
                    .map(|role| SplitPolicyRoleEntry {
                        role: role.role.clone(),
                        rows: role.rows,
                        source_id_fingerprint: role.source_id_fingerprint.clone(),
                    })
                    .collect(),
            });
        }
    }
    Ok(entries)
}

fn paper_declared_split_report<T>(
    rows: &SourceRowManifest<T, String>,
    manifest: &PaperSplitManifestFile,
    path: &Path,
    roles: &[(SplitRole, &'static str, &[String])],
) -> Result<SplitMaterializationReport, ManifestError> {
    let source_to_case = source_id_case_map(rows);
    let mut builder = ExplicitSplitBuilder::new(rows.ordered_case_ids());
    let mut declared_rows = 0_usize;
    for (role, label, source_ids) in roles {
        if source_ids.is_empty() {
            return Err(ManifestError::SplitManifest {
                path: path.to_owned(),
                message: format!("required role `{label}` has no source ids"),
            });
        }
        let cases = split_manifest_case_ids(path, &source_to_case, label, source_ids)?;
        declared_rows += cases.len();
        builder = builder.role_cases(role.clone(), cases);
    }
    if declared_rows != rows.len() {
        return Err(ManifestError::SplitManifest {
            path: path.to_owned(),
            message: format!(
                "declared split covers {declared_rows} rows, but source manifest has {} rows",
                rows.len()
            ),
        });
    }

    let splits = builder
        .build(CaseSetVersion(format!(
            "{}-paper-declared-source-id-v1-rows-{}",
            manifest.dataset_id,
            rows.len()
        )))
        .map_err(|source| ManifestError::Split { source })?;
    let role_labels = roles
        .iter()
        .map(|(role, label, _)| (role.clone(), *label))
        .collect::<Vec<_>>();
    let role_manifests = split_role_manifests(rows, &splits, &role_labels)?;

    Ok(SplitMaterializationReport {
        id: manifest.split_id.clone(),
        method: PAPER_DECLARED_SOURCE_ID_MANIFEST_METHOD.to_owned(),
        exactness: MaterializationExactness::PaperExact,
        train_rows: split_len(&splits, &SplitRole::Train),
        validation_rows: roles
            .iter()
            .any(|(_, label, _)| *label == "validation")
            .then(|| split_len(&splits, &SplitRole::Validation)),
        test_rows: roles
            .iter()
            .any(|(_, label, _)| *label == "held_out_test")
            .then(|| split_len(&splits, &SplitRole::Test)),
        split_fingerprint: Some(fingerprint_hex(splits.fingerprint())),
        role_manifests,
        blocker_ids: Vec::new(),
        acceptance_status: SplitAcceptanceStatus::NotRequired,
    })
}

fn source_id_case_map<T>(rows: &SourceRowManifest<T, String>) -> BTreeMap<String, CaseId> {
    rows.rows()
        .iter()
        .enumerate()
        .map(|(row_index, row)| (row.source_id().to_owned(), CaseId::from_index(row_index)))
        .collect()
}

fn split_manifest_case_ids(
    path: &Path,
    source_to_case: &BTreeMap<String, CaseId>,
    role: &str,
    source_ids: &[String],
) -> Result<Vec<CaseId>, ManifestError> {
    source_ids
        .iter()
        .map(|source_id| {
            source_to_case
                .get(source_id)
                .copied()
                .ok_or_else(|| ManifestError::SplitManifest {
                    path: path.to_owned(),
                    message: format!("role `{role}` references unknown source id `{source_id}`"),
                })
        })
        .collect()
}

fn materialization_blocker_ids(splits: &[SplitMaterializationReport]) -> Vec<String> {
    let mut blocker_ids = Vec::new();
    for split in splits {
        extend_unique(&mut blocker_ids, &split.blocker_ids);
    }
    blocker_ids
}

fn officeqa_difficulty_split_report(
    rows: &SourceRowManifest<OfficeQaInput, String>,
    strata: &BTreeMap<SmolStr, Vec<CaseId>>,
    train_rows: usize,
) -> Result<SplitMaterializationReport, ManifestError> {
    let source_rows = rows.len();
    let id = format!("officeqa_difficulty_train_{train_rows}_val_{OFFICEQA_VALIDATION_ROWS}");
    if source_rows <= train_rows + OFFICEQA_VALIDATION_ROWS {
        return Ok(SplitMaterializationReport {
            id,
            method: "difficulty_stratified_exact_count".to_owned(),
            exactness: MaterializationExactness::Blocked,
            train_rows: u64::try_from(train_rows).expect("train count fits in u64"),
            validation_rows: Some(
                u64::try_from(OFFICEQA_VALIDATION_ROWS).expect("validation count fits in u64"),
            ),
            test_rows: None,
            split_fingerprint: None,
            role_manifests: Vec::new(),
            blocker_ids: vec!["officeqa_insufficient_rows".to_owned()],
            acceptance_status: SplitAcceptanceStatus::Blocked,
        });
    }

    let splits = officeqa_difficulty_splits(source_rows, strata, train_rows)?;
    let role_manifests = split_role_manifests(
        rows,
        &splits,
        &[
            (SplitRole::Train, "train"),
            (SplitRole::Validation, "validation"),
            (SplitRole::Test, "held_out_test"),
        ],
    )?;

    Ok(SplitMaterializationReport {
        id,
        method: "difficulty_stratified_exact_count".to_owned(),
        exactness: MaterializationExactness::PaperCloseSubstitute,
        train_rows: split_len(&splits, &SplitRole::Train),
        validation_rows: Some(split_len(&splits, &SplitRole::Validation)),
        test_rows: Some(split_len(&splits, &SplitRole::Test)),
        split_fingerprint: Some(fingerprint_hex(splits.fingerprint())),
        role_manifests,
        blocker_ids: vec![
            "officeqa_category_split_manifest".to_owned(),
            "officeqa_exact_split_membership".to_owned(),
        ],
        acceptance_status: SplitAcceptanceStatus::PendingPaperClosePolicy,
    })
}

fn officeqa_difficulty_splits(
    source_rows: usize,
    strata: &BTreeMap<SmolStr, Vec<CaseId>>,
    train_rows: usize,
) -> Result<leaven_eval::DatasetSplits, ManifestError> {
    let test_rows = source_rows - train_rows - OFFICEQA_VALIDATION_ROWS;
    StratifiedSplitBuilder::new(strata.clone())
        .map_err(|source| ManifestError::Split { source })?
        .role_count(SplitRole::Train, train_rows)
        .role_count(SplitRole::Validation, OFFICEQA_VALIDATION_ROWS)
        .role_count(SplitRole::Test, test_rows)
        .build(CaseSetVersion(format!(
            "officeqa-difficulty-substitute-v1-rows-{source_rows}-train-{train_rows}-val-{OFFICEQA_VALIDATION_ROWS}"
        )))
        .map_err(|source| ManifestError::Split { source })
}

fn split_len(splits: &leaven_eval::DatasetSplits, role: &SplitRole) -> u64 {
    splits.cases(&role.partition_id()).map_or(0, |cases| {
        u64::try_from(cases.len()).expect("split count fits in u64")
    })
}

fn split_role_manifests<T>(
    rows: &SourceRowManifest<T, String>,
    splits: &leaven_eval::DatasetSplits,
    roles: &[(SplitRole, &str)],
) -> Result<Vec<SplitRoleMaterializationReport>, ManifestError> {
    roles
        .iter()
        .map(|(role, label)| split_role_manifest(rows, splits, role, label))
        .collect()
}

fn split_role_manifest<T>(
    rows: &SourceRowManifest<T, String>,
    splits: &leaven_eval::DatasetSplits,
    role: &SplitRole,
    label: &str,
) -> Result<SplitRoleMaterializationReport, ManifestError> {
    let cases = splits
        .cases(&role.partition_id())
        .ok_or_else(|| ManifestError::LoopBlocked {
            reason: format!("split does not contain required role {label}"),
        })?;
    let source_ids = cases
        .iter()
        .map(|case| {
            let row_index = usize::try_from(case.0).map_err(|_| ManifestError::LoopBlocked {
                reason: format!("split role {label} references oversized case {case}"),
            })?;
            rows.rows()
                .get(row_index)
                .map(|row| row.source_id().to_owned())
                .ok_or_else(|| ManifestError::LoopBlocked {
                    reason: format!("split role {label} references missing case {case}"),
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(SplitRoleMaterializationReport {
        role: label.to_owned(),
        rows: u64::try_from(source_ids.len()).expect("split role row count fits in u64"),
        source_id_fingerprint: source_id_fingerprint(&source_ids),
        source_ids,
    })
}

fn source_id_fingerprint(source_ids: &[String]) -> String {
    let mut hasher = Sha256::new();
    for source_id in source_ids {
        hasher.update(source_id.as_bytes());
        hasher.update(b"\0");
    }
    hex::encode(hasher.finalize())
}

fn strata_reports(strata: &BTreeMap<SmolStr, Vec<CaseId>>) -> Vec<StratumMaterializationReport> {
    strata
        .iter()
        .map(|(name, rows)| StratumMaterializationReport {
            name: name.to_string(),
            rows: u64::try_from(rows.len()).expect("stratum row count fits in u64"),
        })
        .collect()
}

pub fn materialize_sealqa_source(
    input: &ManifestBuildInput,
) -> Result<Option<SealQaSourceMaterialization>, ManifestError> {
    let path = input
        .root
        .join("tmp/replication/evoskill/sealqa/seal-0.parquet");
    if !path.exists() {
        return Ok(None);
    }

    let rows = read_parquet_source_rows(&path, |row| {
        Ok(SourceRow::targeted(
            format!("sealqa:{:03}", row.row_index()),
            SealQaInput {
                question: row.required_string("question")?,
                urls: row.optional_string_list("urls")?,
                topic: row.optional_string("topic")?,
            },
            row.required_string("answer")?,
        ))
    })
    .map_err(|source| ManifestError::Parquet { source })?
    .into_manifest();
    let dataset = rows
        .clone()
        .into_dataset()
        .map_err(|source| ManifestError::Dataset { source })?;
    let strata = sealqa_topic_strata(&rows);
    let split_manifest_path = input.root.join(SEALQA_EXACT_SPLIT_MANIFEST_PATH);
    let split_materializations =
        if let Some(manifest) = read_paper_split_manifest(&split_manifest_path, "sealqa")? {
            vec![sealqa_paper_declared_split_report(
                &rows,
                &manifest,
                &split_manifest_path,
            )?]
        } else {
            vec![sealqa_row_order_split_report(&rows)?]
        };
    let blocker_ids = materialization_blocker_ids(&split_materializations);
    let report = DatasetMaterializationReport {
        dataset_id: "sealqa".to_owned(),
        source_artifact_id: "sealqa_parquet".to_owned(),
        source_status: SourceMaterializationStatus::Materialized,
        source_rows: Some(u64::try_from(rows.len()).expect("row count fits in u64")),
        case_rows: Some(u64::try_from(dataset.cases().len()).expect("case count fits in u64")),
        source_row_fingerprint: Some(fingerprint_hex(rows.fingerprint())),
        source_artifact_sha256: Some(sha256_file(&path)?),
        target_policy: "answers are scorer targets only; runner inputs exclude answers".to_owned(),
        strata: strata_reports(&strata),
        split_materializations,
        blocker_ids,
    };
    Ok(Some(SealQaSourceMaterialization { rows, report }))
}

fn sealqa_topic_strata(
    rows: &SourceRowManifest<SealQaInput, String>,
) -> BTreeMap<SmolStr, Vec<CaseId>> {
    let mut strata = BTreeMap::<SmolStr, Vec<CaseId>>::new();
    for (row_index, row) in rows.rows().iter().enumerate() {
        let topic = row.input().topic.as_deref().unwrap_or("unknown");
        strata
            .entry(SmolStr::new(topic))
            .or_default()
            .push(CaseId::from_index(row_index));
    }
    strata
}

fn sealqa_paper_declared_split_report(
    rows: &SourceRowManifest<SealQaInput, String>,
    manifest: &PaperSplitManifestFile,
    path: &Path,
) -> Result<SplitMaterializationReport, ManifestError> {
    paper_declared_split_report(
        rows,
        manifest,
        path,
        &[
            (SplitRole::Train, "train", manifest.roles.train.as_slice()),
            (
                SplitRole::Test,
                "held_out_test",
                manifest.roles.held_out_test.as_slice(),
            ),
        ],
    )
}

fn sealqa_row_order_split_report(
    rows: &SourceRowManifest<SealQaInput, String>,
) -> Result<SplitMaterializationReport, ManifestError> {
    let source_rows = rows.len();
    let test_rows = source_rows.saturating_sub(SEALQA_TRAIN_ROWS);
    let id = format!("sealqa_row_order_train_{SEALQA_TRAIN_ROWS}_heldout_{test_rows}");
    if source_rows <= SEALQA_TRAIN_ROWS {
        return Ok(SplitMaterializationReport {
            id,
            method: "row_order_10_percent_train_substitute".to_owned(),
            exactness: MaterializationExactness::Blocked,
            train_rows: u64::try_from(SEALQA_TRAIN_ROWS).expect("train count fits in u64"),
            validation_rows: None,
            test_rows: None,
            split_fingerprint: None,
            role_manifests: Vec::new(),
            blocker_ids: vec!["sealqa_insufficient_rows".to_owned()],
            acceptance_status: SplitAcceptanceStatus::Blocked,
        });
    }

    let splits =
        RowOrderSplitBuilder::new((0..source_rows).map(CaseId::from_index).collect::<Vec<_>>())
            .role_range(SplitRole::Train, 0..SEALQA_TRAIN_ROWS)
            .role_range(SplitRole::Test, SEALQA_TRAIN_ROWS..source_rows)
            .build(CaseSetVersion(format!(
                "sealqa-row-order-substitute-v1-rows-{source_rows}-train-{SEALQA_TRAIN_ROWS}"
            )))
            .map_err(|source| ManifestError::Split { source })?;
    let role_manifests = split_role_manifests(
        rows,
        &splits,
        &[
            (SplitRole::Train, "train"),
            (SplitRole::Test, "held_out_test"),
        ],
    )?;
    let mut blocker_ids = vec!["sealqa_split_manifest".to_owned()];
    if source_rows != SEALQA_ROWS {
        blocker_ids.push("sealqa_row_count_mismatch".to_owned());
    }

    Ok(SplitMaterializationReport {
        id,
        method: "row_order_10_percent_train_substitute".to_owned(),
        exactness: MaterializationExactness::PaperCloseSubstitute,
        train_rows: split_len(&splits, &SplitRole::Train),
        validation_rows: None,
        test_rows: Some(split_len(&splits, &SplitRole::Test)),
        split_fingerprint: Some(fingerprint_hex(splits.fingerprint())),
        role_manifests,
        blocker_ids,
        acceptance_status: SplitAcceptanceStatus::PendingPaperClosePolicy,
    })
}

pub fn materialize_browsecomp_transfer_source(
    input: &ManifestBuildInput,
) -> Result<Option<BrowseCompTransferSourceMaterialization>, ManifestError> {
    let path = input.root.join(BROWSECOMP_TRANSFER_SAMPLE_PATH);
    if !path.exists() {
        return Ok(None);
    }

    let rows = read_browsecomp_transfer_rows(&path)?;
    if rows.len() != BROWSECOMP_TRANSFER_ROWS {
        return Err(ManifestError::BrowseCompSample {
            path,
            message: format!(
                "expected {BROWSECOMP_TRANSFER_ROWS} rows from the paper transfer sample, found {}",
                rows.len()
            ),
        });
    }
    let dataset = rows
        .clone()
        .into_dataset()
        .map_err(|source| ManifestError::Dataset { source })?;
    let strata = browsecomp_transfer_strata(&rows);
    let split_materializations = vec![browsecomp_transfer_split_report(&rows)?];
    let report = DatasetMaterializationReport {
        dataset_id: "browsecomp_transfer".to_owned(),
        source_artifact_id: "browsecomp_transfer_sample".to_owned(),
        source_status: SourceMaterializationStatus::Materialized,
        source_rows: Some(u64::try_from(rows.len()).expect("row count fits in u64")),
        case_rows: Some(u64::try_from(dataset.cases().len()).expect("case count fits in u64")),
        source_row_fingerprint: Some(fingerprint_hex(rows.fingerprint())),
        source_artifact_sha256: Some(sha256_file(&path)?),
        target_policy: "answers are scorer targets only; transfer runner inputs exclude answers"
            .to_owned(),
        strata: strata_reports(&strata),
        split_materializations,
        blocker_ids: Vec::new(),
    };
    Ok(Some(BrowseCompTransferSourceMaterialization {
        rows,
        report,
    }))
}

fn read_browsecomp_transfer_rows(
    path: &Path,
) -> Result<SourceRowManifest<BrowseCompTransferInput, String>, ManifestError> {
    let contents = fs::read_to_string(path).map_err(|source| ManifestError::Read {
        path: path.to_owned(),
        source,
    })?;
    let mut rows = Vec::new();
    let mut source_ids = BTreeSet::new();
    for (line_index, line) in contents.lines().enumerate() {
        let line_number = line_index + 1;
        if line.trim().is_empty() {
            return Err(ManifestError::BrowseCompSample {
                path: path.to_owned(),
                message: format!("line {line_number} is blank"),
            });
        }
        let record =
            serde_json::from_str::<BrowseCompTransferJsonlRecord>(line).map_err(|source| {
                ManifestError::BrowseCompSampleJson {
                    path: path.to_owned(),
                    line: line_number,
                    source,
                }
            })?;
        validate_browsecomp_transfer_record(path, line_number, &record, &mut source_ids)?;
        rows.push(SourceRow::targeted(
            record.source_id,
            BrowseCompTransferInput {
                question: record.question,
                stratum: record.stratum,
            },
            record.answer,
        ));
    }
    SourceRowManifest::new(rows).map_err(|source| ManifestError::Dataset { source })
}

fn browsecomp_public_transfer_sample_records(
    path: &Path,
) -> Result<Vec<BrowseCompTransferJsonlRecord>, ManifestError> {
    let rows = read_browsecomp_public_rows(path)?;
    let sample = topic_stratified_browsecomp_sample(path, rows, BROWSECOMP_TRANSFER_ROWS)?;
    Ok(sample
        .into_iter()
        .map(|row| BrowseCompTransferJsonlRecord {
            source_id: row.source_id,
            question: row.question,
            answer: row.answer,
            stratum: Some(row.stratum),
        })
        .collect())
}

fn read_browsecomp_public_rows(
    path: &Path,
) -> Result<Vec<BrowseCompPublicPlainRow>, ManifestError> {
    let mut reader = csv::ReaderBuilder::new()
        .trim(csv::Trim::All)
        .from_path(path)
        .map_err(|source| ManifestError::Csv {
            path: path.to_owned(),
            source,
        })?;
    let mut rows = Vec::new();
    for (index, record) in reader
        .deserialize::<BrowseCompPublicCsvRecord>()
        .enumerate()
    {
        let line = index + 2;
        let record = record.map_err(|source| ManifestError::Csv {
            path: path.to_owned(),
            source,
        })?;
        let source_id = format!("browsecomp-public:{:04}", index + 1);
        let question = decrypt_browsecomp_public_field(
            path,
            line,
            "problem",
            &record.problem,
            &record.canary,
        )?;
        let answer =
            decrypt_browsecomp_public_field(path, line, "answer", &record.answer, &record.canary)?;
        for (field, value) in [
            ("problem", question.as_str()),
            ("answer", answer.as_str()),
            ("problem_topic", record.problem_topic.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(ManifestError::BrowseCompSample {
                    path: path.to_owned(),
                    message: format!("line {line} field `{field}` must not be empty"),
                });
            }
        }
        rows.push(BrowseCompPublicPlainRow {
            source_id,
            question,
            answer,
            stratum: record.problem_topic,
        });
    }
    Ok(rows)
}

fn topic_stratified_browsecomp_sample(
    path: &Path,
    rows: Vec<BrowseCompPublicPlainRow>,
    sample_rows: usize,
) -> Result<Vec<BrowseCompPublicPlainRow>, ManifestError> {
    if rows.len() < sample_rows {
        return Err(ManifestError::BrowseCompSample {
            path: path.to_owned(),
            message: format!(
                "official BrowseComp CSV has {} rows, but {sample_rows} are required",
                rows.len()
            ),
        });
    }
    let total_rows = rows.len();
    let mut by_topic = BTreeMap::<String, Vec<BrowseCompPublicPlainRow>>::new();
    for row in rows {
        by_topic.entry(row.stratum.clone()).or_default().push(row);
    }

    let mut allocations = by_topic
        .iter()
        .map(|(topic, topic_rows)| {
            let scaled = topic_rows.len() * sample_rows;
            (topic.clone(), scaled / total_rows, scaled % total_rows)
        })
        .collect::<Vec<_>>();
    let allocated = allocations
        .iter()
        .map(|(_, count, _)| *count)
        .sum::<usize>();
    let mut remainder =
        sample_rows
            .checked_sub(allocated)
            .ok_or_else(|| ManifestError::BrowseCompSample {
                path: path.to_owned(),
                message: "topic allocation exceeded requested sample rows".to_owned(),
            })?;
    allocations.sort_by(|(left_topic, _, left_rem), (right_topic, _, right_rem)| {
        right_rem
            .cmp(left_rem)
            .then_with(|| left_topic.cmp(right_topic))
    });
    for (_, count, _) in &mut allocations {
        if remainder == 0 {
            break;
        }
        *count += 1;
        remainder -= 1;
    }

    let counts_by_topic = allocations
        .into_iter()
        .map(|(topic, count, _)| (topic, count))
        .collect::<BTreeMap<_, _>>();
    let mut sample = Vec::with_capacity(sample_rows);
    for (topic, mut topic_rows) in by_topic {
        topic_rows.sort_by(|left, right| {
            browsecomp_public_sample_rank(left)
                .cmp(&browsecomp_public_sample_rank(right))
                .then_with(|| left.source_id.cmp(&right.source_id))
        });
        let count = counts_by_topic
            .get(&topic)
            .copied()
            .expect("all topics have allocation counts");
        sample.extend(topic_rows.into_iter().take(count));
    }
    sample.sort_by(|left, right| left.source_id.cmp(&right.source_id));
    Ok(sample)
}

fn browsecomp_public_sample_rank(row: &BrowseCompPublicPlainRow) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"leaven-evoskill-browsecomp-topic-stratified-v1");
    hasher.update(b"\0");
    hasher.update(row.stratum.as_bytes());
    hasher.update(b"\0");
    hasher.update(row.source_id.as_bytes());
    hasher.update(b"\0");
    hasher.update(row.question.as_bytes());
    hasher.update(b"\0");
    hasher.update(row.answer.as_bytes());
    hasher.finalize().into()
}

fn decrypt_browsecomp_public_field(
    path: &Path,
    line: usize,
    field: &str,
    ciphertext_b64: &str,
    canary: &str,
) -> Result<String, ManifestError> {
    let encrypted = BASE64_STANDARD.decode(ciphertext_b64).map_err(|source| {
        ManifestError::BrowseCompSample {
            path: path.to_owned(),
            message: format!("line {line} field `{field}` is not valid base64: {source}"),
        }
    })?;
    let key = browsecomp_public_key(canary, encrypted.len());
    let decrypted = encrypted
        .iter()
        .zip(key)
        .map(|(cipher, key)| *cipher ^ key)
        .collect::<Vec<_>>();
    String::from_utf8(decrypted).map_err(|source| ManifestError::BrowseCompSample {
        path: path.to_owned(),
        message: format!("line {line} field `{field}` is not valid UTF-8 after decrypt: {source}"),
    })
}

fn browsecomp_public_key(canary: &str, length: usize) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(canary.as_bytes());
    let digest = hasher.finalize();
    digest.iter().copied().cycle().take(length).collect()
}

fn validate_browsecomp_transfer_record(
    path: &Path,
    line: usize,
    record: &BrowseCompTransferJsonlRecord,
    source_ids: &mut BTreeSet<String>,
) -> Result<(), ManifestError> {
    for (field, value) in [
        ("source_id", record.source_id.as_str()),
        ("question", record.question.as_str()),
        ("answer", record.answer.as_str()),
    ] {
        if value.trim().is_empty() {
            return Err(ManifestError::BrowseCompSample {
                path: path.to_owned(),
                message: format!("line {line} field `{field}` must not be empty"),
            });
        }
    }
    if !source_ids.insert(record.source_id.clone()) {
        return Err(ManifestError::BrowseCompSample {
            path: path.to_owned(),
            message: format!("line {line} repeats source_id `{}`", record.source_id),
        });
    }
    Ok(())
}

fn browsecomp_transfer_strata(
    rows: &SourceRowManifest<BrowseCompTransferInput, String>,
) -> BTreeMap<SmolStr, Vec<CaseId>> {
    let mut strata = BTreeMap::<SmolStr, Vec<CaseId>>::new();
    for (row_index, row) in rows.rows().iter().enumerate() {
        let stratum = row.input().stratum.as_deref().unwrap_or("unlabeled");
        strata
            .entry(SmolStr::new(stratum))
            .or_default()
            .push(CaseId::from_index(row_index));
    }
    strata
}

fn browsecomp_transfer_split_report(
    rows: &SourceRowManifest<BrowseCompTransferInput, String>,
) -> Result<SplitMaterializationReport, ManifestError> {
    let source_rows = rows.len();
    let splits =
        RowOrderSplitBuilder::new((0..source_rows).map(CaseId::from_index).collect::<Vec<_>>())
            .role_range(SplitRole::Test, 0..source_rows)
            .build(CaseSetVersion(format!(
                "browsecomp-transfer-sample-v1-rows-{source_rows}"
            )))
            .map_err(|source| ManifestError::Split { source })?;
    let role_manifests =
        split_role_manifests(rows, &splits, &[(SplitRole::Test, "held_out_test")])?;

    Ok(SplitMaterializationReport {
        id: format!("browsecomp_transfer_sample_{source_rows}_heldout"),
        method: "operator_supplied_jsonl_transfer_sample".to_owned(),
        exactness: MaterializationExactness::PaperCloseSubstitute,
        train_rows: 0,
        validation_rows: None,
        test_rows: Some(split_len(&splits, &SplitRole::Test)),
        split_fingerprint: Some(fingerprint_hex(splits.fingerprint())),
        role_manifests,
        blocker_ids: Vec::new(),
        acceptance_status: SplitAcceptanceStatus::NotRequired,
    })
}

pub fn run_evoskill_replica_mechanics(
    input: &ManifestBuildInput,
) -> Result<EvoSkillReplicaLoopReport, ManifestError> {
    let sources = materialize_evoskill_sources(input)?;
    let manifest = build_evoskill_replica_manifest_from_sources(input, &sources)?;
    let manifest_fingerprint = manifest_fingerprint_report(&manifest)?;
    let scorer_fingerprint = scorer_fingerprint_report(&manifest.scorer);
    let officeqa = sources
        .officeqa
        .as_ref()
        .ok_or_else(|| ManifestError::LoopBlocked {
            reason: "OfficeQA full CSV is required for the no-spend replica loop".to_owned(),
        })?;
    run_evoskill_replica_mechanics_with_manifest(
        &manifest,
        &manifest_fingerprint,
        &scorer_fingerprint,
        officeqa,
    )
}

fn run_evoskill_replica_mechanics_with_manifest(
    manifest: &EvoSkillReplicaManifest,
    manifest_fingerprint: &ManifestFingerprintReport,
    scorer_fingerprint: &ScorerFingerprintReport,
    officeqa: &OfficeQaSourceMaterialization,
) -> Result<EvoSkillReplicaLoopReport, ManifestError> {
    let run_manifest = evoskill_replica_loop_run_manifest(
        manifest,
        manifest_fingerprint,
        scorer_fingerprint,
        officeqa,
    )?;
    let git = create_agentic_git_harness()?;
    let mut state = initial_replica_loop_state(officeqa, &git)?;
    let mut iterations = Vec::new();
    let mut checkpoint_resume = None;

    for (index, validation_score) in REPLICA_VALIDATION_SCORES.into_iter().enumerate() {
        let iteration = u64::try_from(index + 1).expect("iteration index fits in u64");
        let report =
            run_replica_iteration(&mut state, officeqa, &git, iteration, validation_score)?;
        iterations.push(report);
        if iteration == EVOSKILL_REPLICA_RESUME_AFTER_ITERATION {
            checkpoint_resume = Some(round_trip_replica_checkpoint(&mut state, iteration)?);
        }
    }

    Ok(EvoSkillReplicaLoopReport {
        exactness: run_manifest.train_split_exactness.clone(),
        run_manifest,
        frontier_capacity: u64::try_from(EVOSKILL_REPLICA_FRONTIER_SIZE)
            .expect("frontier capacity fits in u64"),
        iterations,
        feedback_history_rows: u64::try_from(state.feedback_history.len())
            .expect("feedback count fits in u64"),
        final_frontier_members: state.frontier.members().to_vec(),
        final_best_score: state.frontier.best_score(),
        checkpoint_resume: checkpoint_resume.expect("configured loop writes one checkpoint"),
        proxy_rejection: "no-spend agentic Git readback loop proves mechanics only; live OfficeQA/SealQA paper scores remain unproven".to_owned(),
    })
}

fn evoskill_replica_loop_run_manifest(
    manifest: &EvoSkillReplicaManifest,
    manifest_fingerprint: &ManifestFingerprintReport,
    scorer_fingerprint: &ScorerFingerprintReport,
    officeqa: &OfficeQaSourceMaterialization,
) -> Result<EvoSkillReplicaLoopRunManifest, ManifestError> {
    let train_split = officeqa_loop_train_split(&officeqa.report)?;
    let train_role = split_role_report(train_split, "train")?;
    let validation_role = split_role_report(train_split, "validation")?;
    validate_loop_source_matches_manifest(
        manifest,
        &officeqa.report,
        train_split,
        train_role,
        validation_role,
    )?;
    let source_row_fingerprint =
        officeqa
            .report
            .source_row_fingerprint
            .clone()
            .ok_or_else(|| ManifestError::LoopBlocked {
                reason: "OfficeQA loop run manifest requires a source row fingerprint".to_owned(),
            })?;
    let train_split_fingerprint =
        train_split
            .split_fingerprint
            .clone()
            .ok_or_else(|| ManifestError::LoopBlocked {
                reason: "OfficeQA loop run manifest requires a train split fingerprint".to_owned(),
            })?;

    Ok(EvoSkillReplicaLoopRunManifest {
        manifest_schema_version: manifest.schema_version,
        manifest_fingerprint: manifest_fingerprint.fingerprint.clone(),
        scorer_id: scorer_fingerprint.scorer_id.clone(),
        scorer_fingerprint: scorer_fingerprint.fingerprint.clone(),
        source_dataset_id: officeqa.report.dataset_id.clone(),
        source_artifact_id: officeqa.report.source_artifact_id.clone(),
        source_row_fingerprint,
        train_split_id: train_split.id.clone(),
        train_split_exactness: train_split.exactness.clone(),
        train_split_fingerprint,
        train_role_source_id_fingerprint: train_role.source_id_fingerprint.clone(),
        train_rows: train_split.train_rows,
        validation_split_id: train_split.id.clone(),
        validation_split_fingerprint: train_split
            .split_fingerprint
            .clone()
            .expect("train split fingerprint was already required"),
        validation_role_source_id_fingerprint: validation_role.source_id_fingerprint.clone(),
        validation_rows: validation_role.rows,
        validation_policy:
            "evaluate the full OfficeQA validation role after each child before frontier admission"
                .to_owned(),
        sampler_policy: "OfficeQA difficulty-stratified substitute train sampler; two categories x one sample per category per iteration".to_owned(),
        frontier_capacity: manifest.frontier.capacity,
        parent_selection: manifest.frontier.parent_selection.clone(),
        admission_policy: manifest.frontier.admission.clone(),
        eviction_policy: manifest.frontier.eviction.clone(),
        planned_iterations: u64::try_from(REPLICA_VALIDATION_SCORES.len())
            .expect("replica iteration count fits in u64"),
        checkpoint_resume_after_iteration: EVOSKILL_REPLICA_RESUME_AFTER_ITERATION,
        schedule_epochs: manifest.schedule.epochs,
        schedule_train_batch_policy: manifest.schedule.train_batch_policy.clone(),
        schedule_feedback_history: manifest.schedule.feedback_history.clone(),
        git_identity_mode: "commit".to_owned(),
        runtime: "fake_no_spend_agentic_git_workspace".to_owned(),
        validation_score_source: "fixed no-spend scalar sequence after full validation-role traversal; not paper scorer output".to_owned(),
        proof_limit: "mechanics evidence only; not live provider, SealQA judge, transferred BrowseComp skill, or paper-score evidence".to_owned(),
    })
}

fn validate_loop_source_matches_manifest(
    manifest: &EvoSkillReplicaManifest,
    officeqa_report: &DatasetMaterializationReport,
    train_split: &SplitMaterializationReport,
    train_role: &SplitRoleMaterializationReport,
    validation_role: &SplitRoleMaterializationReport,
) -> Result<(), ManifestError> {
    let manifest_officeqa = manifest_materialization(manifest, "officeqa")?;
    let manifest_train_split = manifest_split(manifest_officeqa, &train_split.id)?;
    let manifest_train_role = split_role_report(manifest_train_split, &train_role.role)?;
    let manifest_validation_role = split_role_report(manifest_train_split, &validation_role.role)?;
    if manifest_officeqa.source_artifact_id != officeqa_report.source_artifact_id
        || manifest_officeqa.source_rows != officeqa_report.source_rows
        || manifest_officeqa.case_rows != officeqa_report.case_rows
        || manifest_officeqa.source_row_fingerprint != officeqa_report.source_row_fingerprint
        || manifest_officeqa.source_artifact_sha256 != officeqa_report.source_artifact_sha256
    {
        return Err(ManifestError::LoopBlocked {
            reason: "OfficeQA loop source materialization drifted from the replica manifest"
                .to_owned(),
        });
    }
    if manifest_train_split.exactness != train_split.exactness
        || manifest_train_split.train_rows != train_split.train_rows
        || manifest_train_split.validation_rows != train_split.validation_rows
        || manifest_train_split.test_rows != train_split.test_rows
        || manifest_train_split.split_fingerprint != train_split.split_fingerprint
    {
        return Err(ManifestError::LoopBlocked {
            reason: format!(
                "OfficeQA loop train split `{}` drifted from the replica manifest",
                train_split.id
            ),
        });
    }
    if manifest_train_role.rows != train_role.rows
        || manifest_train_role.source_id_fingerprint != train_role.source_id_fingerprint
        || manifest_train_role.source_ids != train_role.source_ids
    {
        return Err(ManifestError::LoopBlocked {
            reason: format!(
                "OfficeQA loop train role `{}` drifted from the replica manifest",
                train_role.role
            ),
        });
    }
    if manifest_validation_role.rows != validation_role.rows
        || manifest_validation_role.source_id_fingerprint != validation_role.source_id_fingerprint
        || manifest_validation_role.source_ids != validation_role.source_ids
    {
        return Err(ManifestError::LoopBlocked {
            reason: format!(
                "OfficeQA loop validation role `{}` drifted from the replica manifest",
                validation_role.role
            ),
        });
    }
    Ok(())
}

fn manifest_materialization<'a>(
    manifest: &'a EvoSkillReplicaManifest,
    dataset_id: &str,
) -> Result<&'a DatasetMaterializationReport, ManifestError> {
    manifest
        .source_materializations
        .iter()
        .find(|materialization| materialization.dataset_id == dataset_id)
        .ok_or_else(|| ManifestError::LoopBlocked {
            reason: format!("replica manifest is missing `{dataset_id}` materialization"),
        })
}

fn manifest_split<'a>(
    materialization: &'a DatasetMaterializationReport,
    split_id: &str,
) -> Result<&'a SplitMaterializationReport, ManifestError> {
    materialization
        .split_materializations
        .iter()
        .find(|split| split.id == split_id)
        .ok_or_else(|| ManifestError::LoopBlocked {
            reason: format!("replica manifest is missing split `{split_id}`"),
        })
}

fn officeqa_loop_train_split(
    report: &DatasetMaterializationReport,
) -> Result<&SplitMaterializationReport, ManifestError> {
    let expected_train_rows =
        u64::try_from(EVOSKILL_REPLICA_TRAIN_ROWS).expect("replica train rows fit in u64");
    if let Some(exact) = report.split_materializations.iter().find(|split| {
        split.exactness == MaterializationExactness::PaperExact
            && split.train_rows == expected_train_rows
    }) {
        return Ok(exact);
    }
    let expected_id = format!(
        "officeqa_difficulty_train_{EVOSKILL_REPLICA_TRAIN_ROWS}_val_{OFFICEQA_VALIDATION_ROWS}"
    );
    report
        .split_materializations
        .iter()
        .find(|split| split.id == expected_id)
        .ok_or_else(|| ManifestError::LoopBlocked {
            reason: format!("OfficeQA loop train split `{expected_id}` is missing"),
        })
}

fn split_role_report<'a>(
    split: &'a SplitMaterializationReport,
    role: &str,
) -> Result<&'a SplitRoleMaterializationReport, ManifestError> {
    split
        .role_manifests
        .iter()
        .find(|manifest| manifest.role == role)
        .ok_or_else(|| ManifestError::LoopBlocked {
            reason: format!("split `{}` is missing role `{role}`", split.id),
        })
}

fn manifest_fingerprint_report(
    manifest: &EvoSkillReplicaManifest,
) -> Result<ManifestFingerprintReport, ManifestError> {
    let bytes = serde_json::to_vec(manifest)
        .map_err(|source| ManifestError::ManifestSerialize { source })?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(ManifestFingerprintReport {
        schema_version: manifest.schema_version,
        fingerprint: hex::encode(hasher.finalize()),
    })
}

fn scorer_fingerprint_report(scorer: &ScorerManifest) -> ScorerFingerprintReport {
    let mut hasher = Sha256::new();
    hasher.update(scorer.id.as_bytes());
    hasher.update(b"\0");
    for tolerance in &scorer.tolerances {
        hasher.update(tolerance.to_be_bytes());
    }
    hasher.update(b"\0");
    hasher.update(scorer.failure_threshold.to_be_bytes());
    hasher.update(b"\0");
    hasher.update(scorer.implementation_status.as_bytes());
    for template in &scorer.judge_templates {
        hasher.update(b"\0judge-template\0");
        hasher.update(template.id.as_bytes());
        hasher.update(b"\0");
        hasher.update(template.dataset_id.as_bytes());
        hasher.update(b"\0");
        hasher.update(template.source_artifact_id.as_bytes());
        hasher.update(b"\0");
        hasher.update([u8::from(template.source_artifact_exists)]);
        if let Some(bytes) = template.source_artifact_bytes {
            hasher.update(bytes.to_le_bytes());
        }
        hasher.update(b"\0");
        if let Some(sha256) = &template.source_artifact_sha256 {
            hasher.update(sha256.as_bytes());
        }
        hasher.update(b"\0");
        hasher.update(template.runtime_status.as_bytes());
        hasher.update(b"\0");
        hasher.update(template.fingerprint.as_bytes());
    }
    ScorerFingerprintReport {
        scorer_id: scorer.id.clone(),
        fingerprint: hex::encode(hasher.finalize()),
    }
}

fn read_officeqa_prediction_rows(path: &Path) -> Result<Vec<OfficeQaPredictionRow>, ManifestError> {
    let body = fs::read_to_string(path).map_err(|source| ManifestError::Read {
        path: path.to_owned(),
        source,
    })?;
    let mut rows = Vec::new();
    for (line_index, line) in body.lines().enumerate() {
        if line.trim().is_empty() {
            return Err(ManifestError::OfficeQaPrediction {
                path: path.to_owned(),
                message: format!("blank line {}", line_index + 1),
            });
        }
        let row = serde_json::from_str::<OfficeQaPredictionRow>(line).map_err(|source| {
            ManifestError::OfficeQaPredictionJson {
                path: path.to_owned(),
                line: line_index + 1,
                source,
            }
        })?;
        rows.push(row);
    }
    if rows.is_empty() {
        return Err(ManifestError::OfficeQaPrediction {
            path: path.to_owned(),
            message: "prediction file must contain at least one row".to_owned(),
        });
    }
    Ok(rows)
}

fn officeqa_score_result_manifest_from_predictions(
    root: &Path,
    sidecar_path: &Path,
    sources: &EvoSkillSourceMaterializations,
    slots: &[FinalScoreSlot],
    manifest_fingerprint: &ManifestFingerprintReport,
    scorer_fingerprint: &ScorerFingerprintReport,
    predictions: Vec<OfficeQaPredictionRow>,
) -> Result<ScoreResultManifestFile, ManifestError> {
    let slots_by_key = slots
        .iter()
        .map(|slot| (final_score_slot_key(slot), slot))
        .collect::<BTreeMap<_, _>>();
    let targets = officeqa_targets_by_source_id(sources, sidecar_path)?;
    let mut grouped = BTreeMap::<String, Vec<OfficeQaPredictionRow>>::new();
    for row in predictions {
        validate_officeqa_prediction_row_shape(sidecar_path, &row)?;
        grouped
            .entry(officeqa_prediction_slot_key(&row))
            .or_default()
            .push(row);
    }

    let mut metric_calls = 0_u64;
    let mut entries = Vec::new();
    for (key, rows) in grouped {
        let slot = slots_by_key.get(&key).ok_or_else(|| {
            score_result_manifest_error(
                sidecar_path,
                format!("OfficeQA prediction rows target unknown score slot `{key}`"),
            )
        })?;
        validate_officeqa_score_writer_slot(sidecar_path, &key, slot)?;
        let evidence_rows =
            officeqa_score_evidence_rows(sidecar_path, &key, sources, &targets, slot, rows)?;
        let evidence_artifact =
            write_score_evidence_artifact(root, sidecar_path, &key, &evidence_rows)?;
        let scored_rows =
            u64::try_from(evidence_rows.len()).expect("score evidence row count fits in u64");
        let score = evidence_rows.iter().map(|row| row.score).sum::<f64>() / scored_rows as f64;
        metric_calls = metric_calls.saturating_add(scored_rows);
        entries.push(ScoreResultManifestEntry {
            dataset_id: slot.dataset_id.clone(),
            split_id: slot.split_id.clone(),
            split_role: slot.split_role.clone(),
            candidate_role: slot.candidate_role.clone(),
            split_fingerprint: slot.split_fingerprint.clone(),
            role_source_id_fingerprint: slot.role_source_id_fingerprint.clone(),
            expected_rows: slot.expected_rows,
            scored_rows,
            score,
            resolved_blocker_ids: Vec::new(),
            score_evidence_kind: ScoreEvidenceKind::RustScorerReplay,
            score_evidence_approval_id: None,
            evidence_id: officeqa_score_evidence_id(slot),
            evidence_artifact,
        });
    }

    Ok(ScoreResultManifestFile {
        schema_version: 5,
        manifest_fingerprint: manifest_fingerprint.fingerprint.clone(),
        scorer_fingerprint: scorer_fingerprint.fingerprint.clone(),
        cost: FinalReportCost {
            llm_calls: 0,
            metric_calls,
            prompt_tokens: 0,
            completion_tokens: 0,
        },
        entries,
    })
}

fn validate_officeqa_prediction_row_shape(
    path: &Path,
    row: &OfficeQaPredictionRow,
) -> Result<(), ManifestError> {
    if row.dataset_id != "officeqa" {
        return Err(ManifestError::OfficeQaPrediction {
            path: path.to_owned(),
            message: format!(
                "prediction row for `{}` is not supported by the OfficeQA scorer writer",
                row.dataset_id
            ),
        });
    }
    for (field, value) in [
        ("split_id", &row.split_id),
        ("split_role", &row.split_role),
        ("candidate_role", &row.candidate_role),
        ("source_id", &row.source_id),
    ] {
        if value.trim().is_empty() {
            return Err(ManifestError::OfficeQaPrediction {
                path: path.to_owned(),
                message: format!("prediction row field `{field}` must not be empty"),
            });
        }
    }
    Ok(())
}

fn officeqa_targets_by_source_id(
    sources: &EvoSkillSourceMaterializations,
    path: &Path,
) -> Result<BTreeMap<String, String>, ManifestError> {
    let officeqa = sources.officeqa.as_ref().ok_or_else(|| {
        score_result_manifest_error(
            path,
            "OfficeQA predictions require OfficeQA source rows".into(),
        )
    })?;
    Ok(officeqa
        .rows
        .rows()
        .iter()
        .map(|row| {
            (
                row.source_id().to_owned(),
                row.target().expect("OfficeQA rows are targeted").clone(),
            )
        })
        .collect())
}

fn validate_officeqa_score_writer_slot(
    path: &Path,
    key: &str,
    slot: &FinalScoreSlot,
) -> Result<(), ManifestError> {
    if slot.dataset_id != "officeqa" {
        return Err(score_result_manifest_error(
            path,
            format!("OfficeQA score writer cannot report non-OfficeQA slot `{key}`"),
        ));
    }
    if !slot.blocker_ids.is_empty() {
        return Err(score_result_manifest_error(
            path,
            format!(
                "OfficeQA score writer refuses blocked slot `{key}`; resolve source/split blockers before scoring"
            ),
        ));
    }
    if slot.split_fingerprint.is_none() || slot.role_source_id_fingerprint.is_none() {
        return Err(score_result_manifest_error(
            path,
            format!(
                "OfficeQA score writer requires materialized split and role fingerprints for `{key}`"
            ),
        ));
    }
    if slot.expected_rows.is_none() {
        return Err(score_result_manifest_error(
            path,
            format!("OfficeQA score writer requires expected rows for `{key}`"),
        ));
    }
    Ok(())
}

fn officeqa_score_evidence_rows(
    path: &Path,
    key: &str,
    sources: &EvoSkillSourceMaterializations,
    targets: &BTreeMap<String, String>,
    slot: &FinalScoreSlot,
    rows: Vec<OfficeQaPredictionRow>,
) -> Result<Vec<ScoreEvidenceRow>, ManifestError> {
    let expected_rows = slot.expected_rows.expect("validated expected rows");
    let actual_rows = u64::try_from(rows.len()).expect("prediction row count fits in u64");
    if actual_rows != expected_rows {
        return Err(score_result_manifest_error(
            path,
            format!(
                "OfficeQA prediction rows for `{key}` cover {actual_rows} rows, expected {expected_rows}"
            ),
        ));
    }
    let mut predictions = BTreeMap::new();
    for row in rows {
        if predictions
            .insert(row.source_id.clone(), row.prediction.clone())
            .is_some()
        {
            return Err(score_result_manifest_error(
                path,
                format!(
                    "OfficeQA prediction rows for `{key}` repeat source id `{}`",
                    row.source_id
                ),
            ));
        }
    }
    let expected_source_ids = officeqa_score_slot_source_ids(sources, path, key, slot)?;
    if u64::try_from(expected_source_ids.len()).expect("source id count fits in u64")
        != expected_rows
    {
        return Err(score_result_manifest_error(
            path,
            format!("OfficeQA score slot `{key}` source-id manifest does not match expected rows"),
        ));
    }
    let mut evidence_rows = Vec::with_capacity(expected_source_ids.len());
    for source_id in expected_source_ids {
        let prediction = predictions.get(&source_id).ok_or_else(|| {
            score_result_manifest_error(
                path,
                format!("OfficeQA prediction rows for `{key}` are missing source id `{source_id}`"),
            )
        })?;
        let target = targets.get(&source_id).ok_or_else(|| {
            score_result_manifest_error(
                path,
                format!(
                    "OfficeQA prediction rows for `{key}` reference unknown source id `{source_id}`"
                ),
            )
        })?;
        let score = score_evoskill_answer(target, prediction).weighted_score;
        evidence_rows.push(ScoreEvidenceRow {
            source_id,
            prediction: prediction.clone(),
            score,
            judge_template_fingerprint: None,
        });
    }
    if let Some(extra) = predictions
        .keys()
        .find(|source_id| !evidence_rows.iter().any(|row| &row.source_id == *source_id))
    {
        return Err(score_result_manifest_error(
            path,
            format!(
                "OfficeQA prediction rows for `{key}` include source id `{extra}` outside the slot role"
            ),
        ));
    }
    Ok(evidence_rows)
}

fn officeqa_score_slot_source_ids(
    sources: &EvoSkillSourceMaterializations,
    path: &Path,
    key: &str,
    slot: &FinalScoreSlot,
) -> Result<Vec<String>, ManifestError> {
    let officeqa = sources.officeqa.as_ref().ok_or_else(|| {
        score_result_manifest_error(
            path,
            "OfficeQA predictions require OfficeQA source rows".into(),
        )
    })?;
    let split = officeqa
        .report
        .split_materializations
        .iter()
        .find(|split| split.id == slot.split_id)
        .ok_or_else(|| {
            score_result_manifest_error(
                path,
                format!("OfficeQA score slot `{key}` has no materialized split"),
            )
        })?;
    if split.split_fingerprint != slot.split_fingerprint {
        return Err(score_result_manifest_error(
            path,
            format!("OfficeQA score slot `{key}` split fingerprint changed during scoring"),
        ));
    }
    let role = split
        .role_manifests
        .iter()
        .find(|role| role.role == slot.split_role)
        .ok_or_else(|| {
            score_result_manifest_error(
                path,
                format!("OfficeQA score slot `{key}` has no materialized role"),
            )
        })?;
    if Some(&role.source_id_fingerprint) != slot.role_source_id_fingerprint.as_ref() {
        return Err(score_result_manifest_error(
            path,
            format!("OfficeQA score slot `{key}` role fingerprint changed during scoring"),
        ));
    }
    Ok(role.source_ids.clone())
}

fn write_score_evidence_artifact(
    root: &Path,
    sidecar_path: &Path,
    key: &str,
    rows: &[ScoreEvidenceRow],
) -> Result<ScoreEvidenceArtifact, ManifestError> {
    let evidence_id = officeqa_score_evidence_id_from_key(key);
    let relative_path = format!("tmp/replication/evoskill/score-evidence/{evidence_id}.jsonl");
    let path = root.join(&relative_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| ManifestError::Write {
            path: parent.to_owned(),
            source,
        })?;
    }
    let mut body = String::new();
    for row in rows {
        let line = serde_json::to_string(row).map_err(|source| {
            ManifestError::ScoreResultManifestSerialize {
                path: sidecar_path.to_owned(),
                source,
            }
        })?;
        body.push_str(&line);
        body.push('\n');
    }
    fs::write(&path, body.as_bytes()).map_err(|source| ManifestError::Write {
        path: path.clone(),
        source,
    })?;
    let bytes = u64::try_from(body.len()).expect("score evidence artifact size fits in u64");
    Ok(ScoreEvidenceArtifact {
        relative_path,
        sha256: sha256_file(&path)?,
        bytes,
    })
}

fn write_score_result_manifest_file(
    path: &Path,
    manifest: &ScoreResultManifestFile,
) -> Result<(), ManifestError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| ManifestError::Write {
            path: parent.to_owned(),
            source,
        })?;
    }
    let bytes = serde_json::to_vec_pretty(manifest).map_err(|source| {
        ManifestError::ScoreResultManifestSerialize {
            path: path.to_owned(),
            source,
        }
    })?;
    fs::write(path, bytes).map_err(|source| ManifestError::Write {
        path: path.to_owned(),
        source,
    })
}

fn officeqa_prediction_slot_key(row: &OfficeQaPredictionRow) -> String {
    format!(
        "{}|{}|{}|{}",
        row.dataset_id, row.split_id, row.split_role, row.candidate_role
    )
}

fn officeqa_score_evidence_id(slot: &FinalScoreSlot) -> String {
    officeqa_score_evidence_id_from_key(&final_score_slot_key(slot))
}

fn officeqa_score_evidence_id_from_key(key: &str) -> String {
    format!(
        "officeqa-rust-scorer-replay-{}",
        key.replace('|', "-").replace('_', "-")
    )
}

fn read_score_result_manifest(
    root: &Path,
    manifest_fingerprint: &ManifestFingerprintReport,
    scorer_fingerprint: &ScorerFingerprintReport,
) -> Result<Option<ValidatedScoreResultManifest>, ManifestError> {
    let path = root.join(SCORE_RESULT_MANIFEST_PATH);
    if !path.exists() {
        return Ok(None);
    }
    let bytes = fs::read(&path).map_err(|source| ManifestError::Read {
        path: path.clone(),
        source,
    })?;
    let manifest = serde_json::from_slice::<ScoreResultManifestFile>(&bytes).map_err(|source| {
        ManifestError::ScoreResultManifestJson {
            path: path.clone(),
            source,
        }
    })?;
    validate_score_result_manifest(
        root,
        &path,
        manifest,
        manifest_fingerprint,
        scorer_fingerprint,
    )
    .map(Some)
}

fn validate_score_result_manifest(
    root: &Path,
    path: &Path,
    manifest: ScoreResultManifestFile,
    manifest_fingerprint: &ManifestFingerprintReport,
    scorer_fingerprint: &ScorerFingerprintReport,
) -> Result<ValidatedScoreResultManifest, ManifestError> {
    if manifest.schema_version != 5 {
        return Err(score_result_manifest_error(
            path,
            format!(
                "expected schema_version 5, found {}",
                manifest.schema_version
            ),
        ));
    }
    if manifest.manifest_fingerprint != manifest_fingerprint.fingerprint {
        return Err(score_result_manifest_error(
            path,
            format!(
                "manifest fingerprint mismatch: expected `{}`, found `{}`",
                manifest_fingerprint.fingerprint, manifest.manifest_fingerprint
            ),
        ));
    }
    if manifest.scorer_fingerprint != scorer_fingerprint.fingerprint {
        return Err(score_result_manifest_error(
            path,
            format!(
                "scorer fingerprint mismatch: expected `{}`, found `{}`",
                scorer_fingerprint.fingerprint, manifest.scorer_fingerprint
            ),
        ));
    }
    if manifest.entries.is_empty() {
        return Err(score_result_manifest_error(
            path,
            "score result manifest must contain at least one entry".to_owned(),
        ));
    }
    let mut keys = BTreeSet::new();
    let mut external_judge_rows = 0_u64;
    for entry in &manifest.entries {
        validate_score_result_entry_shape(root, path, entry)?;
        let key = score_result_entry_key(entry);
        if !keys.insert(key.clone()) {
            return Err(score_result_manifest_error(
                path,
                format!("duplicate score result entry for `{key}`"),
            ));
        }
        if entry.score_evidence_kind == ScoreEvidenceKind::ExternalJudgeRun {
            external_judge_rows = external_judge_rows.saturating_add(entry.scored_rows);
        }
    }
    if external_judge_rows > manifest.cost.llm_calls {
        return Err(score_result_manifest_error(
            path,
            format!(
                "external judge score evidence covers {external_judge_rows} rows but cost reports only {} llm_calls",
                manifest.cost.llm_calls
            ),
        ));
    }
    let entries = u64::try_from(manifest.entries.len()).expect("score result count fits in u64");
    Ok(ValidatedScoreResultManifest {
        report: ScoreResultManifestReport {
            relative_path: SCORE_RESULT_MANIFEST_PATH.to_owned(),
            schema_version: manifest.schema_version,
            entries,
            manifest_fingerprint: manifest.manifest_fingerprint,
            scorer_fingerprint: manifest.scorer_fingerprint,
            cost: manifest.cost,
        },
        entries: manifest.entries,
    })
}

fn validate_score_result_entry_shape(
    root: &Path,
    path: &Path,
    entry: &ScoreResultManifestEntry,
) -> Result<(), ManifestError> {
    for (field, value) in [
        ("dataset_id", &entry.dataset_id),
        ("split_id", &entry.split_id),
        ("split_role", &entry.split_role),
        ("candidate_role", &entry.candidate_role),
        ("evidence_id", &entry.evidence_id),
    ] {
        if value.trim().is_empty() {
            return Err(score_result_manifest_error(
                path,
                format!("score result field `{field}` must not be empty"),
            ));
        }
    }
    if !(0.0..=1.0).contains(&entry.score) || !entry.score.is_finite() {
        return Err(score_result_manifest_error(
            path,
            format!(
                "score result `{}` has non-finite or out-of-range score {}",
                score_result_entry_key(entry),
                entry.score
            ),
        ));
    }
    if entry.scored_rows == 0 {
        return Err(score_result_manifest_error(
            path,
            format!(
                "score result `{}` must cover at least one row",
                score_result_entry_key(entry)
            ),
        ));
    }
    validate_score_evidence_kind(path, entry)?;
    validate_score_evidence_artifact(root, path, entry)?;
    Ok(())
}

fn validate_score_evidence_kind(
    path: &Path,
    entry: &ScoreResultManifestEntry,
) -> Result<(), ManifestError> {
    let key = score_result_entry_key(entry);
    let allowed = match entry.dataset_id.as_str() {
        "officeqa" => matches!(
            entry.score_evidence_kind,
            ScoreEvidenceKind::RustScorerReplay
        ),
        "sealqa" => matches!(
            entry.score_evidence_kind,
            ScoreEvidenceKind::ExternalJudgeRun
        ),
        "browsecomp_transfer" => matches!(
            entry.score_evidence_kind,
            ScoreEvidenceKind::ExactAnswerReplay | ScoreEvidenceKind::ExternalJudgeRun
        ),
        other => {
            return Err(score_result_manifest_error(
                path,
                format!("score result `{key}` has unsupported dataset `{other}`"),
            ));
        }
    };
    if !allowed {
        return Err(score_result_manifest_error(
            path,
            format!(
                "score result `{key}` uses {:?} evidence for a dataset that requires a different scoring method",
                entry.score_evidence_kind
            ),
        ));
    }
    match entry.score_evidence_kind {
        ScoreEvidenceKind::ExternalJudgeRun => {
            if entry
                .score_evidence_approval_id
                .as_deref()
                .is_none_or(|approval_id| approval_id.trim().is_empty())
            {
                return Err(score_result_manifest_error(
                    path,
                    format!(
                        "score result `{key}` external judge evidence must carry a nonempty approval id"
                    ),
                ));
            }
        }
        ScoreEvidenceKind::RustScorerReplay | ScoreEvidenceKind::ExactAnswerReplay => {
            if entry.score_evidence_approval_id.is_some() {
                return Err(score_result_manifest_error(
                    path,
                    format!(
                        "score result `{key}` non-judge replay evidence must not carry an approval id"
                    ),
                ));
            }
        }
    }
    Ok(())
}

fn validate_score_evidence_artifact(
    root: &Path,
    sidecar_path: &Path,
    entry: &ScoreResultManifestEntry,
) -> Result<(), ManifestError> {
    let key = score_result_entry_key(entry);
    let artifact = &entry.evidence_artifact;
    if artifact.relative_path.trim().is_empty() {
        return Err(score_result_manifest_error(
            sidecar_path,
            format!("score result `{key}` evidence artifact path must not be empty"),
        ));
    }
    let relative_path = Path::new(&artifact.relative_path);
    if !is_safe_relative_path(relative_path) {
        return Err(score_result_manifest_error(
            sidecar_path,
            format!(
                "score result `{key}` evidence artifact path `{}` must be a safe relative path",
                artifact.relative_path
            ),
        ));
    }
    if !is_sha256_hex(&artifact.sha256) {
        return Err(score_result_manifest_error(
            sidecar_path,
            format!("score result `{key}` evidence artifact sha256 must be 64 hex characters"),
        ));
    }
    if artifact.bytes == 0 {
        return Err(score_result_manifest_error(
            sidecar_path,
            format!("score result `{key}` evidence artifact must not be empty"),
        ));
    }

    let artifact_path = root.join(relative_path);
    let metadata = fs::metadata(&artifact_path).map_err(|source| {
        score_result_manifest_error(
            sidecar_path,
            format!(
                "score result `{key}` evidence artifact `{}` is not readable: {source}",
                artifact.relative_path
            ),
        )
    })?;
    if !metadata.is_file() {
        return Err(score_result_manifest_error(
            sidecar_path,
            format!(
                "score result `{key}` evidence artifact `{}` is not a file",
                artifact.relative_path
            ),
        ));
    }
    if metadata.len() != artifact.bytes {
        return Err(score_result_manifest_error(
            sidecar_path,
            format!(
                "score result `{key}` evidence artifact `{}` has {} bytes, expected {}",
                artifact.relative_path,
                metadata.len(),
                artifact.bytes
            ),
        ));
    }
    let actual_sha256 = sha256_file(&artifact_path)?;
    if actual_sha256 != artifact.sha256 {
        return Err(score_result_manifest_error(
            sidecar_path,
            format!(
                "score result `{key}` evidence artifact `{}` sha256 mismatch",
                artifact.relative_path
            ),
        ));
    }
    Ok(())
}

fn is_safe_relative_path(path: &Path) -> bool {
    !path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn apply_score_result_manifest(
    root: &Path,
    sources: &EvoSkillSourceMaterializations,
    scorer: &ScorerManifest,
    slots: &mut [FinalScoreSlot],
    manifest: &ValidatedScoreResultManifest,
) -> Result<(), ManifestError> {
    let path = root.join(SCORE_RESULT_MANIFEST_PATH);
    let mut slot_indexes = BTreeMap::new();
    for (index, slot) in slots.iter().enumerate() {
        let key = final_score_slot_key(slot);
        if slot_indexes.insert(key.clone(), index).is_some() {
            return Err(score_result_manifest_error(
                &path,
                format!("final report has duplicate score slot `{key}`"),
            ));
        }
    }

    for entry in &manifest.entries {
        let key = score_result_entry_key(entry);
        let slot_index = *slot_indexes.get(&key).ok_or_else(|| {
            score_result_manifest_error(
                &path,
                format!("score result entry `{key}` has no matching slot"),
            )
        })?;
        validate_score_result_matches_slot(&path, entry, &slots[slot_index])?;
        validate_score_evidence_rows(root, sources, scorer, &path, entry, &slots[slot_index])?;
        let slot = &mut slots[slot_index];
        slot.score = Some(entry.score);
        slot.score_evidence_id = Some(entry.evidence_id.clone());
        slot.score_evidence_kind = Some(entry.score_evidence_kind);
        slot.score_evidence_approval_id
            .clone_from(&entry.score_evidence_approval_id);
        slot.score_evidence_artifact = Some(entry.evidence_artifact.clone());
        slot.status = FinalScoreStatus::Reported;
        slot.blocker_ids.clear();
    }
    Ok(())
}

fn validate_score_evidence_rows(
    root: &Path,
    sources: &EvoSkillSourceMaterializations,
    scorer: &ScorerManifest,
    sidecar_path: &Path,
    entry: &ScoreResultManifestEntry,
    slot: &FinalScoreSlot,
) -> Result<(), ManifestError> {
    let key = score_result_entry_key(entry);
    let rows = read_score_evidence_rows(root, sidecar_path, entry)?;
    let actual_rows = u64::try_from(rows.len()).expect("score evidence row count fits in u64");
    if actual_rows != entry.scored_rows {
        return Err(score_result_manifest_error(
            sidecar_path,
            format!(
                "score result `{key}` evidence artifact has {actual_rows} rows, expected {}",
                entry.scored_rows
            ),
        ));
    }

    let expected_source_ids = score_result_role_source_ids(sources, sidecar_path, entry, slot)?;
    let expected_source_ids = expected_source_ids.into_iter().collect::<BTreeSet<_>>();
    if expected_source_ids.len() != rows.len() {
        return Err(score_result_manifest_error(
            sidecar_path,
            format!(
                "score result `{key}` role materialization has {} unique source ids, evidence has {} rows",
                expected_source_ids.len(),
                rows.len()
            ),
        ));
    }

    let mut seen_source_ids = BTreeSet::new();
    for row in &rows {
        if row.source_id.trim().is_empty() {
            return Err(score_result_manifest_error(
                sidecar_path,
                format!("score result `{key}` evidence row has empty source_id"),
            ));
        }
        if !(0.0..=1.0).contains(&row.score) || !row.score.is_finite() {
            return Err(score_result_manifest_error(
                sidecar_path,
                format!(
                    "score result `{key}` evidence row `{}` has non-finite or out-of-range score {}",
                    row.source_id, row.score
                ),
            ));
        }
        if !seen_source_ids.insert(row.source_id.clone()) {
            return Err(score_result_manifest_error(
                sidecar_path,
                format!(
                    "score result `{key}` evidence artifact repeats source id `{}`",
                    row.source_id
                ),
            ));
        }
        if !expected_source_ids.contains(&row.source_id) {
            return Err(score_result_manifest_error(
                sidecar_path,
                format!(
                    "score result `{key}` evidence row references source id `{}` outside the current slot role",
                    row.source_id
                ),
            ));
        }
    }

    if let Some(missing_source_id) = expected_source_ids.difference(&seen_source_ids).next() {
        return Err(score_result_manifest_error(
            sidecar_path,
            format!(
                "score result `{key}` evidence artifact is missing source id `{missing_source_id}`"
            ),
        ));
    }

    if entry.dataset_id == "officeqa"
        && entry.score_evidence_kind == ScoreEvidenceKind::RustScorerReplay
    {
        validate_officeqa_score_evidence_rows(sources, sidecar_path, entry, &rows)?;
    }
    if entry.dataset_id == "browsecomp_transfer"
        && entry.score_evidence_kind == ScoreEvidenceKind::ExactAnswerReplay
    {
        validate_browsecomp_score_evidence_rows(sources, sidecar_path, entry, &rows)?;
    }
    validate_judge_template_fingerprint_evidence_rows(scorer, sidecar_path, entry, &rows)?;

    let aggregate_row_count = u32::try_from(rows.len()).map_err(|_| {
        score_result_manifest_error(
            sidecar_path,
            format!("score result `{key}` evidence artifact has too many rows to aggregate"),
        )
    })?;
    let aggregate = rows.iter().map(|row| row.score).sum::<f64>() / f64::from(aggregate_row_count);
    if !score_values_match(aggregate, entry.score) {
        return Err(score_result_manifest_error(
            sidecar_path,
            format!(
                "score result `{key}` evidence aggregate {aggregate} does not match manifest score {}",
                entry.score
            ),
        ));
    }

    Ok(())
}

fn validate_judge_template_fingerprint_evidence_rows(
    scorer: &ScorerManifest,
    sidecar_path: &Path,
    entry: &ScoreResultManifestEntry,
    rows: &[ScoreEvidenceRow],
) -> Result<(), ManifestError> {
    let key = score_result_entry_key(entry);
    match entry.score_evidence_kind {
        ScoreEvidenceKind::ExternalJudgeRun => {
            let expected_fingerprint = scorer
                .judge_templates
                .iter()
                .find(|template| template.dataset_id == entry.dataset_id)
                .map(|template| template.fingerprint.as_str())
                .ok_or_else(|| {
                    score_result_manifest_error(
                        sidecar_path,
                        format!(
                            "score result `{key}` external judge evidence has no pinned judge template fingerprint for dataset `{}`",
                            entry.dataset_id
                        ),
                    )
                })?;
            for row in rows {
                let Some(actual_fingerprint) = row.judge_template_fingerprint.as_deref() else {
                    return Err(score_result_manifest_error(
                        sidecar_path,
                        format!(
                            "score result `{key}` external judge evidence row `{}` must carry the pinned judge template fingerprint",
                            row.source_id
                        ),
                    ));
                };
                if actual_fingerprint != expected_fingerprint {
                    return Err(score_result_manifest_error(
                        sidecar_path,
                        format!(
                            "score result `{key}` external judge evidence row `{}` judge template fingerprint mismatch: expected `{expected_fingerprint}`, found `{actual_fingerprint}`",
                            row.source_id
                        ),
                    ));
                }
            }
        }
        ScoreEvidenceKind::RustScorerReplay | ScoreEvidenceKind::ExactAnswerReplay => {
            for row in rows {
                if row.judge_template_fingerprint.is_some() {
                    return Err(score_result_manifest_error(
                        sidecar_path,
                        format!(
                            "score result `{key}` non-judge replay evidence row `{}` must not carry a judge template fingerprint",
                            row.source_id
                        ),
                    ));
                }
            }
        }
    }
    Ok(())
}

fn read_score_evidence_rows(
    root: &Path,
    sidecar_path: &Path,
    entry: &ScoreResultManifestEntry,
) -> Result<Vec<ScoreEvidenceRow>, ManifestError> {
    let key = score_result_entry_key(entry);
    let artifact_path = root.join(&entry.evidence_artifact.relative_path);
    let body = fs::read_to_string(&artifact_path).map_err(|source| {
        score_result_manifest_error(
            sidecar_path,
            format!(
                "score result `{key}` evidence artifact `{}` is not readable UTF-8 JSONL: {source}",
                entry.evidence_artifact.relative_path
            ),
        )
    })?;

    let mut rows = Vec::new();
    for (line_index, line) in body.lines().enumerate() {
        if line.trim().is_empty() {
            return Err(score_result_manifest_error(
                sidecar_path,
                format!(
                    "score result `{key}` evidence artifact `{}` has blank line {}",
                    entry.evidence_artifact.relative_path,
                    line_index + 1
                ),
            ));
        }
        let row = serde_json::from_str::<ScoreEvidenceRow>(line).map_err(|source| {
            score_result_manifest_error(
                sidecar_path,
                format!(
                    "score result `{key}` evidence artifact `{}` line {} is not a strict score row: {source}",
                    entry.evidence_artifact.relative_path,
                    line_index + 1
                ),
            )
        })?;
        rows.push(row);
    }
    Ok(rows)
}

fn score_result_role_source_ids(
    sources: &EvoSkillSourceMaterializations,
    sidecar_path: &Path,
    entry: &ScoreResultManifestEntry,
    slot: &FinalScoreSlot,
) -> Result<Vec<String>, ManifestError> {
    let key = score_result_entry_key(entry);
    let report = score_result_source_report(sources, &entry.dataset_id).ok_or_else(|| {
        score_result_manifest_error(
            sidecar_path,
            format!("score result `{key}` has no materialized source report"),
        )
    })?;
    let split = report
        .split_materializations
        .iter()
        .find(|split| split.id == entry.split_id)
        .ok_or_else(|| {
            score_result_manifest_error(
                sidecar_path,
                format!("score result `{key}` has no materialized split"),
            )
        })?;
    if split.split_fingerprint != slot.split_fingerprint {
        return Err(score_result_manifest_error(
            sidecar_path,
            format!("score result `{key}` source split fingerprint changed during validation"),
        ));
    }
    let role = split
        .role_manifests
        .iter()
        .find(|role| role.role == entry.split_role)
        .ok_or_else(|| {
            score_result_manifest_error(
                sidecar_path,
                format!("score result `{key}` has no materialized split role"),
            )
        })?;
    if Some(&role.source_id_fingerprint) != slot.role_source_id_fingerprint.as_ref() {
        return Err(score_result_manifest_error(
            sidecar_path,
            format!("score result `{key}` source role fingerprint changed during validation"),
        ));
    }
    Ok(role.source_ids.clone())
}

fn score_result_source_report<'a>(
    sources: &'a EvoSkillSourceMaterializations,
    dataset_id: &str,
) -> Option<&'a DatasetMaterializationReport> {
    match dataset_id {
        "officeqa" => sources
            .officeqa
            .as_ref()
            .map(|materialization| &materialization.report),
        "sealqa" => sources
            .sealqa
            .as_ref()
            .map(|materialization| &materialization.report),
        "browsecomp_transfer" => sources
            .browsecomp_transfer
            .as_ref()
            .map(|materialization| &materialization.report),
        _ => None,
    }
}

fn validate_officeqa_score_evidence_rows(
    sources: &EvoSkillSourceMaterializations,
    sidecar_path: &Path,
    entry: &ScoreResultManifestEntry,
    rows: &[ScoreEvidenceRow],
) -> Result<(), ManifestError> {
    let key = score_result_entry_key(entry);
    let officeqa = sources.officeqa.as_ref().ok_or_else(|| {
        score_result_manifest_error(
            sidecar_path,
            format!("score result `{key}` has no OfficeQA source materialization"),
        )
    })?;
    let targets = officeqa
        .rows
        .rows()
        .iter()
        .map(|row| {
            (
                row.source_id().to_owned(),
                row.target().expect("OfficeQA rows are targeted").clone(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    for row in rows {
        let target = targets.get(&row.source_id).ok_or_else(|| {
            score_result_manifest_error(
                sidecar_path,
                format!(
                    "score result `{key}` OfficeQA row `{}` has no scorer target",
                    row.source_id
                ),
            )
        })?;
        let recomputed = score_evoskill_answer(target, &row.prediction).weighted_score;
        if !score_values_match(recomputed, row.score) {
            return Err(score_result_manifest_error(
                sidecar_path,
                format!(
                    "score result `{key}` row `{}` score {} does not match OfficeQA scorer {recomputed}",
                    row.source_id, row.score
                ),
            ));
        }
    }
    Ok(())
}

fn validate_browsecomp_score_evidence_rows(
    sources: &EvoSkillSourceMaterializations,
    sidecar_path: &Path,
    entry: &ScoreResultManifestEntry,
    rows: &[ScoreEvidenceRow],
) -> Result<(), ManifestError> {
    let key = score_result_entry_key(entry);
    let browsecomp = sources.browsecomp_transfer.as_ref().ok_or_else(|| {
        score_result_manifest_error(
            sidecar_path,
            format!("score result `{key}` has no BrowseComp transfer source materialization"),
        )
    })?;
    let targets = browsecomp
        .rows
        .rows()
        .iter()
        .map(|row| {
            (
                row.source_id().to_owned(),
                row.target()
                    .expect("BrowseComp transfer rows are targeted")
                    .clone(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    for row in rows {
        let target = targets.get(&row.source_id).ok_or_else(|| {
            score_result_manifest_error(
                sidecar_path,
                format!(
                    "score result `{key}` BrowseComp row `{}` has no scorer target",
                    row.source_id
                ),
            )
        })?;
        let recomputed = score_browsecomp_exact_answer(target, &row.prediction);
        if !score_values_match(recomputed, row.score) {
            return Err(score_result_manifest_error(
                sidecar_path,
                format!(
                    "score result `{key}` row `{}` score {} does not match BrowseComp exact-answer scorer {recomputed}",
                    row.source_id, row.score
                ),
            ));
        }
    }
    Ok(())
}

fn score_browsecomp_exact_answer(reference: &str, prediction: &str) -> f64 {
    if normalize_browsecomp_exact_answer(reference) == normalize_browsecomp_exact_answer(prediction)
    {
        1.0
    } else {
        0.0
    }
}

fn normalize_browsecomp_exact_answer(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn score_values_match(left: f64, right: f64) -> bool {
    (left - right).abs() <= 1e-12
}

fn validate_score_result_matches_slot(
    path: &Path,
    entry: &ScoreResultManifestEntry,
    slot: &FinalScoreSlot,
) -> Result<(), ManifestError> {
    let key = score_result_entry_key(entry);
    if entry.split_fingerprint != slot.split_fingerprint {
        return Err(score_result_manifest_error(
            path,
            format!("score result `{key}` split fingerprint does not match the current slot"),
        ));
    }
    if entry.role_source_id_fingerprint != slot.role_source_id_fingerprint {
        return Err(score_result_manifest_error(
            path,
            format!(
                "score result `{key}` role source-id fingerprint does not match the current slot"
            ),
        ));
    }
    if entry.expected_rows != slot.expected_rows {
        return Err(score_result_manifest_error(
            path,
            format!("score result `{key}` expected_rows does not match the current slot"),
        ));
    }
    let expected_rows = slot.expected_rows.ok_or_else(|| {
        score_result_manifest_error(
            path,
            format!("score result `{key}` cannot report a slot without expected rows"),
        )
    })?;
    if entry.scored_rows != expected_rows {
        return Err(score_result_manifest_error(
            path,
            format!(
                "score result `{key}` covers {} rows, expected {expected_rows}",
                entry.scored_rows
            ),
        ));
    }
    let mut resolved = entry.resolved_blocker_ids.clone();
    resolved.sort();
    resolved.dedup();
    if resolved.len() != entry.resolved_blocker_ids.len() {
        return Err(score_result_manifest_error(
            path,
            format!("score result `{key}` repeats a resolved blocker id"),
        ));
    }
    for blocker in &entry.resolved_blocker_ids {
        if !slot.blocker_ids.contains(blocker) {
            return Err(score_result_manifest_error(
                path,
                format!("score result `{key}` claims to resolve non-slot blocker `{blocker}`"),
            ));
        }
        if !score_result_can_resolve_blocker(entry, blocker) {
            return Err(score_result_manifest_error(
                path,
                format!(
                    "score result `{key}` cannot resolve non-score blocker `{blocker}`; resolve source/split blockers before importing score evidence"
                ),
            ));
        }
    }
    for blocker in &slot.blocker_ids {
        if !score_result_can_resolve_blocker(entry, blocker) {
            return Err(score_result_manifest_error(
                path,
                format!(
                    "score result `{key}` cannot resolve non-score blocker `{blocker}`; resolve source/split blockers before importing score evidence"
                ),
            ));
        }
        if !entry.resolved_blocker_ids.contains(blocker) {
            return Err(score_result_manifest_error(
                path,
                format!("score result `{key}` leaves slot blocker `{blocker}` unresolved"),
            ));
        }
    }
    Ok(())
}

fn score_result_can_resolve_blocker(entry: &ScoreResultManifestEntry, blocker: &str) -> bool {
    entry.dataset_id == "sealqa"
        && entry.score_evidence_kind == ScoreEvidenceKind::ExternalJudgeRun
        && blocker == "sealqa_judge_scored_run"
}

fn final_score_slot_key(slot: &FinalScoreSlot) -> String {
    format!(
        "{}|{}|{}|{}",
        slot.dataset_id, slot.split_id, slot.split_role, slot.candidate_role
    )
}

fn score_result_entry_key(entry: &ScoreResultManifestEntry) -> String {
    format!(
        "{}|{}|{}|{}",
        entry.dataset_id, entry.split_id, entry.split_role, entry.candidate_role
    )
}

fn score_result_manifest_error(path: &Path, message: String) -> ManifestError {
    ManifestError::ScoreResultManifest {
        path: path.to_owned(),
        message,
    }
}

fn final_score_slots(manifest: &EvoSkillReplicaManifest) -> Vec<FinalScoreSlot> {
    let mut slots = Vec::new();
    let materializations_by_dataset = manifest
        .source_materializations
        .iter()
        .map(|materialization| (materialization.dataset_id.as_str(), materialization))
        .collect::<BTreeMap<_, _>>();

    for dataset in &manifest.datasets {
        if let Some(materialization) = materializations_by_dataset.get(dataset.id.as_str()) {
            if materialization.split_materializations.is_empty() {
                slots.extend(dataset_placeholder_score_slots(
                    manifest,
                    dataset,
                    &source_universe_blocker_ids(
                        manifest,
                        &dataset.id,
                        &materialization.blocker_ids,
                    ),
                ));
                continue;
            }
            for split in &materialization.split_materializations {
                slots.extend(split_score_slots(manifest, materialization, split));
            }
        } else {
            slots.extend(dataset_placeholder_score_slots(
                manifest,
                dataset,
                &source_universe_blocker_ids(manifest, &dataset.id, &dataset.blocker_ids),
            ));
        }
    }
    slots
}

fn split_score_slots(
    manifest: &EvoSkillReplicaManifest,
    materialization: &DatasetMaterializationReport,
    split: &SplitMaterializationReport,
) -> Vec<FinalScoreSlot> {
    let blocker_ids = scoring_blocker_ids(&materialization.dataset_id, &split.blocker_ids);
    split_score_roles(&materialization.dataset_id, split)
        .into_iter()
        .filter_map(|(role, expected_rows)| {
            expected_rows.map(|rows| {
                let audit = score_slot_audit_for_split_role(split, role);
                score_slots_for_role(
                    manifest,
                    &materialization.dataset_id,
                    &split.id,
                    role,
                    &audit,
                    Some(rows),
                    &blocker_ids,
                )
            })
        })
        .flatten()
        .collect()
}

fn split_score_roles(
    dataset_id: &str,
    split: &SplitMaterializationReport,
) -> Vec<(&'static str, Option<u64>)> {
    if dataset_id == "browsecomp_transfer" {
        return vec![("held_out_test", split.test_rows)];
    }
    vec![
        ("train", Some(split.train_rows)),
        ("validation", split.validation_rows),
        ("held_out_test", split.test_rows),
    ]
}

fn scoring_blocker_ids(dataset_id: &str, split_blocker_ids: &[String]) -> Vec<String> {
    let mut blocker_ids = split_blocker_ids.to_vec();
    if dataset_id == "sealqa" {
        extend_unique(&mut blocker_ids, &["sealqa_judge_scored_run".to_owned()]);
    }
    blocker_ids
}

fn dataset_placeholder_score_slots(
    manifest: &EvoSkillReplicaManifest,
    dataset: &DatasetRequirement,
    blocker_ids: &[String],
) -> Vec<FinalScoreSlot> {
    let blocker_ids = scoring_blocker_ids(&dataset.id, blocker_ids);
    let split_id = format!("{}_paper_split_unmaterialized", dataset.id);
    placeholder_score_roles(dataset)
        .into_iter()
        .flat_map(|(role, expected_rows)| {
            let audit = blocked_score_slot_audit();
            score_slots_for_role(
                manifest,
                &dataset.id,
                &split_id,
                role,
                &audit,
                expected_rows,
                &blocker_ids,
            )
        })
        .collect()
}

fn source_universe_blocker_ids(
    manifest: &EvoSkillReplicaManifest,
    dataset_id: &str,
    fallback: &[String],
) -> Vec<String> {
    manifest
        .source_universe
        .iter()
        .find(|entry| entry.dataset_id == dataset_id)
        .map_or_else(|| fallback.to_vec(), |entry| entry.blocker_ids.clone())
}

fn placeholder_score_roles(dataset: &DatasetRequirement) -> Vec<(&'static str, Option<u64>)> {
    let mut roles = Vec::new();
    if let Some(train_rows) = dataset.train_sizes.first().copied() {
        roles.push(("train", Some(train_rows)));
    }
    if let Some(validation_rows) = dataset.validation_rows {
        roles.push(("validation", Some(validation_rows)));
    }
    roles.push(("held_out_test", held_out_rows(dataset)));
    roles
}

fn held_out_rows(dataset: &DatasetRequirement) -> Option<u64> {
    let paper_rows = dataset.paper_rows?;
    let train_rows = dataset.train_sizes.first().copied().unwrap_or_default();
    let validation_rows = dataset.validation_rows.unwrap_or_default();
    paper_rows.checked_sub(train_rows + validation_rows)
}

fn score_slots_for_role(
    manifest: &EvoSkillReplicaManifest,
    dataset_id: &str,
    split_id: &str,
    split_role: &str,
    audit: &FinalScoreSlotAudit,
    expected_rows: Option<u64>,
    blocker_ids: &[String],
) -> Vec<FinalScoreSlot> {
    score_candidate_roles(dataset_id)
        .into_iter()
        .map(|candidate_role| FinalScoreSlot {
            dataset_id: dataset_id.to_owned(),
            split_id: split_id.to_owned(),
            split_role: split_role.to_owned(),
            split_exactness: audit.split_exactness.clone(),
            split_fingerprint: audit.split_fingerprint.clone(),
            role_source_id_fingerprint: audit.role_source_id_fingerprint.clone(),
            candidate_role: candidate_role.slot_role.to_owned(),
            paper_result_target_ids: paper_result_target_ids(
                manifest,
                dataset_id,
                candidate_role.paper_target_role,
            ),
            expected_rows,
            score: None,
            score_evidence_id: None,
            score_evidence_kind: None,
            score_evidence_approval_id: None,
            score_evidence_artifact: None,
            status: if blocker_ids.is_empty() {
                FinalScoreStatus::NotRun
            } else {
                FinalScoreStatus::Blocked
            },
            blocker_ids: blocker_ids.to_vec(),
        })
        .collect()
}

struct ScoreCandidateRole {
    slot_role: &'static str,
    paper_target_role: &'static str,
}

fn score_candidate_roles(dataset_id: &str) -> Vec<ScoreCandidateRole> {
    match dataset_id {
        "browsecomp_transfer" => vec![
            ScoreCandidateRole {
                slot_role: "baseline",
                paper_target_role: "baseline",
            },
            ScoreCandidateRole {
                slot_role: "sealqa_skill_transfer",
                paper_target_role: "sealqa_skill_transfer",
            },
        ],
        "officeqa" => vec![
            ScoreCandidateRole {
                slot_role: "baseline",
                paper_target_role: "baseline",
            },
            ScoreCandidateRole {
                slot_role: "optimized",
                paper_target_role: "skill_merge",
            },
        ],
        _ => vec![
            ScoreCandidateRole {
                slot_role: "baseline",
                paper_target_role: "baseline",
            },
            ScoreCandidateRole {
                slot_role: "optimized",
                paper_target_role: "optimized",
            },
        ],
    }
}

fn paper_result_target_ids(
    manifest: &EvoSkillReplicaManifest,
    dataset_id: &str,
    paper_candidate_role: &str,
) -> Vec<String> {
    manifest
        .paper_result_targets
        .iter()
        .filter(|target| {
            target.dataset_id == dataset_id && target.candidate_role == paper_candidate_role
        })
        .map(|target| target.id.clone())
        .collect()
}

fn score_slot_audit_for_split_role(
    split: &SplitMaterializationReport,
    role: &str,
) -> FinalScoreSlotAudit {
    FinalScoreSlotAudit {
        split_exactness: split.exactness.clone(),
        split_fingerprint: split.split_fingerprint.clone(),
        role_source_id_fingerprint: split
            .role_manifests
            .iter()
            .find(|manifest| manifest.role == role)
            .map(|manifest| manifest.source_id_fingerprint.clone()),
    }
}

fn blocked_score_slot_audit() -> FinalScoreSlotAudit {
    FinalScoreSlotAudit {
        split_exactness: MaterializationExactness::Blocked,
        split_fingerprint: None,
        role_source_id_fingerprint: None,
    }
}

fn final_report_blockers(
    manifest: &EvoSkillReplicaManifest,
    score_slots: &[FinalScoreSlot],
) -> Vec<ReplicationBlocker> {
    let sealqa_judge_scores_complete = sealqa_judge_score_evidence_complete(score_slots);
    manifest
        .blockers
        .iter()
        .filter(|blocker| {
            !(sealqa_judge_scores_complete && blocker.id == "sealqa_judge_scored_run")
        })
        .cloned()
        .collect()
}

fn final_report_errors(blockers: &[ReplicationBlocker]) -> Vec<FinalReportError> {
    blockers
        .iter()
        .map(|blocker| FinalReportError {
            blocker_id: blocker.id.clone(),
            message: blocker.description.clone(),
        })
        .collect()
}

fn sealqa_judge_score_evidence_complete(score_slots: &[FinalScoreSlot]) -> bool {
    let mut sealqa_slots = score_slots
        .iter()
        .filter(|slot| slot.dataset_id == "sealqa")
        .peekable();
    if sealqa_slots.peek().is_none() {
        return false;
    }
    sealqa_slots.all(|slot| {
        slot.status == FinalScoreStatus::Reported
            && slot.score.is_some()
            && slot.score_evidence_id.is_some()
            && slot.score_evidence_kind == Some(ScoreEvidenceKind::ExternalJudgeRun)
            && slot
                .score_evidence_approval_id
                .as_deref()
                .is_some_and(|approval_id| !approval_id.trim().is_empty())
            && slot.score_evidence_artifact.is_some()
            && !slot
                .blocker_ids
                .iter()
                .any(|blocker| blocker == "sealqa_judge_scored_run")
    })
}

fn final_report_exactness_gaps(manifest: &EvoSkillReplicaManifest) -> Vec<ExactnessGapReport> {
    let mut gaps = Vec::new();
    for revision in &manifest.source_revisions {
        match revision.paper_release_status {
            SourcePaperReleaseStatus::PinnedLocalCheckout => {
                gaps.push(source_revision_exactness_gap(revision));
            }
            SourcePaperReleaseStatus::MissingPath
            | SourcePaperReleaseStatus::NotGitCheckout
            | SourcePaperReleaseStatus::Unresolved
            | SourcePaperReleaseStatus::ProbeFailed => {
                if !revision.blocker_ids.is_empty() {
                    gaps.push(blocked_source_revision_exactness_gap(revision));
                }
            }
        }
    }
    for blocker in &manifest.source_blockers {
        gaps.push(source_blocker_exactness_gap(blocker));
    }
    for materialization in &manifest.source_materializations {
        for split in &materialization.split_materializations {
            if split.exactness != MaterializationExactness::PaperExact {
                gaps.push(split_exactness_gap(materialization, split));
            }
        }
    }
    gaps
}

fn source_revision_exactness_gap(revision: &SourceRevision) -> ExactnessGapReport {
    ExactnessGapReport {
        id: format!("source_revision_{}_local_checkout", revision.id),
        dataset_id: None,
        status: ExactnessGapStatus::PaperReleaseUnverified,
        observed: format!(
            "local checkout `{}` is pinned at `{}`",
            revision.relative_path,
            revision
                .paper_release_head
                .as_deref()
                .or(revision.head.as_deref())
                .unwrap_or("unknown")
        ),
        required_for_paper_exact:
            "paper-release revision or explicitly chosen remote-current source policy".to_owned(),
        paper_close_policy:
            "accepted as the local no-spend paper-close denominator by source_pin_manifest"
                .to_owned(),
        evidence: source_revision_gap_evidence(revision),
        blocker_ids: revision.blocker_ids.clone(),
    }
}

fn blocked_source_revision_exactness_gap(revision: &SourceRevision) -> ExactnessGapReport {
    ExactnessGapReport {
        id: format!("source_revision_{}_blocked", revision.id),
        dataset_id: None,
        status: ExactnessGapStatus::BlockedBeforePaperClose,
        observed: format!(
            "source checkout `{}` has paper-release status `{:?}`",
            revision.relative_path, revision.paper_release_status
        ),
        required_for_paper_exact:
            "paper-release revision or explicitly chosen remote-current source policy".to_owned(),
        paper_close_policy: "source policy must be resolved before paper-close".to_owned(),
        evidence: source_revision_gap_evidence(revision),
        blocker_ids: revision.blocker_ids.clone(),
    }
}

fn source_revision_gap_evidence(revision: &SourceRevision) -> Vec<String> {
    let mut evidence = vec![format!("path={}", revision.relative_path)];
    if let Some(head) = &revision.head {
        evidence.push(format!("head={head}"));
    }
    if let Some(branch) = &revision.branch {
        evidence.push(format!("branch={branch}"));
    }
    if let Some(remote_url) = &revision.remote_url {
        evidence.push(format!("remote_url={remote_url}"));
    }
    if let Some(paper_release_ref) = &revision.paper_release_ref {
        evidence.push(format!("paper_release_ref={paper_release_ref}"));
    }
    if let Some(paper_release_head) = &revision.paper_release_head {
        evidence.push(format!("paper_release_head={paper_release_head}"));
    }
    evidence
}

fn source_blocker_exactness_gap(blocker: &SourceBlockerReport) -> ExactnessGapReport {
    ExactnessGapReport {
        id: format!("source_blocker_{}", blocker.blocker_id),
        dataset_id: Some(blocker.dataset_id.clone()),
        status: ExactnessGapStatus::BlockedBeforePaperClose,
        observed: blocker.note.clone(),
        required_for_paper_exact: source_blocker_paper_exact_requirement(blocker).to_owned(),
        paper_close_policy: "missing source artifact remains a paper-close blocker".to_owned(),
        evidence: source_blocker_gap_evidence(blocker),
        blocker_ids: vec![blocker.blocker_id.clone()],
    }
}

fn source_blocker_paper_exact_requirement(blocker: &SourceBlockerReport) -> &'static str {
    match blocker.status {
        SourceBlockerStatus::UnresolvedSourcePolicy => {
            "explicit paper-release source policy for every referenced checkout"
        }
        SourceBlockerStatus::MissingLocalArtifact => {
            "paper source artifact or approved substitute denominator"
        }
        SourceBlockerStatus::MissingExactSplitManifest => {
            "paper exact split/category membership manifest"
        }
    }
}

fn source_blocker_gap_evidence(blocker: &SourceBlockerReport) -> Vec<String> {
    let mut evidence = vec![format!("status={:?}", blocker.status)];
    for candidate in &blocker.local_path_candidates {
        evidence.push(format!(
            "candidate={} exists={} file={} dir={} bytes={}",
            candidate.relative_path,
            candidate.exists,
            candidate.is_file,
            candidate.is_dir,
            candidate
                .bytes
                .map_or_else(|| "unknown".to_owned(), |bytes| bytes.to_string())
        ));
    }
    evidence
}

fn split_exactness_gap(
    materialization: &DatasetMaterializationReport,
    split: &SplitMaterializationReport,
) -> ExactnessGapReport {
    let accepted = split.exactness == MaterializationExactness::PaperCloseSubstitute
        && split.acceptance_status == SplitAcceptanceStatus::AcceptedPaperClosePolicy
        && split.blocker_ids.is_empty();
    ExactnessGapReport {
        id: format!("split_{}_{}", materialization.dataset_id, split.id),
        dataset_id: Some(materialization.dataset_id.clone()),
        status: if accepted {
            ExactnessGapStatus::AcceptedPaperCloseSubstitute
        } else {
            ExactnessGapStatus::BlockedBeforePaperClose
        },
        observed: format!(
            "split `{}` uses `{}` with {} train rows, {}, and {}",
            split.id,
            split.method,
            split.train_rows,
            optional_rows(split.validation_rows, "validation"),
            optional_rows(split.test_rows, "held-out")
        ),
        required_for_paper_exact: split_paper_exact_requirement(&materialization.dataset_id)
            .to_owned(),
        paper_close_policy: split_paper_close_policy(split).to_owned(),
        evidence: split_gap_evidence(split),
        blocker_ids: split.blocker_ids.clone(),
    }
}

fn split_paper_exact_requirement(dataset_id: &str) -> &'static str {
    match dataset_id {
        "officeqa" => {
            "paper's LLM-clustered category labels plus exact train/validation/held-out membership"
        }
        "sealqa" => "paper's exact train/held-out split membership",
        "browsecomp_transfer" => "paper author's exact 128-example stratified BrowseComp sample",
        _ => "paper-exact split membership",
    }
}

fn split_paper_close_policy(split: &SplitMaterializationReport) -> &'static str {
    if split.exactness == MaterializationExactness::PaperCloseSubstitute
        && split.acceptance_status == SplitAcceptanceStatus::AcceptedPaperClosePolicy
    {
        "accepted by split_policy_manifest as a documented paper-close substitute, not paper-exact"
    } else if split.exactness == MaterializationExactness::PaperCloseSubstitute {
        "requires split_policy_manifest acceptance before paper-close"
    } else {
        "blocked before paper-close"
    }
}

fn split_gap_evidence(split: &SplitMaterializationReport) -> Vec<String> {
    let mut evidence = vec![
        format!("split_id={}", split.id),
        format!("method={}", split.method),
        format!(
            "exactness={}",
            materialization_exactness_name(&split.exactness)
        ),
        format!(
            "acceptance_status={}",
            split_acceptance_status_name(&split.acceptance_status)
        ),
    ];
    if let Some(fingerprint) = &split.split_fingerprint {
        evidence.push(format!("split_fingerprint={fingerprint}"));
    }
    for role in &split.role_manifests {
        evidence.push(format!(
            "role={} rows={} source_id_fingerprint={}",
            role.role, role.rows, role.source_id_fingerprint
        ));
    }
    evidence
}

fn optional_rows(rows: Option<u64>, role: &str) -> String {
    rows.map_or_else(
        || format!("no {role} role"),
        |rows| format!("{rows} {role} rows"),
    )
}

fn materialization_exactness_name(exactness: &MaterializationExactness) -> &'static str {
    match exactness {
        MaterializationExactness::PaperExact => "paper_exact",
        MaterializationExactness::PaperCloseSubstitute => "paper_close_substitute",
        MaterializationExactness::Blocked => "blocked",
    }
}

fn split_acceptance_status_name(status: &SplitAcceptanceStatus) -> &'static str {
    match status {
        SplitAcceptanceStatus::NotRequired => "not_required",
        SplitAcceptanceStatus::PendingPaperClosePolicy => "pending_paper_close_policy",
        SplitAcceptanceStatus::AcceptedPaperClosePolicy => "accepted_paper_close_policy",
        SplitAcceptanceStatus::Blocked => "blocked",
    }
}

fn final_report_paper_close_gates(
    manifest: &EvoSkillReplicaManifest,
    loop_report: Option<&EvoSkillReplicaLoopReport>,
    live_run_gate: &LiveRunGateReport,
    score_slots: &[FinalScoreSlot],
) -> Vec<PaperCloseGateReport> {
    let source_blocker_ids = manifest
        .source_blockers
        .iter()
        .map(|blocker| blocker.blocker_id.clone())
        .collect::<Vec<_>>();
    let sealqa_judge_scores_complete = sealqa_judge_score_evidence_complete(score_slots);
    let paper_scorer_status = if sealqa_judge_scores_complete {
        PaperCloseGateStatus::Proven
    } else {
        PaperCloseGateStatus::ApprovalBlocked
    };
    let paper_scorer_blockers = if sealqa_judge_scores_complete {
        Vec::new()
    } else {
        vec![
            "sealqa_judge_scored_run".to_owned(),
            "live_run_spend_approval".to_owned(),
        ]
    };
    let paper_scorer_note = if sealqa_judge_scores_complete {
        "OfficeQA scorer laws, source-backed SealQA judge template/request surface, and approved external SealQA judge score sidecars cover every SealQA score slot"
    } else {
        "OfficeQA scorer laws and the source-backed SealQA judge template/request surface are proven; complete approved SealQA judge scoring still needs approval/evidence"
    };
    vec![
        paper_close_gate(
            "replica_manifest",
            PaperCloseGateStatus::Proven,
            Vec::new(),
            "schema v13 manifest declares source universe, local source identity, optional validated source pins, optional accepted paper-close substitute split policy, optional BrowseComp transfer JSONL materialization, fingerprints, paper targets, source blockers with checked local candidate evidence, source-backed judge template pins, model pins, scorer, frontier, and schedule",
        ),
        paper_close_gate(
            "source_and_split_materialization",
            if source_blocker_ids.is_empty() {
                PaperCloseGateStatus::Proven
            } else {
                PaperCloseGateStatus::SourceBlocked
            },
            source_blocker_ids,
            "OfficeQA, SealQA, and BrowseComp materializations are auditable; exact paper splits, strict transfer sidecars, or accepted paper-close substitute policy decide whether source blockers remain",
        ),
        paper_close_gate(
            "paper_scorer",
            paper_scorer_status,
            paper_scorer_blockers,
            paper_scorer_note,
        ),
        paper_close_gate(
            "full_loop_mechanics",
            if loop_report.is_some() {
                PaperCloseGateStatus::Proven
            } else {
                PaperCloseGateStatus::SourceBlocked
            },
            if loop_report.is_some() {
                Vec::new()
            } else {
                vec!["officeqa_full_csv_missing".to_owned()]
            },
            "multi-iteration git-program/frontier/checkpoint loop traverses the OfficeQA validation role before admission, but remains mechanics evidence only, not live provider or paper-score evidence",
        ),
        paper_close_gate(
            "live_small_run",
            PaperCloseGateStatus::ApprovalBlocked,
            live_run_gate.blocker_ids.clone(),
            "no provider call or credential probe has run without explicit spend approval",
        ),
        paper_close_gate(
            "final_report_truth",
            PaperCloseGateStatus::Proven,
            Vec::new(),
            "report keeps blocked metrics missing, records exactness gaps, costs, paper-target-linked score slots including unscored or source-blocked BrowseComp transfer slots, source blockers, and approval gates",
        ),
        paper_close_gate(
            "proxy_closeout",
            PaperCloseGateStatus::Proven,
            Vec::new(),
            "typed proxy rejection gates reject fixtures, benchmarks, fake runtime, single-sample inspection, and repo health as completion evidence",
        ),
    ]
}

fn paper_close_gate(
    id: &str,
    status: PaperCloseGateStatus,
    blocker_ids: Vec<String>,
    note: &str,
) -> PaperCloseGateReport {
    PaperCloseGateReport {
        id: id.to_owned(),
        status,
        blocker_ids,
        note: note.to_owned(),
    }
}

fn final_report_ablations(
    manifest: &EvoSkillReplicaManifest,
    score_slots: &[FinalScoreSlot],
) -> Vec<AblationStatusReport> {
    let sealqa_judge_scores_complete = sealqa_judge_score_evidence_complete(score_slots);
    let all_blockers = final_report_blockers(manifest, score_slots)
        .iter()
        .map(|blocker| blocker.id.clone())
        .collect::<Vec<_>>();
    let source_blocked = !manifest.source_blockers.is_empty();
    let main_run_note = if source_blocked {
        "requires paper split/source denominator and live scored run"
    } else if sealqa_judge_scores_complete {
        "source/split denominator and approved SealQA judge evidence are present; remaining scoring work is blocked on approved live execution"
    } else {
        "source/split denominator is materialized under the declared policy; scoring remains blocked on approved live execution and SealQA judge scoring"
    };
    let mut skill_merge_blockers = source_universe_blocker_ids(
        manifest,
        "officeqa",
        &["officeqa_exact_split_membership".to_owned()],
    );
    extend_unique(
        &mut skill_merge_blockers,
        &["live_run_spend_approval".to_owned()],
    );
    let mut browsecomp_blockers = source_universe_blocker_ids(
        manifest,
        "browsecomp_transfer",
        &["browsecomp_transfer_sample".to_owned()],
    );
    let browsecomp_note = if browsecomp_blockers
        .iter()
        .any(|blocker| blocker == "browsecomp_transfer_sample")
    {
        "BrowseComp transfer denominator is absent; no transfer score can be interpreted".to_owned()
    } else {
        let mut score_blockers = Vec::new();
        if !sealqa_judge_scores_complete {
            score_blockers.push("sealqa_judge_scored_run".to_owned());
        }
        score_blockers.push("live_run_spend_approval".to_owned());
        extend_unique(&mut browsecomp_blockers, &score_blockers);
        if sealqa_judge_scores_complete {
            "BrowseComp transfer denominator and approved SealQA judge evidence are present; transferred-skill scoring remains blocked on approved live baseline/transfer runs".to_owned()
        } else {
            "BrowseComp transfer denominator is materialized; scoring remains blocked on approved SealQA skill production and live baseline/transfer runs".to_owned()
        }
    };
    let skill_merge_source_blocked = skill_merge_blockers
        .iter()
        .any(|blocker| blocker == "officeqa_exact_split_membership");
    let skill_merge_note = if skill_merge_source_blocked {
        "OfficeQA skill-merge comparison targets are reported as two paper-source candidates; the run still needs exact split/source denominator and live scoring"
    } else {
        "OfficeQA skill-merge comparison targets are reported as two paper-source candidates; the declared denominator is ready and scoring remains blocked on approved live execution"
    };
    vec![
        AblationStatusReport {
            id: "skill_only".to_owned(),
            status: "blocked".to_owned(),
            blocker_ids: all_blockers.clone(),
            note: main_run_note.to_owned(),
        },
        AblationStatusReport {
            id: "prompt_only".to_owned(),
            status: "blocked".to_owned(),
            blocker_ids: all_blockers,
            note: main_run_note.to_owned(),
        },
        AblationStatusReport {
            id: "skill_merge".to_owned(),
            status: if skill_merge_source_blocked {
                "blocked"
            } else {
                "approval_blocked"
            }
            .to_owned(),
            blocker_ids: skill_merge_blockers,
            note: skill_merge_note.to_owned(),
        },
        AblationStatusReport {
            id: "browsecomp_transfer".to_owned(),
            status: if browsecomp_blockers
                .iter()
                .any(|blocker| blocker == "browsecomp_transfer_sample")
            {
                "blocked"
            } else {
                "approval_blocked"
            }
            .to_owned(),
            blocker_ids: browsecomp_blockers,
            note: browsecomp_note,
        },
    ]
}

fn final_report_live_run_gate(manifest: &EvoSkillReplicaManifest) -> LiveRunGateReport {
    let runtime_model = manifest
        .model_pins
        .iter()
        .find(|pin| pin.role == "paper_agent_runtime")
        .and_then(|pin| pin.leaven_candidate_model.clone());
    LiveRunGateReport {
        status: LiveRunGateStatus::BlockedNoSpendApproval,
        runtime_role: "paper_agent_runtime".to_owned(),
        candidate_model: runtime_model,
        credential_probe_status: "not_probed_no_spend_default".to_owned(),
        spend_approval_status: "not_approved".to_owned(),
        blocker_ids: vec!["live_run_spend_approval".to_owned()],
        note: "bounded live agent run is intentionally not executed without explicit provider spend and credential approval".to_owned(),
    }
}

fn initial_replica_loop_state(
    officeqa: &OfficeQaSourceMaterialization,
    git: &EvoSkillAgenticGitHarness,
) -> Result<EvoSkillReplicaLoopState, ManifestError> {
    let seed = CandidateId::new();
    let mut frontier = TopKFrontier::new(
        NonZeroUsize::new(EVOSKILL_REPLICA_FRONTIER_SIZE)
            .expect("replica frontier capacity is non-zero"),
    );
    observe_replica_frontier(&mut frontier, seed, 0.30)?;
    Ok(EvoSkillReplicaLoopState {
        frontier,
        parent_selector: TopKParentSelector::round_robin(),
        train_sampler: officeqa_train_sampler(officeqa)?,
        candidates: vec![EvoSkillReplicaCandidateState {
            id: seed,
            program: git.seed_program.clone(),
            validation_score: 0.30,
        }],
        feedback_history: Vec::new(),
    })
}

fn run_replica_iteration(
    state: &mut EvoSkillReplicaLoopState,
    officeqa: &OfficeQaSourceMaterialization,
    git: &EvoSkillAgenticGitHarness,
    iteration: u64,
    validation_score: f64,
) -> Result<EvoSkillReplicaIterationReport, ManifestError> {
    let selected_parent = state
        .parent_selector
        .select(&state.frontier)
        .ok_or_else(|| ManifestError::LoopBlocked {
            reason: "replica frontier is empty; no parent can be selected".to_owned(),
        })?;
    let parent_program = candidate_program(&state.candidates, selected_parent)?;
    let parent_revision = program_revision(&parent_program)?;
    let sample = state.train_sampler.next_batch();
    let feedback_rows_seen =
        u64::try_from(state.feedback_history.len()).expect("feedback count fits in u64");
    let attempts = feedback_attempts(officeqa, &sample, iteration);
    let new_feedback = extract_evoskill_failure_feedback(attempts);
    let new_feedback_rows = u64::try_from(new_feedback.len()).expect("feedback count fits in u64");
    state.feedback_history.extend(new_feedback);

    let child = CandidateId::new();
    let (change, child_program) = read_back_agentic_git_child(git, &parent_program, iteration)?;
    let (expected_parent, child_revision) = single_repo_advance(&change)?;
    let validation_rows_evaluated = officeqa_validation_rows_evaluated(officeqa)?;
    observe_replica_frontier(&mut state.frontier, child, validation_score)?;
    let admitted = state.frontier.contains(child);
    state.candidates.push(EvoSkillReplicaCandidateState {
        id: child,
        program: child_program,
        validation_score,
    });

    Ok(EvoSkillReplicaIterationReport {
        iteration,
        selected_parent,
        train_sample_rows: u64::try_from(sample.len()).expect("sample count fits in u64"),
        feedback_rows_seen,
        new_feedback_rows,
        child,
        parent_revision,
        change_expected_parent: revision_string(&expected_parent),
        child_revision: revision_string(&child_revision),
        validation_rows_evaluated,
        validation_score,
        admitted,
        frontier_members_after: state.frontier.members().to_vec(),
    })
}

fn round_trip_replica_checkpoint(
    state: &mut EvoSkillReplicaLoopState,
    after_iteration: u64,
) -> Result<EvoSkillReplicaCheckpointResumeReport, ManifestError> {
    let frontier_before = state.frontier.members().to_vec();
    let parent_selector_cursor_before = state.parent_selector.cursor();
    let checkpoint =
        serde_json::to_vec(state).map_err(|source| ManifestError::Checkpoint { source })?;
    let restored = serde_json::from_slice::<EvoSkillReplicaLoopState>(&checkpoint)
        .map_err(|source| ManifestError::Checkpoint { source })?;
    let frontier_after = restored.frontier.members().to_vec();
    let parent_selector_cursor_after = restored.parent_selector.cursor();
    *state = restored;
    Ok(EvoSkillReplicaCheckpointResumeReport {
        after_iteration,
        frontier_before,
        frontier_after,
        parent_selector_cursor_before,
        parent_selector_cursor_after,
    })
}

fn create_agentic_git_harness() -> Result<EvoSkillAgenticGitHarness, ManifestError> {
    let temp = tempfile::tempdir().map_err(|source| ManifestError::Io {
        action: "create temporary agentic Git harness",
        path: std::env::temp_dir(),
        source,
    })?;
    let source = temp.path().join("program-source");
    let store = temp.path().join("program.git");
    create_local_git_repo(&source, "program.txt", "program base\n")?;
    run_local_git(
        temp.path(),
        ["clone", "--bare", "program-source", "program.git"],
    )?;
    let parent = git_revision_from_repo(&source, "HEAD")?;
    let program = repo_key("program")?;
    let stores = GitProgramStores::new(BTreeMap::from([(program.clone(), store)]))
        .map_err(|source| ManifestError::AgenticGit { source })?;
    let seed_program = GitProgramArtifact::new(
        BTreeMap::from([(
            program.clone(),
            GitRepoArtifact::new(
                RepoRef::global(program.clone()),
                parent,
                None,
                GitArtifactIdentityMode::Commit,
            ),
        )]),
        GitProgramLayout::new(BTreeMap::from([(
            program,
            GitPath::new("repos/program").map_err(|source| ManifestError::Git { source })?,
        )]))
        .map_err(|source| ManifestError::Git { source })?,
    )
    .map_err(|source| ManifestError::Git { source })?;
    Ok(EvoSkillAgenticGitHarness {
        stores,
        seed_program,
        _temp: temp,
    })
}

fn read_back_agentic_git_child(
    git: &EvoSkillAgenticGitHarness,
    parent: &GitProgramArtifact,
    iteration: u64,
) -> Result<(GitProgramChange, GitProgramArtifact), ManifestError> {
    block_on(async {
        let workspace_root = tempfile::tempdir().map_err(|source| ManifestError::Io {
            action: "create temporary agentic Git workspace",
            path: std::env::temp_dir(),
            source,
        })?;
        let mut workspace = LocalWorkspaceFactory::new(workspace_root.path())
            .allocate(WorkspaceConfig::default())
            .await
            .map_err(|source| ManifestError::WorkspaceFactory { source })?;
        let mut view = workspace.view();
        GitProgramMaterializer::new(git.stores.clone())
            .materialize_program(parent, &mut view)
            .map_err(|source| ManifestError::AgenticGit { source })?;
        configure_workspace_git(&mut view)?;
        let body = format!("program child iteration {iteration}\n");
        view.write_file(
            &workspace_path("repos/program/program.txt")?,
            body.as_bytes(),
        )
        .map_err(|source| ManifestError::Workspace { source })?;
        workspace_git(&mut view, ["add", "program.txt"])?;
        workspace_git(&mut view, ["commit", "-m", "replica child"])?;
        let change = GitProgramReadback::new(git.stores.clone())
            .read_back_change(parent, &mut view)
            .map_err(|source| ManifestError::AgenticGit { source })?
            .ok_or_else(|| ManifestError::LoopBlocked {
                reason: "agentic Git readback returned no child proposal".to_owned(),
            })?;
        let child = parent
            .apply_change(&change)
            .map_err(|source| ManifestError::Git { source })?;
        drop(view);
        workspace
            .cleanup()
            .await
            .map_err(|source| ManifestError::Workspace { source })?;
        drop(workspace_root);
        Ok((change, child))
    })
}

fn single_repo_advance(
    change: &GitProgramChange,
) -> Result<(GitRevision, GitRevision), ManifestError> {
    match change {
        GitProgramChange::AdvanceRepo {
            expected_parent,
            child,
            ..
        } => Ok((expected_parent.clone(), child.clone())),
        GitProgramChange::AdvanceRepos { .. } => Err(ManifestError::LoopBlocked {
            reason: "replica readback expected a single program repo child".to_owned(),
        }),
    }
}

fn create_local_git_repo(root: &Path, file: &str, body: &str) -> Result<(), ManifestError> {
    fs::create_dir_all(root).map_err(|source| ManifestError::Io {
        action: "create local git source",
        path: root.to_owned(),
        source,
    })?;
    run_local_git(root, ["init", "--initial-branch=main"])?;
    run_local_git(root, ["config", "user.name", "Leaven Test"])?;
    run_local_git(root, ["config", "user.email", "leaven@example.invalid"])?;
    fs::write(root.join(file), body).map_err(|source| ManifestError::Io {
        action: "write local git source file",
        path: root.join(file),
        source,
    })?;
    run_local_git(root, ["add", file])?;
    run_local_git(root, ["commit", "-m", "base"])?;
    Ok(())
}

fn run_local_git<const N: usize>(cwd: &Path, args: [&str; N]) -> Result<(), ManifestError> {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .map_err(|source| ManifestError::Io {
            action: "run local git",
            path: cwd.to_owned(),
            source,
        })?;
    if output.status.success() {
        return Ok(());
    }
    Err(ManifestError::GitCommand {
        cwd: cwd.to_owned(),
        args: args.join(" "),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}

fn git_revision_from_repo(root: &Path, revision: &str) -> Result<GitRevision, ManifestError> {
    let output = Command::new("git")
        .args(["rev-parse", revision])
        .current_dir(root)
        .output()
        .map_err(|source| ManifestError::Io {
            action: "resolve git revision",
            path: root.to_owned(),
            source,
        })?;
    if !output.status.success() {
        return Err(ManifestError::GitCommand {
            cwd: root.to_owned(),
            args: format!("rev-parse {revision}"),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }
    let id = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    Ok(GitRevision::Commit(
        GitObjectId::new(id).map_err(|source| ManifestError::Git { source })?,
    ))
}

fn configure_workspace_git(
    view: &mut leaven_workspace::WorkspaceView<'_>,
) -> Result<(), ManifestError> {
    workspace_git(view, ["config", "user.name", "Leaven Test"])?;
    workspace_git(view, ["config", "user.email", "leaven@example.invalid"])
}

fn workspace_git<const N: usize>(
    view: &mut leaven_workspace::WorkspaceView<'_>,
    args: [&str; N],
) -> Result<(), ManifestError> {
    let mut command = WorkspaceCommand::new("git");
    command.cwd = Some(workspace_path("repos/program")?);
    command.args = args.into_iter().map(str::to_owned).collect();
    let output = view
        .run_command(command)
        .map_err(|source| ManifestError::Workspace { source })?;
    if output.status.code == Some(0) {
        return Ok(());
    }
    Err(ManifestError::GitCommand {
        cwd: PathBuf::from("workspace:/repos/program"),
        args: args.join(" "),
        stderr: String::from_utf8_lossy(&output.stderr.bytes).into_owned(),
    })
}

fn workspace_path(path: &str) -> Result<WorkspacePath, ManifestError> {
    WorkspacePath::new(path).map_err(|source| ManifestError::AgenticGit {
        source: GitAgenticGitError::WorkspacePath(source),
    })
}

fn officeqa_train_sampler(
    officeqa: &OfficeQaSourceMaterialization,
) -> Result<CategoryRoundRobinSampler, ManifestError> {
    let train_split = officeqa_loop_train_split(&officeqa.report)?;
    let train_role = split_role_report(train_split, "train")?;
    let source_to_case = source_id_case_map(&officeqa.rows);
    let mut pools = BTreeMap::<SmolStr, Vec<CaseId>>::new();
    for source_id in &train_role.source_ids {
        let case =
            source_to_case
                .get(source_id)
                .copied()
                .ok_or_else(|| ManifestError::LoopBlocked {
                    reason: format!(
                        "train split references missing OfficeQA source id `{source_id}`"
                    ),
                })?;
        let row = officeqa_row(officeqa, case)?;
        pools
            .entry(SmolStr::new(row.input().difficulty.as_str()))
            .or_default()
            .push(case);
    }
    CategoryRoundRobinSampler::new(
        pools,
        NonZeroUsize::new(2).expect("replica categories per batch is non-zero"),
        NonZeroUsize::new(1).expect("replica samples per category is non-zero"),
    )
    .map_err(|source| ManifestError::Sampler { source })
}

fn officeqa_validation_rows_evaluated(
    officeqa: &OfficeQaSourceMaterialization,
) -> Result<u64, ManifestError> {
    let split = officeqa_loop_train_split(&officeqa.report)?;
    let validation_role = split_role_report(split, "validation")?;
    let source_to_case = source_id_case_map(&officeqa.rows);
    for source_id in &validation_role.source_ids {
        let case =
            source_to_case
                .get(source_id)
                .copied()
                .ok_or_else(|| ManifestError::LoopBlocked {
                    reason: format!(
                        "validation split references missing OfficeQA source id `{source_id}`"
                    ),
                })?;
        let _row = officeqa_row(officeqa, case)?;
    }
    Ok(validation_role.rows)
}

fn feedback_attempts(
    officeqa: &OfficeQaSourceMaterialization,
    sample: &[leaven_eval::CategorySample],
    iteration: u64,
) -> Vec<EvoSkillAnswerAttempt> {
    sample
        .iter()
        .map(|sample| {
            let row = officeqa_row(officeqa, sample.case)
                .expect("sampler case ids come from the OfficeQA materialization");
            EvoSkillAnswerAttempt {
                source_id: row.source_id().to_owned(),
                ground_truth: row.target().expect("OfficeQA rows are targeted").clone(),
                prediction: format!("incorrect iteration {iteration} {}", sample.category),
            }
        })
        .collect()
}

fn observe_replica_frontier(
    frontier: &mut TopKFrontier,
    candidate: CandidateId,
    score: f64,
) -> Result<(), ManifestError> {
    let score = ScalarEvidence::new(score).map_err(|source| ManifestError::Scalar { source })?;
    let _events = frontier.observe(candidate, AssessmentId::new(), score);
    Ok(())
}

fn candidate_program(
    candidates: &[EvoSkillReplicaCandidateState],
    candidate: CandidateId,
) -> Result<GitProgramArtifact, ManifestError> {
    candidates
        .iter()
        .find(|state| state.id == candidate)
        .map(|state| state.program.clone())
        .ok_or_else(|| ManifestError::LoopBlocked {
            reason: format!("selected parent candidate {candidate} is missing"),
        })
}

fn officeqa_row(
    officeqa: &OfficeQaSourceMaterialization,
    case: CaseId,
) -> Result<&SourceRow<OfficeQaInput, String>, ManifestError> {
    officeqa
        .rows
        .rows()
        .get(usize::try_from(case.0).expect("case id fits in usize"))
        .ok_or_else(|| ManifestError::LoopBlocked {
            reason: format!("OfficeQA case {case} is outside the materialized row universe"),
        })
}

fn parent_program_revision(program: &GitProgramArtifact) -> Result<GitRevision, ManifestError> {
    let key = repo_key("program")?;
    program
        .repo(&key)
        .map(|repo| repo.revision().clone())
        .ok_or_else(|| ManifestError::LoopBlocked {
            reason: "replica Git program is missing the program repo".to_owned(),
        })
}

fn program_revision(program: &GitProgramArtifact) -> Result<String, ManifestError> {
    parent_program_revision(program).map(|revision| revision_string(&revision))
}

fn revision_string(revision: &GitRevision) -> String {
    revision.object_id().as_str().to_owned()
}

fn repo_key(value: &str) -> Result<RepoKey, ManifestError> {
    RepoKey::new(value).map_err(|source| ManifestError::Git { source })
}

fn missing_materialization_report(
    dataset_id: &str,
    source_artifact_id: &str,
    blocker_id: &str,
) -> DatasetMaterializationReport {
    DatasetMaterializationReport {
        dataset_id: dataset_id.to_owned(),
        source_artifact_id: source_artifact_id.to_owned(),
        source_status: SourceMaterializationStatus::MissingArtifact,
        source_rows: None,
        case_rows: None,
        source_row_fingerprint: None,
        source_artifact_sha256: None,
        target_policy: "blocked: source artifact is not present".to_owned(),
        strata: Vec::new(),
        split_materializations: Vec::new(),
        blocker_ids: vec![blocker_id.to_owned()],
    }
}

#[must_use]
pub fn score_evoskill_answer(ground_truth: &str, prediction: &str) -> EvoSkillScoreReport {
    let tolerance_scores = DEFAULT_TOLERANCES
        .into_iter()
        .map(|tolerance| {
            let weight = tolerance_weight(tolerance);
            ToleranceScore {
                tolerance,
                weight,
                score: score_at_tolerance(ground_truth, prediction, tolerance),
            }
        })
        .collect::<Vec<_>>();
    let weight_total = tolerance_scores
        .iter()
        .map(|score| score.weight)
        .sum::<f64>();
    let weighted_score = tolerance_scores
        .iter()
        .map(|score| score.weight * score.score)
        .sum::<f64>()
        / weight_total;
    EvoSkillScoreReport {
        weighted_score,
        is_failure: weighted_score < DEFAULT_FAILURE_THRESHOLD,
        tolerance_scores,
    }
}

#[must_use]
pub fn score_evoskill_attempt(attempt: EvoSkillAnswerAttempt) -> EvoSkillScoredAttempt {
    let score = score_evoskill_answer(&attempt.ground_truth, &attempt.prediction);
    EvoSkillScoredAttempt {
        source_id: attempt.source_id,
        ground_truth: attempt.ground_truth,
        prediction: attempt.prediction,
        score,
    }
}

pub fn extract_evoskill_failure_feedback(
    attempts: impl IntoIterator<Item = EvoSkillAnswerAttempt>,
) -> Vec<EvoSkillFailureFeedbackRow> {
    attempts
        .into_iter()
        .map(score_evoskill_attempt)
        .filter(|attempt| attempt.score.is_failure)
        .map(failure_feedback_row)
        .collect()
}

fn failure_feedback_row(attempt: EvoSkillScoredAttempt) -> EvoSkillFailureFeedbackRow {
    let feedback = format!(
        "case {} failed: weighted score {:.3} below {:.3}; expected `{}`, got `{}`",
        attempt.source_id,
        attempt.score.weighted_score,
        DEFAULT_FAILURE_THRESHOLD,
        attempt.ground_truth,
        attempt.prediction
    );
    EvoSkillFailureFeedbackRow {
        source_id: attempt.source_id,
        ground_truth: attempt.ground_truth,
        prediction: attempt.prediction,
        weighted_score: attempt.score.weighted_score,
        feedback,
    }
}

fn score_at_tolerance(ground_truth: &str, prediction: &str, tolerance: f64) -> f64 {
    if prediction.trim().is_empty() {
        return 0.0;
    }
    let ground_numbers = number_mentions(ground_truth);
    let prediction_numbers = number_mentions(prediction);
    if !ground_numbers.is_empty() {
        if prediction_numbers.is_empty() {
            return 0.0;
        }
        let prediction_numbers = filter_prediction_years(ground_truth, prediction_numbers);
        let normalized_prediction = normalized_text(prediction);
        let text_ok = required_text_tokens(ground_truth)
            .iter()
            .all(|token| normalized_prediction.contains(token));
        let numbers_ok = ground_numbers.iter().all(|ground| {
            prediction_numbers.iter().any(|prediction| {
                numeric_match(ground.base_value, prediction.base_value, tolerance)
            })
        });
        return f64::from(numbers_ok && text_ok);
    }
    let truth = normalized_text(ground_truth);
    let predicted = normalized_text(prediction);
    f64::from(!truth.is_empty() && predicted.contains(&truth))
}

fn tolerance_weight(tolerance: f64) -> f64 {
    1.0 / (1.0 + 20.0 * tolerance)
}

#[derive(Clone, Debug)]
struct NumberMention {
    base_value: f64,
}

fn number_mentions(text: &str) -> Vec<NumberMention> {
    let mut mentions = Vec::new();
    let mut start = None;
    for (index, ch) in text.char_indices() {
        if ch.is_ascii_digit() || ch == '-' || ch == '.' || ch == ',' {
            start.get_or_insert(index);
        } else if let Some(token_start) = start.take() {
            push_number_mention(text, token_start, index, &mut mentions);
        }
    }
    if let Some(token_start) = start {
        push_number_mention(text, token_start, text.len(), &mut mentions);
    }
    mentions
}

fn push_number_mention(text: &str, start: usize, end: usize, mentions: &mut Vec<NumberMention>) {
    let raw = &text[start..end];
    let normalized = raw.replace(',', "");
    if normalized.is_empty() || normalized == "-" || normalized == "." {
        return;
    }
    if let Ok(value) = normalized.parse::<f64>() {
        mentions.push(NumberMention {
            base_value: value * unit_multiplier(context_window(text, start, end)).unwrap_or(1.0),
        });
    }
}

fn context_window(text: &str, start: usize, end: usize) -> &str {
    let context_start = text[..start]
        .char_indices()
        .rev()
        .nth(19)
        .map_or(0, |(index, _)| index);
    let context_end = text[end..]
        .char_indices()
        .nth(20)
        .map_or(text.len(), |(index, _)| end + index);
    &text[context_start..context_end]
}

fn unit_multiplier(context: &str) -> Option<f64> {
    let context = context.to_ascii_lowercase();
    if context.contains("trillion") {
        Some(1_000_000_000_000.0)
    } else if context.contains("billion") {
        Some(1_000_000_000.0)
    } else if context.contains("million") {
        Some(1_000_000.0)
    } else {
        None
    }
}

fn filter_prediction_years(
    ground_truth: &str,
    prediction_numbers: Vec<NumberMention>,
) -> Vec<NumberMention> {
    if ground_truth_allows_years(ground_truth) {
        return prediction_numbers;
    }
    prediction_numbers
        .into_iter()
        .filter(|number| !is_year_like(number.base_value))
        .collect()
}

fn ground_truth_allows_years(ground_truth: &str) -> bool {
    number_mentions(ground_truth)
        .iter()
        .any(|number| is_year_like(number.base_value))
        || !required_text_tokens(ground_truth).is_empty()
}

fn is_year_like(value: f64) -> bool {
    value.fract() == 0.0 && (YEAR_START..=YEAR_END).contains(&value)
}

fn numeric_match(ground_truth: f64, prediction: f64, tolerance: f64) -> bool {
    if ground_truth == 0.0 {
        prediction == 0.0
    } else {
        ((ground_truth - prediction).abs() / ground_truth.abs()) <= tolerance
    }
}

fn required_text_tokens(text: &str) -> Vec<String> {
    normalized_text_without_numbers(text)
        .split_whitespace()
        .filter(|token| !is_ignored_text_token(token))
        .map(str::to_owned)
        .collect()
}

fn normalized_text(text: &str) -> String {
    strip_parentheticals(text)
        .trim()
        .to_ascii_lowercase()
        .replace(['"', '\'', ',', '.'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn normalized_text_without_numbers(text: &str) -> String {
    let stripped = strip_number_runs(text);
    normalized_text(&stripped)
}

fn strip_number_runs(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        if ch.is_ascii_digit() || ch == '-' || ch == '.' || ch == ',' {
            out.push(' ');
        } else {
            out.push(ch);
        }
    }
    out
}

fn strip_parentheticals(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut depth = 0_u32;
    for ch in text.chars() {
        match ch {
            '(' => depth = depth.saturating_add(1),
            ')' => depth = depth.saturating_sub(1),
            _ if depth == 0 => out.push(ch),
            _ => {}
        }
    }
    out
}

fn is_unit_word(token: &str) -> bool {
    matches!(
        token,
        "million" | "millions" | "billion" | "billions" | "trillion" | "trillions"
    )
}

fn is_ignored_text_token(token: &str) -> bool {
    is_unit_word(token) || matches!(token, "and" | "or" | "the" | "a" | "an" | "of")
}

fn source_revisions(
    root: &Path,
    source_pin_manifest: Option<&ValidatedSourcePinManifest>,
) -> Vec<SourceRevision> {
    SOURCE_REVISION_SPECS
        .iter()
        .map(|(id, relative_path)| {
            let pinned = source_pin_manifest.and_then(|manifest| {
                manifest
                    .sources
                    .get(*id)
                    .map(|entry| (manifest.policy, entry))
            });
            source_revision(root, id, relative_path, pinned)
        })
        .collect()
}

fn source_artifacts(root: &Path) -> Result<Vec<SourceArtifact>, ManifestError> {
    Ok(vec![
        source_artifact(
            root,
            "source_pin_manifest",
            "source pin manifest",
            SOURCE_PIN_MANIFEST_PATH,
        )?,
        source_artifact(
            root,
            "split_policy_manifest",
            "paper-close substitute split policy manifest",
            SPLIT_POLICY_MANIFEST_PATH,
        )?,
        source_artifact(
            root,
            "paper_full_source",
            "paper source text",
            "tmp/skill_opt_sources/arx_2603.02766/full_source.md",
        )?,
        source_artifact(
            root,
            SEALQA_JUDGE_SOURCE_ARTIFACT_ID,
            "SealQA auto-grader prompt placeholder from paper appendix",
            "tmp/skill_opt_sources/arx_2603.02766/src/appendix/agent-prompts/auto_grader_placeholder.md",
        )?,
        source_artifact(
            root,
            "officeqa_full_csv",
            "OfficeQA full CSV",
            "tmp/repros/officeqa/officeqa_full.csv",
        )?,
        source_artifact(
            root,
            "officeqa_pro_csv",
            "OfficeQA pro CSV",
            "tmp/repros/officeqa/officeqa_pro.csv",
        )?,
        source_artifact(
            root,
            "officeqa_exact_split_manifest",
            "OfficeQA paper-declared source-id split manifest",
            OFFICEQA_EXACT_SPLIT_MANIFEST_PATH,
        )?,
        source_artifact(
            root,
            "sealqa_parquet",
            "SealQA seal-0 parquet",
            "tmp/replication/evoskill/sealqa/seal-0.parquet",
        )?,
        source_artifact(
            root,
            "sealqa_exact_split_manifest",
            "SealQA paper-declared source-id split manifest",
            SEALQA_EXACT_SPLIT_MANIFEST_PATH,
        )?,
        source_artifact(
            root,
            "officeqa_validation_sample",
            "OfficeQA inspected sample",
            "tmp/paper_exact_samples/evoskill/officeqa/officeqa_pro_first_case.json",
        )?,
        source_artifact(
            root,
            "sealqa_validation_sample",
            "SealQA inspected sample",
            "tmp/paper_exact_samples/evoskill/sealqa/seal_0_first_case.json",
        )?,
        source_artifact(
            root,
            "browsecomp_public_csv",
            "official BrowseComp public encrypted CSV",
            BROWSECOMP_PUBLIC_CSV_PATH,
        )?,
        source_artifact(
            root,
            "browsecomp_transfer_sample",
            "BrowseComp transfer sample",
            BROWSECOMP_TRANSFER_SAMPLE_PATH,
        )?,
    ])
}

fn source_blockers(
    root: &Path,
    materializations: &[DatasetMaterializationReport],
    source_pin_resolved: bool,
) -> Result<Vec<SourceBlockerReport>, ManifestError> {
    let mut reports = Vec::new();
    if !source_pin_resolved {
        reports.push(source_blocker(
            root,
            "source_pin",
            "all",
            SourceBlockerStatus::UnresolvedSourcePolicy,
            &["paper_close_comparison_denominator"],
            &[
                SOURCE_PIN_MANIFEST_PATH,
                "tmp/skill_opt_sources/arx_2603.02766/full_source.md",
                "tmp/repros/evoskill",
                "tmp/repros/officeqa",
            ],
            "schema-v1 local-checkout source pin manifest is absent",
        )?);
    }
    if !materializations
        .iter()
        .find(|materialization| materialization.dataset_id == "officeqa")
        .is_some_and(has_accepted_or_exact_split)
    {
        reports.push(source_blocker(
            root,
            "officeqa_category_split_manifest",
            "officeqa",
            SourceBlockerStatus::MissingLocalArtifact,
            &["officeqa_train_validation_test_split"],
            &[
                "tmp/repros/evoskill/.dataset/new_runs_base/solved_dataset.csv",
                "tmp/repros/evoskill/results/full_run_new_evolved_final_two.pkl",
                "tmp/repros/evoskill/ablation_run_incorrect.csv",
                SPLIT_POLICY_MANIFEST_PATH,
            ],
            "paper references LLM-derived categories/pseudo-labels, but the local source tree only exposes difficulty labels",
        )?);
        reports.push(source_blocker(
            root,
            "officeqa_exact_split_membership",
            "officeqa",
            SourceBlockerStatus::MissingExactSplitManifest,
            &["officeqa_scored_train_validation_held_out_report"],
            &[
                OFFICEQA_EXACT_SPLIT_MANIFEST_PATH,
                SPLIT_POLICY_MANIFEST_PATH,
                "tmp/repros/evoskill/.dataset/new_runs_base/solved_dataset.csv",
                "tmp/repros/evoskill/ablation_run_incorrect.csv",
            ],
            "difficulty-stratified substitute splits are auditable, but paper exact membership is absent",
        )?);
    }
    if !materializations
        .iter()
        .find(|materialization| materialization.dataset_id == "sealqa")
        .is_some_and(has_accepted_or_exact_split)
    {
        reports.push(source_blocker(
            root,
            "sealqa_split_manifest",
            "sealqa",
            SourceBlockerStatus::MissingExactSplitManifest,
            &["sealqa_scored_train_held_out_report"],
            &[
                SEALQA_EXACT_SPLIT_MANIFEST_PATH,
                SPLIT_POLICY_MANIFEST_PATH,
                "tmp/replication/evoskill/sealqa/seal-0.parquet",
            ],
            "seal-0 rows are materialized from Parquet, but the exact 10 percent train versus held-out row ids are not present",
        )?);
    }
    if !materializations
        .iter()
        .find(|materialization| materialization.dataset_id == "browsecomp_transfer")
        .is_some_and(has_materialized_browsecomp_transfer_sample)
    {
        reports.push(source_blocker(
            root,
            "browsecomp_transfer_sample",
            "browsecomp_transfer",
            SourceBlockerStatus::MissingLocalArtifact,
            &["browsecomp_zero_shot_transfer_report"],
            &[
                BROWSECOMP_TRANSFER_SAMPLE_PATH,
                BROWSECOMP_PUBLIC_CSV_PATH,
                "tmp/repros/evoskill/results/deep_cc_runs",
            ],
            "paper reports a 128-example stratified transfer sample; use the strict sidecar when the exact sample is found or generate a declared public BrowseComp substitute from the official encrypted CSV",
        )?);
    }
    Ok(reports)
}

fn source_blocker(
    root: &Path,
    blocker_id: &str,
    dataset_id: &str,
    status: SourceBlockerStatus,
    required_for: &[&str],
    local_path_candidates: &[&str],
    note: &str,
) -> Result<SourceBlockerReport, ManifestError> {
    Ok(SourceBlockerReport {
        blocker_id: blocker_id.to_owned(),
        dataset_id: dataset_id.to_owned(),
        status,
        required_for: required_for.iter().map(ToString::to_string).collect(),
        local_path_candidates: local_path_candidates
            .iter()
            .map(|relative_path| source_blocker_candidate(root, relative_path))
            .collect::<Result<Vec<_>, _>>()?,
        note: note.to_owned(),
    })
}

fn source_blocker_candidate(
    root: &Path,
    relative_path: &str,
) -> Result<SourceBlockerCandidate, ManifestError> {
    let path = root.join(relative_path);
    if !path.exists() {
        return Ok(SourceBlockerCandidate {
            relative_path: relative_path.to_owned(),
            exists: false,
            is_file: false,
            is_dir: false,
            bytes: None,
            sha256: None,
        });
    }

    let metadata = path.metadata().map_err(|source| ManifestError::Read {
        path: path.clone(),
        source,
    })?;
    let is_file = metadata.is_file();
    Ok(SourceBlockerCandidate {
        relative_path: relative_path.to_owned(),
        exists: true,
        is_file,
        is_dir: metadata.is_dir(),
        bytes: is_file.then_some(metadata.len()),
        sha256: if is_file {
            Some(sha256_file(&path)?)
        } else {
            None
        },
    })
}

fn dataset_requirements(
    materializations: &[DatasetMaterializationReport],
) -> Vec<DatasetRequirement> {
    let mut requirements = vec![
        DatasetRequirement {
            id: "officeqa".to_owned(),
            paper_rows: Some(246),
            train_sizes: vec![12, 24, 36],
            validation_rows: Some(17),
            held_out: "paper reports held-out test and skill-merge tables".to_owned(),
            split_status: SplitManifestStatus::BlockedMissingCategoryManifest,
            blocker_ids: vec![
                "officeqa_category_split_manifest".to_owned(),
                "officeqa_exact_split_membership".to_owned(),
            ],
        },
        DatasetRequirement {
            id: "sealqa".to_owned(),
            paper_rows: Some(111),
            train_sizes: vec![11],
            validation_rows: None,
            held_out: "paper uses 10 percent train and held-out remainder".to_owned(),
            split_status: SplitManifestStatus::BlockedMissingSplitManifest,
            blocker_ids: vec!["sealqa_split_manifest".to_owned()],
        },
        DatasetRequirement {
            id: "browsecomp_transfer".to_owned(),
            paper_rows: Some(128),
            train_sizes: Vec::new(),
            validation_rows: None,
            held_out: "transfer-only evaluation from SealQA skill".to_owned(),
            split_status: SplitManifestStatus::BlockedMissingSplitManifest,
            blocker_ids: vec!["browsecomp_transfer_sample".to_owned()],
        },
    ];
    for requirement in &mut requirements {
        if let Some(materialization) = materializations
            .iter()
            .find(|materialization| materialization.dataset_id == requirement.id)
        {
            if has_paper_exact_split(materialization) {
                requirement.split_status = SplitManifestStatus::ExactPublished;
                requirement.blocker_ids.clear();
            } else if has_accepted_or_exact_split(materialization)
                || has_materialized_browsecomp_transfer_sample(materialization)
            {
                requirement.split_status = SplitManifestStatus::PaperCloseSubstituteAccepted;
                requirement.blocker_ids.clear();
            }
        }
    }
    requirements
}

fn has_materialized_browsecomp_transfer_sample(
    materialization: &DatasetMaterializationReport,
) -> bool {
    materialization.dataset_id == "browsecomp_transfer"
        && materialization.source_status == SourceMaterializationStatus::Materialized
        && materialization.source_rows == Some(BROWSECOMP_TRANSFER_ROWS_U64)
        && materialization.blocker_ids.is_empty()
        && materialization.split_materializations.iter().any(|split| {
            split.exactness == MaterializationExactness::PaperCloseSubstitute
                && split.test_rows == Some(BROWSECOMP_TRANSFER_ROWS_U64)
                && split.blocker_ids.is_empty()
        })
}

fn scorer_manifest(artifacts: &[SourceArtifact]) -> ScorerManifest {
    let judge_source = artifacts
        .iter()
        .find(|artifact| artifact.id == SEALQA_JUDGE_SOURCE_ARTIFACT_ID)
        .expect("source_artifacts always includes the SealQA judge source artifact");
    ScorerManifest {
        id: "evoskill-multi-tolerance-v1".to_owned(),
        tolerances: vec![0.0, 0.01, 0.025, 0.05, 0.10],
        failure_threshold: 0.8,
        implementation_status: "Rust OfficeQA scorer law-tested for multi-tolerance weighting, units, years, text, lists, failure feedback rows, and failure extraction; SealQA judge template is pinned but not run".to_owned(),
        judge_templates: vec![sealqa_judge_template_manifest(judge_source)],
    }
}

fn sealqa_judge_template_manifest(source_artifact: &SourceArtifact) -> JudgeTemplateManifest {
    JudgeTemplateManifest {
        id: SEALQA_JUDGE_TEMPLATE_ID.to_owned(),
        dataset_id: "sealqa".to_owned(),
        source_artifact_id: SEALQA_JUDGE_SOURCE_ARTIFACT_ID.to_owned(),
        source_artifact_exists: source_artifact.exists,
        source_artifact_bytes: source_artifact.bytes,
        source_artifact_sha256: source_artifact.sha256.clone(),
        runtime_status: SEALQA_JUDGE_RUNTIME_STATUS.to_owned(),
        fingerprint: sealqa_judge_template_fingerprint(source_artifact.sha256.as_deref()),
    }
}

#[must_use]
pub fn build_sealqa_judge_request(
    template: &JudgeTemplateManifest,
    question: &str,
    prediction: &str,
    reference: &str,
    tolerance: f64,
) -> SealQaJudgeRequest {
    SealQaJudgeRequest {
        template_id: template.id.clone(),
        template_fingerprint: template.fingerprint.clone(),
        system: SEALQA_JUDGE_SYSTEM_PROMPT.to_owned(),
        user: format!(
            "## Inputs\n- `question`: `{question}`\n- `prediction`: `{prediction}`\n- `reference`: `{reference}`\n- `tolerance`: `{tolerance}`"
        ),
        output_contract: SEALQA_JUDGE_OUTPUT_CONTRACT.to_owned(),
    }
}

fn sealqa_judge_template_fingerprint(source_artifact_sha256: Option<&str>) -> String {
    let mut hasher = Sha256::new();
    hasher.update(SEALQA_JUDGE_TEMPLATE_ID.as_bytes());
    hasher.update(b"\0");
    hasher.update(SEALQA_JUDGE_SOURCE_ARTIFACT_ID.as_bytes());
    hasher.update(b"\0");
    if let Some(source_artifact_sha256) = source_artifact_sha256 {
        hasher.update(source_artifact_sha256.as_bytes());
    }
    hasher.update(b"\0");
    hasher.update(SEALQA_JUDGE_SYSTEM_PROMPT.as_bytes());
    hasher.update(b"\0");
    hasher.update(SEALQA_JUDGE_OUTPUT_CONTRACT.as_bytes());
    hasher.update(b"\0");
    hasher.update(SEALQA_JUDGE_NOTES.as_bytes());
    hex::encode(hasher.finalize())
}

fn frontier_manifest() -> FrontierManifest {
    FrontierManifest {
        capacity: 3,
        parent_selection: "round-robin".to_owned(),
        admission: "validate child before frontier admission".to_owned(),
        eviction: "evict weakest frontier member when full".to_owned(),
    }
}

fn schedule_manifest() -> ScheduleManifest {
    ScheduleManifest {
        epochs: 1.5,
        train_batch_policy: "category-aware without-replacement train batches".to_owned(),
        feedback_history: "proposer sees prior failures and feedback history".to_owned(),
    }
}

fn model_pins() -> Vec<ModelPin> {
    vec![
        ModelPin {
            role: "paper_agent_runtime".to_owned(),
            paper_model: "Claude Code Opus 4.5 per paper ledger".to_owned(),
            leaven_candidate_model: Some(
                "Codex gpt-5.4-mini low for approved small live runs".to_owned(),
            ),
            status: "paper-close may use a declared model delta; paper-exact cannot".to_owned(),
        },
        ModelPin {
            role: "underlying_task_model".to_owned(),
            paper_model:
                "frozen underlying model; exact provider/version unresolved in local ledger"
                    .to_owned(),
            leaven_candidate_model: None,
            status: "blocked until source manifest resolves paper pin".to_owned(),
        },
    ]
}

fn paper_result_targets() -> Vec<PaperResultTarget> {
    vec![
        paper_result_target(PaperResultTargetSpec {
            id: "officeqa_baseline_exact_match_table",
            dataset_id: "officeqa",
            candidate_role: "baseline",
            metric: "exact_match_0_percent_tolerance",
            tolerance: 0.0,
            value_percent: 60.6,
            source: "full_source.md table 1 / src/tables/officeqa_results.tex",
            status: PaperResultTargetStatus::Reported,
            ambiguity_group: None,
        }),
        paper_result_target(PaperResultTargetSpec {
            id: "officeqa_skill_merge_exact_match_prose",
            dataset_id: "officeqa",
            candidate_role: "skill_merge",
            metric: "exact_match_0_percent_tolerance",
            tolerance: 0.0,
            value_percent: 67.9,
            source: "full_source.md figure caption and results prose",
            status: PaperResultTargetStatus::AmbiguousCandidate,
            ambiguity_group: Some("officeqa_skill_merge_exact_match"),
        }),
        paper_result_target(PaperResultTargetSpec {
            id: "officeqa_skill_merge_exact_match_table",
            dataset_id: "officeqa",
            candidate_role: "skill_merge",
            metric: "exact_match_0_percent_tolerance",
            tolerance: 0.0,
            value_percent: 68.1,
            source: "full_source.md table 1 / src/tables/officeqa_results.tex",
            status: PaperResultTargetStatus::AmbiguousCandidate,
            ambiguity_group: Some("officeqa_skill_merge_exact_match"),
        }),
        paper_result_target(PaperResultTargetSpec {
            id: "sealqa_baseline_accuracy",
            dataset_id: "sealqa",
            candidate_role: "baseline",
            metric: "llm_judge_accuracy",
            tolerance: 0.0,
            value_percent: 26.6,
            source: "full_source.md SealQA results prose",
            status: PaperResultTargetStatus::Reported,
            ambiguity_group: None,
        }),
        paper_result_target(PaperResultTargetSpec {
            id: "sealqa_optimized_accuracy",
            dataset_id: "sealqa",
            candidate_role: "optimized",
            metric: "llm_judge_accuracy",
            tolerance: 0.0,
            value_percent: 38.7,
            source: "full_source.md SealQA results prose",
            status: PaperResultTargetStatus::Reported,
            ambiguity_group: None,
        }),
        paper_result_target(PaperResultTargetSpec {
            id: "browsecomp_baseline_accuracy",
            dataset_id: "browsecomp_transfer",
            candidate_role: "baseline",
            metric: "accuracy",
            tolerance: 0.0,
            value_percent: 43.5,
            source: "full_source.md zero-shot skill transfer prose",
            status: PaperResultTargetStatus::Reported,
            ambiguity_group: None,
        }),
        paper_result_target(PaperResultTargetSpec {
            id: "browsecomp_sealqa_skill_transfer_accuracy",
            dataset_id: "browsecomp_transfer",
            candidate_role: "sealqa_skill_transfer",
            metric: "accuracy",
            tolerance: 0.0,
            value_percent: 48.8,
            source: "full_source.md zero-shot skill transfer prose",
            status: PaperResultTargetStatus::Reported,
            ambiguity_group: None,
        }),
    ]
}

struct PaperResultTargetSpec<'a> {
    id: &'a str,
    dataset_id: &'a str,
    candidate_role: &'a str,
    metric: &'a str,
    tolerance: f64,
    value_percent: f64,
    source: &'a str,
    status: PaperResultTargetStatus,
    ambiguity_group: Option<&'a str>,
}

fn paper_result_target(spec: PaperResultTargetSpec<'_>) -> PaperResultTarget {
    PaperResultTarget {
        id: spec.id.to_owned(),
        dataset_id: spec.dataset_id.to_owned(),
        candidate_role: spec.candidate_role.to_owned(),
        metric: spec.metric.to_owned(),
        tolerance: spec.tolerance,
        value_percent: spec.value_percent,
        source: spec.source.to_owned(),
        status: spec.status,
        ambiguity_group: spec.ambiguity_group.map(ToOwned::to_owned),
    }
}

fn blockers(source_blockers: &[SourceBlockerReport]) -> Vec<ReplicationBlocker> {
    let mut blockers = source_blockers
        .iter()
        .map(|report| blocker(&report.blocker_id, blocker_description(&report.blocker_id)))
        .collect::<Vec<_>>();
    blockers.extend([
        blocker(
            "sealqa_judge_scored_run",
            "SealQA auto-grader template is pinned, but no approved live LLM-as-judge scored run has executed",
        ),
        blocker(
            "live_run_spend_approval",
            "bounded live agent run requires explicit provider spend and credential approval",
        ),
    ]);
    blockers
}

fn blocker_description(id: &str) -> &'static str {
    match id {
        "source_pin" => {
            "choose paper-release source, local checkout, or current upstream revision before comparing behavior"
        }
        "officeqa_category_split_manifest" => {
            "OfficeQA paper category/pseudo-label split artifact is not present locally"
        }
        "officeqa_exact_split_membership" => {
            "OfficeQA can be difficulty-stratified as a documented substitute, but exact paper split membership is absent"
        }
        "sealqa_split_manifest" => {
            "SealQA seal-0 parquet can be materialized, but Leaven still lacks exact train/held-out split membership"
        }
        "browsecomp_transfer_sample" => {
            "BrowseComp 128-example transfer sample/result source is not present locally"
        }
        _ => "unresolved paper-close replication blocker",
    }
}

fn proxy_rejections() -> Vec<String> {
    proxy_rejection_gates()
        .into_iter()
        .map(|gate| format!("{}: {}", gate.proxy, gate.why_not))
        .collect()
}

fn proxy_rejection_gates() -> Vec<ProxyRejectionGate> {
    vec![
        proxy_rejection_gate(
            "p5_one_iteration_fixture",
            "P5 one-iteration fixture completes",
            "It is product wiring evidence, not OfficeQA/SealQA paper-close replication.",
        ),
        proxy_rejection_gate(
            "git_trust_benchmark",
            "Git materialization/readback trust benchmark passes",
            "It proves substrate isolation and performance, not EvoSkill loop semantics or paper scores.",
        ),
        proxy_rejection_gate(
            "fake_runtime_loop",
            "Fake-runtime loop admits a child",
            "It is useful mechanics evidence, not live agent behavior, validation score quality, or paper-score evidence.",
        ),
        proxy_rejection_gate(
            "single_sample_inspection",
            "One OfficeQA or SealQA sample can be inspected",
            "It does not prove train/validation/test split construction, scorer distribution, or held-out reporting.",
        ),
        proxy_rejection_gate(
            "just_check_repo_health",
            "just check and topology tests pass",
            "They prove repo health only, not paper-close reproduction.",
        ),
    ]
}

fn proxy_rejection_gate(id: &str, proxy: &str, why_not: &str) -> ProxyRejectionGate {
    ProxyRejectionGate {
        id: id.to_owned(),
        status: ProxyRejectionStatus::RejectedAsCompletionEvidence,
        proxy: proxy.to_owned(),
        why_not: why_not.to_owned(),
    }
}

fn source_revision(
    root: &Path,
    id: &str,
    relative_path: &str,
    pinned: Option<(SourcePinPolicy, &SourcePinManifestEntry)>,
) -> SourceRevision {
    let path = root.join(relative_path);
    if !path.exists() {
        return source_revision_status(
            id,
            relative_path,
            SourceRevisionStatus::MissingPath,
            SourceRemoteProbeStatus::MissingPath,
            SourcePaperReleaseStatus::MissingPath,
        );
    }
    if !path.join(".git").exists() {
        return source_revision_status(
            id,
            relative_path,
            SourceRevisionStatus::NotGitCheckout,
            SourceRemoteProbeStatus::NotGitCheckout,
            SourcePaperReleaseStatus::NotGitCheckout,
        );
    }

    let Some(head) = git_stdout(&path, &["rev-parse", "HEAD"]) else {
        return source_revision_status(
            id,
            relative_path,
            SourceRevisionStatus::ProbeFailed,
            SourceRemoteProbeStatus::ProbeFailed,
            SourcePaperReleaseStatus::ProbeFailed,
        );
    };

    let branch = git_stdout(&path, &["branch", "--show-current"]);
    let remote_url = git_stdout(&path, &["config", "--get", "remote.origin.url"]);
    let remote_probe_status = if remote_url.is_some() {
        SourceRemoteProbeStatus::NotProbedNoNetworkDefault
    } else {
        SourceRemoteProbeStatus::MissingRemote
    };
    SourceRevision {
        id: id.to_owned(),
        relative_path: relative_path.to_owned(),
        head: Some(head),
        branch,
        remote_url,
        remote_head: None,
        remote_probe_status,
        paper_release_ref: pinned.map(|(policy, _)| policy.as_str().to_owned()),
        paper_release_head: pinned.map(|(_, entry)| entry.head.clone()),
        paper_release_status: if pinned.is_some() {
            SourcePaperReleaseStatus::PinnedLocalCheckout
        } else {
            SourcePaperReleaseStatus::Unresolved
        },
        status: SourceRevisionStatus::Present,
        blocker_ids: if pinned.is_some() {
            Vec::new()
        } else {
            vec!["source_pin".to_owned()]
        },
    }
}

fn source_revision_status(
    id: &str,
    relative_path: &str,
    status: SourceRevisionStatus,
    remote_probe_status: SourceRemoteProbeStatus,
    paper_release_status: SourcePaperReleaseStatus,
) -> SourceRevision {
    SourceRevision {
        id: id.to_owned(),
        relative_path: relative_path.to_owned(),
        head: None,
        branch: None,
        remote_url: None,
        remote_head: None,
        remote_probe_status,
        paper_release_ref: None,
        paper_release_head: None,
        paper_release_status,
        status,
        blocker_ids: vec!["source_pin".to_owned()],
    }
}

fn git_stdout(path: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(args)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if value.is_empty() { None } else { Some(value) }
}

fn source_artifact(
    root: &Path,
    id: &str,
    role: &str,
    relative_path: &str,
) -> Result<SourceArtifact, ManifestError> {
    let path = root.join(relative_path);
    if !path.exists() {
        return Ok(SourceArtifact {
            id: id.to_owned(),
            role: role.to_owned(),
            relative_path: relative_path.to_owned(),
            exists: false,
            bytes: None,
            sha256: None,
        });
    }
    let metadata = path.metadata().map_err(|source| ManifestError::Read {
        path: path.clone(),
        source,
    })?;
    Ok(SourceArtifact {
        id: id.to_owned(),
        role: role.to_owned(),
        relative_path: relative_path.to_owned(),
        exists: true,
        bytes: Some(metadata.len()),
        sha256: Some(sha256_file(&path)?),
    })
}

fn sha256_file(path: &Path) -> Result<String, ManifestError> {
    let mut file = File::open(path).map_err(|source| ManifestError::Read {
        path: path.to_owned(),
        source,
    })?;
    let mut hasher = Sha256::new();
    let mut buf = [0_u8; 8192];
    loop {
        let read = file.read(&mut buf).map_err(|source| ManifestError::Read {
            path: path.to_owned(),
            source,
        })?;
        if read == 0 {
            break;
        }
        hasher.update(&buf[..read]);
    }
    Ok(hex::encode(hasher.finalize()))
}

fn fingerprint_hex(fingerprint: Fingerprint) -> String {
    hex::encode(fingerprint.0)
}

fn blocker(id: &str, description: &str) -> ReplicationBlocker {
    ReplicationBlocker {
        id: id.to_owned(),
        description: description.to_owned(),
    }
}
