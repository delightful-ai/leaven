//! Surface parts.

/// One part of an artifact, as seen through a surface.
///
/// `Part` is generic over the surface's [`PartId`], [`Address`], and
/// [`View`] types so different surfaces can choose any combination
/// without leaking artifact-specific facts into the generic surface
/// crate. Semantic classification belongs in the surface-defined
/// `View` or in downstream capability traits over that view; a part
/// has no framework-wide intrinsic kind.
///
/// [`PartId`]: crate::EditSurface::PartId
/// [`Address`]: crate::EditSurface::Address
/// [`View`]: crate::EditSurface::View
pub struct Part<Id, Address, View> {
    /// Surface's notion of identity for this part.
    pub id: Id,
    /// Externally-visible locator. Conventionally stringifiable.
    pub address: Address,
    /// Surface-defined payload (a slice, a parsed struct, etc.).
    pub view: View,
}

/// Wrapper around a surface-defined view payload.
///
/// Currently a thin newtype; exists so future versions can attach
/// view-level metadata (truncation flags, lazy-loaders, etc.)
/// without changing every existing surface.
pub struct PartView<T> {
    /// The wrapped payload.
    pub inner: T,
}
