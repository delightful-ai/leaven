//! GEPA validation policies.

/// Marker trait for validation policies.
pub trait ValidationPolicy {}

/// Validate on the full configured set.
#[derive(Clone, Debug, Default)]
pub struct FullValidation;

impl ValidationPolicy for FullValidation {}

/// Screen on a minibatch, then validate admitted candidates.
#[derive(Clone, Debug, Default)]
pub struct MinibatchThenValidation;

impl ValidationPolicy for MinibatchThenValidation {}
