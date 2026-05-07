//! Surface addresses.

/// Generic stringly-typed address used by surfaces that don't need a
/// richer locator type.
///
/// Surfaces with structured locators (paths, line ranges, changeset
/// ids) usually define their own [`Address`] type. `PartAddress`
/// exists for the common case where a single string suffices and for
/// crossing serialization, CLI, and prompt boundaries.
///
/// [`Address`]: crate::EditSurface::Address
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct PartAddress(pub String);
