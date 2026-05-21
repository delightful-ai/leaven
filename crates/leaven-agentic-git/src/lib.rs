//! Agentic Git program materialization and readback adapters.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use leaven_artifact_git::{
    GitArtifactError, GitObjectId, GitPath, GitProgramArtifact, GitProgramChange, GitRepoChange,
    GitRevision, RepoKey,
};
use leaven_core::OptimizationProblem;
use leaven_engine::{MaterializationReport, MaterializeContext, MaterializeError, Materializer};
use leaven_kernel::{Cost, Metered, RunId};
use leaven_workspace::{Command, CommandOutput, WorkspacePath, WorkspacePathError, WorkspaceView};
use leaven_workspace_git::GitWorkspaceGitError;

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
        let repo_count = parent.repos().len();
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

#[derive(Debug, thiserror::Error)]
pub enum GitAgenticGitError {
    #[error("git program store map is empty")]
    MissingStores,
    #[error("missing durable git store for repo `{repo}`")]
    MissingStore { repo: RepoKey },
    #[error("missing git program layout for repo `{repo}`")]
    MissingLayout { repo: RepoKey },
    #[error("tree materialization is not implemented for repo `{repo}`")]
    UnsupportedTreeMaterialization { repo: RepoKey },
    #[error("tree readback is not implemented for repo `{repo}`")]
    UnsupportedTreeReadback { repo: RepoKey },
    #[error("git bundle `{path}` does not contain a head")]
    EmptyBundle { path: String },
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

fn run_git_vec(
    workspace: &mut WorkspaceView<'_>,
    cwd: Option<&WorkspacePath>,
    args: Vec<String>,
) -> Result<(), GitAgenticGitError> {
    let output = run_git_command_vec(workspace, cwd, args)?;
    ensure_success(&output, "git")
}

fn run_git_command<const N: usize>(
    workspace: &mut WorkspaceView<'_>,
    cwd: Option<&WorkspacePath>,
    args: [&str; N],
) -> Result<CommandOutput, GitAgenticGitError> {
    run_git_command_vec(
        workspace,
        cwd,
        args.into_iter().map(str::to_owned).collect(),
    )
}

fn run_git_command_vec(
    workspace: &mut WorkspaceView<'_>,
    cwd: Option<&WorkspacePath>,
    args: Vec<String>,
) -> Result<CommandOutput, GitAgenticGitError> {
    let mut command = Command::new("git");
    command.cwd = cwd.cloned();
    command.args = args;
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

fn apply_patch_bytes(
    workspace: &mut WorkspaceView<'_>,
    checkout: &WorkspacePath,
    patch: &[u8],
) -> Result<(), GitAgenticGitError> {
    let filename = format!(".git/leaven-output-{}.patch", RunId::new());
    workspace.write_file(&checkout.join(&filename)?, patch)?;
    run_git_vec(
        workspace,
        Some(checkout),
        vec!["apply".to_owned(), "--binary".to_owned(), filename.clone()],
    )?;
    remove_workspace_file(workspace, checkout, &filename)
}

fn export_commit_bundle(
    workspace: &mut WorkspaceView<'_>,
    checkout: &WorkspacePath,
    parent: &GitObjectId,
    _child: &GitObjectId,
) -> Result<Vec<u8>, GitAgenticGitError> {
    let filename = format!(".git/leaven-readback-{}.bundle", RunId::new());
    let excluded_parent = format!("^{parent}");
    run_git_vec(
        workspace,
        Some(checkout),
        vec![
            "bundle".to_owned(),
            "create".to_owned(),
            filename.clone(),
            "HEAD".to_owned(),
            excluded_parent,
        ],
    )?;
    let bundle = workspace.read_file(&checkout.join(&filename)?)?;
    remove_workspace_file(workspace, checkout, &filename)?;
    Ok(bundle)
}

fn output_proposal_path(
    workspace: &mut WorkspaceView<'_>,
    repo: &RepoKey,
    repo_count: usize,
    extension: &str,
) -> Result<Option<WorkspacePath>, GitAgenticGitError> {
    let output = WorkspacePath::new("output")?;
    let repo_specific = output.join(format!("{}.{}", repo.as_str(), extension))?;
    if workspace_file_exists(workspace, &repo_specific)? {
        return Ok(Some(repo_specific));
    }
    if repo_count == 1 {
        let generic = output.join(format!("proposal.{extension}"))?;
        if workspace_file_exists(workspace, &generic)? {
            return Ok(Some(generic));
        }
    }
    Ok(None)
}

fn workspace_file_exists(
    workspace: &mut WorkspaceView<'_>,
    path: &WorkspacePath,
) -> Result<bool, GitAgenticGitError> {
    let mut command = Command::new("test");
    command.args = vec!["-f".to_owned(), path.as_str().to_owned()];
    let output = workspace.run_command(command)?;
    Ok(output.status.code == Some(0))
}

fn remove_workspace_file(
    workspace: &mut WorkspaceView<'_>,
    cwd: &WorkspacePath,
    path: &str,
) -> Result<(), GitAgenticGitError> {
    let mut command = Command::new("rm");
    command.cwd = Some(cwd.clone());
    command.args = vec!["-f".to_owned(), path.to_owned()];
    ensure_success(&workspace.run_command(command)?, "rm")
}

fn bundle_head(bundle: &Path) -> Result<GitObjectId, GitAgenticGitError> {
    let output = host_git(
        None,
        "git bundle list-heads",
        vec![
            OsString::from("bundle"),
            OsString::from("list-heads"),
            bundle.as_os_str().to_os_string(),
        ],
    )?;
    let text = String::from_utf8(output)?;
    let head = text
        .split_whitespace()
        .next()
        .ok_or_else(|| GitAgenticGitError::EmptyBundle {
            path: bundle.display().to_string(),
        })?;
    Ok(GitObjectId::new(head)?)
}

fn ensure_expected_parent(
    repo: &Path,
    child: &GitObjectId,
    parent: &GitObjectId,
) -> Result<(), GitAgenticGitError> {
    let output = host_git(
        Some(repo),
        "git rev-list parent check",
        vec![
            OsString::from("rev-list"),
            OsString::from("--parents"),
            OsString::from("-n"),
            OsString::from("1"),
            OsString::from(child.as_str()),
        ],
    )?;
    let text = String::from_utf8(output)?;
    if text
        .split_whitespace()
        .skip(1)
        .any(|p| p == parent.as_str())
    {
        return Ok(());
    }
    Err(GitAgenticGitError::Git(
        GitWorkspaceGitError::UnexpectedParent {
            commit: child.clone(),
            expected_parent: parent.clone(),
        },
    ))
}

fn host_git(
    cwd: Option<&Path>,
    program: &'static str,
    args: Vec<OsString>,
) -> Result<Vec<u8>, GitAgenticGitError> {
    let mut command = std::process::Command::new("git");
    command.args(args);
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    let output = command
        .output()
        .map_err(|source| GitWorkspaceGitError::CommandIo { program, source })?;
    if output.status.success() {
        return Ok(output.stdout);
    }
    Err(GitAgenticGitError::Git(GitWorkspaceGitError::Command {
        program,
        status: output.status.code(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }))
}

struct BundleImportCleanup {
    path: PathBuf,
}

impl BundleImportCleanup {
    fn remove(self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

impl Drop for BundleImportCleanup {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}
