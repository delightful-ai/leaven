//! Workspace allocation config.

use crate::{FilesystemPolicy, NetworkPolicy};

#[derive(Clone, Debug, Default)]
pub struct WorkspaceConfig {
    pub filesystem: FilesystemPolicy,
    pub network: NetworkPolicy,
}
