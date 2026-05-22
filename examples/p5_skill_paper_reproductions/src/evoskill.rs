use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::Read;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::process::Command;

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
    CategoryRoundRobinSampler, SourceRow, SourceRowManifest, SplitRole, StratifiedSplitBuilder,
};
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
const EVOSKILL_REPLICA_FRONTIER_SIZE: usize = 3;
const EVOSKILL_REPLICA_TRAIN_ROWS: usize = 12;
const EVOSKILL_REPLICA_RESUME_AFTER_ITERATION: u64 = 2;
const REPLICA_CHILD_SCORES: [f64; 4] = [0.60, 0.20, 0.10, 0.80];

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
    pub datasets: Vec<DatasetRequirement>,
    pub source_materializations: Vec<DatasetMaterializationReport>,
    pub scorer: ScorerManifest,
    pub frontier: FrontierManifest,
    pub schedule: ScheduleManifest,
    pub model_pins: Vec<ModelPin>,
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
    pub status: SourceRevisionStatus,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceRevisionStatus {
    MissingPath,
    NotGitCheckout,
    Present,
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
#[serde(rename_all = "snake_case")]
pub enum SplitManifestStatus {
    ExactPublished,
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
    pub blocker_ids: Vec<String>,
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

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ScorerManifest {
    pub id: String,
    pub tolerances: Vec<f64>,
    pub failure_threshold: f64,
    pub implementation_status: String,
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

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ReplicationBlocker {
    pub id: String,
    pub description: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct EvoSkillFinalReport {
    pub schema_version: u32,
    pub exactness: ExactnessClass,
    pub manifest: EvoSkillReplicaManifest,
    pub loop_report: Option<EvoSkillReplicaLoopReport>,
    pub scorer_fingerprint: ScorerFingerprintReport,
    pub score_slots: Vec<FinalScoreSlot>,
    pub cost: FinalReportCost,
    pub errors: Vec<FinalReportError>,
    pub ablations: Vec<AblationStatusReport>,
    pub proxy_rejections: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ScorerFingerprintReport {
    pub scorer_id: String,
    pub fingerprint: String,
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
    pub candidate_role: String,
    pub expected_rows: Option<u64>,
    pub score: Option<f64>,
    pub status: FinalScoreStatus,
    pub blocker_ids: Vec<String>,
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
    pub frontier_capacity: u64,
    pub iterations: Vec<EvoSkillReplicaIterationReport>,
    pub feedback_history_rows: u64,
    pub final_frontier_members: Vec<CandidateId>,
    pub final_best_score: Option<f64>,
    pub checkpoint_resume: EvoSkillReplicaCheckpointResumeReport,
    pub proxy_rejection: String,
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
    pub child_score: f64,
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
    score: f64,
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
    #[error("EvoSkill replica loop is blocked: {reason}")]
    LoopBlocked { reason: String },
}

pub fn build_evoskill_replica_manifest(
    input: &ManifestBuildInput,
) -> Result<EvoSkillReplicaManifest, ManifestError> {
    Ok(EvoSkillReplicaManifest {
        schema_version: 2,
        paper: PaperTarget {
            id: "evoskill".to_owned(),
            arxiv_id: "2603.02766".to_owned(),
            title: "EvoSkill".to_owned(),
        },
        exactness: ExactnessClass::BlockedBeforePaperClose,
        source_revisions: source_revisions(&input.root),
        artifacts: source_artifacts(&input.root)?,
        datasets: dataset_requirements(),
        source_materializations: source_materializations(input)?,
        scorer: scorer_manifest(),
        frontier: frontier_manifest(),
        schedule: schedule_manifest(),
        model_pins: model_pins(),
        blockers: blockers(),
        proxy_rejections: proxy_rejections(),
    })
}

pub fn build_evoskill_final_report(
    input: &ManifestBuildInput,
) -> Result<EvoSkillFinalReport, ManifestError> {
    let manifest = build_evoskill_replica_manifest(input)?;
    let loop_report = if has_materialized_officeqa(&manifest) {
        Some(run_evoskill_replica_mechanics(input)?)
    } else {
        None
    };
    let scorer_fingerprint = scorer_fingerprint_report(&manifest.scorer);
    let score_slots = final_score_slots(&manifest);
    let errors = final_report_errors(&manifest);
    let ablations = final_report_ablations(&manifest);
    let exactness = manifest.exactness.clone();
    let proxy_rejections = manifest.proxy_rejections.clone();

    Ok(EvoSkillFinalReport {
        schema_version: 1,
        exactness,
        manifest,
        loop_report,
        scorer_fingerprint,
        score_slots,
        cost: FinalReportCost::default(),
        errors,
        ablations,
        proxy_rejections,
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
    let split_materializations = officeqa_difficulty_split_reports(rows.len(), &strata)?;
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
        blocker_ids: vec![
            "officeqa_category_split_manifest".to_owned(),
            "officeqa_exact_split_membership".to_owned(),
        ],
    };
    Ok(Some(OfficeQaSourceMaterialization { rows, report }))
}

fn source_materializations(
    input: &ManifestBuildInput,
) -> Result<Vec<DatasetMaterializationReport>, ManifestError> {
    let officeqa = materialize_officeqa_source(input)?.map_or_else(
        || {
            missing_materialization_report(
                "officeqa",
                "officeqa_full_csv",
                "officeqa_full_csv_missing",
            )
        },
        |materialization| materialization.report,
    );
    Ok(vec![officeqa, sealqa_materialization_report(&input.root)?])
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
    source_rows: usize,
    strata: &BTreeMap<SmolStr, Vec<CaseId>>,
) -> Result<Vec<SplitMaterializationReport>, ManifestError> {
    let mut reports = Vec::new();
    for train_rows in OFFICEQA_TRAIN_SIZES {
        reports.push(officeqa_difficulty_split_report(
            source_rows,
            strata,
            train_rows,
        )?);
    }
    Ok(reports)
}

fn officeqa_difficulty_split_report(
    source_rows: usize,
    strata: &BTreeMap<SmolStr, Vec<CaseId>>,
    train_rows: usize,
) -> Result<SplitMaterializationReport, ManifestError> {
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
            blocker_ids: vec!["officeqa_insufficient_rows".to_owned()],
        });
    }

    let splits = officeqa_difficulty_splits(source_rows, strata, train_rows)?;

    Ok(SplitMaterializationReport {
        id,
        method: "difficulty_stratified_exact_count".to_owned(),
        exactness: MaterializationExactness::PaperCloseSubstitute,
        train_rows: split_len(&splits, &SplitRole::Train),
        validation_rows: Some(split_len(&splits, &SplitRole::Validation)),
        test_rows: Some(split_len(&splits, &SplitRole::Test)),
        split_fingerprint: Some(fingerprint_hex(splits.fingerprint())),
        blocker_ids: vec![
            "officeqa_category_split_manifest".to_owned(),
            "officeqa_exact_split_membership".to_owned(),
        ],
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

fn strata_reports(strata: &BTreeMap<SmolStr, Vec<CaseId>>) -> Vec<StratumMaterializationReport> {
    strata
        .iter()
        .map(|(name, rows)| StratumMaterializationReport {
            name: name.to_string(),
            rows: u64::try_from(rows.len()).expect("stratum row count fits in u64"),
        })
        .collect()
}

fn sealqa_materialization_report(
    root: &Path,
) -> Result<DatasetMaterializationReport, ManifestError> {
    let path = root.join("tmp/replication/evoskill/sealqa/seal-0.parquet");
    let source_artifact_sha256 = if path.exists() {
        Some(sha256_file(&path)?)
    } else {
        None
    };
    Ok(DatasetMaterializationReport {
        dataset_id: "sealqa".to_owned(),
        source_artifact_id: "sealqa_parquet".to_owned(),
        source_status: if path.exists() {
            SourceMaterializationStatus::BlockedMissingRowReader
        } else {
            SourceMaterializationStatus::MissingArtifact
        },
        source_rows: None,
        case_rows: None,
        source_row_fingerprint: None,
        source_artifact_sha256,
        target_policy: "blocked: no Leaven-owned seal-0 parquet row reader yet".to_owned(),
        strata: Vec::new(),
        split_materializations: Vec::new(),
        blocker_ids: vec![
            "sealqa_parquet_row_reader".to_owned(),
            "sealqa_split_manifest".to_owned(),
        ],
    })
}

pub fn run_evoskill_replica_mechanics(
    input: &ManifestBuildInput,
) -> Result<EvoSkillReplicaLoopReport, ManifestError> {
    let officeqa =
        materialize_officeqa_source(input)?.ok_or_else(|| ManifestError::LoopBlocked {
            reason: "OfficeQA full CSV is required for the no-spend replica loop".to_owned(),
        })?;
    let git = create_agentic_git_harness()?;
    let mut state = initial_replica_loop_state(&officeqa, &git)?;
    let mut iterations = Vec::new();
    let mut checkpoint_resume = None;

    for (index, child_score) in REPLICA_CHILD_SCORES.into_iter().enumerate() {
        let iteration = u64::try_from(index + 1).expect("iteration index fits in u64");
        let report = run_replica_iteration(&mut state, &officeqa, &git, iteration, child_score)?;
        iterations.push(report);
        if iteration == EVOSKILL_REPLICA_RESUME_AFTER_ITERATION {
            checkpoint_resume = Some(round_trip_replica_checkpoint(&mut state, iteration)?);
        }
    }

    Ok(EvoSkillReplicaLoopReport {
        exactness: MaterializationExactness::PaperCloseSubstitute,
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

fn has_materialized_officeqa(manifest: &EvoSkillReplicaManifest) -> bool {
    manifest
        .source_materializations
        .iter()
        .any(|materialization| {
            materialization.dataset_id == "officeqa"
                && materialization.source_status == SourceMaterializationStatus::Materialized
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
    ScorerFingerprintReport {
        scorer_id: scorer.id.clone(),
        fingerprint: hex::encode(hasher.finalize()),
    }
}

fn final_score_slots(manifest: &EvoSkillReplicaManifest) -> Vec<FinalScoreSlot> {
    let mut slots = Vec::new();
    for materialization in &manifest.source_materializations {
        if materialization.split_materializations.is_empty() {
            slots.extend(dataset_placeholder_score_slots(manifest, materialization));
            continue;
        }
        for split in &materialization.split_materializations {
            slots.extend(split_score_slots(materialization, split));
        }
    }
    slots
}

fn split_score_slots(
    materialization: &DatasetMaterializationReport,
    split: &SplitMaterializationReport,
) -> Vec<FinalScoreSlot> {
    let roles = [
        ("train", Some(split.train_rows)),
        ("validation", split.validation_rows),
        ("held_out_test", split.test_rows),
    ];
    roles
        .into_iter()
        .filter_map(|(role, expected_rows)| {
            expected_rows.map(|rows| {
                score_slots_for_role(
                    &materialization.dataset_id,
                    &split.id,
                    role,
                    Some(rows),
                    &split.blocker_ids,
                )
            })
        })
        .flatten()
        .collect()
}

fn dataset_placeholder_score_slots(
    manifest: &EvoSkillReplicaManifest,
    materialization: &DatasetMaterializationReport,
) -> Vec<FinalScoreSlot> {
    let dataset = manifest
        .datasets
        .iter()
        .find(|dataset| dataset.id == materialization.dataset_id);
    let split_id = format!("{}_paper_split_unmaterialized", materialization.dataset_id);
    let mut roles = Vec::new();
    if let Some(dataset) = dataset {
        let train_rows = dataset.train_sizes.first().copied();
        roles.push(("train", train_rows));
        if dataset.validation_rows.is_some() {
            roles.push(("validation", dataset.validation_rows));
        }
        roles.push(("held_out_test", held_out_rows(dataset)));
    } else {
        roles.push(("train", None));
        roles.push(("validation", None));
        roles.push(("held_out_test", None));
    }
    roles
        .into_iter()
        .flat_map(|(role, expected_rows)| {
            score_slots_for_role(
                &materialization.dataset_id,
                &split_id,
                role,
                expected_rows,
                &materialization.blocker_ids,
            )
        })
        .collect()
}

fn held_out_rows(dataset: &DatasetRequirement) -> Option<u64> {
    let paper_rows = dataset.paper_rows?;
    let train_rows = dataset.train_sizes.first().copied().unwrap_or_default();
    let validation_rows = dataset.validation_rows.unwrap_or_default();
    paper_rows.checked_sub(train_rows + validation_rows)
}

fn score_slots_for_role(
    dataset_id: &str,
    split_id: &str,
    split_role: &str,
    expected_rows: Option<u64>,
    blocker_ids: &[String],
) -> Vec<FinalScoreSlot> {
    ["baseline", "optimized"]
        .into_iter()
        .map(|candidate_role| FinalScoreSlot {
            dataset_id: dataset_id.to_owned(),
            split_id: split_id.to_owned(),
            split_role: split_role.to_owned(),
            candidate_role: candidate_role.to_owned(),
            expected_rows,
            score: None,
            status: if blocker_ids.is_empty() {
                FinalScoreStatus::NotRun
            } else {
                FinalScoreStatus::Blocked
            },
            blocker_ids: blocker_ids.to_vec(),
        })
        .collect()
}

fn final_report_errors(manifest: &EvoSkillReplicaManifest) -> Vec<FinalReportError> {
    manifest
        .blockers
        .iter()
        .map(|blocker| FinalReportError {
            blocker_id: blocker.id.clone(),
            message: blocker.description.clone(),
        })
        .collect()
}

fn final_report_ablations(manifest: &EvoSkillReplicaManifest) -> Vec<AblationStatusReport> {
    let all_blockers = manifest
        .blockers
        .iter()
        .map(|blocker| blocker.id.clone())
        .collect::<Vec<_>>();
    vec![
        AblationStatusReport {
            id: "skill_only".to_owned(),
            status: "blocked".to_owned(),
            blocker_ids: all_blockers.clone(),
            note: "requires paper split/source denominator and live scored run".to_owned(),
        },
        AblationStatusReport {
            id: "prompt_only".to_owned(),
            status: "blocked".to_owned(),
            blocker_ids: all_blockers,
            note: "requires paper split/source denominator and live scored run".to_owned(),
        },
        AblationStatusReport {
            id: "skill_merge".to_owned(),
            status: "blocked".to_owned(),
            blocker_ids: vec![
                "officeqa_exact_split_membership".to_owned(),
                "officeqa_reported_result_target".to_owned(),
            ],
            note: "OfficeQA skill-merge reporting target is unresolved before paper-close"
                .to_owned(),
        },
        AblationStatusReport {
            id: "browsecomp_transfer".to_owned(),
            status: "blocked".to_owned(),
            blocker_ids: vec!["browsecomp_transfer_sample".to_owned()],
            note: "BrowseComp transfer sample/result source is absent locally".to_owned(),
        },
    ]
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
            score: 0.30,
        }],
        feedback_history: Vec::new(),
    })
}

fn run_replica_iteration(
    state: &mut EvoSkillReplicaLoopState,
    officeqa: &OfficeQaSourceMaterialization,
    git: &EvoSkillAgenticGitHarness,
    iteration: u64,
    child_score: f64,
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
    observe_replica_frontier(&mut state.frontier, child, child_score)?;
    let admitted = state.frontier.contains(child);
    state.candidates.push(EvoSkillReplicaCandidateState {
        id: child,
        program: child_program,
        score: child_score,
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
        child_score,
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
    let strata = officeqa_difficulty_strata(&officeqa.rows);
    let splits =
        officeqa_difficulty_splits(officeqa.rows.len(), &strata, EVOSKILL_REPLICA_TRAIN_ROWS)?;
    let train_cases = splits
        .cases(&SplitRole::Train.partition_id())
        .ok_or_else(|| ManifestError::LoopBlocked {
            reason: "difficulty substitute did not produce a train split".to_owned(),
        })?;
    let mut pools = BTreeMap::<SmolStr, Vec<CaseId>>::new();
    for case in train_cases {
        let row = officeqa_row(officeqa, *case)?;
        pools
            .entry(SmolStr::new(row.input().difficulty.as_str()))
            .or_default()
            .push(*case);
    }
    CategoryRoundRobinSampler::new(
        pools,
        NonZeroUsize::new(2).expect("replica categories per batch is non-zero"),
        NonZeroUsize::new(1).expect("replica samples per category is non-zero"),
    )
    .map_err(|source| ManifestError::Sampler { source })
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

fn source_revisions(root: &Path) -> Vec<SourceRevision> {
    vec![
        source_revision(root, "evoskill_repo", "tmp/repros/evoskill"),
        source_revision(root, "officeqa_repo", "tmp/repros/officeqa"),
    ]
}

fn source_artifacts(root: &Path) -> Result<Vec<SourceArtifact>, ManifestError> {
    Ok(vec![
        source_artifact(
            root,
            "paper_full_source",
            "paper source text",
            "tmp/skill_opt_sources/arx_2603.02766/full_source.md",
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
            "sealqa_parquet",
            "SealQA seal-0 parquet",
            "tmp/replication/evoskill/sealqa/seal-0.parquet",
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
    ])
}

fn dataset_requirements() -> Vec<DatasetRequirement> {
    vec![
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
                "officeqa_reported_result_target".to_owned(),
            ],
        },
        DatasetRequirement {
            id: "sealqa".to_owned(),
            paper_rows: Some(111),
            train_sizes: vec![11],
            validation_rows: None,
            held_out: "paper uses 10 percent train and held-out remainder".to_owned(),
            split_status: SplitManifestStatus::BlockedMissingSplitManifest,
            blocker_ids: vec![
                "sealqa_parquet_row_reader".to_owned(),
                "sealqa_split_manifest".to_owned(),
            ],
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
    ]
}

fn scorer_manifest() -> ScorerManifest {
    ScorerManifest {
        id: "evoskill-multi-tolerance-v1".to_owned(),
        tolerances: vec![0.0, 0.01, 0.025, 0.05, 0.10],
        failure_threshold: 0.8,
        implementation_status: "Rust scorer law-tested for multi-tolerance weighting, units, years, text, lists, failure feedback rows, and failure extraction".to_owned(),
    }
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

fn blockers() -> Vec<ReplicationBlocker> {
    vec![
        blocker(
            "source_pin",
            "choose paper-release source, local checkout, or current upstream revision before comparing behavior",
        ),
        blocker(
            "officeqa_category_split_manifest",
            "OfficeQA paper category/pseudo-label split artifact is not present locally",
        ),
        blocker(
            "officeqa_exact_split_membership",
            "OfficeQA can be difficulty-stratified as a documented substitute, but exact paper split membership is absent",
        ),
        blocker(
            "sealqa_parquet_row_reader",
            "SealQA parquet is pinned locally, but Leaven does not yet have a no-spend Rust row reader for seal-0",
        ),
        blocker(
            "sealqa_split_manifest",
            "SealQA seal-0 parquet is pinned, but Leaven still lacks exact train/held-out split membership",
        ),
        blocker(
            "browsecomp_transfer_sample",
            "BrowseComp 128-example transfer sample/result source is not present locally",
        ),
        blocker(
            "officeqa_reported_result_target",
            "OfficeQA prose and table report disagree between 67.9 and 68.1 percent",
        ),
    ]
}

fn proxy_rejections() -> Vec<String> {
    vec![
        "P5 one-iteration fixture is product wiring evidence, not paper-close completion".to_owned(),
        "Git trust benchmark proves substrate isolation/performance, not EvoSkill paper semantics"
            .to_owned(),
        "Fake runtime child admission is mechanics evidence, not live paper behavior".to_owned(),
        "Single OfficeQA/SealQA sample inspection does not prove split construction or held-out reporting".to_owned(),
        "just check/topology proves repo health only".to_owned(),
    ]
}

fn source_revision(root: &Path, id: &str, relative_path: &str) -> SourceRevision {
    let path = root.join(relative_path);
    let (head, status) = if !path.exists() {
        (None, SourceRevisionStatus::MissingPath)
    } else if !path.join(".git").exists() {
        (None, SourceRevisionStatus::NotGitCheckout)
    } else {
        match Command::new("git")
            .arg("-C")
            .arg(&path)
            .arg("rev-parse")
            .arg("HEAD")
            .output()
        {
            Ok(output) if output.status.success() => (
                Some(String::from_utf8_lossy(&output.stdout).trim().to_owned()),
                SourceRevisionStatus::Present,
            ),
            _ => (None, SourceRevisionStatus::ProbeFailed),
        }
    };
    SourceRevision {
        id: id.to_owned(),
        relative_path: relative_path.to_owned(),
        head,
        status,
    }
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
