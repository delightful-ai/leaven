//! Agentic Git program materialization and readback adapters.

mod error;
mod git_ops;
mod program;
mod stores;

pub use error::GitAgenticGitError;
pub use program::{GitProgramMaterializer, GitProgramReadback};
pub use stores::GitProgramStores;
