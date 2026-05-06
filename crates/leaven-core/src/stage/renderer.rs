//! `Renderer` — turns opaque artifacts/evidence into consumer-specific
//! views (prompts, JSON, debug HTML, workspace layouts).
//!
//! v0.2 split rendering into two trait families:
//!
//! - `Renderer<P, T, Target>` — value-returning (prompt context, JSON
//!   blob, debug HTML).
//! - `WorkspaceRenderer<P, T>` — side-effecting (writes files into a
//!   sandbox).
//!
//! Stubs only at this stage.

use crate::ids::RendererId;

pub trait Renderer: Send + Sync {
    fn id(&self) -> RendererId;
}
