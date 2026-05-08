//! Artifact contract.

use leaven_kernel::{ContentId, Fingerprint};

/// The domain value being optimized.
///
/// An artifact is opaque to the framework except for two things: it has an
/// [`ArtifactIdentity`], and a typed [`Change`](Artifact::Change) can be
/// applied to it functionally. Everything else — components, decomposition,
/// rendering, workspace materialization — is a *chosen lens* over the
/// artifact (`leaven-surface::EditSurface`), not an intrinsic property.
///
/// Artifacts can be anything: text prompts, struct trees of prompt
/// modules, git repositories, jj operation logs, harness programs, skill
/// directories, CUDA kernels, weight checkpoints. The framework cares only
/// that they identify and that changes apply.
///
/// # Trait laws
///
/// - **Functional apply.** `apply_change` must be pure: same artifact +
///   same change either fails the same way or produces the same observable
///   content. No interior mutation that affects later evaluations.
/// - **Apply does not mutate self.** A failed apply must not change the
///   original artifact's state. The trait reads `&self` to make this
///   obvious, but implementors using `RefCell` or similar must respect it
///   anyway.
/// - **Identity is stable per state.** A given artifact value has one
///   identity. Two artifacts that report the same [`ArtifactIdentity::Content`]
///   are observationally equivalent for evaluation purposes — the cache
///   trusts this absolutely.
///
/// # Identity vs caching
///
/// Returning [`ArtifactIdentity::Content`] activates deterministic
/// evaluation caching. Returning [`ArtifactIdentity::External`] does not —
/// the cache cannot trust an external identifier as a hash. For
/// content-addressed external handles (git commit hashes, IPFS CIDs,
/// docker image digests), use [`ContentAddressed`] and let the handle
/// double as the [`ContentId`].
pub trait Artifact: Clone + Send + Sync + 'static {
    /// Typed change applied to produce a new artifact state.
    ///
    /// The change is whatever shape best describes a transformation of
    /// this artifact: a string replacement, a multi-file patch, a
    /// component-edit struct, an enum of edit variants. The framework
    /// transports `Change` values through proposals and applies them on
    /// behalf of optimizers; it never inspects them.
    type Change: Clone + std::fmt::Debug + Send + Sync + 'static;

    /// Error returned by [`apply_change`](Artifact::apply_change) and
    /// [`validate`](Artifact::validate) when an artifact-specific
    /// invariant is violated.
    type ApplyError: std::error::Error + Send + Sync + 'static;

    /// Stable identity of this artifact state.
    ///
    /// Implementors should return [`ArtifactIdentity::Content`] whenever
    /// possible — content identity unlocks the evaluation cache. Use
    /// [`ArtifactIdentity::External`] only when the underlying state has
    /// no addressable content but does have a stable external label.
    fn identity(&self) -> ArtifactIdentity;

    /// Identity that deterministic evaluator caches may trust.
    ///
    /// This is deliberately separate from [`Artifact::identity`]. Graph
    /// identity may be an external mutable handle such as a branch name,
    /// workspace path, or database row. Returning a cache identity promises
    /// that the value identifies immutable evaluation-relevant content.
    #[must_use]
    fn cache_identity(&self) -> Option<CacheIdentity> {
        None
    }

    /// Check artifact-level invariants and surface any violation as
    /// feedback for the proposer.
    ///
    /// `validate` is part of the proposer-iteration loop, not just a
    /// graph-insertion guard. When an authored artifact fails validation
    /// — a syntactically-broken harness, a partially-merged config, a
    /// schema-violating struct — the resulting [`ApplyError`] is fed
    /// back to the proposer so it can fix the artifact and try again.
    /// Sharing the error type with [`apply_change`] means a single retry
    /// loop handles "didn't validate" and "couldn't apply" uniformly.
    ///
    /// The default impl is a no-op for artifacts whose only invalid
    /// states are unreachable through `apply_change`. Override when
    /// constructing the artifact via deserialization, parsing, or
    /// aggregation could yield an internally inconsistent state.
    ///
    /// # Errors
    ///
    /// Returns the artifact's [`ApplyError`] when an invariant is
    /// violated. The error should describe the violation precisely
    /// enough that a proposer reading it can decide how to repair the
    /// artifact.
    ///
    /// [`ApplyError`]: Artifact::ApplyError
    /// [`apply_change`]: Artifact::apply_change
    fn validate(&self) -> Result<(), Self::ApplyError> {
        Ok(())
    }

    /// Apply a typed change and return the resulting artifact state.
    ///
    /// # Errors
    ///
    /// Returns the artifact's [`ApplyError`](Artifact::ApplyError) when
    /// the change cannot be applied (invalid edit, validation failure,
    /// ambiguous merge resolution, etc.). On error, `self` must remain
    /// unchanged — callers rely on this to retry against the original
    /// state.
    fn apply_change(&self, change: &Self::Change) -> Result<Self, Self::ApplyError>;
}

/// Identity of an artifact state.
///
/// The two variants encode a real distinction in caching power:
///
/// - [`Content`](ArtifactIdentity::Content) is a deterministic hash of
///   observationally-relevant state. Two content IDs being equal means
///   the artifacts produce the same evaluation results given the same
///   inputs. The evaluation cache keys on this directly.
/// - [`External`](ArtifactIdentity::External) is a stable external
///   label without a hash guarantee. Useful when the underlying value
///   lives in an external system that already has stable identity, but
///   the framework cannot use it as a cache key without an explicit
///   user-supplied cache key alongside.
#[derive(Clone, Debug, Eq, PartialEq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ArtifactIdentity {
    /// 32-byte content hash. Cache-friendly.
    Content(ContentId),
    /// Stable external label without a hash guarantee. Not enough on
    /// its own for deterministic caching.
    External(String),
}

/// Cache-safe identity for deterministic evaluator reuse.
#[derive(
    Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, serde::Serialize, serde::Deserialize,
)]
pub enum CacheIdentity {
    /// The artifact state is content-addressed by this digest.
    Content(ContentId),
    /// The external reference is immutable by law for this artifact type.
    ExternalContent(String),
    /// Caller-supplied stable cache fingerprint.
    User(Fingerprint),
}

/// Stronger capability for artifacts that are intrinsically content-addressed.
///
/// Implement this when the artifact's stable identity *is* a hash — for
/// example, a git tree pointer or a CAS-stored blob. Implementors should
/// also return [`ArtifactIdentity::Content`] from
/// [`Artifact::identity`] using the same [`ContentId`].
pub trait ContentAddressed: Artifact {
    /// Returns the artifact's content identity.
    fn content_id(&self) -> ContentId;
}
