use std::fmt::Debug;

use leaven_agent::{AgentContextRef, AgentInstructions};
use leaven_agentic::AgentPromptTarget;
use leaven_agentic_skill::SkillWorkspaceLayout;
use leaven_artifact_skill::SkillBank;
use leaven_core::OptimizationProblem;
use leaven_engine::{RenderContext, RenderError, Renderer};
use leaven_gepa::{ReflectiveExample, ReflectiveSideInfoValue};
use leaven_kernel::{Cost, Metered};
use leaven_workspace::WorkspacePath;

use crate::SkillBankGepaReflectionInput;

/// Renders GEPA skill-bank reflection input into provider-neutral agent instructions.
#[derive(Clone, Debug, Default)]
pub struct GepaSkillBankReflectionRenderer {
    layout: SkillWorkspaceLayout,
}

impl GepaSkillBankReflectionRenderer {
    /// Constructs a renderer with an explicit skill workspace layout.
    #[must_use]
    pub fn new(layout: SkillWorkspaceLayout) -> Self {
        Self { layout }
    }

    /// Returns the layout used by this renderer.
    #[must_use]
    pub const fn layout(&self) -> &SkillWorkspaceLayout {
        &self.layout
    }

    /// Renders an input into the exact agent instructions used by the
    /// `Renderer` implementation.
    pub fn render_input<Part: Debug>(
        &self,
        input: &SkillBankGepaReflectionInput<Part>,
    ) -> Result<AgentInstructions, RenderError> {
        render_instructions(input, &self.layout)
    }
}

impl<P, Part> Renderer<P, SkillBankGepaReflectionInput<Part>, AgentPromptTarget>
    for GepaSkillBankReflectionRenderer
where
    P: OptimizationProblem<Artifact = SkillBank>,
    Part: Debug + Send + Sync,
{
    type View = AgentInstructions;

    async fn render(
        &self,
        value: &SkillBankGepaReflectionInput<Part>,
        _target: AgentPromptTarget,
        _ctx: RenderContext<'_, P>,
    ) -> Result<Metered<Self::View>, RenderError> {
        Ok(Metered::new(self.render_input(value)?, Cost::zero()))
    }
}

fn render_instructions<Part: Debug>(
    input: &SkillBankGepaReflectionInput<Part>,
    layout: &SkillWorkspaceLayout,
) -> Result<AgentInstructions, RenderError> {
    let files = context_refs(&input.artifact, layout)?;
    let mut task = String::new();
    task.push_str("# GEPA Reflection\n\n");
    task.push_str("Improve the selected skill-bank artifact part by editing the materialized workspace in place.\n\n");
    task.push_str("## Selected Part\n");
    task.push_str(&input.part_label);
    task.push_str("\n\n");
    task.push_str("## Selected Part Debug\n");
    task.push_str(&format!("{:?}", input.part));
    task.push_str("\n\n");
    if let Some(attempt) = input.attempt_index {
        task.push_str("## Attempt\n");
        task.push_str(&attempt.to_string());
        task.push_str("\n\n");
    }
    task.push_str("## Materialized Artifact\n");
    task.push_str("The parent SkillBank is already written to these workspace files. Inspect the current artifact before editing it.\n\n");
    for file in &files {
        task.push_str("- ");
        task.push_str(file.path.as_str());
        task.push('\n');
    }
    task.push('\n');
    task.push_str(&render_examples(&input.examples));
    task.push_str("\n## Output\n");
    task.push_str("Modify the workspace files only. The stage reads the final workspace tree back into a SkillBank change.\n");

    Ok(AgentInstructions {
        system: Some(
            "You are a Leaven GEPA reflection agent for Agent Skills. Preserve valid SKILL.md frontmatter and make only changes supported by the reflective examples."
                .to_owned(),
        ),
        task,
        context: files,
    })
}

fn context_refs(
    bank: &SkillBank,
    layout: &SkillWorkspaceLayout,
) -> Result<Vec<AgentContextRef>, RenderError> {
    let mut refs = Vec::new();
    for (skill_name, folder) in bank.folders() {
        for path in folder.entries().keys() {
            let workspace_path = workspace_path(layout, skill_name.as_str(), path.as_str())
                .map_err(|source| {
                    RenderError::Message(format!("failed to render skill path: {source}"))
                })?;
            refs.push(AgentContextRef {
                label: format!("{skill_name}/{path}"),
                path: workspace_path,
                media_type: Some("text/markdown".to_owned()),
            });
        }
    }
    Ok(refs)
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
