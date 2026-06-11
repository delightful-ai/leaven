use std::ffi::{OsStr, OsString};
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
    run_git_command_env_vec(workspace, cwd, &[], args)
}

fn run_git_command_env_vec(
    workspace: &mut WorkspaceView<'_>,
    cwd: Option<&WorkspacePath>,
    env: &[(String, String)],
    args: Vec<String>,
) -> Result<CommandOutput, GitAgenticGitError> {
    let mut command = Command::new("git");
    command.cwd = cwd.cloned();
    command.args = args;
    command.env = env.iter().cloned().collect();
    Ok(workspace.run_command(command)?)
}

fn run_git_env<const N: usize>(
    workspace: &mut WorkspaceView<'_>,
    cwd: Option<&WorkspacePath>,
    env: &[(String, String)],
    args: [&str; N],
) -> Result<(), GitAgenticGitError> {
    let output = run_git_command_env_vec(
        workspace,
        cwd,
        env,
        args.into_iter().map(str::to_owned).collect(),
    )?;
    ensure_success(&output, "git")
}

fn run_git_output_env<const N: usize>(
    workspace: &mut WorkspaceView<'_>,
    cwd: Option<&WorkspacePath>,
    env: &[(String, String)],
    args: [&str; N],
) -> Result<Vec<u8>, GitAgenticGitError> {
    let output = run_git_command_env_vec(
        workspace,
        cwd,
        env,
        args.into_iter().map(str::to_owned).collect(),
    )?;
    if output.status.code == Some(0) {
        return Ok(output.stdout.bytes);
    }
    Err(GitAgenticGitError::Git(GitWorkspaceGitError::Command {
        program: "git",
        status: output.status.code,
        stderr: String::from_utf8_lossy(&output.stderr.bytes).into_owned(),
    }))
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

/// Reports whether the checked-out worktree differs in content from the parent
/// commit it was materialized at.
///
/// The decision is content-truthful, not stat-cache-truthful. A freshly
/// materialized checkout writes its index, then a foreign process (the agent)
/// rewrites a tracked file in place. A stat-based check (`git status`,
/// `git diff-index`, even `git update-index --refresh`) can trust the cached
/// stat and mis-read a real edit as clean when the post-write stat collides
/// with the index entry — silently dropping the agent's edit as "no changes".
/// This is configuration-dependent (for example `core.checkStat=minimal` or an
/// active `core.fsmonitor` widens the window) and racy under load.
///
/// To remove that dependence, this re-hashes the whole worktree into a scratch
/// index that holds no stat cache, then compares the resulting tree object id
/// against the parent commit's tree. Two trees are equal exactly when the
/// worktree content matches the parent content, regardless of file mtime, the
/// stat cache, or `core.checkStat`. The scratch index leaves the checkout's own
/// index untouched.
pub fn worktree_differs_from_parent(
    workspace: &mut WorkspaceView<'_>,
    checkout: &WorkspacePath,
) -> Result<bool, GitAgenticGitError> {
    let parent_tree = run_git_output(workspace, Some(checkout), ["rev-parse", "HEAD^{tree}"])?;
    let parent_tree = String::from_utf8(parent_tree)?.trim().to_owned();
    let worktree_tree = worktree_tree(workspace, checkout)?;
    Ok(worktree_tree != parent_tree)
}

/// Hashes the entire worktree into a scratch index and returns its tree object
/// id.
///
/// A unique `GIT_INDEX_FILE` under `.git` keeps the checkout's own index
/// untouched. `read-tree HEAD` seeds the scratch index from the parent so
/// removals are detected, then `add -A` re-reads and re-hashes every path's
/// content (no stat-cache shortcut applies to a fresh index), and `write-tree`
/// records the worktree's content tree. The scratch index file is removed
/// afterward.
fn worktree_tree(
    workspace: &mut WorkspaceView<'_>,
    checkout: &WorkspacePath,
) -> Result<String, GitAgenticGitError> {
    let scratch = format!(".git/leaven-dirty-{}.index", RunId::new());
    let scratch_env = vec![("GIT_INDEX_FILE".to_owned(), scratch.clone())];
    let result = (|| {
        run_git_env(
            workspace,
            Some(checkout),
            &scratch_env,
            ["read-tree", "HEAD"],
        )?;
        run_git_env(workspace, Some(checkout), &scratch_env, ["add", "-A"])?;
        let tree = run_git_output_env(workspace, Some(checkout), &scratch_env, ["write-tree"])?;
        Ok(String::from_utf8(tree)?.trim().to_owned())
    })();
    remove_workspace_file(workspace, checkout, &scratch)?;
    result
}

pub fn freeze_worktree(
    workspace: &mut WorkspaceView<'_>,
    checkout: &WorkspacePath,
) -> Result<(), GitAgenticGitError> {
    // Stage content, not stat. A plain `git add -A` trusts the index stat cache
    // for tracked paths, so an agent edit whose post-write stat collides with
    // the entry would be staged as the *old* content and the frozen child would
    // silently carry the seed rather than the edit. `add --renormalize -A`
    // re-reads and re-hashes every tracked path's content (defeating the stat
    // collision) and the following `add -A` stages new and removed paths, so the
    // snapshot commit always reflects the worktree content.
    run_git(workspace, Some(checkout), ["add", "--renormalize", "-A"])?;
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
    host_git_with(cwd, program, args, &[], None)
}

/// Runs a host `git` invocation with explicit environment overrides and
/// optional stdin bytes.
///
/// The seed builder uses this to pin author/committer identity and dates (so
/// identical kit content yields an identical commit id) and to feed blob and
/// tree content to plumbing commands over stdin.
pub fn host_git_with(
    cwd: Option<&Path>,
    program: &'static str,
    args: Vec<OsString>,
    env: &[(&str, &OsStr)],
    stdin: Option<&[u8]>,
) -> Result<Vec<u8>, GitAgenticGitError> {
    use std::io::Write as _;
    use std::process::Stdio;

    let mut command = std::process::Command::new("git");
    command.args(args);
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    for (key, value) in env {
        command.env(key, value);
    }
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    command.stdin(if stdin.is_some() {
        Stdio::piped()
    } else {
        Stdio::null()
    });
    let mut child = command
        .spawn()
        .map_err(|source| GitWorkspaceGitError::CommandIo { program, source })?;
    if let Some(bytes) = stdin
        && let Some(mut handle) = child.stdin.take()
    {
        handle
            .write_all(bytes)
            .map_err(|source| GitWorkspaceGitError::CommandIo { program, source })?;
    }
    let output = child
        .wait_with_output()
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
