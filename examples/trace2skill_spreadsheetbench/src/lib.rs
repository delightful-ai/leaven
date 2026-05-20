//! `Trace2Skill` `SpreadsheetBench` manifest lowering.

mod patch_bridge;
mod patch_replay;

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use leaven_eval::{
    Case, Dataset, DatasetSplitManifest, RowOrderSplitBuilder, SplitRole, SplitUsePolicy,
};
use leaven_evidence::{
    AgentAnalystCallEvidence, AgentAnalystCallEvidenceInput, AgentAnalystCallStatus,
    AgentAnalystFanoutEvidence, AgentAnalystRole, AgentTrajectoryAnalysisKind,
    AgentTrajectoryAnalysisRecord, AgentTrajectoryCorpusEvidence, AgentTrajectoryEvidence,
    AgentTrajectoryEvidenceInput, AgentTrajectoryOutcome, CommandEvidence, OutputRecord,
};
use leaven_kernel::{
    AgentSessionId, BlobRef, CaseId, Fingerprint, FingerprintBuilder, MetadataKey, MetadataValue,
};
use serde::Deserialize;

pub use patch_bridge::{
    Trace2SkillPatchError, Trace2SkillPatchLowering, Trace2SkillPatchLoweringInput,
    apply_trace2skill_json_patch, lower_trace2skill_json_patch,
};
pub use patch_replay::{
    Trace2SkillJsonPatchArtifact, Trace2SkillJsonPatchMergeBatch, Trace2SkillJsonPatchMergeInput,
    Trace2SkillJsonPatchMergeLevel, Trace2SkillJsonPatchReplay, Trace2SkillJsonPatchReplayInput,
    Trace2SkillPatchReplayError, Trace2SkillSavedJsonPatchReplayInput,
    replay_trace2skill_json_patch_merge, replay_trace2skill_saved_json_patch_outputs,
};

const VERIFIED_400_VERSION: &str = "trace2skill-spreadsheetbench-verified-400-v1";
const VERIFIED_400_ROWS: usize = 400;
const VERIFIED_400_EVOLVING_ROWS: usize = 200;

/// Lowered `Trace2Skill` `SpreadsheetBench` manifest.
#[derive(Clone, Debug)]
pub struct Trace2SkillSpreadsheetBenchManifest {
    /// `SpreadsheetBench` rows as Leaven evaluation cases.
    pub dataset: Dataset<Case<SpreadsheetBenchTask, SpreadsheetBenchAnswerSpec>>,
    /// Paper split manifest: first 200 rows train/evolving, last 200 rows held-out test.
    pub split_manifest: DatasetSplitManifest,
}

/// `SpreadsheetBench` task input fields used to run the upstream agent.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SpreadsheetBenchTask {
    /// Natural-language spreadsheet instruction.
    pub instruction: String,
    /// Upstream spreadsheet directory path, relative to the dataset root.
    pub spreadsheet_path: String,
    /// Upstream instruction taxonomy.
    pub instruction_type: String,
    /// Optional source data range supplied by the upstream manifest.
    pub data_position: Option<String>,
}

/// `SpreadsheetBench` expected answer location.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SpreadsheetBenchAnswerSpec {
    /// Expected output range.
    pub answer_position: String,
    /// Optional sheet for the expected output range.
    pub answer_sheet: Option<String>,
}

