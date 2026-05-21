use std::collections::BTreeMap;

use crate::{GitArtifact, GitArtifactError, GitPath, GitRef, GitRefKey};

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum GitChange {
    WriteFile { path: GitPath, bytes: Vec<u8> },
    RemoveFile { path: GitPath },
    ReplaceFiles(BTreeMap<GitPath, Vec<u8>>),
    UpsertRef(GitRef),
    RemoveRef(GitRefKey),
    Atomic(Vec<Self>),
}

impl GitChange {
    pub(crate) fn apply_to(&self, artifact: &mut GitArtifact) -> Result<(), GitArtifactError> {
        match self {
            Self::WriteFile { path, bytes } => {
                artifact.write_file(path.clone(), bytes.clone());
                Ok(())
            }
            Self::RemoveFile { path } => artifact.remove_file(path),
            Self::ReplaceFiles(files) => {
                artifact.replace_files(files.clone());
                Ok(())
            }
            Self::UpsertRef(reference) => {
                artifact.upsert_ref(reference.clone());
                Ok(())
            }
            Self::RemoveRef(key) => artifact.remove_ref(key),
            Self::Atomic(changes) => {
                for change in changes {
                    change.apply_to(artifact)?;
                }
                Ok(())
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum GitFsOp {
    Write { path: GitPath, bytes: Vec<u8> },
    Remove { path: GitPath },
}
