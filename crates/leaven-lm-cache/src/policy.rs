use serde::{Deserialize, Serialize};

/// Response-cache behavior for one cached LM wrapper.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum LmCachePolicy {
    /// Do not read or write the response cache.
    Never,
    /// Read existing entries and write misses.
    #[default]
    ReadWrite,
    /// Read existing entries but do not write misses.
    ReadOnly,
    /// Bypass reads and overwrite the entry with the fresh provider response.
    Refresh,
}
