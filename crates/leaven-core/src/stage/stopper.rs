//! `Stopper` — votes on early termination.
//!
//! Stub: implementations live in `leaven-engine` (budget stoppers,
//! plateau detectors, external triggers).

pub trait Stopper: Send + Sync {}
