//! Evidence storage contracts.

use leaven_core::Evidence;
use leaven_kernel::EvidenceRef;

use crate::StoreError;

pub trait EvidenceStore<E: Evidence>: Send + Sync {
    fn put(&self, evidence: E) -> Result<EvidenceRef, StoreError>;
    fn get(&self, reference: &EvidenceRef) -> Result<E, StoreError>;
}
