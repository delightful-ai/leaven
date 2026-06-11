//! Agentic Git program materialization and readback adapters.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::path::Path;

use leaven_artifact_git::{
    GitObjectId, GitProgramArtifact, GitProgramChange, GitRepoChange, GitRevision, RepoKey,
};
use leaven_core::OptimizationProblem;
use leaven_engine::{MaterializationReport, MaterializeContext, MaterializeError, Materializer};
use leaven_kernel::{Cost, Metered, RunId};
use leaven_workspace::{WorkspacePath, WorkspaceView};
use leaven_workspace_git::GitWorkspaceGitError;

use crate::{
    GitAgenticGitError, GitProgramStores,
    git_ops::{
        BundleImportCleanup, apply_patch_bytes, bundle_head, checked_out_bytes, current_head,
        ensure_expected_parent, ensure_parent_dir, export_commit_bundle, freeze_worktree, host_git,
        materialization_bundle, output_proposal_path, remove_workspace_file, run_git,
        run_git_output, run_git_vec, workspace_path, worktree_differs_from_parent,
    },
};

#[derive(Clone, Debug)]
pub struct GitProgramMaterializer {
    stores: GitProgramStores,
}

impl GitProgramMaterializer {
    #[must_use]
    pub const fn new(stores: GitProgramStores) -> Self {
        Self { stores }
    }

    pub fn materialize_program(
        &self,
        artifact: &GitProgramArtifact,
        workspace: &mut WorkspaceView<'_>,
    ) -> Result<MaterializationReport, GitAgenticGitError> {
        let mut files_written = 0;
        let mut bytes_written = 0;
        for (repo, repo_artifact) in artifact.repos() {
            let layout = artifact
                .layout()
                .path_for(repo)
                .ok_or_else(|| GitAgenticGitError::MissingLayout { repo: repo.clone() })?;
            let checkout = workspace_path(layout)?;
            ensure_parent_dir(workspace, &checkout)?;
            let commit = commit_revision(
                repo,
                repo_artifact.revision(),
                GitRevisionUse::Materialization,
            )?;
            let bundle = materialization_bundle(self.stores.store_for(repo)?, commit)?;
            let bundle_name = format!(".git/leaven-materialize-{}.bundle", RunId::new());

            run_git(workspace, None, ["init", checkout.as_str()])?;
            workspace.write_file(&checkout.join(&bundle_name)?, &bundle)?;
            let materialized_ref = format!("refs/leaven/materialized/{commit}");
            let fetch_refspec = format!("+{commit}:{materialized_ref}");
            run_git_vec(
                workspace,
                Some(&checkout),
                vec![
                    "fetch".to_owned(),
                    "--no-tags".to_owned(),
                    "--no-write-fetch-head".to_owned(),
                    bundle_name.clone(),
                    fetch_refspec,
                ],
            )?;
            remove_workspace_file(workspace, &checkout, &bundle_name)?;
            run_git(
                workspace,
                Some(&checkout),
                ["checkout", "--detach", commit.as_str()],
            )?;

            let tracked = run_git_output(workspace, Some(&checkout), ["ls-files", "-z"])?;
            files_written += tracked
                .split(|byte| *byte == 0)
                .filter(|p| !p.is_empty())
                .count();
            bytes_written += checked_out_bytes(workspace, &checkout)?;
        }
        Ok(MaterializationReport {
            files_written,
            bytes_written,
            truncations: Vec::new(),
        })
    }
}

impl<P> Materializer<P, GitProgramArtifact> for GitProgramMaterializer
where
    P: OptimizationProblem<Artifact = GitProgramArtifact>,
{
    async fn materialize_into(
        &self,
        value: &GitProgramArtifact,
        workspace: &mut WorkspaceView<'_>,
        _ctx: MaterializeContext<'_, P>,
    ) -> Result<Metered<MaterializationReport>, MaterializeError> {
        let report = self
            .materialize_program(value, workspace)
            .map_err(|error| MaterializeError::Message(error.to_string()))?;
        Ok(Metered::new(report, Cost::zero()))
    }
}

