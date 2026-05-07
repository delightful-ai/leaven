//! Workspace substrate for side-effectful stages.

pub mod command;
pub mod config;
pub mod error;
pub mod factory;
pub mod path;
pub mod policy;
pub mod view;
pub mod workspace;

pub use command::{Command, CommandOutput, ExitStatus};
pub use config::WorkspaceConfig;
pub use error::{FactoryError, WorkspaceError, WorkspacePathError};
pub use factory::WorkspaceFactory;
pub use path::WorkspacePath;
pub use policy::{FilesystemPolicy, NetworkPolicy};
pub use view::WorkspaceView;
pub use workspace::{Workspace, WorkspaceBackend};

pub mod prelude {
    //! Common workspace imports.

    pub use crate::{
        Command, CommandOutput, FactoryError, Workspace, WorkspaceBackend, WorkspaceConfig,
        WorkspaceError, WorkspaceFactory, WorkspacePath, WorkspacePathError, WorkspaceView,
    };
}
