//! Umbrella re-exports for the `leaven` library.
//!
//! End users typically depend only on this crate. It re-exports the cold
//! core, the engine, and the standard library impls. Authors of new
//! optimizers or new traits should depend on `leaven-core` directly.

pub use leaven_core as core;
pub use leaven_core::prelude::*;
