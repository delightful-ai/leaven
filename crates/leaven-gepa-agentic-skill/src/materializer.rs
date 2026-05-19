use leaven_agentic_skill::SkillWorkspaceLayout;
use leaven_artifact_skill::SkillBank;
use leaven_core::OptimizationProblem;
use leaven_engine::{MaterializationReport, MaterializeContext, MaterializeError, Materializer};
use leaven_kernel::{Cost, Metered};
use leaven_workspace::{WorkspacePath, WorkspacePathError, WorkspaceView};

use crate::SkillBankGepaReflectionInput;

/// Materializes the parent skill bank carried by a GEPA reflection input.
#[derive(Clone, Debug, Default)]
pub struct SkillBankGepaReflectionMaterializer {
    layout: SkillWorkspaceLayout,
}

impl SkillBankGepaReflectionMaterializer {
    /// Constructs a materializer with an explicit skill workspace layout.
    #[must_use]
    pub fn new(layout: SkillWorkspaceLayout) -> Self {
        Self { layout }
    }

    /// Returns the layout used by this materializer.
    #[must_use]
    pub const fn layout(&self) -> &SkillWorkspaceLayout {
        &self.layout
    }
}

impl<P, Part> Materializer<P, SkillBankGepaReflectionInput<Part>>
    for SkillBankGepaReflectionMaterializer
where
    P: OptimizationProblem<Artifact = SkillBank>,
    Part: Send + Sync,
{
    async fn materialize_into(
        &self,
        input: &SkillBankGepaReflectionInput<Part>,
        workspace: &mut WorkspaceView<'_>,
        _ctx: MaterializeContext<'_, P>,
    ) -> Result<Metered<MaterializationReport>, MaterializeError> {
        materialize_bank(&input.artifact, &self.layout, workspace)
    }
}

fn materialize_bank(
    bank: &SkillBank,
    layout: &SkillWorkspaceLayout,
    workspace: &mut WorkspaceView<'_>,
) -> Result<Metered<MaterializationReport>, MaterializeError> {
    let mut files_written = 0;
    let mut bytes_written = 0;

    for (skill_name, folder) in bank.folders() {
        for (path, file) in folder.entries() {
            let workspace_path = workspace_path(layout, skill_name.as_str(), path.as_str())?;
            workspace.write_file(&workspace_path, file.bytes())?;
            if file.permissions().executable {
                workspace.set_executable(&workspace_path, true)?;
            }
            files_written += 1;
            bytes_written += u64::try_from(file.bytes().len())
                .expect("usize fits into u64 on supported Leaven targets");
        }
    }

    Ok(Metered::new(
        MaterializationReport {
            files_written,
            bytes_written,
            truncations: Vec::new(),
        },
        Cost::zero(),
    ))
}

fn workspace_path(
    layout: &SkillWorkspaceLayout,
    skill_name: &str,
    skill_path: &str,
) -> Result<WorkspacePath, WorkspacePathError> {
    let skill_root = if layout.skills_root.as_str().is_empty() {
        WorkspacePath::new(skill_name)?
    } else {
        layout.skills_root.join(skill_name)?
    };
    skill_root.join(skill_path)
}