/// Loads the official 400-row `Trace2Skill` `SpreadsheetBench`-Verified manifest.
pub fn load_verified_400_manifest(
    path: &Path,
) -> Result<Trace2SkillSpreadsheetBenchManifest, Trace2SkillManifestError> {
    let bytes = fs::read(path)?;
    let rows: Vec<SpreadsheetBenchRow> = serde_json::from_slice(&bytes)?;
    if rows.len() != VERIFIED_400_ROWS {
        return Err(Trace2SkillManifestError::UnexpectedRowCount {
            expected: VERIFIED_400_ROWS,
            actual: rows.len(),
        });
    }

    let cases = rows
        .into_iter()
        .enumerate()
        .map(|(row_index, row)| {
            let source_id = row.id.to_source_id();
            Case::from_source_row(
                row_index,
                source_id,
                SpreadsheetBenchTask {
                    instruction: row.instruction,
                    spreadsheet_path: row.spreadsheet_path,
                    instruction_type: row.instruction_type,
                    data_position: row.data_position,
                },
                Some(SpreadsheetBenchAnswerSpec {
                    answer_position: row.answer_position,
                    answer_sheet: row.answer_sheet,
                }),
            )
        })
        .collect::<Vec<_>>();

    let ordered_cases = cases.iter().map(|case| case.id).collect::<Vec<_>>();
    let dataset = Dataset::from_cases(cases)?;
    let splits = RowOrderSplitBuilder::new(ordered_cases)
        .role_range(SplitRole::Train, 0..VERIFIED_400_EVOLVING_ROWS)
        .role_range(
            SplitRole::Test,
            VERIFIED_400_EVOLVING_ROWS..VERIFIED_400_ROWS,
        )
        .build(leaven_core::CaseSetVersion(VERIFIED_400_VERSION.to_owned()))?;
    let split_manifest = DatasetSplitManifest::new(
        splits,
        [SplitRole::Train, SplitRole::Test],
        SplitUsePolicy::gepa_train_val_test(),
    )?;

    Ok(Trace2SkillSpreadsheetBenchManifest {
        dataset,
        split_manifest,
    })
}

/// Upstream run artifacts used to build a Leaven trajectory corpus.
#[derive(Clone, Copy)]
pub struct Trace2SkillRunArtifactInput<'a> {
    /// Upstream `results.json` path.
    pub results_file: &'a Path,
    /// Directory containing upstream chat-history logs.
    pub log_dir: Option<&'a Path>,
    /// Upstream log format, usually `markdown` or `jsonl`.
    pub log_format: &'a str,
    /// Directory containing upstream parsed/analysis reports.
    pub analysis_dir: Option<&'a Path>,
}

/// Builds the 200-row training/evolving trajectory corpus from upstream artifacts.
pub fn build_training_corpus_from_run_artifacts(
    manifest: &Trace2SkillSpreadsheetBenchManifest,
    input: Trace2SkillRunArtifactInput<'_>,
) -> Result<AgentTrajectoryCorpusEvidence, Trace2SkillManifestError> {
    let train_cases = manifest
        .split_manifest
        .cases_for_role(&SplitRole::Train)
        .ok_or(Trace2SkillManifestError::MissingTrainingSplit)?;
    let case_sources = source_id_by_case(manifest)?;
    let case_by_source = case_sources
        .iter()
        .map(|(case, source_id)| (source_id.clone(), *case))
        .collect::<BTreeMap<_, _>>();
    let mut corpus = AgentTrajectoryCorpusEvidence::new(
        train_cases
            .iter()
            .map(|case| {
                case_sources
                    .get(case)
                    .cloned()
                    .ok_or(Trace2SkillManifestError::MissingSourceId { case: *case })
            })
            .collect::<Result<Vec<_>, _>>()?,
    )?;

    let bytes = fs::read(input.results_file)?;
    let run: UpstreamResultsFile = serde_json::from_slice(&bytes)?;
    let fingerprint = model_config_fingerprint(&run, input.log_format);
    for result in run.results {
        let task_id = result.id.to_source_id();
        let Some(case_id) = case_by_source.get(&task_id).copied() else {
            continue;
        };
        if !train_cases.contains(&case_id) {
            continue;
        }
        let trajectory = AgentTrajectoryEvidence::new(AgentTrajectoryEvidenceInput {
            session_id: AgentSessionId::new(),
            case_id: Some(case_id),
            task_id: task_id.clone(),
            outcome: result.outcome(),
            model_id: run.model.clone(),
            model_config_fingerprint: fingerprint,
            transcript: artifact_record(
                input.log_dir,
                log_filename(&run.agent_name, &task_id, input.log_format),
            )?,
            commands: CommandEvidence::new(Vec::new()),
        })
        .with_analysis_records(analysis_records(input.analysis_dir, &task_id));
        corpus.push(trajectory)?;
    }

    Ok(corpus)
}

