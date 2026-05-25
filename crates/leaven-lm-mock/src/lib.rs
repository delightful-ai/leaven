//! Deterministic scripted LM implementations for tests and offline examples.
//!
//! This crate implements provider-neutral [`leaven_lm::Lm`] behavior without
//! network, credentials, provider retries, or prompt-sensitive branching.

mod client;
mod script;

pub use client::MockLm;
pub use script::MockLmScript;
