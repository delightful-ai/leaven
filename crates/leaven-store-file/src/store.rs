//! Aggregate file-backed store.

use std::path::{Path, PathBuf};

use bytes::Bytes;
use leaven_kernel::{BlobRef, CheckpointId};
use leaven_store::{BlobStore, BlobWrite, CheckpointBytes, CheckpointStore, StoreError};

use crate::FileCheckpointStore;

/// Local filesystem store implementing blob and checkpoint capabilities.
///
/// The aggregate layout is intentionally boring:
///
/// ```text
/// <root>/
///   blobs/<uuid>.blob
///   checkpoints/<checkpoint-id>.json
///   checkpoints/LATEST
/// ```
///
/// Evidence stores remain typed and separate because evidence schemas are
/// problem-owned.
#[derive(Clone, Debug)]
pub struct FileStore {
    name: String,
    root: PathBuf,
    blobs: PathBuf,
    checkpoints: FileCheckpointStore,
}

impl FileStore {
    /// Opens or creates an aggregate file store rooted at `root`.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] if the root, blob, or checkpoint directories
    /// cannot be created.
    pub fn open(root: impl Into<PathBuf>) -> Result<Self, StoreError> {
        Self::open_named("file", root)
    }

    /// Opens or creates an aggregate file store with a custom blob store name.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] if the root, blob, or checkpoint directories
    /// cannot be created.
    pub fn open_named(
        name: impl Into<String>,
        root: impl Into<PathBuf>,
    ) -> Result<Self, StoreError> {
        let root = root.into();
        std::fs::create_dir_all(&root).map_err(|err| operation_failed("open", &root, &err))?;
        let blobs = root.join("blobs");
        std::fs::create_dir_all(&blobs).map_err(|err| operation_failed("open", &blobs, &err))?;
        let checkpoints = FileCheckpointStore::open(root.join("checkpoints"))?;
        Ok(Self {
            name: name.into(),
            root,
            blobs,
            checkpoints,
        })
    }

    /// Returns the aggregate root directory.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Returns the checkpoint capability view.
    #[must_use]
    pub fn checkpoint_store(&self) -> &FileCheckpointStore {
        &self.checkpoints
    }
}

impl BlobStore for FileStore {
    fn put(&self, write: BlobWrite) -> Result<BlobRef, StoreError> {
        let key = format!("{}.blob", uuid::Uuid::new_v4());
        let path = self.blobs.join(&key);
        std::fs::write(&path, write.bytes)
            .map_err(|err| operation_failed("put_blob", &path, &err))?;
        Ok(BlobRef {
            store: self.name.clone(),
            key,
        })
    }

    fn get(&self, reference: &BlobRef) -> Result<Bytes, StoreError> {
        if reference.store != self.name {
            return Err(StoreError::BlobNotFound(reference.clone()));
        }
        let path = blob_path(&self.blobs, &reference.key)?;
        std::fs::read(&path)
            .map(Bytes::from)
            .map_err(|_| StoreError::BlobNotFound(reference.clone()))
    }
}

impl CheckpointStore for FileStore {
    fn put(&self, checkpoint: CheckpointBytes) -> Result<CheckpointId, StoreError> {
        self.checkpoints.put(checkpoint)
    }

    fn get(&self, id: CheckpointId) -> Result<CheckpointBytes, StoreError> {
        self.checkpoints.get(id)
    }

    fn latest(&self) -> Result<Option<CheckpointId>, StoreError> {
        self.checkpoints.latest()
    }

    fn mark_latest(&self, id: CheckpointId) -> Result<(), StoreError> {
        self.checkpoints.mark_latest(id)
    }
}

fn blob_path(root: &Path, key: &str) -> Result<PathBuf, StoreError> {
    if key.contains('/') || key.contains('\\') || key == "." || key == ".." {
        return Err(StoreError::OperationFailed {
            store: root.display().to_string(),
            operation: "blob_path",
            reason: format!("invalid blob key `{key}`"),
            retryable: Some(false),
        });
    }
    Ok(root.join(key))
}

fn operation_failed(operation: &'static str, path: &Path, err: &std::io::Error) -> StoreError {
    StoreError::OperationFailed {
        store: path.display().to_string(),
        operation,
        reason: err.to_string(),
        retryable: Some(false),
    }
}