#[derive(Clone, Debug)]
pub struct GitProgramReadback {
    stores: GitProgramStores,
}

impl GitProgramReadback {
    #[must_use]
    pub const fn new(stores: GitProgramStores) -> Self {
        Self { stores }
    }

    pub fn read_back_change(
        &self,
        parent: &GitProgramArtifact,
        workspace: &mut WorkspaceView<'_>,
    ) -> Result<Option<GitProgramChange>, GitAgenticGitError> {
        let mut changes = BTreeMap::new();
        let repo_count = parent.repos().len();
        for (repo, repo_artifact) in parent.repos() {
            let checkout = workspace_path(
                parent
                    .layout()
                    .path_for(repo)
                    .ok_or_else(|| GitAgenticGitError::MissingLayout { repo: repo.clone() })?,
            )?;
            let parent_commit =
                commit_revision(repo, repo_artifact.revision(), GitRevisionUse::Readback)?;

            if let Some(imported) =
                self.import_output_proposal(repo, repo_count, &checkout, parent_commit, workspace)?
            {
                changes.insert(
                    repo.clone(),
                    GitRepoChange::AdvanceTo {
                        expected_parent: repo_artifact.revision().clone(),
                        child: imported,
                    },
                );
                continue;
            }

            if !worktree_differs_from_parent(workspace, &checkout)? {
                let head = current_head(workspace, &checkout)?;
                if &head == parent_commit {
                    continue;
                }
                let imported =
                    self.import_child(repo, &checkout, &head, parent_commit, workspace)?;
                changes.insert(
                    repo.clone(),
                    GitRepoChange::AdvanceTo {
                        expected_parent: repo_artifact.revision().clone(),
                        child: imported,
                    },
                );
                continue;
            }

            freeze_worktree(workspace, &checkout)?;
            let child = current_head(workspace, &checkout)?;
            let imported = self.import_child(repo, &checkout, &child, parent_commit, workspace)?;
            changes.insert(
                repo.clone(),
                GitRepoChange::AdvanceTo {
                    expected_parent: repo_artifact.revision().clone(),
                    child: imported,
                },
            );
        }

        if changes.is_empty() {
            return Ok(None);
        }
        if changes.len() == 1 {
            let (repo, change) = changes.into_iter().next().expect("length checked above");
            let GitRepoChange::AdvanceTo {
                expected_parent,
                child,
            } = change;
            return Ok(Some(GitProgramChange::AdvanceRepo {
                repo,
                expected_parent,
                child,
            }));
        }
        Ok(Some(GitProgramChange::AdvanceRepos {
            repo_changes: changes,
        }))
    }

    fn import_output_proposal(
        &self,
        repo: &RepoKey,
        repo_count: usize,
        checkout: &WorkspacePath,
        parent: &GitObjectId,
        workspace: &mut WorkspaceView<'_>,
    ) -> Result<Option<GitRevision>, GitAgenticGitError> {
        if let Some(bundle) = output_proposal_path(workspace, repo, repo_count, "bundle")? {
            let bytes = workspace.read_file(&bundle)?;
            return self.import_bundle_bytes(repo, &bytes, parent).map(Some);
        }
        if let Some(patch) = output_proposal_path(workspace, repo, repo_count, "patch")? {
            let patch = workspace.read_file(&patch)?;
            run_git(
                workspace,
                Some(checkout),
                ["reset", "--hard", parent.as_str()],
            )?;
            run_git(workspace, Some(checkout), ["clean", "-fd"])?;
            apply_patch_bytes(workspace, checkout, &patch)?;
            freeze_worktree(workspace, checkout)?;
            let child = current_head(workspace, checkout)?;
            return self
                .import_child(repo, checkout, &child, parent, workspace)
                .map(Some);
        }
        Ok(None)
    }

