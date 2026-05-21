use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::{
    compare_trace2skill_one_case_answer, file_artifact, inspect_trace2skill_one_case,
    Trace2SkillFileArtifact, Trace2SkillManifestError, Trace2SkillOneCaseAnswerReport,
    Trace2SkillOneCaseComparisonInput, Trace2SkillOneCaseInput, Trace2SkillOneCaseInspection,
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
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Trace2SkillOneCaseRunStatus {
    /// The run directory is staged, but Leaven has not executed a live spreadsheet agent.
    BlockedMissingLiveSpreadsheetAgent,
    /// A caller supplied an output workbook and the report includes a score.
    ScoredCandidateWorkbook,
}

/// Source artifacts used to prepare a one-case run directory.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
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
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
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
