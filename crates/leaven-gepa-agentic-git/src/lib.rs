//! GEPA agentic reflection adapter for Git program artifacts.
//!
//! Status: behavior-bearing bridge scaffold.
//!
//! The owning product-backend spec is
//! `docs/specs/agentic_trace_reflection_product_backend.md`. This crate is the
//! topology home for the GEPA reflection bridge over `GitProgramArtifact`.
//! Keep this file maps-only: behavior belongs in named modules.

mod input;
mod materializer;
mod parser;
mod reflector;
mod renderer;

pub use input::GitProgramGepaReflectionInput;
pub use materializer::GitProgramGepaReflectionMaterializer;
pub use parser::GitProgramGepaReflectionParser;
pub use reflector::GepaGitProgramAgenticReflector;
pub use renderer::GepaGitProgramReflectionRenderer;

pub mod prelude {
    pub use crate::{
        GepaGitProgramAgenticReflector, GepaGitProgramReflectionRenderer,
        GitProgramGepaReflectionInput, GitProgramGepaReflectionMaterializer,
        GitProgramGepaReflectionParser,
    };
}
