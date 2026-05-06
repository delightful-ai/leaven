//! Workspace factories.

use crate::{FactoryError, Workspace, WorkspaceConfig};

#[allow(async_fn_in_trait)]
pub trait WorkspaceFactory: Send + Sync {
    async fn allocate(&self, config: WorkspaceConfig) -> Result<Workspace, FactoryError>;
}
