//! Run-associated type bundle.

use crate::artifact::Artifact;
use crate::evidence::Evidence;

/// The bundle of associated types that parameterize a single run.
///
/// One run is parameterized by exactly one `OptimizationProblem`. All
/// proposers, evaluators, populations, and preference relations in that
/// run agree on these associated types. This is what makes
/// [`Optimizer<P>`], [`Proposer<P>`], etc. talk to each other without
/// generic explosions at every call site.
///
/// [`Optimizer<P>`]: https://docs.rs/leaven-engine/latest/leaven_engine/trait.Optimizer.html
/// [`Proposer<P>`]: https://docs.rs/leaven-engine/latest/leaven_engine/trait.Proposer.html
///
/// # Mixed shapes
///
/// When a run needs to mix evidence shapes (scalar + pairwise) or
/// annotation shapes (reflection + merge + edit) the user defines an
/// enum and uses it as `Evidence` or `ProposalAnnotations`. This is the
/// rust-native answer: the enum tells the truth about all shapes that
/// can occur, and the type system flags every unhandled variant. There
/// is no `dyn Evidence` escape hatch in the cold core.
pub trait OptimizationProblem: Send + Sync + 'static {
    /// Domain artifact being optimized.
    type Artifact: Artifact;
    /// Per-evaluation user-supplied input. Often a task spec, prompt,
    /// or data row. May be `()` for unscoped runs.
    type Case: Send + Sync + 'static;
    /// Evidence shape returned by evaluators in this run.
    type Evidence: Evidence;
    /// Typed annotations attached to proposals (reflection notes,
    /// MIPRO surrogate predictions, MuF/Edit behavioral claims, etc.).
    /// Distinct from [`MetadataBag`], which carries operational
    /// breadcrumbs only.
    ///
    /// [`MetadataBag`]: leaven_kernel::MetadataBag
    type ProposalAnnotations: Clone + std::fmt::Debug + Send + Sync + 'static;
}
