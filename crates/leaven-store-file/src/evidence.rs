//! JSON evidence store on a local filesystem path.

use std::marker::PhantomData;
use std::path::{Path, PathBuf};

use bytes::Bytes;
use leaven_kernel::{CheckpointId, EvidenceRef};
use leaven_store::{CheckpointBytes, CheckpointStore, Evidence, EvidenceStore, StoreError};
use parking_lot::Mutex;
use serde::Serialize;
use serde::de::DeserializeOwned;

/// File-backed evidence store for serializable evidence values.
///
/// Each evidence item is written as `<root>/<key>.json`. Keys are monotonically
/// increasing decimal strings. Reopening an existing store resumes at
/// `max(existing_key) + 1`, so long-running examples can append evidence after
/// process restart without overwriting earlier records.
pub struct FileEvidenceStore<E> {
    name: String,
    root: PathBuf,
    next_key: Mutex<u64>,
    marker: PhantomData<E>,
}

/// File-backed checkpoint store for opaque checkpoint bytes.
///
/// Checkpoints are immutable files named by [`CheckpointId`]. The store also
/// writes `LATEST` with the most recent checkpoint id so consumers can implement
/// "resume from latest" without inventing their own pointer file.
#[derive(Clone, Debug)]
pub struct FileCheckpointStore {
    root: PathBuf,
}

/// JSON-encoded typed checkpoints over a [`FileCheckpointStore`].
///
/// This is a convenience layer for long-running local examples and operator
/// paths. The durable store remains byte-oriented; the type parameter names the
/// explicit checkpoint schema the caller owns.
#[derive(Clone, Debug)]
pub struct FileJsonCheckpointStore<T> {
    store: FileCheckpointStore,
    marker: PhantomData<T>,
}

impl FileCheckpointStore {
    /// Opens or creates a checkpoint store root.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] if the root directory cannot be created.
    pub fn open(root: impl Into<PathBuf>) -> Result<Self, StoreError> {
        let root = root.into();
        std::fs::create_dir_all(&root).map_err(|err| operation_failed("open", &root, &err))?;
        Ok(Self { root })
    }

    /// Returns the local root directory.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Returns the checkpoint id named by `LATEST`, when present.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] if `LATEST` exists but cannot be read or parsed.
    pub fn latest(&self) -> Result<Option<CheckpointId>, StoreError> {
        let latest = self.root.join("LATEST");
        if !latest.exists() {
            return Ok(None);
        }
        let raw = std::fs::read_to_string(&latest)
            .map_err(|err| operation_failed("latest", &latest, &err))?;
        let uuid = uuid::Uuid::parse_str(raw.trim())
            .map_err(|err| StoreError::Serialization(err.to_string()))?;
        Ok(Some(CheckpointId::from_uuid(uuid)))
    }
}

impl<T> FileJsonCheckpointStore<T> {
    /// Opens or creates a typed JSON checkpoint store root.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] if the root directory cannot be created.
    pub fn open(root: impl Into<PathBuf>) -> Result<Self, StoreError> {
        Ok(Self::from_store(FileCheckpointStore::open(root)?))
    }

    /// Wraps an existing byte checkpoint store.
    #[must_use]
    pub fn from_store(store: FileCheckpointStore) -> Self {
        Self {
            store,
            marker: PhantomData,
        }
    }

    /// Returns the underlying byte checkpoint store.
    #[must_use]
    pub fn raw_store(&self) -> &FileCheckpointStore {
        &self.store
    }

    /// Returns the local root directory.
    #[must_use]
    pub fn root(&self) -> &Path {
        self.store.root()
    }
}

impl<T> FileJsonCheckpointStore<T>
where
    T: Serialize + DeserializeOwned,
{
    /// Writes a typed checkpoint as pretty JSON and marks it latest.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when serialization or storage fails.
    pub fn put(&self, checkpoint: &T) -> Result<CheckpointId, StoreError> {
        let bytes = serde_json::to_vec_pretty(checkpoint)
            .map_err(|err| StoreError::Serialization(err.to_string()))?;
        self.store.put(CheckpointBytes(Bytes::from(bytes)))
    }

    /// Reads a typed checkpoint by id.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when storage or deserialization fails.
    pub fn get(&self, id: CheckpointId) -> Result<T, StoreError> {
        let bytes = self.store.get(id)?;
        serde_json::from_slice(&bytes.0).map_err(|err| StoreError::Serialization(err.to_string()))
    }

    /// Reads the checkpoint named by `LATEST`, when present.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the latest pointer or typed checkpoint cannot
    /// be read.
    pub fn latest(&self) -> Result<Option<(CheckpointId, T)>, StoreError> {
        let Some(id) = self.store.latest()? else {
            return Ok(None);
        };
        Ok(Some((id, self.get(id)?)))
    }
}

