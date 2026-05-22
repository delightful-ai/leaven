use std::collections::BTreeMap;
use std::marker::PhantomData;

use leaven_agentic::{ArtifactReflector, ReadbackDiagnostic, ReadbackResult};
use leaven_agentic_skill::{SkillBankDiff, SkillWorkspaceLayout};
use leaven_artifact_skill::{
    SkillBank, SkillBankChange, SkillBankError, SkillFile, SkillFilePartId, SkillFilePermissions,
    SkillFolder, SkillName, SkillPath,
};
use leaven_workspace::{WorkspaceError, WorkspacePath, WorkspacePathError, WorkspaceView};
use thiserror::Error;

use crate::SkillBankReflectionInput;

/// Artifact reflector for editing a materialized `SkillBank` in place.
#[derive(Clone, Debug, Default)]
pub struct SkillBankReflector<Part = String> {
    layout: SkillWorkspaceLayout,
    marker: PhantomData<Part>,
}

impl<Part> SkillBankReflector<Part> {
    #[must_use]
    pub const fn new(layout: SkillWorkspaceLayout) -> Self {
        Self {
            layout,
            marker: PhantomData,
        }
    }

    #[must_use]
    pub const fn layout(&self) -> &SkillWorkspaceLayout {
        &self.layout
    }
}

impl<Part> ArtifactReflector for SkillBankReflector<Part>
where
    Part: SkillPartScope + Send + Sync,
{
    type Input = SkillBankReflectionInput<Part>;
    type Change = SkillBankChange;
    type Error = SkillBankReflectionError;

    fn reflection_id(&self) -> &'static str {
        "leaven.gepa.skill_bank.v1"
    }

    async fn project(
        &self,
        input: &Self::Input,
        view: &mut WorkspaceView<'_>,
    ) -> Result<(), Self::Error> {
        materialize_bank(&input.artifact, &self.layout, view)?;
        Ok(())
    }

    async fn read_back(
        &self,
        input: &Self::Input,
        view: &WorkspaceView<'_>,
        _session: &leaven_agent::AgentSession,
    ) -> Result<ReadbackResult<Self::Change>, Self::Error> {
        let child = match read_skill_bank(view, &self.layout) {
            Ok(child) => child,
            Err(error @ SkillBankReflectionError::Workspace(_)) => return Err(error),
            Err(error) => {
                return Ok(ReadbackResult::Invalid {
                    diagnostics: vec![ReadbackDiagnostic {
                        path: None,
                        message: format!(
                            "workspace did not contain a readable skill bank: {error}"
                        ),
                    }],
                });
            }
        };
        if let Err(error) = child.validate() {
            return Ok(ReadbackResult::Invalid {
                diagnostics: vec![ReadbackDiagnostic {
                    path: None,
                    message: format!("parsed skill bank was invalid: {error}"),
                }],
            });
        }
        let Some(change) = SkillBankDiff::diff(&input.artifact, &child) else {
            return Ok(ReadbackResult::Empty);
        };
        if !input
            .part
            .change_matches_selected_part(&change, &input.part_label)
        {
            return Ok(ReadbackResult::Invalid {
                diagnostics: vec![ReadbackDiagnostic {
                    path: None,
                    message: format!(
                        "skill-bank diff changed files outside selected part {}",
                        input.part_label
                    ),
                }],
            });
        }
        if let Err(error) = input.artifact.apply_skill_change(&change) {
            return Ok(ReadbackResult::Invalid {
                diagnostics: vec![ReadbackDiagnostic {
                    path: None,
                    message: format!("skill-bank diff did not apply cleanly: {error}"),
                }],
            });
        }
        Ok(ReadbackResult::Valid(change))
    }
}

