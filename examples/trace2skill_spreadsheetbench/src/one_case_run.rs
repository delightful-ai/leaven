use std::{
    fs,
    path::{Path, PathBuf},
};

use leaven_evidence::{
    AgentAnalystCallEvidence, AgentAnalystCallEvidenceInput, AgentAnalystCallStatus,
    AgentAnalystFanoutEvidence, AgentTrajectoryAnalysisKind, AgentTrajectoryAnalysisRecord,
    AgentTrajectoryEvidence, AgentTrajectoryEvidenceInput, AgentTrajectoryOutcome, CommandEvidence,
    OutputRecord,
};
use leaven_kernel::{AgentSessionId, BlobRef, FingerprintBuilder};

use crate::{
    compare_trace2skill_one_case_answer, file_artifact, inspect_trace2skill_one_case,
    stage2_prompt, Trace2SkillFileArtifact, Trace2SkillManifestError,
    Trace2SkillOneCaseAnswerReport, Trace2SkillOneCaseComparisonInput, Trace2SkillOneCaseInput,
    Trace2SkillOneCaseInspection,
};

/// Files needed to prepare a durable one-case `Trace2Skill` run directory.
#[derive(Clone, Copy, Debug)]
pub struct Trace2SkillOneCaseRunInput<'a> {
    /// Exact materialized case inputs.
    pub case: Trace2SkillOneCaseInput<'a>,
    /// Directory where run inputs, manifest, logs, and eventual output live.
    pub run_dir: &'a Path,
    /// Optional candidate workbook to score while preparing the run.
    pub output_workbook: Option<&'a Path>,
}

/// No-spend run preparation status.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Trace2SkillOneCaseRunStatus {
    /// The run directory is staged, but Leaven has not executed a live spreadsheet agent.
    BlockedMissingLiveSpreadsheetAgent,
    /// A caller supplied an output workbook and the report includes a score.
    ScoredCandidateWorkbook,
}

/// Source artifacts used to prepare a one-case run directory.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Trace2SkillOneCaseRunSourceArtifacts {
    /// One-row `SpreadsheetBench` case JSON.
    pub case_file: PathBuf,
    /// Source directory containing the upstream prompt and workbooks.
    pub spreadsheet_dir: PathBuf,
    /// Upstream spreadsheet-agent system prompt.
    pub system_prompt_file: PathBuf,
    /// Released upstream skill directory `SKILL.md`.
    pub released_skill_file: PathBuf,
}

/// Manifest written into a prepared one-case run directory.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Trace2SkillOneCaseRunManifest {
    /// Upstream `SpreadsheetBench` case id.
    pub case_id: String,
    /// Preparation status.
    pub status: Trace2SkillOneCaseRunStatus,
    /// First missing live primitive when the run is only staged.
    pub missing_primitive: Option<String>,
    /// Durable run directory.
    pub run_dir: PathBuf,
    /// Prompt file intended for the live spreadsheet agent.
    pub prompt_file: PathBuf,
    /// Staged input workbook.
    pub init_workbook: PathBuf,
    /// Staged golden workbook for scorer use only.
    pub golden_workbook: PathBuf,
    /// Expected output workbook path.
    pub output_workbook: PathBuf,
    /// Source artifacts copied or referenced by this run.
    pub source_artifacts: Trace2SkillOneCaseRunSourceArtifacts,
    /// Score report when a candidate output workbook already exists.
    pub score_report: Option<Trace2SkillOneCaseAnswerReport>,
}

/// Report returned after preparing a durable one-case run directory.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct Trace2SkillOneCaseRunReport {
    /// Upstream `SpreadsheetBench` case id.
    pub case_id: String,
    /// Preparation status.
    pub status: Trace2SkillOneCaseRunStatus,
    /// First missing live primitive when the run is only staged.
    pub missing_primitive: Option<String>,
    /// Durable run directory.
    pub run_dir: PathBuf,
    /// Prompt file intended for the live spreadsheet agent.
    pub prompt_file: Trace2SkillFileArtifact,
    /// JSON manifest written into `run_dir`.
    pub manifest_file: Trace2SkillFileArtifact,
    /// Staged input workbook.
    pub init_workbook: Trace2SkillFileArtifact,
    /// Staged golden workbook for scorer use only.
    pub golden_workbook: Trace2SkillFileArtifact,
    /// Expected output workbook path.
    pub output_workbook: PathBuf,
    /// Source artifacts copied or referenced by this run.
    pub source_artifacts: Trace2SkillOneCaseRunSourceArtifacts,
    /// Score report when a candidate output workbook already exists.
    pub score_report: Option<Trace2SkillOneCaseAnswerReport>,
}

