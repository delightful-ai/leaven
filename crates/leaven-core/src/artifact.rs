//! `Artifact` — the thing being optimized — and its content identity.
//!
//! An artifact is a typed value with a typed change. Same content
//! produces the same [`ContentId`]; same change applied to same artifact
//! either fails the same way or produces a value with the same
//! `ContentId`. These laws are what the evaluation cache trusts.
//!
//! The cold core does not assume artifacts are text, files, or even
//! materialised — only that they have a deterministic identity and a
//! pure `apply` for typed changes.

use std::fmt;

use serde::{Deserialize, Serialize};

/// Cryptographic content identity for an artifact state.
///
/// **Law.** `content_id` is a deterministic hash of every bit of
/// observable state on the artifact. Two artifacts with the same
/// `content_id` must be observationally indistinguishable through any
/// sequence of the library's operations. Equivalently: the cache is
/// allowed to substitute one for the other.
///
/// 32 bytes is enough for SHA-256 and BLAKE3; the cold core does not
/// pin a hash function. Implementors should use a derive macro
/// (`#[derive(Optimize)]`, planned in `leaven-derive`) for
/// safe-by-default field hashing rather than rolling their own.
#[derive(Clone, Copy, Eq, PartialEq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
pub struct ContentId(pub [u8; 32]);

impl ContentId {
    pub const BYTES: usize = 32;

    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// All-zero sentinel, useful for tests. Real artifacts must never
    /// produce this value.
    #[must_use]
    pub const fn zero() -> Self {
        Self([0; 32])
    }
}

impl fmt::Debug for ContentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ContentId({})", hex::encode(self.0))
    }
}

impl fmt::Display for ContentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Show only the leading bytes for readability; full identity
        // remains accessible via `as_bytes`.
        let prefix = hex::encode(&self.0[..8]);
        write!(f, "cid:{prefix}…")
    }
}

/// The thing being optimized.
///
/// # Laws
///
/// 1. `apply` is **functional**. Same `(artifact, change)` either fails
///    deterministically with the same error, or produces an artifact
///    whose `content_id` is determined by the input.
/// 2. `content_id` is a **deterministic, collision-resistant** hash
///    of every observationally relevant state component. The cache
///    trusts it; lying about it produces silently incorrect cache
///    results.
/// 3. A failed `apply` does **not** mutate the original artifact.
///    `apply` takes `&self` to make this physical.
pub trait Artifact: Clone + std::fmt::Debug + Send + Sync + 'static {
    /// Typed change description. Must be self-contained: applying a
    /// change does not consult any state outside `(self, change)`.
    type Change: Clone + std::fmt::Debug + Send + Sync + 'static;

    /// Domain-specific apply error. Use a thiserror enum.
    type ApplyError: std::error::Error + Send + Sync + 'static;

    /// Deterministic identity of this artifact state. See [`ContentId`].
    fn content_id(&self) -> ContentId;

    /// Apply a typed change. Pure and deterministic — see trait laws.
    ///
    /// # Errors
    /// Return `Err` for any domain-level rejection of the change
    /// (invalid syntax, broken interface, dependent component missing,
    /// …). The cold core records the error in the run graph; it does
    /// not retry.
    fn apply(&self, change: &Self::Change) -> Result<Self, Self::ApplyError>;
}

/// Optional capability for component-addressed artifacts (prompt
/// modules, skill packs, file trees). Used by component-aware
/// proposers, gates, and selectors.
pub trait Decomposable: Artifact {
    type ComponentId: Eq + std::hash::Hash + Clone + Send + Sync + 'static;

    fn components(&self) -> Vec<Component<Self::ComponentId>>;
}

/// A named, content-addressed component within a [`Decomposable`]
/// artifact.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Component<Id> {
    pub id: Id,
    pub content_id: ContentId,
}
