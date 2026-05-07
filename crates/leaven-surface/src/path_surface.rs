//! Path-based surface vocabulary.
//!
//! Shared types for surfaces that key on filesystem paths — directory
//! artifacts, git/jj path surfaces, skill directories. Path-keyed
//! surfaces treat rename as remove-then-add: identity is bound to the
//! path, not the content. Surfaces that need rename continuity
//! (frontmatter id, change id, etc.) define their own `PartId` and
//! parse it out of the underlying path or content.

use std::path::PathBuf;

/// Path-based [`PartId`].
///
/// Identity is the path itself. Two artifact states that share the
/// same path-keyed parts at the same paths produce equal `PathPartId`s.
///
/// [`PartId`]: crate::EditSurface::PartId
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct PathPartId(pub PathBuf);

/// Path-based [`Address`].
///
/// Coincides with [`PathPartId`] for surfaces where the externally-
/// visible locator and the internal id are the same path. Surfaces
/// with parsed ids may use this as `Address` while keeping a richer
/// `PartId`.
///
/// [`Address`]: crate::EditSurface::Address
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct PathAddress(pub PathBuf);

/// Configuration knobs for path-keyed surfaces.
///
/// Used by surfaces that walk a directory tree and need to decide
/// what to expose as parts. The default skips hidden entries (those
/// whose path component starts with `.`).
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PathSurfaceConfig {
    /// Whether to include entries whose path components start with `.`.
    pub include_hidden: bool,
}