/// Files needed to score a prepared one-case run directory.
#[derive(Clone, Copy, Debug)]
pub struct Trace2SkillOneCaseRunScoringInput<'a> {
    /// Prepared run directory containing `manifest.json` and output workbook.
    pub run_dir: &'a Path,
    /// Model or solver identity used to produce the output workbook.
    pub model_id: &'a str,
    /// Transcript/log artifact from the live or external spreadsheet-agent run.
    pub transcript_file: &'a Path,
}

/// Report returned after scoring a prepared one-case run directory.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct Trace2SkillOneCaseRunScoreReport {
    /// Upstream `SpreadsheetBench` case id.
    pub case_id: String,
    /// Updated run status.
    pub status: Trace2SkillOneCaseRunStatus,
    /// Durable run directory.
    pub run_dir: PathBuf,
    /// Scored output workbook.
    pub output_workbook: Trace2SkillFileArtifact,
    /// Transcript/log artifact used for trajectory evidence.
    pub transcript_file: Trace2SkillFileArtifact,
    /// Score report artifact written into `run_dir`.
    pub score_file: Trace2SkillFileArtifact,
    /// Trajectory evidence artifact written into `run_dir`.
    pub trajectory_file: Trace2SkillFileArtifact,
    /// Updated JSON manifest.
    pub manifest_file: Trace2SkillFileArtifact,
    /// Exact workbook score report.
    pub score_report: Trace2SkillOneCaseAnswerReport,
}

/// Files needed to derive a pending Stage 2 analyst fan-out from a scored run.
#[derive(Clone, Copy, Debug)]
pub struct Trace2SkillOneCaseAnalystFanoutInput<'a> {
    /// Prepared and scored run directory containing `trajectory.json`.
    pub run_dir: &'a Path,
    /// Upstream `skill_evolver/prompts` directory.
    pub upstream_prompt_dir: &'a Path,
}

/// Report returned after writing a one-case Stage 2 analyst fan-out.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
pub struct Trace2SkillOneCaseAnalystFanoutReport {
    /// Upstream `SpreadsheetBench` case id.
    pub case_id: String,
    /// Durable run directory.
    pub run_dir: PathBuf,
    /// Scored trajectory artifact consumed by the fan-out.
    pub trajectory_file: Trace2SkillFileArtifact,
    /// Prompt artifact written for the pending analyst call.
    pub prompt_file: Trace2SkillFileArtifact,
    /// Fan-out evidence artifact written into `run_dir`.
    pub fanout_file: Trace2SkillFileArtifact,
    /// Upstream prompt template files embedded into the prompt artifact.
    pub source_prompt_files: Vec<Trace2SkillFileArtifact>,
    /// Caller-declared call manifest.
    pub expected_call_ids: Vec<String>,
    /// Pending call ids after materialization.
    pub pending_call_ids: Vec<String>,
}