#[derive(Debug, Error)]
pub enum SkillBankReflectionError {
    #[error(transparent)]
    Workspace(#[from] WorkspaceError),
    #[error(transparent)]
    Path(#[from] WorkspacePathError),
    #[error("workspace did not contain a valid skill bank")]
    SkillBank(#[source] SkillBankError),
    #[error("workspace skill name was invalid")]
    SkillName(#[source] leaven_artifact_skill::SkillNameError),
    #[error("workspace skill path was invalid")]
    SkillPath(#[source] leaven_artifact_skill::SkillPathError),
}

fn materialize_bank(
    bank: &SkillBank,
    layout: &SkillWorkspaceLayout,
    view: &mut WorkspaceView<'_>,
) -> Result<(), SkillBankReflectionError> {
    for (skill_name, folder) in bank.folders() {
        for (path, file) in folder.entries() {
            let workspace_path = workspace_path(layout, skill_name.as_str(), path.as_str())?;
            view.write_file(&workspace_path, file.bytes())?;
            view.set_executable(&workspace_path, file.permissions().executable)?;
        }
    }
    Ok(())
}

fn read_skill_bank(
    view: &WorkspaceView<'_>,
    layout: &SkillWorkspaceLayout,
) -> Result<SkillBank, SkillBankReflectionError> {
    let root_view;
    let root = if layout.skills_root.as_str().is_empty() {
        view
    } else {
        root_view = view.subdir(layout.skills_root.clone())?;
        &root_view
    };
    let paths = root.list_files(&WorkspacePath::root())?;
    let mut grouped: BTreeMap<SkillName, BTreeMap<SkillPath, SkillFile>> = BTreeMap::new();

    for path in paths {
        let (skill_name, skill_path) = skill_path_from_workspace(&path)?;
        let skill_name =
            SkillName::new(skill_name.to_owned()).map_err(SkillBankReflectionError::SkillName)?;
        let bytes = root.read_file(&path)?;
        let executable = root.is_executable(&path)?;
        grouped.entry(skill_name).or_default().insert(
            skill_path,
            SkillFile::with_permissions(bytes, SkillFilePermissions { executable }),
        );
    }

    grouped
        .into_iter()
        .map(|(name, entries)| SkillFolder::from_entries(name, entries))
        .collect::<Result<Vec<_>, SkillBankError>>()
        .and_then(SkillBank::from_folders)
        .map_err(SkillBankReflectionError::SkillBank)
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

fn skill_path_from_workspace(
    path: &WorkspacePath,
) -> Result<(&str, SkillPath), SkillBankReflectionError> {
    let (skill_name, skill_path) = path.as_str().split_once('/').ok_or_else(|| {
        SkillBankReflectionError::Path(WorkspacePathError::EmptyComponent(path.as_str().to_owned()))
    })?;
    Ok((
        skill_name,
        SkillPath::new(skill_path.to_owned()).map_err(SkillBankReflectionError::SkillPath)?,
    ))
}

pub trait SkillPartScope {
    fn change_matches_selected_part(&self, change: &SkillBankChange, part_label: &str) -> bool;
}

impl SkillPartScope for String {
    fn change_matches_selected_part(&self, change: &SkillBankChange, part_label: &str) -> bool {
        let label = if self.is_empty() {
            part_label
        } else {
            self.as_str()
        };
        let Some((selected_skill, selected_path)) = label.split_once('/') else {
            return change_touches_only_skill(change, label);
        };
        change_touches_only_file(change, selected_skill, selected_path)
    }
}

impl SkillPartScope for SkillFilePartId {
    fn change_matches_selected_part(&self, change: &SkillBankChange, _part_label: &str) -> bool {
        change_touches_only_file(change, self.skill.as_str(), self.path.as_str())
    }
}

fn change_touches_only_skill(change: &SkillBankChange, selected_skill: &str) -> bool {
    match change {
        SkillBankChange::CreateSkill { folder } => folder.name().as_str() == selected_skill,
        SkillBankChange::ReplaceSkill { name, .. } | SkillBankChange::RemoveSkill { name } => {
            name.as_str() == selected_skill
        }
        SkillBankChange::RenameSkill { from, to } => {
            from.as_str() == selected_skill && to.as_str() == selected_skill
        }
        SkillBankChange::WriteFile { skill, .. }
        | SkillBankChange::RemoveFile { skill, .. }
        | SkillBankChange::RenameFile { skill, .. }
        | SkillBankChange::SetExecutable { skill, .. } => skill.as_str() == selected_skill,
        SkillBankChange::Atomic(changes) => changes
            .iter()
            .all(|change| change_touches_only_skill(change, selected_skill)),
    }
}

fn change_touches_only_file(
    change: &SkillBankChange,
    selected_skill: &str,
    selected_path: &str,
) -> bool {
    match change {
        SkillBankChange::WriteFile { skill, path, .. }
        | SkillBankChange::RemoveFile { skill, path }
        | SkillBankChange::SetExecutable { skill, path, .. } => {
            skill.as_str() == selected_skill && path.as_str() == selected_path
        }
        SkillBankChange::RenameFile { skill, from, to } => {
            skill.as_str() == selected_skill
                && from.as_str() == selected_path
                && to.as_str() == selected_path
        }
        SkillBankChange::Atomic(changes) => changes
            .iter()
            .all(|change| change_touches_only_file(change, selected_skill, selected_path)),
        SkillBankChange::CreateSkill { .. }
        | SkillBankChange::ReplaceSkill { .. }
        | SkillBankChange::RemoveSkill { .. }
        | SkillBankChange::RenameSkill { .. } => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selected_part_scope_rejects_renames_that_create_out_of_scope_targets() {
        let selected = SkillName::new("alpha").unwrap();
        let other = SkillName::new("beta").unwrap();
        let selected_path = SkillPath::skill_md();
        let other_path = SkillPath::new("references/other.md").unwrap();

        assert!(!change_touches_only_skill(
            &SkillBankChange::RenameSkill {
                from: selected.clone(),
                to: other,
            },
            selected.as_str(),
        ));
        assert!(!change_touches_only_file(
            &SkillBankChange::RenameFile {
                skill: selected.clone(),
                from: selected_path.clone(),
                to: other_path,
            },
            selected.as_str(),
            selected_path.as_str(),
        ));
        assert!(change_touches_only_file(
            &SkillBankChange::WriteFile {
                skill: selected.clone(),
                path: selected_path.clone(),
                file: SkillFile::text("---\nname: alpha\ndescription: ok\n---\nBody.\n"),
            },
            selected.as_str(),
            selected_path.as_str(),
        ));
    }
}
