use std::ffi::OsString;
use std::fmt::Write as _;
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
        let source_url = local_file_url(&request.source)?;
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
                    OsString::from(source_url.as_str()),
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

fn local_file_url(path: &Path) -> Result<String, GitWorkspaceGitError> {
    let path = std::fs::canonicalize(path)?;
    let path = path.to_str().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "git projection source path is not UTF-8",
        )
    })?;
    Ok(format!("file://{}", percent_encode_path(path)))
}

fn percent_encode_path(path: &str) -> String {
    let mut encoded = String::new();
    for byte in path.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'/' | b'-' | b'_' | b'.' | b'~' | b':' => {
                encoded.push(char::from(byte));
            }
            _ => {
                encoded.push('%');
                let _ = write!(encoded, "{byte:02X}");
            }
        }
    }
    encoded
}
