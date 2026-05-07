//! Part selections.

/// Declarative subset of an artifact's parts.
///
/// Used by selectors and renderers that operate on "some parts" —
/// e.g. a component-level proposer that wants to mutate only the
/// parts a component-selector named, or a renderer that materializes
/// only the parts visible to the proposer's read scope. The default
/// `Id` parameter is [`PartAddress`] for surfaces that don't need a
/// richer locator type; pass an explicit `Id` when working with a
/// surface whose parts are addressed by something other than a string.
///
/// [`PartAddress`]: crate::PartAddress
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum PartSelection<Id = crate::PartAddress> {
    /// Every part the surface produces.
    All,
    /// Only the listed parts. Order is preserved; duplicates are
    /// implementation-defined (typically deduplicated downstream).
    Only(Vec<Id>),
}
