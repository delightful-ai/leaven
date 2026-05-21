use std::collections::BTreeMap;
use std::time::Duration;

use leaven_workspace::{
    CapturedOutput, CommandUser, ExitStatus, WorkspacePath, WorkspacePathError,
};

use crate::{FirkinContainerId, FirkinGuestPath, FirkinImageRef, FirkinProductPodId};

pub trait FirkinWorkspaceRuntime: Send + Sync + 'static {
    fn allocate_container(
        &self,
        request: FirkinWorkspaceAllocation,
    ) -> Result<FirkinContainerId, FirkinRuntimeError>;

    fn write_file(
        &self,
        container: &FirkinContainerId,
        path: &FirkinGuestPath,
        bytes: &[u8],
    ) -> Result<(), FirkinRuntimeError>;

    fn read_file(
        &self,
        container: &FirkinContainerId,
        path: &FirkinGuestPath,
    ) -> Result<Vec<u8>, FirkinRuntimeError>;

    fn list_files(
        &self,
        container: &FirkinContainerId,
        root: &FirkinGuestPath,
    ) -> Result<Vec<WorkspacePath>, FirkinRuntimeError>;

    fn run_command(
        &self,
        container: &FirkinContainerId,
        request: FirkinCommandRequest,
    ) -> Result<FirkinCommandResult, FirkinRuntimeError>;

    fn remove_container(&self, container: FirkinContainerId) -> Result<(), FirkinRuntimeError>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FirkinWorkspaceAllocation {
    product_pod_id: FirkinProductPodId,
    workspace_root: FirkinGuestPath,
    volume_mount_root: FirkinGuestPath,
    image: FirkinImageRef,
}

impl FirkinWorkspaceAllocation {
    #[must_use]
    pub fn new(
        product_pod_id: FirkinProductPodId,
        workspace_root: FirkinGuestPath,
        image: FirkinImageRef,
    ) -> Self {
        Self {
            product_pod_id,
            volume_mount_root: workspace_root.clone(),
            workspace_root,
            image,
        }
    }

    #[must_use]
    pub fn with_volume_mount_root(mut self, volume_mount_root: FirkinGuestPath) -> Self {
        self.volume_mount_root = volume_mount_root;
        self
    }

    #[must_use]
    pub const fn product_pod_id(&self) -> &FirkinProductPodId {
        &self.product_pod_id
    }

    #[must_use]
    pub const fn workspace_root(&self) -> &FirkinGuestPath {
        &self.workspace_root
    }

    #[must_use]
    pub const fn volume_mount_root(&self) -> &FirkinGuestPath {
        &self.volume_mount_root
    }

    #[must_use]
    pub const fn image(&self) -> &FirkinImageRef {
        &self.image
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FirkinCommandRequest {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: FirkinGuestPath,
    pub env: BTreeMap<String, String>,
    pub stdin: Vec<u8>,
    pub timeout: Option<Duration>,
    pub max_stdout_bytes: Option<u64>,
    pub max_stderr_bytes: Option<u64>,
    pub user: Option<CommandUser>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FirkinCommandResult {
    pub status: ExitStatus,
    pub stdout: CapturedOutput,
    pub stderr: CapturedOutput,
    pub duration: Duration,
}

#[derive(Debug, thiserror::Error)]
pub enum FirkinRuntimeError {
    #[error("invalid Firkin placement field `{field}` with value `{value}`: {reason}")]
    InvalidPlacement {
        field: &'static str,
        value: String,
        reason: &'static str,
    },

    #[error("Firkin runtime operation `{operation}` failed: {reason}")]
    Runtime {
        operation: &'static str,
        reason: String,
    },

    #[error(transparent)]
    WorkspacePath(#[from] WorkspacePathError),
}
