//! Store wiring for product-level optimization runs.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use leaven_core::Artifact;
use leaven_core::OptimizationProblem;
use leaven_engine::{
    EvaluationCacheStoreError, RunPersistence, SqliteEvaluationCache, StoreRunPersistence,
};
use leaven_store::{EvidenceStore, StoreError};
use leaven_store_file::{FileEvidenceStore, FileStore};
use leaven_store_inline::InlineEvidenceStore;
use serde::{Serialize, de::DeserializeOwned};
use thiserror::Error;

use crate::OptimizeError;

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

pub struct LocalOptimizeStore<P>
where
    P: OptimizationProblem,
{
    pub(super) store: OptimizeStore<P>,
    pub(super) persistence: StoreRunPersistence<FileStore>,
    pub(super) evaluation_cache: SqliteEvaluationCache,
    pub(super) run_dir: PathBuf,
}

impl<P> LocalOptimizeStore<P>
where
    P: OptimizationProblem,
    P::Artifact: Serialize + DeserializeOwned,
    <P::Artifact as Artifact>::Change: Serialize + DeserializeOwned,
    P::Evidence: Clone + Serialize + DeserializeOwned,
    P::ProposalAnnotations: Serialize + DeserializeOwned,
{
    pub fn open(run_dir: impl AsRef<Path>) -> Result<Self, LocalOptimizeStoreOpenError> {
        let run_dir = run_dir.as_ref().to_path_buf();
        let file_store =
            FileStore::open(&run_dir).map_err(|source| LocalOptimizeStoreOpenError::Store {
                operation: "open local run store",
                source,
            })?;
        let evidence =
            FileEvidenceStore::<P::Evidence>::open("leaven-run", run_dir.join("evidence"))
                .map_err(|source| LocalOptimizeStoreOpenError::Store {
                    operation: "open local run evidence store",
                    source,
                })?;
        let persistence = StoreRunPersistence::new(file_store);
        let evaluation_cache =
            SqliteEvaluationCache::open(run_dir.join("run.sqlite")).map_err(|source| {
                LocalOptimizeStoreOpenError::EvaluationCache {
                    operation: "open sqlite evaluation cache",
                    source,
                }
            })?;
        let store = OptimizeStore::durable(evidence, persistence.clone());
        Ok(Self {
            store,
            persistence,
            evaluation_cache,
            run_dir,
        })
    }
}

#[derive(Debug, Error)]
pub enum LocalOptimizeStoreOpenError {
    #[error("run store failed during {operation}")]
    Store {
        operation: &'static str,
        #[source]
        source: StoreError,
    },
    #[error("evaluation cache failed during {operation}")]
    EvaluationCache {
        operation: &'static str,
        #[source]
        source: EvaluationCacheStoreError,
    },
}

impl LocalOptimizeStoreOpenError {
    pub fn into_optimize_error(self) -> OptimizeError {
        match self {
            Self::Store { operation, source } => OptimizeError::Store { operation, source },
            Self::EvaluationCache { operation, source } => {
                OptimizeError::EvaluationCache { operation, source }
            }
        }
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

#[cfg(test)]
mod tests {
    use leaven_core::{ArtifactIdentity, Evidence};
    use leaven_kernel::{ContentId, RunId};
    use serde::{Deserialize, Serialize};

    use super::*;

    #[test]
    fn local_optimize_store_open_wires_file_persistence_and_sqlite_cache() {
        let run_dir = test_run_dir("open");

        let local = LocalOptimizeStore::<TestProblem>::open(&run_dir).unwrap();

        assert_eq!(local.run_dir, run_dir);
        assert!(local.store.persistence().is_some());
        assert_eq!(
            local.evaluation_cache.path(),
            local.run_dir.join("run.sqlite")
        );
        assert!(local.run_dir.join("evidence").is_dir());
        assert!(local.run_dir.join("run.sqlite").is_file());
        std::fs::remove_dir_all(&run_dir).unwrap();
    }

    #[test]
    fn local_optimize_store_open_maps_sqlite_cache_failures_to_optimize_errors() {
        let run_dir = test_run_dir("sqlite-cache-error");
        std::fs::create_dir_all(run_dir.join("run.sqlite")).unwrap();

        let result = LocalOptimizeStore::<TestProblem>::open(&run_dir);
        assert!(result.is_err());
        let error = result.err().unwrap().into_optimize_error();

        assert!(matches!(
            error,
            OptimizeError::EvaluationCache {
                operation: "open sqlite evaluation cache",
                ..
            }
        ));
        std::fs::remove_dir_all(&run_dir).unwrap();
    }

    #[test]
    fn local_optimize_store_open_maps_evidence_store_failures_to_optimize_errors() {
        let run_dir = test_run_dir("evidence-store-error");
        std::fs::create_dir_all(&run_dir).unwrap();
        std::fs::write(run_dir.join("evidence"), b"not a directory").unwrap();

        let result = LocalOptimizeStore::<TestProblem>::open(&run_dir);
        assert!(result.is_err());
        let error = result.err().unwrap().into_optimize_error();

        assert!(matches!(
            error,
            OptimizeError::Store {
                operation: "open local run evidence store",
                ..
            }
        ));
        std::fs::remove_dir_all(&run_dir).unwrap();
    }

    #[test]
    fn test_problem_helpers_are_exercised_by_store_tests() {
        assert_eq!(TestArtifactError.to_string(), "test artifact error");
        assert!(matches!(
            TestArtifact.identity(),
            ArtifactIdentity::Content(_)
        ));
        TestArtifact.apply_change(&()).unwrap();
        let store = OptimizeStore::<TestProblem>::inline("test");
        let converted = store.into_optimize_store();
        assert!(converted.persistence().is_none());
    }

    fn test_run_dir(label: &str) -> PathBuf {
        std::env::temp_dir()
            .join("leaven-run-store-tests")
            .join(label)
            .join(RunId::new().to_string())
    }

    #[derive(Clone, Debug, Serialize, Deserialize)]
    struct TestArtifact;

    #[derive(Debug)]
    struct TestArtifactError;

    impl std::fmt::Display for TestArtifactError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("test artifact error")
        }
    }

    impl std::error::Error for TestArtifactError {}

    impl Artifact for TestArtifact {
        type Change = ();
        type ApplyError = TestArtifactError;

        fn identity(&self) -> ArtifactIdentity {
            ArtifactIdentity::Content(ContentId::from_bytes([1; 32]))
        }

        fn apply_change(&self, _change: &Self::Change) -> Result<Self, Self::ApplyError> {
            Ok(self.clone())
        }
    }

    #[derive(Clone, Debug, Serialize, Deserialize)]
    struct TestEvidence;

    impl Evidence for TestEvidence {}

    struct TestProblem;

    impl OptimizationProblem for TestProblem {
        type Artifact = TestArtifact;
        type Case = ();
        type Evidence = TestEvidence;
        type ProposalAnnotations = ();
    }
}
