use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use leaven_artifact_git::{GitObjectId, GitPath, RepoKey};
use leaven_kernel::RunId;
use leaven_workspace::{Command, CommandOutput, WorkspacePath, WorkspacePathError, WorkspaceView};
use leaven_workspace_git::GitWorkspaceGitError;

use crate::GitAgenticGitError;

pub fn workspace_path(path: &GitPath) -> Result<WorkspacePath, WorkspacePathError> {
    WorkspacePath::new(path.as_str())
}

pub fn ensure_parent_dir(
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

pub fn run_git<const N: usize>(
    workspace: &mut WorkspaceView<'_>,
    cwd: Option<&WorkspacePath>,
    args: [&str; N],
) -> Result<(), GitAgenticGitError> {
    let output = run_git_command(workspace, cwd, args)?;
    ensure_success(&output, "git")
}

pub fn run_git_output<const N: usize>(
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

pub fn run_git_vec(
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

pub fn current_head(
    workspace: &mut WorkspaceView<'_>,
    checkout: &WorkspacePath,
) -> Result<GitObjectId, GitAgenticGitError> {
    let output = run_git_output(workspace, Some(checkout), ["rev-parse", "HEAD"])?;
    Ok(GitObjectId::new(String::from_utf8(output)?.trim())?)
}

pub fn repo_dirty(
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

pub fn freeze_worktree(
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

pub fn checked_out_bytes(
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

pub fn apply_patch_bytes(
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

pub fn export_commit_bundle(
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

pub fn output_proposal_path(
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

pub fn remove_workspace_file(
    workspace: &mut WorkspaceView<'_>,
    cwd: &WorkspacePath,
    path: &str,
) -> Result<(), GitAgenticGitError> {
    let mut command = Command::new("rm");
    command.cwd = Some(cwd.clone());
    command.args = vec!["-f".to_owned(), path.to_owned()];
    ensure_success(&workspace.run_command(command)?, "rm")
}

pub fn materialization_bundle(
    durable: &Path,
    commit: &GitObjectId,
) -> Result<Vec<u8>, GitAgenticGitError> {
    let temp = std::env::temp_dir().join(format!("leaven-git-materialize-{}", RunId::new()));
    fs::create_dir_all(&temp).map_err(|source| GitWorkspaceGitError::CommandIo {
        program: "create materialization bundle directory",
        source,
    })?;
    let cleanup = BundleImportCleanup { path: temp.clone() };
    let bundle_path = temp.join("materialization.bundle");
    let temp_ref = format!("refs/leaven/materialize/{}", RunId::new());
    host_git(
        Some(durable),
        "git update-ref materialization",
        vec![
            OsString::from("update-ref"),
            OsString::from(&temp_ref),
            OsString::from(commit.as_str()),
        ],
    )?;
    let bundle_result = host_git(
        Some(durable),
        "git bundle create materialization",
        vec![
            OsString::from("bundle"),
            OsString::from("create"),
            bundle_path.as_os_str().to_os_string(),
            OsString::from(&temp_ref),
        ],
    );
    host_git(
        Some(durable),
        "git delete materialization ref",
        vec![
            OsString::from("update-ref"),
            OsString::from("-d"),
            OsString::from(&temp_ref),
        ],
    )?;
    bundle_result?;
    let bundle = fs::read(&bundle_path).map_err(|source| GitWorkspaceGitError::CommandIo {
        program: "read materialization bundle",
        source,
    })?;
    cleanup.remove();
    Ok(bundle)
}

pub fn bundle_head(bundle: &Path) -> Result<GitObjectId, GitAgenticGitError> {
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

pub fn ensure_expected_parent(
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

pub fn host_git(
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

pub struct BundleImportCleanup {
    pub path: PathBuf,
}

impl BundleImportCleanup {
    pub fn remove(self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

impl Drop for BundleImportCleanup {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}
