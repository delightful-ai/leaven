//! Optimizer trait.

use leaven_core::OptimizationProblem;
use leaven_kernel::CandidateId;

use crate::RunGraphView;

#[allow(async_fn_in_trait)]
pub trait Optimizer<P: OptimizationProblem>: Send {
    async fn initialize(
        &mut self,
        _ctx: &mut crate::RunContext<'_, P>,
    ) -> Result<(), OptimizerError> {
        Ok(())
    }

    async fn step(
        &mut self,
        ctx: &mut crate::RunContext<'_, P>,
    ) -> Result<StepStatus, OptimizerError>;

    fn best_candidate(&self, graph: RunGraphView<'_, P>) -> Option<CandidateId>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum StepStatus {
    Continue,
    Done,
}

#[derive(Debug, thiserror::Error)]
pub enum OptimizerError {
    #[error("optimizer failed: {0}")]
    Message(String),
    #[error("optimizer failed: {message}")]
    WithSource {
        message: String,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync + 'static>,
    },
}

impl OptimizerError {
    #[must_use]
    pub fn with_source(
        message: impl Into<String>,
        source: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        Self::WithSource {
            message: message.into(),
            source: Box::new(source),
        }
    }
}
