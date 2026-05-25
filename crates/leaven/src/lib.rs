//! Leaven: optimize anything in Rust.
//!
//! This is the umbrella crate. It is an import experience, not an
//! implementation crate.
//!
//! # Public routes
//!
//! Individual symbols are routed by audience, not by sophistication tier:
//!
//! - [`prelude`] — ordinary users: define an [`OptimizationProblem`], call
//!   [`optimize`], write a scorer, implement [`Artifact`] or [`EditSurface`].
//! - [`extend`] — users implementing a *piece of the machine*: a custom
//!   optimizer, proposer, selector, gate, evaluator, store, or LM provider.
//! - [`plumbing`] — `#[doc(hidden)]`; public only so a sibling crate or a
//!   contract test can reach it, never for an external caller.
//!
//! Standard implementations and the GEPA optimizer live behind the namespaced
//! crate aliases below (`leaven::stdlib`, `leaven::gepa`, `leaven::engine`,
//! ...), not in the prelude.
//!
//! [`OptimizationProblem`]: prelude::OptimizationProblem
//! [`optimize`]: prelude::optimize
//! [`Artifact`]: prelude::Artifact
//! [`EditSurface`]: prelude::EditSurface

pub mod extend;
pub mod prelude;

#[doc(hidden)]
pub mod plumbing;

// --- Crate aliases: namespaced access to the underlying workspace crates. ---

pub use leaven_core as core;
pub use leaven_engine as engine;
pub use leaven_eval as eval;
pub use leaven_kernel as kernel;
pub use leaven_lm as lm;
pub use leaven_run as run;
pub use leaven_surface as surface;

#[cfg(feature = "std")]
pub use leaven_std as stdlib;

#[cfg(feature = "gepa")]
pub mod gepa;

#[cfg(feature = "workspace")]
pub use leaven_workspace as workspace;

#[cfg(feature = "workspace-git")]
pub use leaven_workspace_git as workspace_git;

#[cfg(feature = "workspace-firkin")]
pub use leaven_workspace_firkin as workspace_firkin;

#[cfg(feature = "agentic")]
pub use leaven_agentic as agentic;

#[cfg(feature = "agentic-git")]
pub use leaven_agentic_git as agentic_git;

#[cfg(feature = "skill")]
pub use leaven_artifact_skill as skill;

#[cfg(feature = "agentic-skill")]
pub use leaven_agentic_skill as agentic_skill;

#[cfg(feature = "lm-cache")]
pub use leaven_lm_cache as lm_cache;

#[cfg(feature = "lm-openai")]
pub use leaven_lm_openai as lm_openai;