/// Prepares a durable one-case run directory without executing a spreadsheet agent.
pub fn prepare_trace2skill_one_case_run(
    input: Trace2SkillOneCaseRunInput<'_>,
) -> Result<Trace2SkillOneCaseRunReport, Trace2SkillManifestError> {
    let inspection = inspect_trace2skill_one_case(input.case)?;
    fs::create_dir_all(input.run_dir)?;

    let init_workbook = input
        .run_dir
        .join(format!("1_{}_init.xlsx", inspection.case_id));
    let golden_workbook = input
        .run_dir
        .join(format!("1_{}_golden.xlsx", inspection.case_id));
    fs::copy(&inspection.init_workbook.path, &init_workbook)?;
    fs::copy(&inspection.golden_workbook.path, &golden_workbook)?;

    let output_workbook = input
        .output_workbook
        .map(Path::to_path_buf)
        .unwrap_or_else(|| {
            input
                .run_dir
                .join(format!("{}_output.xlsx", inspection.case_id))
        });
    let prompt_path = input.run_dir.join("agent_prompt.md");
    fs::write(
        &prompt_path,
        render_run_agent_prompt(
            input.case,
            &inspection,
            input.run_dir,
            &init_workbook,
            &output_workbook,
        )?,
    )?;

    let score_report = if output_workbook.is_file() {
        Some(compare_trace2skill_one_case_answer(
            Trace2SkillOneCaseComparisonInput {
                case_file: input.case.case_file,
                candidate_workbook: &output_workbook,
                golden_workbook: &golden_workbook,
            },
        )?)
    } else {
        None
    };
    let status = if score_report.is_some() {
        Trace2SkillOneCaseRunStatus::ScoredCandidateWorkbook
    } else {
        Trace2SkillOneCaseRunStatus::BlockedMissingLiveSpreadsheetAgent
    };
    let missing_primitive = (status
        == Trace2SkillOneCaseRunStatus::BlockedMissingLiveSpreadsheetAgent)
        .then_some("live_spreadsheet_agent_execution".to_owned());
    let source_artifacts = Trace2SkillOneCaseRunSourceArtifacts {
        case_file: input.case.case_file.to_path_buf(),
        spreadsheet_dir: input.case.spreadsheet_dir.to_path_buf(),
        system_prompt_file: input.case.system_prompt_file.to_path_buf(),
        released_skill_file: input.case.released_skill_file.to_path_buf(),
    };
    let manifest_path = input.run_dir.join("manifest.json");
    let manifest = Trace2SkillOneCaseRunManifest {
        case_id: inspection.case_id.clone(),
        status,
        missing_primitive: missing_primitive.clone(),
        run_dir: input.run_dir.to_path_buf(),
        prompt_file: prompt_path.clone(),
        init_workbook: init_workbook.clone(),
        golden_workbook: golden_workbook.clone(),
        output_workbook: output_workbook.clone(),
        source_artifacts: source_artifacts.clone(),
        score_report: score_report.clone(),
    };
    fs::write(&manifest_path, serde_json::to_vec_pretty(&manifest)?)?;

    Ok(Trace2SkillOneCaseRunReport {
        case_id: inspection.case_id,
        status,
        missing_primitive,
        run_dir: input.run_dir.to_path_buf(),
        prompt_file: file_artifact(&prompt_path)?,
        manifest_file: file_artifact(&manifest_path)?,
        init_workbook: file_artifact(&init_workbook)?,
        golden_workbook: file_artifact(&golden_workbook)?,
        output_workbook,
        source_artifacts,
        score_report,
    })
}

/// Scores a prepared one-case run directory after an output workbook exists.
pub fn score_trace2skill_one_case_run(
    input: Trace2SkillOneCaseRunScoringInput<'_>,
) -> Result<Trace2SkillOneCaseRunScoreReport, Trace2SkillManifestError> {
    let manifest_path = input.run_dir.join("manifest.json");
    let mut manifest: Trace2SkillOneCaseRunManifest =
        serde_json::from_slice(&fs::read(&manifest_path)?)?;
    let output_workbook = file_artifact(&manifest.output_workbook)?;
    let transcript_file = file_artifact(input.transcript_file)?;
    let score_report = compare_trace2skill_one_case_answer(Trace2SkillOneCaseComparisonInput {
        case_file: &manifest.source_artifacts.case_file,
        candidate_workbook: &manifest.output_workbook,
        golden_workbook: &manifest.golden_workbook,
    })?;

    let score_path = input.run_dir.join("score_report.json");
    fs::write(&score_path, serde_json::to_vec_pretty(&score_report)?)?;
    let trajectory = build_scored_trajectory(input, &manifest, &score_report, &score_path);
    let trajectory_path = input.run_dir.join("trajectory.json");
    fs::write(&trajectory_path, serde_json::to_vec_pretty(&trajectory)?)?;

    manifest.status = Trace2SkillOneCaseRunStatus::ScoredCandidateWorkbook;
    manifest.missing_primitive = None;
    manifest.score_report = Some(score_report.clone());
    fs::write(&manifest_path, serde_json::to_vec_pretty(&manifest)?)?;

    Ok(Trace2SkillOneCaseRunScoreReport {
        case_id: manifest.case_id,
        status: Trace2SkillOneCaseRunStatus::ScoredCandidateWorkbook,
        run_dir: input.run_dir.to_path_buf(),
        output_workbook,
        transcript_file,
        score_file: file_artifact(&score_path)?,
        trajectory_file: file_artifact(&trajectory_path)?,
        manifest_file: file_artifact(&manifest_path)?,
        score_report,
    })
}

