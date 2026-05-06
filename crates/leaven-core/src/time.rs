//! Time aliases.
//!
//! All run-graph timestamps are UTC. We do not mix timezone-naive and
//! timezone-aware times in graph state.

use chrono::{DateTime, Utc};

/// All graph timestamps are UTC `DateTime`s.
pub type Timestamp = DateTime<Utc>;

/// Current UTC timestamp. Tests should not call this directly; pass a
/// clock through context instead.
#[must_use]
pub fn now() -> Timestamp {
    Utc::now()
}