/// Builds the Stage 2 analyst-call manifest from imported training trajectories.
pub fn build_stage2_analyst_fanout_from_training_corpus(
    corpus: &AgentTrajectoryCorpusEvidence,
) -> Result<AgentAnalystFanoutEvidence, Trace2SkillManifestError> {
    let call_ids = corpus
        .trajectories()
        .iter()
        .enumerate()
        .map(|(index, trajectory)| analyst_call_id(trajectory, index))
        .collect::<Vec<_>>();
    let mut fanout = AgentAnalystFanoutEvidence::new(call_ids)?;
    for (index, trajectory) in corpus.trajectories().iter().enumerate() {
        let role = analyst_role(trajectory.outcome());
        let task_id = trajectory.task_id().to_owned();
        fanout.push(AgentAnalystCallEvidence::new(
            AgentAnalystCallEvidenceInput {
                call_id: analyst_call_id(trajectory, index),
                role,
                source_task_ids: vec![task_id.clone()],
                prompt: OutputRecord::inline(format!(
                    "Trace2Skill Stage 2 analyst prompt scaffold for task {task_id}"
                )),
                response: None,
                status: AgentAnalystCallStatus::Pending,
                retry_count: 0,
                support_count: 1,
            },
        )?)?;
    }
    Ok(fanout)
}

fn analyst_call_id(trajectory: &AgentTrajectoryEvidence, index: usize) -> String {
    let prefix = match trajectory.outcome() {
        AgentTrajectoryOutcome::Success => "success",
        AgentTrajectoryOutcome::Failure { .. } => "error",
    };
    format!("{prefix}-{}-{}", trajectory.task_id(), index + 1)
}

fn analyst_role(outcome: &AgentTrajectoryOutcome) -> AgentAnalystRole {
    match outcome {
        AgentTrajectoryOutcome::Success => AgentAnalystRole::Success,
        AgentTrajectoryOutcome::Failure { .. } => AgentAnalystRole::Error,
    }
}

fn source_id_by_case(
    manifest: &Trace2SkillSpreadsheetBenchManifest,
) -> Result<BTreeMap<CaseId, String>, Trace2SkillManifestError> {
    let mut by_case = BTreeMap::new();
    for (case_id, case) in manifest.dataset.cases() {
        let source_id = case
            .metadata
            .get(&MetadataKey::from("source_id"))
            .ok_or(Trace2SkillManifestError::MissingSourceId { case: *case_id })?;
        let MetadataValue::String(source_id) = source_id else {
            return Err(Trace2SkillManifestError::InvalidSourceId { case: *case_id });
        };
        by_case.insert(*case_id, source_id.clone());
    }
    Ok(by_case)
}

fn artifact_record(
    root: Option<&Path>,
    filename: String,
) -> Result<OutputRecord, Trace2SkillManifestError> {
    let Some(root) = root else {
        return Ok(OutputRecord::inline(""));
    };
    let path = root.join(filename);
    if !path.exists() {
        return Err(Trace2SkillManifestError::MissingArtifact { path });
    }
    Ok(OutputRecord::blob(BlobRef {
        store: "trace2skill-run-artifacts".to_owned(),
        key: path.display().to_string(),
    }))
}

fn analysis_records(root: Option<&Path>, task_id: &str) -> Vec<AgentTrajectoryAnalysisRecord> {
    let Some(root) = root else {
        return Vec::new();
    };
    let error_path = root.join(format!("error_analysis_{task_id}.md"));
    if error_path.exists() {
        return vec![AgentTrajectoryAnalysisRecord::new(
            AgentTrajectoryAnalysisKind::Error,
            error_path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("error_analysis.md"),
            OutputRecord::blob(BlobRef {
                store: "trace2skill-run-artifacts".to_owned(),
                key: error_path.display().to_string(),
            }),
        )];
    }
    let success_path = root.join(format!("success_analysis_{task_id}.md"));
    if success_path.exists() {
        return vec![AgentTrajectoryAnalysisRecord::new(
            AgentTrajectoryAnalysisKind::Success,
            success_path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("success_analysis.md"),
            OutputRecord::blob(BlobRef {
                store: "trace2skill-run-artifacts".to_owned(),
                key: success_path.display().to_string(),
            }),
        )];
    }
    Vec::new()
}

fn log_filename(agent_name: &str, task_id: &str, format: &str) -> String {
    let extension = if format == "jsonl" { "jsonl" } else { "md" };
    format!("{agent_name}_{task_id}.{extension}")
}

