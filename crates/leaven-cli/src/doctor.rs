use leaven_agent::{AgentContextRef, AgentInstructions, AgentRunRequest, OutputContract};
use leaven_gepa::{ReflectRequest, ReflectiveExample, ReflectiveSideInfoValue};
use leaven_kernel::CandidateId;
use leaven_workspace::WorkspacePath;
use serde::Serialize;

use crate::fixture::{fixture_reflect_request, fixture_workspace_files};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DoctorCommand {
    Summary,
    ProposalRender { format: OutputFormat },
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
            Self::ProposalRender { format } => proposal_render(format),
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

fn proposal_render(format: OutputFormat) -> Result<String, DoctorError> {
    let request = fixture_reflect_request();
    let files = fixture_workspace_files()?;
    let instructions = render_gepa_skill_bank_reflection(&request, &files)?;
    let output_contract = OutputContract::WorkspaceDiff {
        roots: vec![WorkspacePath::new(".agents/skills")?],
    };
    let run_request = AgentRunRequest::new(instructions, output_contract);
    let report = ProposalRenderDoctor {
        stage: "gepa.reflect.proposal",
        route: "AgenticProposer<SkillBankMaterializer, GepaSkillBankReflectionRenderer, SkillBankWorkspaceProposalParser>",
        proof: "render_only",
        parent: request.parent.to_string(),
        part: request.part_label.clone(),
        workspace_files: files,
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

fn render_gepa_skill_bank_reflection(
    request: &ReflectRequest<String>,
    files: &[WorkspaceFile],
) -> Result<AgentInstructions, DoctorError> {
    let mut task = String::new();
    task.push_str("# GEPA Reflection\n\n");
    task.push_str("You are improving the selected skill-bank artifact part.\n\n");
    task.push_str("## Selected Part\n");
    task.push_str(&request.part_label);
    task.push_str("\n\n");
    task.push_str("## Materialized Artifact\n");
    task.push_str("The current parent artifact is already written in the workspace. Inspect and edit the relevant files in place.\n\n");
    for file in files {
        task.push_str("- ");
        task.push_str(file.path.as_str());
        task.push('\n');
    }
    task.push('\n');
    task.push_str(&render_examples(&request.examples));
    task.push_str("\n## Output\n");
    task.push_str("Edit the workspace. The stage will read the final workspace tree back as an artifact-native change.\n");

    let context = files
        .iter()
        .map(|file| AgentContextRef {
            label: file.label.clone(),
            path: file.path.clone(),
            media_type: Some(file.media_type.clone()),
        })
        .collect();

    Ok(AgentInstructions {
        system: Some(
            "You are a Leaven proposal-stage reflection agent. Preserve valid Agent Skills layout and make only artifact changes that address the reflective feedback."
                .to_owned(),
        ),
        task,
        context,
    })
}

fn render_examples(examples: &[ReflectiveExample]) -> String {
    if examples.is_empty() {
        return "## Reflective Examples\n(no reflective examples selected)\n".to_owned();
    }

    let mut rendered = String::from("## Reflective Examples\n\n");
    for (index, example) in examples.iter().enumerate() {
        rendered.push_str("### Example ");
        rendered.push_str(&(index + 1).to_string());
        rendered.push('\n');
        for (name, value) in &example.side_info {
            rendered.push_str("#### ");
            rendered.push_str(name.trim());
            rendered.push('\n');
            render_side_info(&mut rendered, value, 5);
        }
        if !example.input.is_empty() {
            rendered.push_str("#### Input\n");
            rendered.push_str(example.input.trim());
            rendered.push('\n');
        }
        if let Some(output) = &example.output {
            rendered.push_str("#### Output\n");
            rendered.push_str(output.trim());
            rendered.push('\n');
        }
        if let Some(score) = example.score {
            rendered.push_str("#### Score\n");
            rendered.push_str(&score.to_string());
            rendered.push('\n');
        }
        if !example.feedback.is_empty() {
            rendered.push_str("#### Feedback\n");
            rendered.push_str(example.feedback.trim());
            rendered.push('\n');
        }
        rendered.push('\n');
    }
    rendered
}

fn render_side_info(rendered: &mut String, value: &ReflectiveSideInfoValue, level: usize) {
    match value {
        ReflectiveSideInfoValue::Text(text) => {
            rendered.push_str(text.trim());
            rendered.push_str("\n\n");
        }
        ReflectiveSideInfoValue::Mapping(fields) => {
            for (name, child) in fields {
                rendered.push_str(&"#".repeat(level.min(6)));
                rendered.push(' ');
                rendered.push_str(name.trim());
                rendered.push('\n');
                render_side_info(rendered, child, level + 1);
            }
        }
        ReflectiveSideInfoValue::List(items) => {
            for (index, child) in items.iter().enumerate() {
                rendered.push_str(&"#".repeat(level.min(6)));
                rendered.push_str(" Item ");
                rendered.push_str(&(index + 1).to_string());
                rendered.push('\n');
                render_side_info(rendered, child, level + 1);
            }
        }
    }
}

#[derive(Clone, Debug, Serialize)]
struct ProposalRenderDoctor {
    stage: &'static str,
    route: &'static str,
    proof: &'static str,
    parent: String,
    part: String,
    workspace_files: Vec<WorkspaceFile>,
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
            output.push_str(&file.media_type);
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

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct WorkspaceFile {
    pub label: String,
    pub path: WorkspacePath,
    pub media_type: String,
}

impl WorkspaceFile {
    pub fn markdown(label: impl Into<String>, path: impl AsRef<str>) -> Result<Self, DoctorError> {
        Ok(Self {
            label: label.into(),
            path: WorkspacePath::new(path)?,
            media_type: "text/markdown".to_owned(),
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DoctorError {
    #[error(transparent)]
    WorkspacePath(#[from] leaven_workspace::WorkspacePathError),
    #[error("failed to serialize doctor output as JSON")]
    SerializeJson(#[source] serde_json::Error),
}

pub(crate) fn fixed_parent() -> CandidateId {
    CandidateId::from_uuid(uuid::uuid!("00000000-0000-0000-0000-000000000001"))
}