/// Writes a pending Stage 2 analyst fan-out for a scored one-case run.
pub fn prepare_trace2skill_one_case_analyst_fanout(
    input: Trace2SkillOneCaseAnalystFanoutInput<'_>,
) -> Result<Trace2SkillOneCaseAnalystFanoutReport, Trace2SkillManifestError> {
    let manifest_path = input.run_dir.join("manifest.json");
    let manifest: Trace2SkillOneCaseRunManifest =
        serde_json::from_slice(&fs::read(&manifest_path)?)?;
    let trajectory_path = input.run_dir.join("trajectory.json");
    let trajectory: AgentTrajectoryEvidence = serde_json::from_slice(&fs::read(&trajectory_path)?)?;
    if manifest.case_id != trajectory.task_id() {
        return Err(Trace2SkillManifestError::RunTaskMismatch {
            manifest_case_id: manifest.case_id,
            trajectory_task_id: trajectory.task_id().to_owned(),
        });
    }

    let prompt_source_paths =
        stage2_prompt::stage2_prompt_source_paths(input.upstream_prompt_dir, trajectory.outcome());
    let source_prompt_files = prompt_source_paths
        .iter()
        .map(|path| file_artifact(path))
        .collect::<Result<Vec<_>, _>>()?;
    let prompt_path = input.run_dir.join("stage2_analyst_prompt.md");
    let call_id = one_case_analyst_call_id(&trajectory);
    let score_report = manifest.score_report.as_ref().map_or_else(
        || "none".to_owned(),
        |score| {
            format!(
                "{} ({}/{})",
                score.score, score.matched_cells, score.total_cells
            )
        },
    );
    fs::write(
        &prompt_path,
        stage2_prompt::render_stage2_analyst_prompt(stage2_prompt::Stage2AnalystPromptInput {
            call_id: &call_id,
            task_id: trajectory.task_id(),
            outcome: trajectory.outcome(),
            upstream_prompt_dir: input.upstream_prompt_dir,
            prompt_source_paths: &prompt_source_paths,
            extra_lines: &[
                stage2_prompt::Stage2PromptLine {
                    label: "trajectory_file",
                    value: trajectory_path.display().to_string(),
                },
                stage2_prompt::Stage2PromptLine {
                    label: "score_report_file",
                    value: input.run_dir.join("score_report.json").display().to_string(),
                },
                stage2_prompt::Stage2PromptLine {
                    label: "score",
                    value: score_report,
                },
                stage2_prompt::Stage2PromptLine {
                    label: "source_skill",
                    value: manifest.source_artifacts.released_skill_file.display().to_string(),
                },
                stage2_prompt::Stage2PromptLine {
                    label: "source_case",
                    value: manifest.source_artifacts.case_file.display().to_string(),
                },
            ],
            final_instruction: "The analyst must consume `trajectory.json` and its referenced transcript/analysis payloads, then produce a Trace2Skill JSON patch response for the released skill. This artifact deliberately stops before model execution, parsing, or merge.",
        })?,
    )?;

    let mut fanout = AgentAnalystFanoutEvidence::new([call_id.clone()])?;
    fanout.push(AgentAnalystCallEvidence::new(
        AgentAnalystCallEvidenceInput {
            call_id,
            role: stage2_prompt::stage2_analyst_role(trajectory.outcome()),
            source_task_ids: vec![trajectory.task_id().to_owned()],
            prompt: OutputRecord::blob(BlobRef {
                store: "trace2skill-one-case-run".to_owned(),
                key: prompt_path.display().to_string(),
            }),
            response: None,
            status: AgentAnalystCallStatus::Pending,
            retry_count: 0,
            support_count: 1,
        },
    )?)?;

    let expected_call_ids = fanout.expected_call_ids().to_vec();
    let pending_call_ids = fanout
        .pending_call_ids()
        .into_iter()
        .map(str::to_owned)
        .collect();
    let fanout_path = input.run_dir.join("stage2_fanout.json");
    fs::write(&fanout_path, serde_json::to_vec_pretty(&fanout)?)?;

    Ok(Trace2SkillOneCaseAnalystFanoutReport {
        case_id: manifest.case_id,
        run_dir: input.run_dir.to_path_buf(),
        trajectory_file: file_artifact(&trajectory_path)?,
        prompt_file: file_artifact(&prompt_path)?,
        fanout_file: file_artifact(&fanout_path)?,
        source_prompt_files,
        expected_call_ids,
        pending_call_ids,
    })
}

