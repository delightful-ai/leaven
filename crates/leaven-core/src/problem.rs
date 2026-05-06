//! Run-associated type bundle.

use crate::artifact::Artifact;
use crate::evidence::Evidence;

/// One run is parameterized by exactly one `OptimizationProblem`. All
/// proposers, evaluators, populations, and preference relations in the
/// run agree on these associated types.
pub trait OptimizationProblem: Send + Sync + 'static {
    type Artifact: Artifact;
    type Case: Send + Sync + 'static;
    type Evidence: Evidence;
    type ProposalAnnotations: Clone + std::fmt::Debug + Send + Sync + 'static;
}
