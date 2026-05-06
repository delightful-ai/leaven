//! Trait surfaces for the configurable stages of an optimization:
//!
//! - [`proposer::Proposer`] — produces typed proposal batches.
//! - [`evaluator::Evaluator`] — runs candidates against the world.
//! - [`renderer::Renderer`] — turns opaque values into consumer views.
//! - [`callback::Callback`] — observes events.
//! - [`stopper::Stopper`] — votes on early termination.
//!
//! Full async surfaces land alongside the engine; the synchronous
//! marker traits here are enough for the cold core to type-check.

pub mod callback;
pub mod evaluator;
pub mod proposer;
pub mod renderer;
pub mod stopper;
