use leaven_artifact_git::{GitPath, GitProgramArtifact, GitProgramChange, RepoKey};
use leaven_kernel::Fingerprint;
use leaven_surface::{EditSurface, Part, SurfaceError, SurfaceFingerprint};

/// Edit surface over a [`GitProgramArtifact`] that exposes each program repo as
/// one selectable part.
///
/// This is the surface the agentic Git reflection path runs over. GEPA selects a
/// repo part through it (the part id is the [`RepoKey`], the address is the
/// repo's materialization layout [`GitPath`]); the agentic reflector then
/// materializes the whole program, lets the agent edit files in place, and reads
/// a typed [`GitProgramChange`] back. The change does not flow through
/// [`change_part`] in that path, but a programmatic caller that already holds a
/// typed change can lower it through this surface, so the surface is
/// behavior-bearing rather than a placeholder.
#[derive(Clone, Copy, Debug, Default)]
pub struct GitProgramPathSurface;

impl EditSurface<GitProgramArtifact> for GitProgramPathSurface {
    type PartId = RepoKey;
    type Address = GitPath;
    type View<'a> = &'a str;
    type Edit = GitProgramChange;

    fn fingerprint(&self) -> SurfaceFingerprint {
        SurfaceFingerprint(Fingerprint::from_bytes([7; 32]))
    }

    fn parts<'a>(
        &self,
        artifact: &'a GitProgramArtifact,
    ) -> Result<Vec<Part<Self::PartId, Self::Address, Self::View<'a>>>, SurfaceError> {
        artifact
            .repos()
            .keys()
            .map(|repo| {
                let layout = artifact.layout().path_for(repo).ok_or_else(|| {
                    SurfaceError::Message(format!(
                        "git program repo `{repo}` has no materialization layout"
                    ))
                })?;
                Ok(Part {
                    id: repo.clone(),
                    address: layout.clone(),
                    view: layout.as_str(),
                })
            })
            .collect()
    }

    fn change_part(
        &self,
        artifact: &GitProgramArtifact,
        id: Self::PartId,
        edit: Self::Edit,
    ) -> Result<GitProgramChange, SurfaceError> {
        if artifact.repo(&id).is_none() {
            return Err(SurfaceError::UnknownPart);
        }
        match &edit {
            GitProgramChange::AdvanceRepo { repo, .. } if repo == &id => Ok(edit),
            GitProgramChange::AdvanceRepos { repo_changes } if repo_changes.contains_key(&id) => {
                Ok(edit)
            }
            _ => Err(SurfaceError::Message(format!(
                "git program change does not target the selected repo `{id}`"
            ))),
        }
    }
}
