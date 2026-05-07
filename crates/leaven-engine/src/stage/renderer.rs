//! Renderer stage traits.

use leaven_core::OptimizationProblem;
use leaven_kernel::Metered;

#[allow(async_fn_in_trait)]
pub trait Renderer<P: OptimizationProblem, T, Target>: Send + Sync {
    type View;

    async fn render(
        &self,
        value: &T,
        target: Target,
        ctx: crate::RenderContext<'_, P>,
    ) -> Result<Metered<Self::View>, RenderError>;
}

#[allow(async_fn_in_trait)]
pub trait Materializer<P: OptimizationProblem, T>: Send + Sync {
    async fn materialize_into(
        &self,
        value: &T,
        workspace: &mut leaven_workspace::WorkspaceView<'_>,
        ctx: crate::MaterializeContext<'_, P>,
    ) -> Result<Metered<MaterializationReport>, MaterializeError>;
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MaterializationReport {
    pub files_written: usize,
    pub bytes_written: u64,
    pub truncations: Vec<TruncationNote>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TruncationNote {
    pub path: Option<String>,
    pub reason: String,
}

#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    #[error("render failed: {0}")]
    Message(String),
}

#[derive(Debug, thiserror::Error)]
pub enum MaterializeError {
    #[error("materialize failed: {0}")]
    Message(String),
    #[error(transparent)]
    Workspace(#[from] leaven_workspace::WorkspaceError),
    #[error(transparent)]
    Path(#[from] leaven_workspace::WorkspacePathError),
}
