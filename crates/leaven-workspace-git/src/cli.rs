use std::ffi::OsString;
use std::path::Path;

use crate::GitWorkspaceGitError;

pub fn run_git(
    cwd: Option<&Path>,
    program: &'static str,
    args: Vec<OsString>,
) -> Result<Vec<u8>, GitWorkspaceGitError> {
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
    Err(GitWorkspaceGitError::Command {
        program,
        status: output.status.code(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}

pub fn run_git_fsck(repository: &Path) -> Result<(), GitWorkspaceGitError> {
    match run_git(
        Some(repository),
        "git fsck",
        vec![OsString::from("fsck"), OsString::from("--strict")],
    ) {
        Ok(_) => Ok(()),
        Err(GitWorkspaceGitError::Command { stderr, .. }) => Err(GitWorkspaceGitError::Fsck {
            repository: repository.display().to_string(),
            stderr,
        }),
        Err(error) => Err(error),
    }
}
