//! Agentic Git program materialization and readback adapters.

mod error;
mod program;
mod stores;

pub use error::GitAgenticGitError;
pub use program::{GitProgramMaterializer, GitProgramReadback};
pub use stores::GitProgramStores;
