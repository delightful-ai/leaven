use std::fmt::{Debug, Write as _};

use leaven_agent::{AgentContextRef, AgentInstructions};
use leaven_agentic::AgentPromptTarget;
use leaven_artifact_git::GitProgramArtifact;
use leaven_core::OptimizationProblem;
use leaven_engine::{RenderContext, RenderError, Renderer};
use leaven_gepa::{ReflectiveExample, ReflectiveSideInfoValue};
use leaven_kernel::{Cost, Metered};
use leaven_workspace::WorkspacePath;

use crate::GitProgramGepaReflectionInput;
use crate::materializer::reflection_brief_path;

/// Renders GEPA Git-program reflection input into provider-neutral agent instructions.
#[derive(Clone, Debug, Default)]
pub struct GepaGitProgramReflectionRenderer;

impl GepaGitProgramReflectionRenderer {
    /// Renders an input into the exact agent instructions used by the
    /// `Renderer` implementation.
    pub fn render_input<Part: Debug>(
        &self,
        input: &GitProgramGepaReflectionInput<Part>,
    ) -> Result<AgentInstructions, RenderError> {
        render_instructions(input)
    }
}

impl<P, Part> Renderer<P, GitProgramGepaReflectionInput<Part>, AgentPromptTarget>
    for GepaGitProgramReflectionRenderer
where
    P: OptimizationProblem<Artifact = GitProgramArtifact>,
    Part: Debug + Send + Sync,
{
    type View = AgentInstructions;

    async fn render(
        &self,
        value: &GitProgramGepaReflectionInput<Part>,
        _target: AgentPromptTarget,
        _ctx: RenderContext<'_, P>,
    ) -> Result<Metered<Self::View>, RenderError> {
        Ok(Metered::new(self.render_input(value)?, Cost::zero()))
    }
}

fn render_instructions<Part: Debug>(
    input: &GitProgramGepaReflectionInput<Part>,
) -> Result<AgentInstructions, RenderError> {
    let files = context_refs(&input.artifact)?;
    let mut task = String::new();
    task.push_str("# GEPA Git Program Reflection\n\n");
    task.push_str("Improve the selected Git program artifact by editing the materialized workspace in place.\n\n");
    task.push_str("## Selected Part\n");
    task.push_str(&input.part_label);
    task.push_str("\n\n");
    task.push_str("## Selected Part Debug\n");
    write!(&mut task, "{:?}", input.part).expect("writing to a String cannot fail");
    task.push_str("\n\n");
    if let Some(attempt) = input.attempt_index {
        task.push_str("## Attempt\n");
        task.push_str(&attempt.to_string());
        task.push_str("\n\n");
    }
    task.push_str("## Materialized Repositories\n");
    task.push_str("The parent GitProgramArtifact is already checked out at these workspace paths. Inspect the current artifact before editing it.\n\n");
    for (repo, path) in input.artifact.layout().entries() {
        task.push_str("- ");
        task.push_str(repo.as_str());
        task.push_str(": ");
        task.push_str(path.as_str());
        task.push('\n');
    }
    task.push('\n');
    task.push_str(&render_examples(&input.examples));
    task.push_str("\n## Output\n");
    task.push_str("Either edit the checked-out repositories in place, or for a single-repo program write `output/proposal.patch` or `output/proposal.bundle`.\n");
    task.push_str("The stage imports the final repository state into durable Git storage and records a typed GitProgramChange proposal.\n");

    Ok(AgentInstructions {
        system: Some(
            "You are a Leaven GEPA reflection agent for Git program artifacts. Make only repo changes supported by the reflective examples."
                .to_owned(),
        ),
        task,
        context: files,
    })
}

fn context_refs(artifact: &GitProgramArtifact) -> Result<Vec<AgentContextRef>, RenderError> {
    let mut refs = vec![AgentContextRef {
        label: "GEPA reflection brief".to_owned(),
        path: reflection_brief_path(),
        media_type: Some("text/markdown".to_owned()),
    }];
    for (repo, path) in artifact.layout().entries() {
        refs.push(AgentContextRef {
            label: format!("repo/{}", repo.as_str()),
            path: WorkspacePath::new(path.as_str()).map_err(|source| {
                RenderError::Message(format!("failed to render Git program path: {source}"))
            })?,
            media_type: None,
        });
    }
    Ok(refs)
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
