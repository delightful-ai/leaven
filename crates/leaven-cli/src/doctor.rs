use std::path::PathBuf;

use leaven_agent::{AgentContextRef, AgentRunRequest, OutputContract};
use leaven_agentic_skill::SkillWorkspaceLayout;
use leaven_gepa_agentic_skill::GepaSkillBankReflectionRenderer;
use leaven_gepa_agentic_skill::SkillBankGepaReflectionInput;
use leaven_workspace::WorkspacePath;
use serde::Serialize;

use crate::fixture::{fixture_reflection_input, fixture_workspace_layout};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DoctorCommand {
    Summary,
    ProposalRender {
        format: OutputFormat,
        input_json: Option<PathBuf>,
    },
    Help,
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
            Self::Help => Ok(crate::HELP.to_owned()),
        }
    }
}

fn summary() -> String {
    "\
Leaven doctor

Available checks:
  proposal-render  render-only simulation of the GEPA skill-bank proposal-stage handoff

The doctor surface is inspection-only: it does not run live providers, mutate a run graph, or claim optimizer success.
"
    .to_owned()
}

fn proposal_render(
    format: OutputFormat,
    input_json: Option<PathBuf>,
) -> Result<String, DoctorError> {
    let layout = fixture_workspace_layout()?;
    let input = match input_json {
        Some(path) => read_reflection_input(&path)?,
        None => fixture_reflection_input(),
    };
    let renderer = GepaSkillBankReflectionRenderer::new(layout.clone());
    let instructions = renderer.render_input(&input)?;
    let output_contract = OutputContract::WorkspaceDiff {
        roots: vec![output_root(&layout)],
    };
    let workspace_files = instructions.context.clone();
    let run_request = AgentRunRequest::new(instructions, output_contract);
    let report = ProposalRenderDoctor {
        stage: "gepa.reflect.proposal",
        route: "GepaSkillBankAgenticReflector -> AgenticProposer<SkillBankGepaReflectionMaterializer, GepaSkillBankReflectionRenderer, SkillBankGepaReflectionParser>",
        proof: "render_only",
        parent: input.parent.to_string(),
        part: input.part_label.clone(),
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

fn read_reflection_input(
    path: &std::path::Path,
) -> Result<SkillBankGepaReflectionInput<String>, DoctorError> {
    let bytes = std::fs::read(path).map_err(|source| DoctorError::ReadInput {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_slice(&bytes).map_err(|source| DoctorError::ParseInput {
        path: path.to_path_buf(),
        source,
    })
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

impl ProposalRenderDoctor {
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

#[derive(Debug, thiserror::Error)]
pub enum DoctorError {
    #[error(transparent)]
    WorkspacePath(#[from] leaven_workspace::WorkspacePathError),
    #[error(transparent)]
    Render(#[from] leaven_engine::RenderError),
    #[error("failed to read reflection input `{}`", path.display())]
    ReadInput {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse reflection input `{}` as SkillBankGepaReflectionInput JSON", path.display())]
    ParseInput {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("failed to serialize doctor output as JSON")]
    SerializeJson(#[source] serde_json::Error),
}

fn output_root(layout: &SkillWorkspaceLayout) -> WorkspacePath {
    if layout.skills_root.as_str().is_empty() {
        WorkspacePath::root()
    } else {
        layout.skills_root.clone()
    }
}