fn model_config_fingerprint(run: &UpstreamResultsFile, log_format: &str) -> Fingerprint {
    let mut builder = FingerprintBuilder::new();
    builder
        .update("trace2skill-spreadsheetbench")
        .update(&run.agent_name)
        .update(&run.model)
        .update(run.seed.map_or_else(String::new, |seed| seed.to_string()))
        .update(log_format);
    builder.finish()
}

#[derive(Debug, thiserror::Error)]
pub enum Trace2SkillManifestError {
    /// Manifest file could not be read.
    #[error("failed to read Trace2Skill manifest: {0}")]
    Read(#[from] std::io::Error),
    /// Manifest JSON could not be parsed.
    #[error("failed to parse Trace2Skill manifest JSON: {0}")]
    Json(#[from] serde_json::Error),
    /// Manifest row count does not match the official 400-row release.
    #[error("expected {expected} Trace2Skill rows, found {actual}")]
    UnexpectedRowCount {
        /// Expected row count.
        expected: usize,
        /// Actual row count.
        actual: usize,
    },
    /// Leaven dataset construction failed.
    #[error(transparent)]
    Dataset(#[from] leaven_eval::DatasetError),
    /// Leaven split construction failed.
    #[error(transparent)]
    Splits(#[from] leaven_eval::DatasetSplitsError),
    /// Training split is absent from the lowered manifest.
    #[error("Trace2Skill training split is missing")]
    MissingTrainingSplit,
    /// A lowered case lacks source-id metadata.
    #[error("case {case} is missing Trace2Skill source_id metadata")]
    MissingSourceId {
        /// Case missing source id.
        case: CaseId,
    },
    /// A lowered case has non-string source-id metadata.
    #[error("case {case} has non-string Trace2Skill source_id metadata")]
    InvalidSourceId {
        /// Case with invalid source id.
        case: CaseId,
    },
    /// A caller supplied an artifact directory but the expected file was absent.
    #[error("Trace2Skill artifact is missing: {path}")]
    MissingArtifact {
        /// Expected artifact path.
        path: PathBuf,
    },
    /// Trajectory corpus refused an upstream result.
    #[error(transparent)]
    Corpus(#[from] leaven_evidence::AgentTrajectoryCorpusError),
    /// Analyst call evidence refused invalid input.
    #[error(transparent)]
    AnalystCall(#[from] leaven_evidence::AgentAnalystCallError),
    /// Analyst fan-out evidence refused invalid input.
    #[error(transparent)]
    AnalystFanout(#[from] leaven_evidence::AgentAnalystFanoutError),
}

#[derive(Debug, Deserialize)]
struct SpreadsheetBenchRow {
    id: SpreadsheetBenchSourceId,
    instruction: String,
    spreadsheet_path: String,
    instruction_type: String,
    answer_position: String,
    answer_sheet: Option<String>,
    data_position: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum SpreadsheetBenchSourceId {
    String(String),
    Number(u64),
}

impl SpreadsheetBenchSourceId {
    fn to_source_id(&self) -> String {
        match self {
            Self::String(value) => value.clone(),
            Self::Number(value) => value.to_string(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct UpstreamResultsFile {
    agent_name: String,
    model: String,
    seed: Option<u64>,
    results: Vec<UpstreamResult>,
}

#[derive(Debug, Deserialize)]
struct UpstreamResult {
    id: SpreadsheetBenchSourceId,
    success: bool,
    error: Option<String>,
    #[serde(default)]
    test_cases: Vec<UpstreamTestCaseResult>,
}

impl UpstreamResult {
    fn outcome(&self) -> AgentTrajectoryOutcome {
        if self.success {
            AgentTrajectoryOutcome::Success
        } else {
            AgentTrajectoryOutcome::Failure {
                reason: self.failure_reason(),
            }
        }
    }

    fn failure_reason(&self) -> String {
        self.error
            .as_ref()
            .filter(|reason| !reason.is_empty())
            .cloned()
            .or_else(|| {
                self.test_cases
                    .iter()
                    .find_map(|test_case| test_case.error.clone())
            })
            .unwrap_or_else(|| "upstream result marked unsuccessful".to_owned())
    }
}

#[derive(Debug, Deserialize)]
struct UpstreamTestCaseResult {
    error: Option<String>,
}
