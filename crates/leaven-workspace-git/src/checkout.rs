use std::collections::BTreeMap;
use std::path::Path;

use leaven_artifact_git::{
    GitArtifact, GitObjectId, GitPath, GitRef, GitRefKey, GitRefKind, GitRefName, GitRefTarget,
};

use crate::GitWorkspaceGitError;

pub struct GitCheckout;

impl GitCheckout {
    pub fn capture(root: &Path) -> Result<GitArtifact, GitWorkspaceGitError> {
        let files = capture_files(root)?;
        let refs = capture_refs(root)?;
        Ok(GitArtifact::from_parts(files, refs))
    }

    pub fn restore_ref(root: &Path, key: &GitRefKey) -> Result<(), GitWorkspaceGitError> {
        let target = match key.kind() {
            GitRefKind::Branch => format!("refs/heads/{}", key.name()),
            GitRefKind::Tag => format!("refs/tags/{}", key.name()),
        };
        let _ = run_git(root, "git checkout", ["checkout", "--quiet", &target])?;
        Ok(())
    }

    pub fn delete_ref(root: &Path, key: &GitRefKey) -> Result<(), GitWorkspaceGitError> {
        match key.kind() {
            GitRefKind::Branch => {
                let _ = run_git(root, "git branch -D", ["branch", "-D", key.name().as_str()])?;
            }
            GitRefKind::Tag => {
                let _ = run_git(root, "git tag -d", ["tag", "-d", key.name().as_str()])?;
            }
        }
        Ok(())
    }
}

fn capture_files(root: &Path) -> Result<BTreeMap<GitPath, Vec<u8>>, GitWorkspaceGitError> {
    let output = run_git(root, "git ls-files", ["ls-files", "-z"])?;
    let paths = String::from_utf8(output)?;
    let mut files = BTreeMap::new();
    for path in paths.split('\0').filter(|path| !path.is_empty()) {
        let git_path = GitPath::new(path)?;
        let bytes = std::fs::read(root.join(path))?;
        files.insert(git_path, bytes);
    }
    Ok(files)
}

fn capture_refs(root: &Path) -> Result<BTreeMap<GitRefKey, GitRef>, GitWorkspaceGitError> {
    let output = run_git(
        root,
        "git for-each-ref",
        [
            "for-each-ref",
            "--format=%(refname)\t%(objectname)",
            "refs/heads",
            "refs/tags",
        ],
    )?;
    let text = String::from_utf8(output)?;
    let mut refs = BTreeMap::new();
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let (raw_name, object_id) = line
            .split_once('\t')
            .ok_or_else(|| GitWorkspaceGitError::MalformedRefLine(line.to_owned()))?;
        let Some((kind, name)) = parse_ref_name(raw_name) else {
            continue;
        };
        let reference = GitRef::new(
            kind,
            GitRefName::new(name)?,
            GitRefTarget::Object(GitObjectId::new(object_id)?),
        );
        refs.insert(reference.key().clone(), reference);
    }
    Ok(refs)
}

fn parse_ref_name(raw: &str) -> Option<(GitRefKind, &str)> {
    raw.strip_prefix("refs/heads/")
        .map(|name| (GitRefKind::Branch, name))
        .or_else(|| {
            raw.strip_prefix("refs/tags/")
                .map(|name| (GitRefKind::Tag, name))
        })
}

fn run_git<const N: usize>(
    root: &Path,
    program: &'static str,
    args: [&str; N],
) -> Result<Vec<u8>, GitWorkspaceGitError> {
    let output = std::process::Command::new("git")
        .args(args)
        .current_dir(root)
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
