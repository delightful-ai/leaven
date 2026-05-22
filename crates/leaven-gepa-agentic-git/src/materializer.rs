use leaven_agentic_git::GitProgramMaterializer;
use leaven_artifact_git::GitProgramArtifact;
use leaven_core::OptimizationProblem;
use leaven_engine::{MaterializationReport, MaterializeContext, MaterializeError, Materializer};
use leaven_kernel::{Cost, Metered};
use leaven_workspace::{Command, WorkspacePath, WorkspaceView};

use crate::GitProgramGepaReflectionInput;

/// Materializes the parent Git program carried by a GEPA reflection input.
#[derive(Clone, Debug)]
pub struct GitProgramGepaReflectionMaterializer {
    inner: GitProgramMaterializer,
}

impl GitProgramGepaReflectionMaterializer {
    /// Constructs a materializer from the Git program materialization adapter.
    #[must_use]
    pub const fn new(inner: GitProgramMaterializer) -> Self {
        Self { inner }
    }

    /// Returns the underlying Git program materializer.
    #[must_use]
    pub const fn inner(&self) -> &GitProgramMaterializer {
        &self.inner
    }

    /// Materializes an input into an already allocated workspace view.
    pub fn materialize_input<Part>(
        &self,
        input: &GitProgramGepaReflectionInput<Part>,
        workspace: &mut WorkspaceView<'_>,
    ) -> Result<Metered<MaterializationReport>, MaterializeError> {
        let mut report = self
            .inner
            .materialize_program(&input.artifact, workspace)
            .map_err(|error| MaterializeError::Message(error.to_string()))?;
        write_reflection_brief(input, workspace)?;
        report.files_written += 1;
        report.bytes_written += u64::try_from(reflection_brief(input).len())
            .expect("usize fits into u64 on supported Leaven targets");
        Ok(Metered::new(report, Cost::zero()))
    }
}

impl<P, Part> Materializer<P, GitProgramGepaReflectionInput<Part>>
    for GitProgramGepaReflectionMaterializer
where
    P: OptimizationProblem<Artifact = GitProgramArtifact>,
    Part: Send + Sync,
{
    async fn materialize_into(
        &self,
        input: &GitProgramGepaReflectionInput<Part>,
        workspace: &mut WorkspaceView<'_>,
        _ctx: MaterializeContext<'_, P>,
    ) -> Result<Metered<MaterializationReport>, MaterializeError> {
        self.materialize_input(input, workspace)
    }
}

fn write_reflection_brief<Part>(
    input: &GitProgramGepaReflectionInput<Part>,
    workspace: &mut WorkspaceView<'_>,
) -> Result<(), MaterializeError> {
    ensure_reflection_dir(workspace)?;
    workspace.write_file(&reflection_brief_path(), reflection_brief(input).as_bytes())?;
    Ok(())
}

fn ensure_reflection_dir(workspace: &mut WorkspaceView<'_>) -> Result<(), MaterializeError> {
    let mut command = Command::new("mkdir");
    command.args = vec!["-p".to_owned(), ".leaven".to_owned()];
    let output = workspace.run_command(command)?;
    if output.status.code == Some(0) {
        return Ok(());
    }
    Err(MaterializeError::Message(format!(
        "failed to create GEPA reflection directory: {}",
        String::from_utf8_lossy(&output.stderr.bytes)
    )))
}

fn reflection_brief<Part>(input: &GitProgramGepaReflectionInput<Part>) -> String {
    let mut brief = String::new();
    brief.push_str("# GEPA Git Program Reflection\n\n");
    brief.push_str("## Parent Candidate\n");
    brief.push_str(&input.parent.to_string());
    brief.push_str("\n\n");
    brief.push_str("## Selected Part\n");
    brief.push_str(&input.part_label);
    brief.push_str("\n\n");
    if let Some(attempt) = input.attempt_index {
        brief.push_str("## Attempt\n");
        brief.push_str(&attempt.to_string());
        brief.push_str("\n\n");
    }
    brief.push_str("## Output Contract\n");
    brief.push_str(
        "Edit checked-out repos in place, or write output/proposal.patch or output/proposal.bundle for single-repo programs.\n",
    );
    brief
}

pub fn reflection_brief_path() -> WorkspacePath {
    WorkspacePath::new(".leaven/gepa-reflection.md").expect("static reflection brief path is valid")
}
