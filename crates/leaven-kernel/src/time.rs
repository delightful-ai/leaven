//! Time primitives.

use chrono::{DateTime, Utc};

pub type Timestamp = DateTime<Utc>;

#[must_use]
pub fn now() -> Timestamp {
    Utc::now()
}
