//! Inline aggregate store.

use std::collections::HashMap;

use bytes::Bytes;
use leaven_kernel::{BlobRef, CheckpointId};
use leaven_store::{BlobStore, BlobWrite, CheckpointBytes, CheckpointStore, StoreError};
use parking_lot::Mutex;

/// In-memory blob and checkpoint store for tests and local dry runs.
pub struct InlineStore {
    name: String,
    blobs: Mutex<HashMap<String, Bytes>>,
    checkpoints: Mutex<HashMap<CheckpointId, CheckpointBytes>>,
    latest_checkpoint: Mutex<Option<CheckpointId>>,
}

impl InlineStore {
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            blobs: Mutex::new(HashMap::new()),
            checkpoints: Mutex::new(HashMap::new()),
            latest_checkpoint: Mutex::new(None),
        }
    }
}

impl Default for InlineStore {
    fn default() -> Self {
        Self::new("inline")
    }
}

impl BlobStore for InlineStore {
    fn put(&self, write: BlobWrite) -> Result<BlobRef, StoreError> {
        let mut blobs = self.blobs.lock();
        let key = blobs.len().to_string();
        blobs.insert(key.clone(), write.bytes);
        Ok(BlobRef {
            store: self.name.clone(),
            key,
        })
    }

    fn get(&self, reference: &BlobRef) -> Result<Bytes, StoreError> {
        if reference.store != self.name {
            return Err(StoreError::BlobNotFound(reference.clone()));
        }
        self.blobs
            .lock()
            .get(&reference.key)
            .cloned()
            .ok_or_else(|| StoreError::BlobNotFound(reference.clone()))
    }
}

impl CheckpointStore for InlineStore {
    fn put(&self, checkpoint: CheckpointBytes) -> Result<CheckpointId, StoreError> {
        let id = CheckpointId::new();
        self.checkpoints.lock().insert(id, checkpoint);
        Ok(id)
    }

    fn get(&self, id: CheckpointId) -> Result<CheckpointBytes, StoreError> {
        self.checkpoints
            .lock()
            .get(&id)
            .cloned()
            .ok_or_else(|| StoreError::OperationFailed {
                store: self.name.clone(),
                operation: "get_checkpoint",
                reason: format!("checkpoint `{id}` was not found"),
                retryable: Some(false),
            })
    }

    fn latest(&self) -> Result<Option<CheckpointId>, StoreError> {
        Ok(*self.latest_checkpoint.lock())
    }

    fn mark_latest(&self, id: CheckpointId) -> Result<(), StoreError> {
        if !self.checkpoints.lock().contains_key(&id) {
            return Err(StoreError::OperationFailed {
                store: self.name.clone(),
                operation: "mark_latest_checkpoint",
                reason: format!("checkpoint `{id}` was not found"),
                retryable: Some(false),
            });
        }
        *self.latest_checkpoint.lock() = Some(id);
        Ok(())
    }
}
