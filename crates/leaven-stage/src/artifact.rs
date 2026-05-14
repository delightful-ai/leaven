use leaven_core::Artifact;
use leaven_kernel::Cost;
use leaven_workspace::{WorkspaceError, WorkspaceFactoryContextError, WorkspaceSlot};

use crate::{Diagnostic, WorkspaceEntryReceipt, WorkspaceSetupError};

#[allow(async_fn_in_trait)]
pub trait MaterializableArtifact: Artifact {
    async fn write_to(
        &self,
        slot: &mut WorkspaceSlot<'_>,
    ) -> Result<MaterializationReport, WorkspaceSetupError>;

    async fn read_back_change(
        &self,
        slot: &WorkspaceSlot<'_>,
    ) -> Result<Option<Self::Change>, ArtifactReadbackError>;
}

#[allow(async_fn_in_trait)]
pub trait ReconstructibleArtifact: MaterializableArtifact {
    async fn parse_from(slot: &WorkspaceSlot<'_>) -> Result<Self, ArtifactReadbackError>
    where
        Self: Sized;
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct MaterializationReport {
    pub entries: Vec<WorkspaceEntryReceipt>,
    pub diagnostics: Vec<Diagnostic>,
    pub cost: Cost,
}

#[derive(Debug, thiserror::Error)]
pub enum ArtifactReadbackError {
    #[error(transparent)]
    Workspace(#[from] WorkspaceError),
    #[error(transparent)]
    FactoryContext(#[from] WorkspaceFactoryContextError),
    #[error("artifact readback failed: {0}")]
    InvalidArtifact(String),
}
