//! Blob storage contracts.

use bytes::Bytes;
use leaven_kernel::BlobRef;

use crate::StoreError;

pub struct BlobWrite {
    pub bytes: Bytes,
    pub content_type: Option<String>,
}

pub trait BlobStore: Send + Sync {
    fn put(&self, write: BlobWrite) -> Result<BlobRef, StoreError>;
    fn get(&self, reference: &BlobRef) -> Result<Bytes, StoreError>;
}
