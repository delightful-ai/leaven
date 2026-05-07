//! Time primitives.
//!
//! UTC, microsecond-resolution timestamps via `chrono`. The kernel
//! re-exports them as a single type alias so other crates don't have to
//! agree on a chrono import path or pin a specific chrono version
//! through their own surfaces.

use chrono::{DateTime, Utc};

/// UTC timestamp used by run-graph entries and events.
pub type Timestamp = DateTime<Utc>;

/// Returns the current UTC timestamp.
///
/// Goes through `chrono::Utc::now()`. Tests that need deterministic
/// timestamps should inject a clock at a higher level rather than
/// shadow this function.
#[must_use]
pub fn now() -> Timestamp {
    Utc::now()
}
