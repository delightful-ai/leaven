//! Store wiring for product-level optimization runs.

use std::sync::Arc;

use leaven_core::OptimizationProblem;
use leaven_engine::RunPersistence;
use leaven_store::EvidenceStore;
use leaven_store_inline::InlineEvidenceStore;

/// Evidence and optional checkpoint persistence used by a product run.
pub struct OptimizeStore<P>
where
    P: OptimizationProblem,
{
    evidence: Arc<dyn EvidenceStore<P::Evidence>>,
    persistence: Option<Arc<dyn RunPersistence<P>>>,
}

impl<P> Clone for OptimizeStore<P>
where
    P: OptimizationProblem,
{
    fn clone(&self) -> Self {
        Self {
            evidence: Arc::clone(&self.evidence),
            persistence: self.persistence.as_ref().map(Arc::clone),
        }
    }
}

impl<P> OptimizeStore<P>
where
    P: OptimizationProblem,
{
    /// Build a store from an evidence capability only.
    #[must_use]
    pub fn evidence<S>(evidence: S) -> Self
    where
        S: EvidenceStore<P::Evidence> + 'static,
    {
        Self {
            evidence: Arc::new(evidence),
            persistence: None,
        }
    }

    /// Build a store from evidence and checkpoint persistence capabilities.
    #[must_use]
    pub fn durable<S, R>(evidence: S, persistence: R) -> Self
    where
        S: EvidenceStore<P::Evidence> + 'static,
        R: RunPersistence<P> + 'static,
    {
        Self {
            evidence: Arc::new(evidence),
            persistence: Some(Arc::new(persistence)),
        }
    }

    /// Build the default inline evidence store for non-resumable runs.
    #[must_use]
    pub fn inline(name: impl Into<String>) -> Self
    where
        P::Evidence: Clone + 'static,
    {
        Self::evidence(InlineEvidenceStore::<P::Evidence>::new(name))
    }

    pub(crate) fn evidence_store(&self) -> &dyn EvidenceStore<P::Evidence> {
        self.evidence.as_ref()
    }

    pub(crate) fn persistence(&self) -> Option<Arc<dyn RunPersistence<P>>> {
        self.persistence.as_ref().map(Arc::clone)
    }
}

/// Converts public store inputs into product-run store wiring.
pub trait IntoOptimizeStore<P>
where
    P: OptimizationProblem,
{
    /// Convert into an [`OptimizeStore`].
    fn into_optimize_store(self) -> OptimizeStore<P>;
}

impl<P> IntoOptimizeStore<P> for OptimizeStore<P>
where
    P: OptimizationProblem,
{
    fn into_optimize_store(self) -> Self {
        self
    }
}
