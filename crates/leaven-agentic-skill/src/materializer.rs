//! Skill-bank workspace materializer.

use leaven_artifact_skill::SkillBank;
use leaven_core::OptimizationProblem;
use leaven_engine::{MaterializationReport, MaterializeContext, MaterializeError, Materializer};
use leaven_kernel::{Cost, Metered};
use leaven_workspace::{WorkspacePath, WorkspacePathError, WorkspaceView};

use crate::{SkillBankProposalInput, SkillWorkspaceLayout};

/// Materializes a parent [`SkillBank`] into a workspace.
#[derive(Clone, Debug, Default)]
pub struct SkillBankMaterializer {
    layout: SkillWorkspaceLayout,
}

impl SkillBankMaterializer {
    /// Constructs a materializer with an explicit layout.
    #[must_use]
    pub fn new(layout: SkillWorkspaceLayout) -> Self {
        Self { layout }
    }

    /// Returns the layout.
    #[must_use]
    pub const fn layout(&self) -> &SkillWorkspaceLayout {
        &self.layout
    }
}

impl<P> Materializer<P, SkillBankProposalInput> for SkillBankMaterializer
where
    P: OptimizationProblem<Artifact = SkillBank>,
{
    async fn materialize_into(
        &self,
        input: &SkillBankProposalInput,
        workspace: &mut WorkspaceView<'_>,
        ctx: MaterializeContext<'_, P>,
    ) -> Result<Metered<MaterializationReport>, MaterializeError> {
        let bank = ctx
            .graph()
            .artifact(input.parent)
            .ok_or_else(|| MaterializeError::Message("parent skill bank not found".to_owned()))?;
        let mut files_written = 0;
        let mut bytes_written = 0;

        for (skill_name, folder) in bank.folders() {
            for (path, file) in folder.entries() {
                let workspace_path =
                    workspace_path(&self.layout, skill_name.as_str(), path.as_str())?;
                workspace.write_file(&workspace_path, file.bytes())?;
                if file.permissions().executable {
                    workspace.set_executable(&workspace_path, true)?;
                }
                files_written += 1;
                bytes_written += u64::try_from(file.bytes().len()).map_err(|_err| {
                    MaterializeError::Message("skill file byte length overflowed u64".to_owned())
                })?;
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
