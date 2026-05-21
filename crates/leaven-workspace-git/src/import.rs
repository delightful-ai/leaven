use std::ffi::OsString;
use std::path::PathBuf;

use leaven_artifact_git::{GitObjectId, GitRevision};

use crate::GitWorkspaceGitError;
use crate::cli::{run_git, run_git_fsck};

#[derive(Clone, Debug)]
pub struct GitCommitImportRequest {
    pub source: PathBuf,
    pub durable_store: PathBuf,
    pub commit: GitObjectId,
    pub expected_parent: GitObjectId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImportedGitCommit {
    revision: GitRevision,
}

impl ImportedGitCommit {
    #[must_use]
    pub const fn revision(&self) -> &GitRevision {
        &self.revision
    }
}

pub struct GitCommitImporter;

impl GitCommitImporter {
    pub fn import_commit(
        request: GitCommitImportRequest,
    ) -> Result<ImportedGitCommit, GitWorkspaceGitError> {
        run_git_fsck(&request.source)?;
        ensure_expected_parent(&request)?;

        let import_ref = format!("refs/leaven/imported/{}", request.commit);
        let refspec = format!("+{}:{import_ref}", request.commit);
        run_git(
            Some(&request.durable_store),
            "git fetch imported commit",
            vec![
                OsString::from("fetch"),
                request.source.as_os_str().to_os_string(),
                OsString::from(refspec),
            ],
        )?;
        run_git_fsck(&request.durable_store)?;

        Ok(ImportedGitCommit {
            revision: GitRevision::Commit(request.commit),
        })
    }
}

fn ensure_expected_parent(request: &GitCommitImportRequest) -> Result<(), GitWorkspaceGitError> {
    let output = run_git(
        Some(&request.source),
        "git rev-list parent check",
        vec![
            OsString::from("rev-list"),
            OsString::from("--parents"),
            OsString::from("-n"),
            OsString::from("1"),
            OsString::from(request.commit.as_str()),
        ],
    )?;
    let text = String::from_utf8(output)?;
    let has_parent = text
        .split_whitespace()
        .skip(1)
        .any(|parent| parent == request.expected_parent.as_str());
    if has_parent {
        return Ok(());
    }
    Err(GitWorkspaceGitError::UnexpectedParent {
        commit: request.commit.clone(),
        expected_parent: request.expected_parent.clone(),
    })
}
