use std::ffi::OsString;
use std::path::{Path, PathBuf};

use leaven_artifact_git::{GitRefKey, GitRefKind};

use crate::GitWorkspaceGitError;
use crate::cli::{run_git, run_git_fsck};

#[derive(Clone, Debug)]
pub struct GitProjectionRequest {
    pub source: PathBuf,
    pub destination: PathBuf,
    pub allowed_refs: Vec<GitRefKey>,
}

#[derive(Clone, Debug)]
pub struct GitProjection {
    path: PathBuf,
}

impl GitProjection {
    pub fn create_bare(request: GitProjectionRequest) -> Result<Self, GitWorkspaceGitError> {
        run_git(
            None,
            "git init --bare",
            vec![
                OsString::from("init"),
                OsString::from("--bare"),
                request.destination.as_os_str().to_os_string(),
            ],
        )?;

        for reference in &request.allowed_refs {
            let full_ref = full_ref(reference);
            let refspec = format!("+{full_ref}:{full_ref}");
            run_git(
                Some(&request.destination),
                "git fetch projection ref",
                vec![
                    OsString::from("fetch"),
                    OsString::from("--no-tags"),
                    OsString::from("--no-write-fetch-head"),
                    request.source.as_os_str().to_os_string(),
                    OsString::from(refspec),
                ],
            )?;
        }

        run_git_fsck(&request.destination)?;
        Ok(Self {
            path: request.destination,
        })
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

fn full_ref(reference: &GitRefKey) -> String {
    match reference.kind() {
        GitRefKind::Branch => format!("refs/heads/{}", reference.name()),
        GitRefKind::Tag => format!("refs/tags/{}", reference.name()),
    }
}
