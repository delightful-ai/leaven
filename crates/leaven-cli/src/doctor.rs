use std::path::PathBuf;

use futures::executor::block_on;
use leaven_agent::{
    AgentContextRef, AgentRunRequest, FakeAgentAction, FakeAgentRuntime, OutputContract,
};
use leaven_agentic::{AgenticProposerConfig, ArtifactReflector};
use leaven_agentic_skill::SkillWorkspaceLayout;
use leaven_artifact_skill::{SkillFilePartId, SkillFileSurface, SkillName, SkillPath};
use leaven_core::{Evidence, InfoRef, OptimizationProblem};
use leaven_engine::{BudgetLedger, RunContext, RunGraph};
use leaven_gepa::{GepaReflector, ReflectRequest};
use leaven_gepa_agentic_skill::{
    GepaSkillBankAgenticReflector, SkillBankReflectionInput, SkillBankReflector,
};
use leaven_kernel::{ProposerId, RunId};
use leaven_workspace::{WorkspaceConfig, WorkspaceFactory, WorkspacePath};
use leaven_workspace_local::LocalWorkspaceFactory;
use serde::Serialize;

use crate::fixture::{fixture_reflection_input, fixture_workspace_layout};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DoctorCommand {
    Summary,
    ProposalRender {
        format: OutputFormat,
        input_json: Option<PathBuf>,
    },
    ProposalMaterialize {
        format: OutputFormat,
        input_json: Option<PathBuf>,
    },
    ProposalRoundtrip {
        format: OutputFormat,
        input_json: Option<PathBuf>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputFormat {
    Text,
    Json,
}

impl DoctorCommand {
    pub fn run(self) -> Result<String, DoctorError> {
        match self {
            Self::Summary => Ok(summary()),
            Self::ProposalRender { format, input_json } => proposal_render(format, input_json),
            Self::ProposalMaterialize { format, input_json } => {
                proposal_materialize(format, input_json)
            }
            Self::ProposalRoundtrip { format, input_json } => {
                proposal_roundtrip(format, input_json)
            }
        }
    }
}

fn summary() -> String {
    "\
Leaven doctor

Available checks:
  proposal-render  render-only simulation of the GEPA skill-bank proposal-stage handoff
  proposal-materialize  writes the reflected SkillBank input into a temp workspace and reports files
  proposal-roundtrip  simulates one workspace edit and applies the parsed proposal through RunContext

The doctor surface is local-only: it does not run live providers or claim optimizer quality.
"
    .to_owned()
}

fn proposal_render(
    format: OutputFormat,
    input_json: Option<PathBuf>,
) -> Result<String, DoctorError> {
    let input = match input_json {
        Some(path) => read_reflection_input(&path)?,
        None => fixture_reflection_input(),
    };
    let instructions =
        leaven_agent::AgentInstructions::task("Read TASK.md and edit target/current in place.");
    let output_contract = OutputContract::WorkspaceDiff {
        roots: vec![WorkspacePath::new("target/current").expect("constant path is valid")],
        surface_fingerprint: None,
    };
    let workspace_files = vec![
        AgentContextRef {
            label: "manifest".to_owned(),
            path: WorkspacePath::new("MANIFEST.json").expect("constant path is valid"),
            media_type: Some("application/json".to_owned()),
        },
        AgentContextRef {
            label: "task".to_owned(),
            path: WorkspacePath::new("TASK.md").expect("constant path is valid"),
            media_type: Some("text/markdown".to_owned()),
        },
    ];
    let run_request = AgentRunRequest::new(instructions, output_contract);
    let report = ProposalRenderDoctor {
        stage: "gepa.reflect.proposal",
        route: "GepaSkillBankAgenticReflector -> ReflectionWorkspace<SkillBankReflector>",
        proof: "render_only",
        parent: input.parent.to_string(),
        part: input.part_label,
        workspace_files,
        agent_request: run_request,
        gaps: vec![
            "No agent session is executed.",
            "No workspace mutation is parsed back into a ProposalBatch.",
            "No proposal is applied through RunContext in this doctor command.",
        ],
    };

    match format {
        OutputFormat::Text => Ok(report.to_text()),
        OutputFormat::Json => {
            serde_json::to_string_pretty(&report).map_err(DoctorError::SerializeJson)
        }
    }
}

fn proposal_materialize(
    format: OutputFormat,
    input_json: Option<PathBuf>,
) -> Result<String, DoctorError> {
    let layout = fixture_workspace_layout()?;
    let input = load_input(input_json)?;
    let report = block_on(materialize_report(input, layout))?;
    format_report(&report, format)
}

async fn materialize_report(
    input: SkillBankReflectionInput<String>,
    layout: SkillWorkspaceLayout,
) -> Result<ProposalMaterializeDoctor, DoctorError> {
    let mut workspace = LocalWorkspaceFactory::temp()
        .allocate(WorkspaceConfig::default())
        .await
        .map_err(|source| DoctorError::WorkspaceAllocate(source.to_string()))?;
    let result = {
        let view = workspace.view();
        let mut current = view
            .subdir(WorkspacePath::new("target/current").expect("constant path is valid"))
            .map_err(|source| DoctorError::Workspace(source.to_string()))?;
        SkillBankReflector::<String>::new(layout.clone())
            .project(&input, &mut current)
            .await
            .map_err(|source| DoctorError::Workspace(source.to_string()))?;
        let files = read_workspace_files(
            &view,
            &WorkspacePath::new("target/current").expect("constant path is valid"),
        )?;
        let local_mount = view.local_mount().map(|path| path.display().to_string());
        let files_written = files.len();
        let bytes_written = files
            .iter()
            .map(|file| u64::try_from(file.bytes).expect("usize fits u64"))
            .sum();
        ProposalMaterializeDoctor {
            stage: "gepa.reflect.materialize",
            proof: "materialize_only",
            parent: input.parent.to_string(),
            part: input.part_label,
            local_mount,
            files_written,
            bytes_written,
            files,
            gaps: vec![
                "No agent session is executed.",
                "No workspace mutation is parsed back into a ProposalBatch.",
                "No proposal is applied through RunContext in this doctor command.",
            ],
        }
    };
    workspace
        .cleanup()
        .await
        .map_err(|source| DoctorError::WorkspaceCleanup(source.to_string()))?;
    Ok(result)
}

fn proposal_roundtrip(
    format: OutputFormat,
    input_json: Option<PathBuf>,
) -> Result<String, DoctorError> {
    let layout = fixture_workspace_layout()?;
    let input = load_input(input_json)?;
    let report = block_on(roundtrip_report(input, layout))?;
    format_report(&report, format)
}

async fn roundtrip_report(
    input: SkillBankReflectionInput<String>,
    layout: SkillWorkspaceLayout,
) -> Result<ProposalRoundtripDoctor, DoctorError> {
    let (skill, write_path, bytes) = simulated_skill_edit(&input, &layout)?;
    let mut graph = RunGraph::<DoctorSkillProblem>::new(RunId::new());
    let mut budget = BudgetLedger::default();
    let parent = {
        let mut ctx = RunContext::<DoctorSkillProblem>::new(&mut graph, &mut budget);
        ctx.insert_seed(input.artifact.clone(), 0)
            .map_err(|source| DoctorError::RunContext(source.to_string()))?
    };
    let mut request = ReflectRequest::for_part(
        parent,
        SkillFilePartId {
            skill: skill.clone(),
            path: SkillPath::skill_md(),
        },
        format!("{skill}/SKILL.md"),
    )
    .with_examples(input.examples.clone())
    .with_source_refs(
        input
            .source_refs
            .iter()
            .cloned()
            .chain(std::iter::once(InfoRef::Candidate(parent))),
    );
    if let Some(attempt_index) = input.attempt_index {
        request = request.with_attempt_index(attempt_index);
    }
    let mut reflector = GepaSkillBankAgenticReflector::new(
        AgenticProposerConfig::new(ProposerId::from("doctor/gepa-skill-roundtrip")),
        LocalWorkspaceFactory::temp(),
        FakeAgentRuntime::new(vec![
            FakeAgentAction::ReadFile {
                path: write_path.clone(),
            },
            FakeAgentAction::WriteFile {
                path: write_path.clone(),
                bytes,
            },
        ]),
        layout,
    );
    let mut ctx = RunContext::<DoctorSkillProblem>::new(&mut graph, &mut budget);
    let child = reflector
        .reflect_candidate(&mut ctx, &SkillFileSurface, request)
        .await
        .map_err(|source| DoctorError::Optimizer(source.to_string()))?
        .ok_or_else(|| {
            DoctorError::Optimizer("simulated roundtrip produced no child".to_owned())
        })?;
    let proposal = ctx
        .graph()
        .proposal_that_created(child)
        .ok_or_else(|| DoctorError::Optimizer("child has no creating proposal".to_owned()))?;
    Ok(ProposalRoundtripDoctor {
        stage: "gepa.reflect.roundtrip",
        proof: "simulated_agent_apply",
        parent: parent.to_string(),
        child: child.to_string(),
        proposal_count: 1,
        informed_by_count: proposal.provenance().informed_by_refs().len(),
        edited_path: write_path,
        gaps: vec![
            "The agent runtime is deterministic FakeAgentRuntime.",
            "No live provider session is executed.",
            "Optimizer quality is not evaluated by this doctor command.",
        ],
    })
}

fn read_reflection_input(
    path: &std::path::Path,
) -> Result<SkillBankReflectionInput<String>, DoctorError> {
    let bytes = std::fs::read(path).map_err(|source| DoctorError::ReadInput {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_slice(&bytes).map_err(|source| DoctorError::ParseInput {
        path: path.to_path_buf(),
        source,
    })
}

fn load_input(
    input_json: Option<PathBuf>,
) -> Result<SkillBankReflectionInput<String>, DoctorError> {
    match input_json {
        Some(path) => read_reflection_input(&path),
        None => Ok(fixture_reflection_input()),
    }
}

fn format_report<T>(report: &T, format: OutputFormat) -> Result<String, DoctorError>
where
    T: DoctorText + Serialize,
{
    match format {
        OutputFormat::Text => Ok(report.to_text()),
        OutputFormat::Json => {
            serde_json::to_string_pretty(report).map_err(DoctorError::SerializeJson)
        }
    }
}

trait DoctorText {
    fn to_text(&self) -> String;
}

#[derive(Clone, Debug, Serialize)]
struct ProposalRenderDoctor {
    stage: &'static str,
    route: &'static str,
    proof: &'static str,
    parent: String,
    part: String,
    workspace_files: Vec<AgentContextRef>,
    agent_request: AgentRunRequest,
    gaps: Vec<&'static str>,
}

impl DoctorText for ProposalRenderDoctor {
    fn to_text(&self) -> String {
        let mut output = String::new();
        output.push_str("Stage: ");
        output.push_str(self.stage);
        output.push('\n');
        output.push_str("Route: ");
        output.push_str(self.route);
        output.push('\n');
        output.push_str("Proof: ");
        output.push_str(self.proof);
        output.push('\n');
        output.push_str("Parent: ");
        output.push_str(&self.parent);
        output.push('\n');
        output.push_str("Part: ");
        output.push_str(&self.part);
        output.push_str("\n\nWorkspace files:\n");
        for file in &self.workspace_files {
            output.push_str("  - ");
            output.push_str(file.path.as_str());
            output.push_str(" (");
            output.push_str(file.media_type.as_deref().unwrap_or("unknown"));
            output.push_str(")\n");
        }
        output.push_str("\nAgent system:\n");
        output.push_str(
            self.agent_request
                .instructions
                .system
                .as_deref()
                .unwrap_or("(none)"),
        );
        output.push_str("\n\nAgent task:\n");
        output.push_str(&self.agent_request.instructions.task);
        output.push_str("\nGaps:\n");
        for gap in &self.gaps {
            output.push_str("  - ");
            output.push_str(gap);
            output.push('\n');
        }
        output
    }
}

#[derive(Clone, Debug, Serialize)]
struct ProposalMaterializeDoctor {
    stage: &'static str,
    proof: &'static str,
    parent: String,
    part: String,
    local_mount: Option<String>,
    files_written: usize,
    bytes_written: u64,
    files: Vec<WorkspaceFileReport>,
    gaps: Vec<&'static str>,
}

impl DoctorText for ProposalMaterializeDoctor {
    fn to_text(&self) -> String {
        let mut output = format!(
            "Stage: {}\nProof: {}\nParent: {}\nPart: {}\nFiles written: {}\nBytes written: {}\n",
            self.stage, self.proof, self.parent, self.part, self.files_written, self.bytes_written
        );
        if let Some(local_mount) = &self.local_mount {
            output.push_str("Local mount: ");
            output.push_str(local_mount);
            output.push('\n');
        }
        output.push_str("\nWorkspace files:\n");
        for file in &self.files {
            output.push_str("  - ");
            output.push_str(file.path.as_str());
            output.push_str(" bytes=");
            output.push_str(&file.bytes.to_string());
            output.push('\n');
        }
        output.push_str("\nGaps:\n");
        for gap in &self.gaps {
            output.push_str("  - ");
            output.push_str(gap);
            output.push('\n');
        }
        output
    }
}

#[derive(Clone, Debug, Serialize)]
struct ProposalRoundtripDoctor {
    stage: &'static str,
    proof: &'static str,
    parent: String,
    child: String,
    proposal_count: usize,
    informed_by_count: usize,
    edited_path: WorkspacePath,
    gaps: Vec<&'static str>,
}

impl DoctorText for ProposalRoundtripDoctor {
    fn to_text(&self) -> String {
        let mut output = format!(
            "Stage: {}\nProof: {}\nParent: {}\nChild: {}\nProposal count: {}\nInformed-by refs: {}\nEdited path: {}\n\nGaps:\n",
            self.stage,
            self.proof,
            self.parent,
            self.child,
            self.proposal_count,
            self.informed_by_count,
            self.edited_path
        );
        for gap in &self.gaps {
            output.push_str("  - ");
            output.push_str(gap);
            output.push('\n');
        }
        output
    }
}

#[derive(Clone, Debug, Serialize)]
struct WorkspaceFileReport {
    path: WorkspacePath,
    bytes: usize,
    preview: String,
}

#[derive(Debug, thiserror::Error)]
pub enum DoctorError {
    #[error(transparent)]
    WorkspacePath(#[from] leaven_workspace::WorkspacePathError),
    #[error(transparent)]
    Render(#[from] leaven_engine::RenderError),
    #[error(transparent)]
    Materialize(#[from] leaven_engine::MaterializeError),
    #[error("workspace allocation failed: {0}")]
    WorkspaceAllocate(String),
    #[error("workspace operation failed: {0}")]
    Workspace(String),
    #[error("workspace cleanup failed: {0}")]
    WorkspaceCleanup(String),
    #[error("run context operation failed: {0}")]
    RunContext(String),
    #[error("optimizer operation failed: {0}")]
    Optimizer(String),
    #[error("reflection input skill bank is empty")]
    EmptySkillBank,
    #[error("reflection input part is not a materialized skill SKILL.md path: {0}")]
    InvalidReflectionPart(String),
    #[error("failed to read reflection input `{}`", path.display())]
    ReadInput {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse reflection input `{}` as SkillBankReflectionInput JSON", path.display())]
    ParseInput {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("failed to serialize doctor output as JSON")]
    SerializeJson(#[source] serde_json::Error),
}

#[derive(Clone, Debug)]
struct DoctorSkillProblem;

impl OptimizationProblem for DoctorSkillProblem {
    type Artifact = leaven_artifact_skill::SkillBank;
    type Case = ();
    type Evidence = DoctorEvidence;
    type ProposalAnnotations = ();
}

#[derive(Clone, Debug)]
struct DoctorEvidence;

impl Evidence for DoctorEvidence {}

fn read_workspace_files(
    view: &leaven_workspace::WorkspaceView<'_>,
    root: &WorkspacePath,
) -> Result<Vec<WorkspaceFileReport>, DoctorError> {
    let mut files = Vec::new();
    for path in view
        .list_files(root)
        .map_err(|source| DoctorError::Workspace(source.to_string()))?
    {
        let bytes = view
            .read_file(&path)
            .map_err(|source| DoctorError::Workspace(source.to_string()))?;
        files.push(WorkspaceFileReport {
            path,
            bytes: bytes.len(),
            preview: preview(&bytes),
        });
    }
    Ok(files)
}

fn preview(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    text.chars().take(240).collect()
}

fn simulated_skill_edit(
    input: &SkillBankReflectionInput<String>,
    layout: &SkillWorkspaceLayout,
) -> Result<(SkillName, WorkspacePath, Vec<u8>), DoctorError> {
    let (name, folder) = selected_skill_folder(input, layout)?;
    let write_path = WorkspacePath::new("target/current")?
        .join(workspace_path(layout, name.as_str(), "SKILL.md")?.as_str())?;
    let description = format!(
        "{} Doctor simulated GEPA reflection edit.",
        folder.manifest().description.as_str().trim_end_matches('.')
    );
    let body = format!(
        "{}\n\nDoctor simulated edit: inspect the materialized skill before changing it.\n",
        folder.body().as_str().trim()
    );
    Ok((
        name.clone(),
        write_path,
        format!(
            "---\nname: {}\ndescription: {}\n---\n{}\n",
            name.as_str(),
            description,
            body
        )
        .into_bytes(),
    ))
}

fn selected_skill_folder<'a>(
    input: &'a SkillBankReflectionInput<String>,
    layout: &SkillWorkspaceLayout,
) -> Result<(SkillName, &'a leaven_artifact_skill::SkillFolder), DoctorError> {
    if input.artifact.is_empty() {
        return Err(DoctorError::EmptySkillBank);
    }
    let part = input
        .part
        .strip_prefix(layout.skills_root.as_str())
        .and_then(|stripped| stripped.strip_prefix('/'))
        .unwrap_or(&input.part);
    let (skill, path) = part
        .split_once('/')
        .ok_or_else(|| DoctorError::InvalidReflectionPart(input.part.clone()))?;
    let path =
        SkillPath::new(path).map_err(|_| DoctorError::InvalidReflectionPart(input.part.clone()))?;
    if !path.is_skill_md() {
        return Err(DoctorError::InvalidReflectionPart(input.part.clone()));
    }
    let skill = SkillName::new(skill)
        .map_err(|_| DoctorError::InvalidReflectionPart(input.part.clone()))?;
    let folder = input
        .artifact
        .get(&skill)
        .ok_or_else(|| DoctorError::InvalidReflectionPart(input.part.clone()))?;
    Ok((skill, folder))
}

fn workspace_path(
    layout: &SkillWorkspaceLayout,
    skill_name: &str,
    skill_path: &str,
) -> Result<WorkspacePath, leaven_workspace::WorkspacePathError> {
    let skill_root = if layout.skills_root.as_str().is_empty() {
        WorkspacePath::new(skill_name)?
    } else {
        layout.skills_root.join(skill_name)?
    };
    skill_root.join(skill_path)
}
