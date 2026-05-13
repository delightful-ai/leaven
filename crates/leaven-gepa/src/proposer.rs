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
