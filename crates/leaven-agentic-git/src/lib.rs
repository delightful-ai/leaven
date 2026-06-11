//! Agentic Git program materialization and readback adapters.

mod error;
mod git_ops;
mod program;
mod seed;
mod stores;

pub use error::GitAgenticGitError;
pub use program::{GitProgramMaterializer, GitProgramReadback};
pub use seed::{GitProgramSeed, build_program_seed, read_revision_files};
pub use stores::GitProgramStores;