impl CheckpointStore for FileCheckpointStore {
    fn put(&self, checkpoint: CheckpointBytes) -> Result<CheckpointId, StoreError> {
        let id = CheckpointId::new();
        let path = checkpoint_path(&self.root, id);
        std::fs::write(&path, checkpoint.0).map_err(|err| operation_failed("put", &path, &err))?;
        let latest = self.root.join("LATEST");
        std::fs::write(&latest, id.to_string())
            .map_err(|err| operation_failed("latest", &latest, &err))?;
        Ok(id)
    }

    fn get(&self, id: CheckpointId) -> Result<CheckpointBytes, StoreError> {
        let path = checkpoint_path(&self.root, id);
        let bytes = std::fs::read(&path).map_err(|err| StoreError::OperationFailed {
            store: path.display().to_string(),
            operation: "get",
            reason: err.to_string(),
            retryable: Some(false),
        })?;
        Ok(CheckpointBytes(Bytes::from(bytes)))
    }

    fn latest(&self) -> Result<Option<CheckpointId>, StoreError> {
        Self::latest(self)
    }
}

impl<E> FileEvidenceStore<E> {
    /// Opens or creates a file evidence store.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] if the root directory cannot be created or read.
    pub fn open(name: impl Into<String>, root: impl Into<PathBuf>) -> Result<Self, StoreError> {
        let root = root.into();
        std::fs::create_dir_all(&root).map_err(|err| operation_failed("open", &root, &err))?;
        let next_key = next_key(&root)?;
        Ok(Self {
            name: name.into(),
            root,
            next_key: Mutex::new(next_key),
            marker: PhantomData,
        })
    }

    /// Returns the local root directory.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }
}

impl<E> EvidenceStore<E> for FileEvidenceStore<E>
where
    E: Evidence + Clone + Serialize + DeserializeOwned,
{
    fn put(&self, evidence: E) -> Result<EvidenceRef, StoreError> {
        let mut guard = self.next_key.lock();
        let key = guard.to_string();
        let path = evidence_path(&self.root, &key)?;
        let bytes = serde_json::to_vec_pretty(&evidence)
            .map_err(|err| StoreError::Serialization(err.to_string()))?;
        std::fs::write(&path, bytes).map_err(|err| operation_failed("put", &path, &err))?;
        *guard += 1;
        Ok(EvidenceRef {
            store: self.name.clone(),
            key,
        })
    }

    fn get(&self, reference: &EvidenceRef) -> Result<E, StoreError> {
        if reference.store != self.name {
            return Err(StoreError::EvidenceNotFound(reference.clone()));
        }
        let path = evidence_path(&self.root, &reference.key)?;
        let bytes =
            std::fs::read(&path).map_err(|_| StoreError::EvidenceNotFound(reference.clone()))?;
        serde_json::from_slice(&bytes).map_err(|err| StoreError::Serialization(err.to_string()))
    }
}

fn next_key(root: &Path) -> Result<u64, StoreError> {
    let mut max_key = None;
    for entry in std::fs::read_dir(root).map_err(|err| operation_failed("scan", root, &err))? {
        let entry = entry.map_err(|err| operation_failed("scan", root, &err))?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        let Ok(key) = stem.parse::<u64>() else {
            continue;
        };
        max_key = Some(max_key.map_or(key, |current: u64| current.max(key)));
    }
    Ok(max_key.map_or(0, |key| key + 1))
}

fn evidence_path(root: &Path, key: &str) -> Result<PathBuf, StoreError> {
    if key.is_empty() || !key.chars().all(|ch| ch.is_ascii_digit()) {
        return Err(StoreError::OperationFailed {
            store: root.display().to_string(),
            operation: "evidence_path",
            reason: format!("invalid evidence key `{key}`"),
            retryable: Some(false),
        });
    }
    Ok(root.join(format!("{key}.json")))
}

fn checkpoint_path(root: &Path, id: CheckpointId) -> PathBuf {
    root.join(format!("{id}.checkpoint"))
}

fn operation_failed(operation: &'static str, path: &Path, err: &std::io::Error) -> StoreError {
    StoreError::OperationFailed {
        store: path.display().to_string(),
        operation,
        reason: err.to_string(),
        retryable: Some(false),
    }
}
