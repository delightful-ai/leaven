//! Workspace substrate for side-effectful stages.

pub mod command;
pub mod config;
pub mod context;
pub mod error;
pub mod factory;
pub mod fingerprint;
pub mod path;
pub mod policy;
pub mod slot;
pub mod view;
pub mod workspace;

pub use command::{
    CapturedOutput, Command, CommandLimits, CommandOutput, CommandStdin, CommandUser, ExitStatus,
};
pub use config::WorkspaceConfig;
pub use context::{
    WorkspaceFactoryContext, WorkspaceFactoryContextBuilder, WorkspaceFactoryContextError,
};
pub use error::{FactoryError, WithWorkspaceError, WorkspaceError, WorkspacePathError};
pub use factory::WorkspaceFactory;
pub use fingerprint::{
    WorkspaceFileFingerprint, WorkspaceTreeFingerprint, fingerprint_file, fingerprint_tree,
};
pub use path::WorkspacePath;
pub use policy::{FilesystemPolicy, NetworkPolicy};
pub use slot::WorkspaceSlot;
pub use view::WorkspaceView;
pub use workspace::{Workspace, WorkspaceBackend, with_workspace};

pub mod prelude {
    //! Common workspace imports.

    pub use crate::{
        CapturedOutput, Command, CommandLimits, CommandOutput, CommandStdin, CommandUser,
        FactoryError, WithWorkspaceError, Workspace, WorkspaceBackend, WorkspaceConfig,
        WorkspaceError, WorkspaceFactory, WorkspaceFactoryContext, WorkspaceFactoryContextBuilder,
        WorkspaceFactoryContextError, WorkspaceFileFingerprint, WorkspacePath, WorkspacePathError,
        WorkspaceSlot, WorkspaceTreeFingerprint, WorkspaceView, fingerprint_file,
        fingerprint_tree, with_workspace,
    };
}
