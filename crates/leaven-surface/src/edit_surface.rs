//! Edit surface trait.

use leaven_core::Artifact;

use crate::{Part, SurfaceError};

/// A chosen projection over an [`Artifact`].
///
/// A surface defines what an artifact's "parts" are, how those parts
/// are addressed, what a partial view of one looks like, and how an
/// edit to a part becomes an artifact-native [`Change`]. The same
/// artifact may have multiple surfaces. For example, a git repo might expose
/// a path-based surface or a skill-frontmatter surface, each chosen by a
/// different optimizer or renderer.
///
/// [`Change`]: Artifact::Change
///
/// # Identity model
///
/// `PartId` is the surface's notion of part identity. For
/// path-keyed surfaces this is a path; for logical-id surfaces it's
/// whatever the surface parses out (a skill id, a changeset id, an
/// agent-kit slot). `Address` is the surface's externally-visible
/// locator — usually stringifiable for prompts, CLIs, and agents.
/// They may coincide (path surface) or diverge (frontmatter surface
/// where `PartId` is parsed from a `Address` path).
///
/// # Trait laws
///
/// - **`change_part` is pure.** It returns an artifact-native
///   [`Change`] without mutating the artifact. Application is the
///   framework's job, not the surface's.
/// - **`fingerprint` reflects interpretation, not artifact state.**
///   Bump it when layout rules, filters, parsing, or id-extraction
///   change. The same surface seeing two different artifacts must
///   produce the same fingerprint.
/// - **Path-based surfaces don't preserve identity across renames.**
///   Rename is remove + add for path surfaces. Surfaces that extract
///   a logical id may preserve identity if the id survives the rename
///   (e.g. frontmatter id stays put even when the file moves).
pub trait EditSurface<A: Artifact>: Send + Sync {
    /// Identity of one part as the surface sees it. Used for
    /// dedup, caching, and `change_part`. Often differs from
    /// [`Address`](Self::Address) for surfaces that parse logical ids.
    type PartId: Eq + std::hash::Hash + Clone + Send + Sync + 'static;

    /// Externally-visible locator for a part. Typically stringifiable
    /// so it can travel through prompts, agents, and CLIs.
    type Address: Eq + std::hash::Hash + Clone + Send + Sync + 'static;

    /// A surface-defined view payload for one part.
    ///
    /// The lifetime borrows from the artifact passed to [`parts`]
    /// so views can hold references into the artifact without copying.
    /// Views may be slices, parsed structs, or anything else the
    /// surface chooses to project.
    ///
    /// [`parts`]: Self::parts
    type View<'a>: Send + Sync
    where
        A: 'a;

    /// Surface-native description of an edit to one part.
    ///
    /// `Edit` is the language a proposer or renderer speaks when
    /// asking for a part-level change ("replace this paragraph",
    /// "set field `instructions` to this string"). [`change_part`]
    /// turns it into an artifact-native [`Change`].
    ///
    /// [`change_part`]: Self::change_part
    /// [`Change`]: Artifact::Change
    type Edit: Clone + Send + Sync + 'static;

    /// Stable identity of this surface definition.
    ///
    /// Bump it whenever interpretation changes — layout rules,
    /// filters, parsing, id extraction, ignored-file logic — so
    /// downstream cache keys invalidate.
    fn fingerprint(&self) -> SurfaceFingerprint;

    /// Project the artifact into its parts.
    ///
    /// # Errors
    ///
    /// Returns [`SurfaceError`] when the artifact cannot be projected
    /// — for example, when parsing required by the surface fails or
    /// when the artifact is in an inconsistent state the surface
    /// refuses to interpret.
    #[allow(clippy::type_complexity)]
    fn parts<'a>(
        &self,
        artifact: &'a A,
    ) -> Result<Vec<Part<Self::PartId, Self::Address, Self::View<'a>>>, SurfaceError>;

    /// Translate a surface-native edit into an artifact-native
    /// [`Change`] without mutating the artifact.
    ///
    /// The returned change is what an optimizer puts inside a
    /// [`ProposalEffect::Change`]; the framework will apply it later
    /// via [`Artifact::apply_change`].
    ///
    /// # Errors
    ///
    /// Returns [`SurfaceError::UnknownPart`] when `id` does not
    /// resolve to any part of `artifact`, or [`SurfaceError::Message`]
    /// when the edit is rejected by surface-specific validation.
    ///
    /// [`Change`]: Artifact::Change
    /// [`ProposalEffect::Change`]: leaven_core::ProposalEffect::Change
    fn change_part(
        &self,
        artifact: &A,
        id: Self::PartId,
        edit: Self::Edit,
    ) -> Result<A::Change, SurfaceError>;
}

/// Stable fingerprint of a surface definition.
///
/// Surfaces are configuration: a path filter, a parser, an id
/// extractor. Their fingerprint enters cache keys downstream, so two
/// runs that disagree about how to interpret an artifact don't pool
/// each other's evaluations. Bump the fingerprint whenever any
/// interpretation rule changes.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct SurfaceFingerprint(pub leaven_kernel::Fingerprint);
