//! Evidence marker.

/// Marker trait for run-wide evidence types. Implementors are normally
/// problem-specific enums. Bound for thread-safety only.
pub trait Evidence: Send + Sync + 'static {}
