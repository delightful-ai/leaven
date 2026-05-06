//! `OptimizationProblem` — the run-wide algebra.
//!
//! A `Problem` ties together the artifact under optimization, the
//! evaluation case shape, the evidence shape, and the typed proposal
//! annotations. Run-wide enums are deliberate: if a run mixes evidence
//! shapes (scalar + pairwise + agent traces) the user defines a
//! sum type and the rest of the library follows.

use crate::artifact::Artifact;
use crate::evidence::Evidence;

/// One run is parameterized by exactly one `OptimizationProblem`. All
/// proposers, evaluators, populations, and preference relations in the
/// run agree on these associated types.
pub trait OptimizationProblem: Send + Sync + 'static {
    /// The thing being optimized.
    type Artifact: Artifact;

    /// One unit of input handed to the evaluator (an example,
    /// task, dataset row, environment seed, …). The cold core does
    /// not interpret it.
    type Case: Send + Sync + 'static;

    /// The evidence shape returned by every evaluator in the run.
    /// If a run mixes shapes, define an enum.
    type Evidence: Evidence;

    /// Typed semantic payload attached to proposals. Distinct from
    /// [`crate::metadata::MetadataBag`]: annotations are read by
    /// algorithms (gates, selectors, claim filters); metadata is for
    /// debugging/operational data.
    type ProposalAnnotations: Clone + std::fmt::Debug + Send + Sync + 'static;
}
