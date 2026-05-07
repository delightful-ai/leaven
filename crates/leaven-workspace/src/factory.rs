//! Workspace factories.

use std::future::Future;

use crate::{FactoryError, Workspace, WorkspaceConfig};

pub trait WorkspaceFactory: Send + Sync {
    fn allocate(
        &self,
        config: WorkspaceConfig,
    ) -> impl Future<Output = Result<Workspace, FactoryError>> + Send + '_;
}
