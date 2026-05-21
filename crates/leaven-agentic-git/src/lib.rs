//! Agentic Git program materialization and readback adapters.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use leaven_artifact_git::{
    GitArtifactError, GitObjectId, GitPath, GitProgramArtifact, GitProgramChange, GitRepoChange,
    GitRevision, RepoKey,
};
use leaven_core::OptimizationProblem;
use leaven_engine::{MaterializationReport, MaterializeContext, MaterializeError, Materializer};
use leaven_kernel::{Cost, Metered};
use leaven_workspace::{Command, CommandOutput, WorkspacePath, WorkspacePathError, WorkspaceView};
use leaven_workspace_git::{GitCommitImportRequest, GitCommitImporter, GitWorkspaceGitError};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitProgramStores {
    stores: BTreeMap<RepoKey, PathBuf>,
}

impl GitProgramStores {
    pub fn new(stores: BTreeMap<RepoKey, PathBuf>) -> Result<Self, GitAgenticGitError> {
        if stores.is_empty() {
            return Err(GitAgenticGitError::MissingStores);
        }
        Ok(Self { stores })
    }

    fn store_for(&self, repo: &RepoKey) -> Result<&Path, GitAgenticGitError> {
        self.stores
            .get(repo)
            .map(PathBuf::as_path)
            .ok_or_else(|| GitAgenticGitError::MissingStore { repo: repo.clone() })
    }
}

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

            run_git(workspace, None, ["init", checkout.as_str()])?;
            run_git(
                workspace,
                Some(&checkout),
                [
                    "remote",
                    "add",
                    "origin",
                    self.stores.store_for(repo)?.to_string_lossy().as_ref(),
                ],
            )?;
            run_git(
                workspace,
                Some(&checkout),
                [
                    "fetch",
                    "--no-tags",
                    "--no-write-fetch-head",
                    "origin",
                    repo_artifact.revision().object_id().as_str(),
                ],
            )?;
            match repo_artifact.revision() {
                GitRevision::Commit(commit) => {
                    run_git(
                        workspace,
                        Some(&checkout),
                        ["checkout", "--detach", commit.as_str()],
                    )?;
                }
                GitRevision::Tree(_) => {
                    return Err(GitAgenticGitError::UnsupportedTreeMaterialization {
                        repo: repo.clone(),
                    });
                }
            }

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
        for (repo, repo_artifact) in parent.repos() {
            let checkout = workspace_path(
                parent
                    .layout()
                    .path_for(repo)
                    .ok_or_else(|| GitAgenticGitError::MissingLayout { repo: repo.clone() })?,
            )?;
            let parent_commit = match repo_artifact.revision() {
                GitRevision::Commit(commit) => commit,
                GitRevision::Tree(_) => {
                    return Err(GitAgenticGitError::UnsupportedTreeReadback { repo: repo.clone() });
                }
            };

            if !repo_dirty(workspace, &checkout)? {
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

    fn import_child(
        &self,
        repo: &RepoKey,
        checkout: &WorkspacePath,
        child: &GitObjectId,
        parent: &GitObjectId,
        workspace: &WorkspaceView<'_>,
    ) -> Result<GitRevision, GitAgenticGitError> {
        let source = checkout_host_path(workspace, checkout)?;
        let imported = GitCommitImporter::import_commit(GitCommitImportRequest {
            source,
            durable_store: self.stores.store_for(repo)?.to_path_buf(),
            commit: child.clone(),
            expected_parent: parent.clone(),
        })?;
        Ok(imported.revision().clone())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum GitAgenticGitError {
    #[error("git program store map is empty")]
    MissingStores,
    #[error("missing durable git store for repo `{repo}`")]
    MissingStore { repo: RepoKey },
    #[error("missing git program layout for repo `{repo}`")]
    MissingLayout { repo: RepoKey },
    #[error("workspace does not expose a local mount required for git readback import")]
    MissingLocalMount,
    #[error("tree materialization is not implemented for repo `{repo}`")]
    UnsupportedTreeMaterialization { repo: RepoKey },
    #[error("tree readback is not implemented for repo `{repo}`")]
    UnsupportedTreeReadback { repo: RepoKey },
    #[error(transparent)]
    Workspace(#[from] leaven_workspace::WorkspaceError),
    #[error(transparent)]
    WorkspacePath(#[from] WorkspacePathError),
    #[error(transparent)]
    Git(#[from] GitWorkspaceGitError),
    #[error(transparent)]
    GitArtifact(#[from] GitArtifactError),
    #[error(transparent)]
    Utf8(#[from] std::string::FromUtf8Error),
}

fn workspace_path(path: &GitPath) -> Result<WorkspacePath, WorkspacePathError> {
    WorkspacePath::new(path.as_str())
}

fn ensure_parent_dir(
    workspace: &mut WorkspaceView<'_>,
    path: &WorkspacePath,
) -> Result<(), GitAgenticGitError> {
    let Some((parent, _)) = path.as_str().rsplit_once('/') else {
        return Ok(());
    };
    let mut command = Command::new("mkdir");
    command.args = vec!["-p".to_owned(), parent.to_owned()];
    ensure_success(&workspace.run_command(command)?, "mkdir -p")
}

fn run_git<const N: usize>(
    workspace: &mut WorkspaceView<'_>,
    cwd: Option<&WorkspacePath>,
    args: [&str; N],
) -> Result<(), GitAgenticGitError> {
    let output = run_git_command(workspace, cwd, args)?;
    ensure_success(&output, "git")
}

fn run_git_output<const N: usize>(
    workspace: &mut WorkspaceView<'_>,
    cwd: Option<&WorkspacePath>,
    args: [&str; N],
) -> Result<Vec<u8>, GitAgenticGitError> {
    let output = run_git_command(workspace, cwd, args)?;
    if output.status.code == Some(0) {
        return Ok(output.stdout.bytes);
    }
    Err(GitAgenticGitError::Git(GitWorkspaceGitError::Command {
        program: "git",
        status: output.status.code,
        stderr: String::from_utf8_lossy(&output.stderr.bytes).into_owned(),
    }))
}

fn run_git_command<const N: usize>(
    workspace: &mut WorkspaceView<'_>,
    cwd: Option<&WorkspacePath>,
    args: [&str; N],
) -> Result<CommandOutput, GitAgenticGitError> {
    let mut command = Command::new("git");
    command.cwd = cwd.cloned();
    command.args = args.into_iter().map(str::to_owned).collect();
    Ok(workspace.run_command(command)?)
}

fn ensure_success(output: &CommandOutput, program: &'static str) -> Result<(), GitAgenticGitError> {
    if output.status.code == Some(0) {
        return Ok(());
    }
    Err(GitAgenticGitError::Git(GitWorkspaceGitError::Command {
        program,
        status: output.status.code,
        stderr: String::from_utf8_lossy(&output.stderr.bytes).into_owned(),
    }))
}

fn current_head(
    workspace: &mut WorkspaceView<'_>,
    checkout: &WorkspacePath,
) -> Result<GitObjectId, GitAgenticGitError> {
    let output = run_git_output(workspace, Some(checkout), ["rev-parse", "HEAD"])?;
    Ok(GitObjectId::new(String::from_utf8(output)?.trim())?)
}

fn repo_dirty(
    workspace: &mut WorkspaceView<'_>,
    checkout: &WorkspacePath,
) -> Result<bool, GitAgenticGitError> {
    Ok(!run_git_output(
        workspace,
        Some(checkout),
        ["status", "--porcelain=v1", "-z"],
    )?
    .is_empty())
}

fn freeze_worktree(
    workspace: &mut WorkspaceView<'_>,
    checkout: &WorkspacePath,
) -> Result<(), GitAgenticGitError> {
    run_git(workspace, Some(checkout), ["add", "-A"])?;
    run_git(
        workspace,
        Some(checkout),
        [
            "-c",
            "user.name=Leaven",
            "-c",
            "user.email=leaven@example.invalid",
            "commit",
            "-m",
            "leaven workspace snapshot",
        ],
    )
}

fn checked_out_bytes(
    workspace: &WorkspaceView<'_>,
    checkout: &WorkspacePath,
) -> Result<u64, GitAgenticGitError> {
    let files = workspace.list_files(checkout)?;
    let mut total = 0_u64;
    for file in files {
        if file.as_str().contains("/.git/") || file.as_str().ends_with("/.git") {
            continue;
        }
        total += u64::try_from(workspace.read_file(&file)?.len())
            .expect("usize fits into u64 on supported Leaven targets");
    }
    Ok(total)
}

fn checkout_host_path(
    workspace: &WorkspaceView<'_>,
    checkout: &WorkspacePath,
) -> Result<PathBuf, GitAgenticGitError> {
    let mount = workspace
        .local_mount()
        .ok_or(GitAgenticGitError::MissingLocalMount)?;
    Ok(mount
        .join(workspace.root().to_host_relative())
        .join(checkout.to_host_relative()))
}
