//! Evaluator stage trait.

use futures::future::LocalBoxFuture;
use leaven_core::{Assessment, OptimizationProblem, ResolvedEvaluationRequest};
use leaven_kernel::{EvaluatorId, Fingerprint, Metered};

use crate::CachePolicy;

/// Static evaluator capability.
///
/// Evaluators turn resolved evaluation requests into metered assessment
/// evidence. The fingerprint and cache policy are part of the cache contract:
/// if either changes, cached assessment IDs must not be reused.
#[allow(async_fn_in_trait)]
pub trait Evaluator<P: OptimizationProblem>: Send + Sync {
    /// Stable evaluator identity for events, budget accounting, and errors.
    fn id(&self) -> EvaluatorId;
    /// Content fingerprint for cache keys.
    fn fingerprint(&self) -> Fingerprint;

    /// Cache policy for a resolved request.
    fn cache_policy(&self, _request: &ResolvedEvaluationRequest) -> CachePolicy {
        CachePolicy::Never
    }

    /// Evaluate candidates and return metered assessments.
    async fn evaluate(
        &self,
        request: ResolvedEvaluationRequest,
        ctx: crate::EvaluationContext<'_, P>,
    ) -> Result<Metered<Vec<Assessment<P>>>, EvaluationError>;
}

/// Object-safe evaluator capability for heterogeneous evaluator registries.
pub trait DynEvaluator<P: OptimizationProblem>: Send + Sync {
    /// Stable evaluator identity for events, budget accounting, and errors.
    fn id(&self) -> EvaluatorId;
    /// Dispatch an evaluator call through an object-safe future.
    fn evaluate_boxed<'a>(
        &'a self,
        request: ResolvedEvaluationRequest,
        ctx: crate::EvaluationContext<'a, P>,
    ) -> LocalBoxFuture<'a, Result<Metered<Vec<Assessment<P>>>, EvaluationError>>;
}

impl<P, T> DynEvaluator<P> for T
where
    P: OptimizationProblem,
    T: Evaluator<P>,
{
    fn id(&self) -> EvaluatorId {
        Evaluator::id(self)
    }

    fn evaluate_boxed<'a>(
        &'a self,
        request: ResolvedEvaluationRequest,
        ctx: crate::EvaluationContext<'a, P>,
    ) -> LocalBoxFuture<'a, Result<Metered<Vec<Assessment<P>>>, EvaluationError>> {
        Box::pin(self.evaluate(request, ctx))
    }
}

/// Failures an evaluator can report at the capability boundary.
#[derive(Debug, thiserror::Error)]
pub enum EvaluationError {
    /// Generic evaluator failure when no better public variant applies.
    #[error("evaluation failed: {0}")]
    Message(String),
    /// Evaluator failure with a structured source error.
    #[error("evaluation failed: {message}")]
    WithSource {
        message: String,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync + 'static>,
    },
}

impl EvaluationError {
    /// Build an evaluator error while preserving a source error chain.
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
