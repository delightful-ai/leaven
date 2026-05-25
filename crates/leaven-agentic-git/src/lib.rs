//! Agentic Git program materialization and readback adapters.

mod program;

pub use program::{
    GitAgenticGitError, GitProgramMaterializer, GitProgramReadback, GitProgramStores,
};
