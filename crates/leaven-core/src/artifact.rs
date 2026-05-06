//! Artifact contract.

use leaven_kernel::ContentId;

/// The thing being optimized.
pub trait Artifact: Clone + Send + Sync + 'static {
    type Change: Clone + std::fmt::Debug + Send + Sync + 'static;
    type ApplyError: std::error::Error + Send + Sync + 'static;

    /// Stable identity of this artifact state.
    fn identity(&self) -> ArtifactIdentity;

    /// Validate this artifact state before it enters a run graph.
    fn validate(&self) -> Result<(), Self::ApplyError> {
        Ok(())
    }

    /// Apply a typed change.
    fn apply_change(&self, change: &Self::Change) -> Result<Self, Self::ApplyError>;
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum ArtifactIdentity {
    Content(ContentId),
    External(String),
}

/// Stronger capability for content-addressed artifacts.
pub trait ContentAddressed: Artifact {
    fn content_id(&self) -> ContentId;
}
