//! Surface errors.

/// Failure modes for surface operations.
///
/// Surfaces fail in two broad ways: a referenced part doesn't exist
/// ([`UnknownPart`]) or a surface-specific invariant is violated
/// ([`Message`]). The set is deliberately small — surfaces with
/// rich error vocabularies should wrap their internal errors into
/// `Message` rather than expanding this enum.
///
/// [`UnknownPart`]: SurfaceError::UnknownPart
/// [`Message`]: SurfaceError::Message
#[derive(Debug, thiserror::Error)]
pub enum SurfaceError {
    /// The requested part id does not resolve in the artifact.
    ///
    /// Returned by [`change_part`] when the caller refers to a part
    /// the surface cannot find. May indicate a stale id from a
    /// previous artifact version.
    ///
    /// [`change_part`]: crate::EditSurface::change_part
    #[error("surface part was not found")]
    UnknownPart,

    /// Generic surface-specific failure. Carries a free-form message
    /// describing the violation (parsing failure, validation
    /// rejection, ambiguous edit, etc.).
    #[error("surface operation failed: {0}")]
    Message(String),
}
