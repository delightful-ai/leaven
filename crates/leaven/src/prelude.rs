//! Imports for ordinary Leaven users.
//!
//! This is the entry route for users who define an [`OptimizationProblem`],
//! call [`optimize`], write a scorer, and implement [`Artifact`] or
//! [`EditSurface`]. It carries only ordinary product vocabulary.
//!
//! Users implementing a *piece of the machine* (a custom optimizer, proposer,
//! selector, gate, evaluator, store, or LM provider) want [`crate::extend`]
//! instead. Engine and cold-algebra internals are not re-exported here.
//!
//! Standard implementations live behind namespaced facades, not this prelude:
//! `leaven::stdlib` for standard artifacts/evidence/populations and
//! `leaven::gepa` for the GEPA optimizer. The `public_surface_contract` test
//! enforces that this route stays ordinary-only.

// --- Defining a problem and its artifacts. ---

pub use leaven_core::Artifact;
pub use leaven_core::ArtifactIdentity;
pub use leaven_core::OptimizationProblem;

// --- Reading results: assessments, evidence, preferences. ---

pub use leaven_core::Assessment;
pub use leaven_core::Evidence;
pub use leaven_core::PairOrder;
pub use leaven_core::Preference;

// --- Edit surfaces over artifacts. ---

pub use leaven_surface::EditSurface;
pub use leaven_surface::Part;
pub use leaven_surface::PartAddress;
pub use leaven_surface::PartSelection;
pub use leaven_surface::SurfaceError;
pub use leaven_surface::SurfaceFingerprint;

// --- Budget and identity an ordinary user touches. ---

pub use leaven_kernel::Budget;
pub use leaven_kernel::Cost;
pub use leaven_kernel::CostUnit;

// --- The optimize workflow. ---

pub use leaven_run::BestCandidate;
pub use leaven_run::OptimizeBuilder;
pub use leaven_run::OptimizeError;
pub use leaven_run::Optimized;
pub use leaven_run::RunError;
pub use leaven_run::RunEventSummary;
pub use leaven_run::Score;
pub use leaven_run::ScoreError;
pub use leaven_run::StandardRunSummary;
pub use leaven_run::optimize;