    fn import_child(
        &self,
        repo: &RepoKey,
        checkout: &WorkspacePath,
        child: &GitObjectId,
        parent: &GitObjectId,
        workspace: &mut WorkspaceView<'_>,
    ) -> Result<GitRevision, GitAgenticGitError> {
        let bundle = export_commit_bundle(workspace, checkout, parent, child)?;
        self.import_bundle_bytes(repo, &bundle, parent)
    }

    fn import_bundle(
        &self,
        repo: &RepoKey,
        bundle: &Path,
        parent: &GitObjectId,
    ) -> Result<GitRevision, GitAgenticGitError> {
        let child = bundle_head(bundle)?;
        let durable = self.stores.store_for(repo)?;
        let temp = std::env::temp_dir().join(format!("leaven-git-bundle-{}", RunId::new()));
        host_git(
            None,
            "git init --bare",
            vec![
                OsString::from("init"),
                OsString::from("--bare"),
                temp.as_os_str().to_os_string(),
            ],
        )?;
        let cleanup = BundleImportCleanup { path: temp.clone() };
        host_git(
            Some(&temp),
            "git fetch bundle parent",
            vec![
                OsString::from("fetch"),
                durable.as_os_str().to_os_string(),
                OsString::from(format!("+{parent}:refs/leaven/parents/{parent}")),
            ],
        )?;
        host_git(
            Some(&temp),
            "git bundle verify",
            vec![
                OsString::from("bundle"),
                OsString::from("verify"),
                bundle.as_os_str().to_os_string(),
            ],
        )?;
        host_git(
            Some(&temp),
            "git fetch bundle",
            vec![
                OsString::from("fetch"),
                bundle.as_os_str().to_os_string(),
                OsString::from(format!("+{child}:refs/leaven/proposals/{child}")),
            ],
        )?;
        host_git(
            Some(&temp),
            "git fsck",
            vec![OsString::from("fsck"), OsString::from("--strict")],
        )?;
        ensure_expected_parent(&temp, &child, parent)?;

        host_git(
            Some(durable),
            "git fetch bundle proposal",
            vec![
                OsString::from("fetch"),
                temp.as_os_str().to_os_string(),
                OsString::from(format!("+{child}:refs/leaven/imported/{child}")),
            ],
        )?;
        host_git(
            Some(durable),
            "git fsck",
            vec![OsString::from("fsck"), OsString::from("--strict")],
        )?;
        cleanup.remove();
        Ok(GitRevision::Commit(child))
    }

    fn import_bundle_bytes(
        &self,
        repo: &RepoKey,
        bundle: &[u8],
        parent: &GitObjectId,
    ) -> Result<GitRevision, GitAgenticGitError> {
        let temp = std::env::temp_dir().join(format!("leaven-git-proposal-{}", RunId::new()));
        fs::create_dir_all(&temp).map_err(|source| GitWorkspaceGitError::CommandIo {
            program: "create temp bundle directory",
            source,
        })?;
        let cleanup = BundleImportCleanup { path: temp.clone() };
        let bundle_path = temp.join("proposal.bundle");
        fs::write(&bundle_path, bundle).map_err(|source| GitWorkspaceGitError::CommandIo {
            program: "write temp bundle",
            source,
        })?;
        let imported = self.import_bundle(repo, &bundle_path, parent)?;
        cleanup.remove();
        Ok(imported)
    }
}

#[derive(Clone, Copy, Debug)]
enum GitRevisionUse {
    Materialization,
    Readback,
}

fn commit_revision<'a>(
    repo: &RepoKey,
    revision: &'a GitRevision,
    usage: GitRevisionUse,
) -> Result<&'a GitObjectId, GitAgenticGitError> {
    match revision {
        GitRevision::Commit(commit) => Ok(commit),
        GitRevision::Tree(_) => match usage {
            GitRevisionUse::Materialization => {
                Err(GitAgenticGitError::NonCommitMaterialization { repo: repo.clone() })
            }
            GitRevisionUse::Readback => {
                Err(GitAgenticGitError::NonCommitReadback { repo: repo.clone() })
            }
        },
    }
}
