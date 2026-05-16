//! Product-run store preparation.

use std::path::{Path, PathBuf};

use leaven_core::{Artifact, OptimizationProblem};
use leaven_engine::{SqliteEvaluationCache, StoreRunPersistence};
use leaven_kernel::{CheckpointId, RunId};
use leaven_store::CheckpointStore;
use serde::{Serialize, de::DeserializeOwned};

use crate::{
    OptimizeError, OptimizeStore,
    store::{LocalOptimizeStore, LocalOptimizeStoreOpenError},
};

pub enum StoreSource {
    DefaultDurable,
    RunDir(PathBuf),
    Ephemeral,
}

pub enum StoreConfig<P>
where
    P: OptimizationProblem,
{
    Source(StoreSource),
    Explicit(OptimizeStore<P>),
}

impl<P> StoreConfig<P>
where
    P: OptimizationProblem,
{
    pub fn into_source(self) -> StoreSource {
        match self {
            Self::Source(source) => source,
            Self::Explicit(_) => StoreSource::DefaultDurable,
        }
    }
}

pub struct PreparedStore<P>
where
    P: OptimizationProblem,
{
    pub store: OptimizeStore<P>,
    pub run_dir: Option<PathBuf>,
    pub local_persistence: Option<StoreRunPersistence<leaven_store_file::FileStore>>,
    pub evaluation_cache: Option<SqliteEvaluationCache>,
    pub start: StoreStart<P>,
}

pub enum StoreStart<P>
where
    P: OptimizationProblem,
{
    Fresh {
        run_id: RunId,
    },
    Resume {
        checkpoint: Box<leaven_engine::RunCheckpoint>,
        restored: Box<leaven_engine::RestoredRunState<P>>,
    },
}

pub fn prepare_store<P>(
    config: StoreConfig<P>,
    run_id: RunId,
) -> Result<PreparedStore<P>, OptimizeError>
where
    P: OptimizationProblem,
    P::Artifact: Serialize + DeserializeOwned,
    <P::Artifact as Artifact>::Change: Serialize + DeserializeOwned,
    P::Evidence: Clone + Serialize + DeserializeOwned,
    P::ProposalAnnotations: Serialize + DeserializeOwned,
{
    match config {
        StoreConfig::Source(StoreSource::DefaultDurable) => {
            prepare_local_store(&default_local_run_dir(run_id), run_id)
        }
        StoreConfig::Source(StoreSource::RunDir(run_dir)) => prepare_local_store(&run_dir, run_id),
        StoreConfig::Source(StoreSource::Ephemeral) => Ok(PreparedStore {
            store: OptimizeStore::inline("leaven-run"),
            run_dir: None,
            local_persistence: None,
            evaluation_cache: None,
            start: StoreStart::Fresh { run_id },
        }),
        StoreConfig::Explicit(store) => Ok(PreparedStore {
            store,
            run_dir: None,
            local_persistence: None,
            evaluation_cache: None,
            start: StoreStart::Fresh { run_id },
        }),
    }
}

fn prepare_local_store<P>(run_dir: &Path, run_id: RunId) -> Result<PreparedStore<P>, OptimizeError>
where
    P: OptimizationProblem,
    P::Artifact: Serialize + DeserializeOwned,
    <P::Artifact as Artifact>::Change: Serialize + DeserializeOwned,
    P::Evidence: Clone + Serialize + DeserializeOwned,
    P::ProposalAnnotations: Serialize + DeserializeOwned,
{
    let local = LocalOptimizeStore::<P>::open(run_dir)
        .map_err(LocalOptimizeStoreOpenError::into_optimize_error)?;
    let restored = local
        .persistence
        .latest_checkpoint::<P>()
        .map_err(|source| OptimizeError::Resume {
            operation: "read latest checkpoint",
            source,
        })?;
    let start = match restored {
        Some(restored) => StoreStart::Resume {
            checkpoint: Box::new(restored.checkpoint.clone()),
            restored: Box::new(restored),
        },
        None => StoreStart::Fresh { run_id },
    };
    Ok(PreparedStore {
        store: local.store,
        run_dir: Some(local.run_dir),
        local_persistence: Some(local.persistence),
        evaluation_cache: Some(local.evaluation_cache),
        start,
    })
}

/// Returns the Leaven-managed local run directory for a fresh run id.
#[must_use]
pub fn default_local_run_dir(run_id: RunId) -> PathBuf {
    PathBuf::from(".leaven")
        .join("runs")
        .join(run_id.to_string())
}

pub fn has_persistence<P>(store: &PreparedStore<P>) -> bool
where
    P: OptimizationProblem,
{
    store.store.persistence().is_some()
}

pub fn latest_checkpoint<P>(store: &PreparedStore<P>) -> Result<Option<CheckpointId>, OptimizeError>
where
    P: OptimizationProblem,
{
    let Some(persistence) = &store.local_persistence else {
        return Ok(None);
    };
    persistence
        .store()
        .latest()
        .map_err(|source| OptimizeError::Store {
            operation: "read latest checkpoint pointer",
            source,
        })
}
