//! Store wiring for product-level optimization runs.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use leaven_core::Artifact;
use leaven_core::OptimizationProblem;
use leaven_engine::{RunPersistence, StoreRunPersistence};
use leaven_store::{EvidenceStore, StoreError};
use leaven_store_file::{FileEvidenceStore, FileStore};
use leaven_store_inline::InlineEvidenceStore;
use serde::{Serialize, de::DeserializeOwned};

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

pub(crate) struct LocalOptimizeStore<P>
where
    P: OptimizationProblem,
{
    pub(crate) store: OptimizeStore<P>,
    pub(crate) persistence: StoreRunPersistence<FileStore>,
    pub(crate) run_dir: PathBuf,
}

impl<P> LocalOptimizeStore<P>
where
    P: OptimizationProblem,
    P::Artifact: Serialize + DeserializeOwned,
    <P::Artifact as Artifact>::Change: Serialize + DeserializeOwned,
    P::Evidence: Clone + Serialize + DeserializeOwned,
    P::ProposalAnnotations: Serialize + DeserializeOwned,
{
    pub(crate) fn open(run_dir: impl AsRef<Path>) -> Result<Self, StoreError> {
        let run_dir = run_dir.as_ref().to_path_buf();
        let file_store = FileStore::open(&run_dir)?;
        let evidence =
            FileEvidenceStore::<P::Evidence>::open("leaven-run", run_dir.join("evidence"))?;
        let persistence = StoreRunPersistence::new(file_store);
        let store = OptimizeStore::durable(evidence, persistence.clone());
        Ok(Self {
            store,
            persistence,
            run_dir,
        })
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
