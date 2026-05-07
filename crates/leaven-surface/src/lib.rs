//! Explicit edit/read surfaces over artifacts.
//!
//! An [`Artifact`] is the intrinsic thing being optimized; a *surface*
//! is a chosen projection over it. The split exists because not every
//! artifact has an obvious "decomposition." A struct of prompt modules
//! can be addressed by field name; a git repository can be addressed
//! by path, by skill-frontmatter id, by changeset; a jj operation log
//! can be addressed by conflict region, by changeset, or by revset
//! expression — and any one repo can host *several* of those surfaces
//! depending on what an optimizer wants to edit.
//!
//! [`Artifact`]: leaven_core::Artifact
//!
//! # Vocabulary
//!
//! - [`EditSurface`] — the trait. Picks a projection over an artifact:
//!   what counts as a part, how parts are addressed, what an edit
//!   produces (an artifact-native [`Change`]).
//! - [`Part`] — one named, addressable unit exposed by a surface.
//!   Parts are values, not references — the surface decides what the
//!   [`PartView`] payload looks like, including any semantic
//!   classification.
//! - [`PartAddress`] — string locator for a part. Addresses are
//!   stringly-typed because they cross renderer/agent/CLI boundaries.
//! - [`PartSelection`] — declarative subset of parts (`All` or
//!   `Only(...)`).
//! - [`SurfaceFingerprint`] — stable identity of the surface
//!   *definition*. Bumps when layout rules, parsing, or filtering
//!   change so cache keys invalidate correctly.
//!
//! # Surface laws
//!
//! - **Surface identity is scoped to the surface, not the artifact.**
//!   A path-based surface and a logical-id surface over the same
//!   artifact will report different parts and different addresses for
//!   the same content.
//! - **Path-based surfaces are remove + add under rename.** Identity
//!   continuity across rename requires a surface that extracts a
//!   logical id (e.g. parsing skill frontmatter, reading a jj
//!   change-id).
//! - **`change_part` is pure.** It produces an artifact-native
//!   [`Change`] without mutating the artifact. The framework applies
//!   the change separately so failure paths and retries work
//!   uniformly.
//! - **Bump the [`SurfaceFingerprint`] when interpretation changes.**
//!   The fingerprint is mixed into evaluation cache keys; silently
//!   leaving it stale poisons the cache.
//!
//! [`Change`]: leaven_core::Artifact::Change

pub mod address;
pub mod edit_surface;
pub mod error;
pub mod part;
pub mod path_surface;
pub mod selection;

pub use address::PartAddress;
pub use edit_surface::{EditSurface, SurfaceFingerprint};
pub use error::SurfaceError;
pub use part::{Part, PartView};
pub use path_surface::{PathAddress, PathPartId, PathSurfaceConfig};
pub use selection::PartSelection;

pub mod prelude {
    //! Common surface imports.
    //!
    //! Bring this in when writing or consuming an [`EditSurface`]; it
    //! covers the trait, the part vocabulary, addresses, selections,
    //! and the surface error type.

    pub use crate::{
        EditSurface, Part, PartAddress, PartSelection, PartView, SurfaceError, SurfaceFingerprint,
    };
}
