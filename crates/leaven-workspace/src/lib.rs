//! Workspace substrate for side-effectful stages.

pub mod command;
pub mod config;
pub mod error;
pub mod factory;
pub mod path;
pub mod policy;
pub mod view;
pub mod workspace;

pub use command::{
    CapturedOutput, Command, CommandLimits, CommandOutput, CommandStdin, CommandUser, ExitStatus,
};
pub use config::WorkspaceConfig;
pub use error::{FactoryError, WithWorkspaceError, WorkspaceError, WorkspacePathError};
pub use factory::WorkspaceFactory;
pub use path::WorkspacePath;
pub use policy::{FilesystemPolicy, NetworkPolicy};
pub use view::WorkspaceView;
pub use workspace::{Workspace, WorkspaceBackend, with_workspace};

pub mod prelude {
    //! Common workspace imports.

    pub use crate::{
        CapturedOutput, Command, CommandLimits, CommandOutput, CommandStdin, CommandUser,
        FactoryError, WithWorkspaceError, Workspace, WorkspaceBackend, WorkspaceConfig,
        WorkspaceError, WorkspaceFactory, WorkspacePath, WorkspacePathError, WorkspaceView,
        with_workspace,
    };
}
