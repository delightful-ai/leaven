//! Workspace file and tree fingerprints.

use leaven_kernel::{Fingerprint, FingerprintBuilder};
use serde::{Deserialize, Serialize};

use crate::{WorkspaceError, WorkspacePath, WorkspaceView};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WorkspaceFileFingerprint {
    pub path: WorkspacePath,
    pub fingerprint: Fingerprint,
    pub bytes: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WorkspaceTreeFingerprint {
    pub root: WorkspacePath,
    pub fingerprint: Fingerprint,
    pub files: Vec<WorkspaceFileFingerprint>,
}

pub fn fingerprint_file(
    view: &WorkspaceView<'_>,
    path: &WorkspacePath,
) -> Result<WorkspaceFileFingerprint, WorkspaceError> {
    let bytes = view.read_file(path)?;
    let mut builder = FingerprintBuilder::new();
    builder
        .update(b"leaven.workspace.file.v1")
        .update(path.as_str().as_bytes())
        .update((bytes.len() as u64).to_le_bytes())
        .update(&bytes);
    Ok(WorkspaceFileFingerprint {
        path: path.clone(),
        fingerprint: builder.finish(),
        bytes: bytes.len() as u64,
    })
}

pub fn fingerprint_tree(
    view: &WorkspaceView<'_>,
    root: &WorkspacePath,
) -> Result<WorkspaceTreeFingerprint, WorkspaceError> {
    let mut paths = view.list_files(root)?;
    paths.sort();

    let mut files = Vec::with_capacity(paths.len());
    let mut builder = FingerprintBuilder::new();
    builder.update(b"leaven.workspace.tree.v1");

    for path in paths {
        let file = fingerprint_file(view, &path)?;
        builder
            .update(file.path.as_str().as_bytes())
            .update(file.bytes.to_le_bytes())
            .update(file.fingerprint.0);
        files.push(file);
    }

    Ok(WorkspaceTreeFingerprint {
        root: root.clone(),
        fingerprint: builder.finish(),
        files,
    })
}
