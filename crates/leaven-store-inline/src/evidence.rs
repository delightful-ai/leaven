//! Inline evidence store.

use std::collections::HashMap;

use leaven_kernel::EvidenceRef;
use leaven_store::{Evidence, EvidenceStore, StoreError};
use parking_lot::Mutex;

pub struct InlineEvidenceStore<E: Evidence> {
    name: String,
    evidence: Mutex<HashMap<String, E>>,
}

impl<E: Evidence> InlineEvidenceStore<E> {
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            evidence: Mutex::new(HashMap::new()),
        }
    }
}

impl<E> EvidenceStore<E> for InlineEvidenceStore<E>
where
    E: Evidence + Clone,
{
    fn put(&self, evidence: E) -> Result<EvidenceRef, StoreError> {
        let mut guard = self.evidence.lock();
        let key = guard.len().to_string();
        guard.insert(key.clone(), evidence);
        Ok(EvidenceRef {
            store: self.name.clone(),
            key,
        })
    }

    fn get(&self, reference: &EvidenceRef) -> Result<E, StoreError> {
        if reference.store != self.name {
            return Err(StoreError::EvidenceNotFound(reference.clone()));
        }
        self.evidence
            .lock()
            .get(&reference.key)
            .cloned()
            .ok_or_else(|| StoreError::EvidenceNotFound(reference.clone()))
    }
}
