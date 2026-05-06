//! `Callback` — observes [`crate::graph::events::RunEvent`]s.
//!
//! Stub: the full surface (sync vs async, structured payloads, error
//! handling) lands with the engine.

pub trait Callback: Send + Sync {}
