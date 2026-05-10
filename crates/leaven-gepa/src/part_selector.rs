//! Part selection strategies for GEPA.

use leaven_core::Artifact;
use leaven_surface::{EditSurface, SurfaceError};

/// Chooses which surface part GEPA should mutate.
pub trait PartSelector<A, S>
where
    A: Artifact,
    S: EditSurface<A>,
{
    /// Select one part of an artifact through the supplied surface.
    fn select_part(&mut self, artifact: &A, surface: &S) -> Result<S::PartId, SurfaceError>;
}

/// Private state exposed by part selectors that participate in GEPA resume.
pub trait CheckpointPartSelector {
    /// Serializable selector state.
    type State: serde::Serialize + serde::de::DeserializeOwned;

    /// Captures selector state.
    fn checkpoint_state(&self) -> Self::State;

    /// Restores selector state.
    fn restore_state(&mut self, state: Self::State);
}

/// Paper-baseline selector: cycle through surface parts deterministically.
#[derive(Clone, Debug, Default)]
pub struct RoundRobinPart {
    next: usize,
}

impl RoundRobinPart {
    /// Build a round-robin selector.
    #[must_use]
    pub const fn new() -> Self {
        Self { next: 0 }
    }
}

impl<A, S> PartSelector<A, S> for RoundRobinPart
where
    A: Artifact,
    S: EditSurface<A>,
{
    fn select_part(&mut self, artifact: &A, surface: &S) -> Result<S::PartId, SurfaceError> {
        let parts = surface.parts(artifact)?;
        if parts.is_empty() {
            return Err(SurfaceError::Message(
                "round-robin selector found no surface parts".to_owned(),
            ));
        }
        let selected = parts[self.next % parts.len()].id.clone();
        self.next = self.next.wrapping_add(1);
        Ok(selected)
    }
}

impl CheckpointPartSelector for RoundRobinPart {
    type State = usize;

    fn checkpoint_state(&self) -> Self::State {
        self.next
    }

    fn restore_state(&mut self, state: Self::State) {
        self.next = state;
    }
}

/// Placeholder name for trace-aware selection over worst evidence.
#[derive(Clone, Debug, Default)]
pub struct WorstEvidencePart;
