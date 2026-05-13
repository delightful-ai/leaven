//! Test fixtures for GEPA's reflective proposer slot.
//!
//! Nothing in this module is part of GEPA's production contract. The types
//! exist so smoke tests, mechanics demos, and Milestone A scaffolds can fill
//! GEPA's `Reflect` type parameter without committing to a real reflector.
//!
//! Reach these types through the explicit `leaven_gepa::fixtures::` path
//! (or `leaven::gepa::fixtures::` when the umbrella `gepa` feature is on).
//! They are intentionally absent from every prelude.

use leaven_core::Artifact;
use leaven_surface::{EditSurface, SurfaceError};

use crate::SurfaceProposer;

/// Milestone A scaffolding: returns a single pre-stored edit regardless of
/// artifact, surface, part, feedback, evidence, trace, or budget.
///
/// This is **not reflection**. It exists only because GEPA's `Reflect` type
/// parameter needs a concrete type while smoke tests, parity demos, and the
/// P8 mechanics example exercise loop plumbing (parent selection, surface
/// edit application, evaluation, acceptance, population observation,
/// checkpoint state) without an end-to-end async reflector.
///
/// `ReflectiveMutation` is reserved as the public name for the real async
/// evidence-aware reflector. See `docs/specs/gepa_optimizer_surface.md`
/// Milestone B (lines 704-713) and the surface-requirements doc under
/// `reviews/2026-05-11-fuckery-extermination-today/refinement/`.
///
/// TODO(phase-4): delete this fixture when the real reflector lands and
/// migrate every caller to that path.
#[derive(Clone, Debug)]
pub struct FixedEditProposer<E> {
    edit: E,
}

impl<E> FixedEditProposer<E> {
    /// Build a fixture proposer that always returns the supplied edit.
    #[must_use]
    pub const fn new(edit: E) -> Self {
        Self { edit }
    }
}

impl<A, S> SurfaceProposer<A, S> for FixedEditProposer<S::Edit>
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
