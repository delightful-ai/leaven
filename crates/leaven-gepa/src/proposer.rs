//! Surface-native GEPA proposers.

use leaven_core::Artifact;
use leaven_surface::{EditSurface, SurfaceError};

/// Produces a surface-native edit for one selected part.
pub trait SurfaceProposer<A, S>
where
    A: Artifact,
    S: EditSurface<A>,
{
    /// Propose an edit to `part`.
    fn propose_edit(
        &mut self,
        artifact: &A,
        surface: &S,
        part: &S::PartId,
    ) -> Result<S::Edit, SurfaceError>;
}

/// Deterministic reflective mutation fixture.
#[derive(Clone, Debug)]
pub struct ReflectiveMutation<E> {
    edit: E,
}

impl<E> ReflectiveMutation<E> {
    /// Build a proposer that always returns the supplied edit.
    #[must_use]
    pub const fn new(edit: E) -> Self {
        Self { edit }
    }
}

impl<A, S> SurfaceProposer<A, S> for ReflectiveMutation<S::Edit>
where
    A: Artifact,
    S: EditSurface<A>,
{
    fn propose_edit(
        &mut self,
        _artifact: &A,
        _surface: &S,
        _part: &S::PartId,
    ) -> Result<S::Edit, SurfaceError> {
        Ok(self.edit.clone())
    }
}

/// Configuration placeholder for reflective mutation.
#[derive(Clone, Debug, Default)]
pub struct ReflectiveMutationConfig;

/// System-aware merge proposer placeholder.
#[derive(Clone, Debug, Default)]
pub struct SystemAwareMerge;
