//! Renderer stage traits.

use std::future::Future;

use leaven_core::OptimizationProblem;
use leaven_kernel::Metered;

pub trait Renderer<P: OptimizationProblem, T, Target>: Send + Sync {
    type View;

    fn render<'a>(
        &'a self,
        value: &'a T,
        target: Target,
        ctx: crate::RenderContext<'a, P>,
    ) -> impl Future<Output = Result<Metered<Self::View>, RenderError>> + Send + 'a;
}

pub trait Materializer<P: OptimizationProblem, T>: Send + Sync {
    fn materialize_into<'a>(
        &'a self,
        value: &'a T,
        workspace: &'a mut leaven_workspace::WorkspaceView<'_>,
        ctx: crate::MaterializeContext<'a, P>,
    ) -> impl Future<Output = Result<Metered<MaterializationReport>, MaterializeError>> + Send + 'a;
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
