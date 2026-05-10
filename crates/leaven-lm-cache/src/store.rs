use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;

use parking_lot::Mutex;

use crate::{LmCacheEntry, LmCacheError, LmCacheKey};

/// Storage capability for LM response-cache entries.
pub trait LmCacheStore: Send + Sync {
    /// Reads a cache entry.
    fn get(
        &self,
        key: LmCacheKey,
    ) -> impl Future<Output = Result<Option<LmCacheEntry>, LmCacheError>> + Send + '_;

    /// Writes a cache entry.
    fn put(
        &self,
        key: LmCacheKey,
        entry: LmCacheEntry,
    ) -> impl Future<Output = Result<(), LmCacheError>> + Send + '_;
}

/// In-memory response-cache backend.
#[derive(Clone, Default)]
pub struct InMemoryLmCache {
    entries: Arc<Mutex<HashMap<LmCacheKey, LmCacheEntry>>>,
}

impl InMemoryLmCache {
    /// Returns the number of retained entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.lock().len()
    }

    /// Returns true when no entries are retained.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.lock().is_empty()
    }
}

impl LmCacheStore for InMemoryLmCache {
    async fn get(&self, key: LmCacheKey) -> Result<Option<LmCacheEntry>, LmCacheError> {
        Ok(self.entries.lock().get(&key).cloned())
    }

    async fn put(&self, key: LmCacheKey, entry: LmCacheEntry) -> Result<(), LmCacheError> {
        self.entries.lock().insert(key, entry);
        Ok(())
    }
}
