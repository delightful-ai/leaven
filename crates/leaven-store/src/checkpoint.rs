//! Checkpoint storage contracts.

use bytes::Bytes;
use leaven_kernel::CheckpointId;

use crate::StoreError;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckpointBytes(pub Bytes);

pub trait CheckpointStore: Send + Sync {
    fn put(&self, checkpoint: CheckpointBytes) -> Result<CheckpointId, StoreError>;
    fn get(&self, id: CheckpointId) -> Result<CheckpointBytes, StoreError>;
    fn latest(&self) -> Result<Option<CheckpointId>, StoreError>;
    fn mark_latest(&self, id: CheckpointId) -> Result<(), StoreError>;
}
