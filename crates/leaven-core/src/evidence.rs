//! Evidence marker.

/// Marker trait for run-wide evidence types.
///
/// Evidence is *opaque to the cold core*. Whether an assessment carries a
/// scalar score, a per-case vector, a pairwise judgment, an agent
/// trajectory blob, or a mixed enum is the user's choice — the algebra
/// here only requires it be sendable across threads and `'static`.
///
/// # Why a marker, not a richer trait
///
/// Optimizer authors should be able to pick the smallest evidence shape
/// their algorithm needs. Forcing every evidence type to expose
/// `score()` (cardinal optimizers want it; pairwise optimizers don't)
/// or `attribution()` (`TextGrad` needs it; `KeepBest` doesn't) would push
/// uniformity into a place where heterogeneity is the point.
///
/// Capability traits — `AttributableEvidence`, `PairwiseEvidence`,
/// `CommandEvidence`, `DiffEvidence`, etc. — live in `leaven-evidence`.
/// Stages bind only what they need.
///
/// # Run-wide enum convention
///
/// When a run mixes evidence shapes, define a problem-specific enum that
/// implements `Evidence` and dispatch on the variant in your stages.
/// This is deliberate: the enum tells the truth about all shapes that
/// can occur in this run, and the type system flags every unhandled
/// variant.
pub trait Evidence: Send + Sync + 'static {}
