//! JJ-backed artifact vocabulary for materialized file snapshots.

pub mod artifact;
pub mod change;

pub use artifact::JjArtifact;
pub use change::JjChange;