fn render_run_agent_prompt(
    input: Trace2SkillOneCaseInput<'_>,
    inspection: &Trace2SkillOneCaseInspection,
    run_dir: &Path,
    init_workbook: &Path,
    output_workbook: &Path,
) -> Result<String, Trace2SkillManifestError> {
    let system_prompt = fs::read_to_string(input.system_prompt_file)?;
    let released_skill = fs::read_to_string(input.released_skill_file)?;
    let upstream_prompt = fs::read_to_string(input.spreadsheet_dir.join("prompt.txt"))?;
    let answer_sheet = inspection.answer_sheet.as_deref().unwrap_or("");
    let data_position = inspection.data_position.as_deref().unwrap_or("");
    Ok(format!(
        "# Trace2Skill SpreadsheetBench Case {case_id}\n\n\
         ## System Prompt\n{system_prompt}\n\n\
         ## Released Skill\n{released_skill}\n\n\
         ## Dataset Instruction\n{instruction}\n\n\
         ## Upstream prompt.txt\n{upstream_prompt}\n\n\
         ## Files\n\
         - working_directory: {working_directory}\n\
         - spreadsheet_path: {spreadsheet_path}\n\
         - output_path: {output_workbook}\n\
         - instruction_type: {instruction_type}\n\
         - answer_sheet: {answer_sheet}\n\
         - answer_position: {answer_position}\n\
         - data_position: {data_position}\n",
        case_id = inspection.case_id,
        instruction = inspection.instruction,
        working_directory = run_dir.display(),
        spreadsheet_path = init_workbook.display(),
        output_workbook = output_workbook.display(),
        instruction_type = inspection.instruction_type,
        answer_position = inspection.answer_position,
    ))
}

fn one_case_analyst_call_id(trajectory: &AgentTrajectoryEvidence) -> String {
    let prefix = match trajectory.outcome() {
        AgentTrajectoryOutcome::Success => "success",
        AgentTrajectoryOutcome::Failure { .. } => "error",
    };
    format!("{prefix}-{}-1", trajectory.task_id())
}

fn build_scored_trajectory(
    input: Trace2SkillOneCaseRunScoringInput<'_>,
    manifest: &Trace2SkillOneCaseRunManifest,
    score_report: &Trace2SkillOneCaseAnswerReport,
    score_path: &Path,
) -> AgentTrajectoryEvidence {
    let outcome = if score_report.passed {
        AgentTrajectoryOutcome::Success
    } else {
        AgentTrajectoryOutcome::Failure {
            reason: format!(
                "Trace2Skill exact workbook score {} ({}/{})",
                score_report.score, score_report.matched_cells, score_report.total_cells
            ),
        }
    };
    let mut fingerprint = FingerprintBuilder::new();
    fingerprint
        .update("trace2skill-one-case-run")
        .update(input.model_id)
        .update(&manifest.case_id)
        .update(manifest.output_workbook.display().to_string());
    let fingerprint = fingerprint.finish();
    AgentTrajectoryEvidence::new(AgentTrajectoryEvidenceInput {
        session_id: AgentSessionId::new(),
        case_id: None,
        task_id: manifest.case_id.clone(),
        outcome,
        model_id: input.model_id.to_owned(),
        model_config_fingerprint: fingerprint,
        transcript: OutputRecord::blob(BlobRef {
            store: "trace2skill-one-case-run".to_owned(),
            key: input.transcript_file.display().to_string(),
        }),
        commands: CommandEvidence::new(Vec::new()),
    })
    .with_analysis_records([AgentTrajectoryAnalysisRecord::new(
        AgentTrajectoryAnalysisKind::Custom("trace2skill_one_case_score".to_owned()),
        score_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("score_report.json"),
        OutputRecord::blob(BlobRef {
            store: "trace2skill-one-case-run".to_owned(),
            key: score_path.display().to_string(),
        }),
    )])
}
