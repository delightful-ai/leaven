//! Public run-builder refusal modes.

use leaven_engine::{
    EvaluationCacheStoreError, OptimizerError, RunContextError, RunPersistenceError,
};
use leaven_eval::DatasetError;
use leaven_store::StoreError;

use crate::compatibility::{ResumeCompatibilityError, RuntimeKind};

/// Errors returned by the high-level optimization builder.
#[derive(Debug, thiserror::Error)]
pub enum OptimizeError {
    /// The caller did not choose a budget.
    #[error("explicit budget is required; call `.budget(...)` or `.budget(Budget::unlimited())`")]
    MissingBudget,
    /// The caller did not install a scoring function.
    #[error("score function is required")]
    MissingScore,
    /// Validation or test cases were supplied without train cases.
    #[error("validation/test cases require at least one train case")]
    HeldOutWithoutTrain,
    /// The supplied case envelopes were not a valid dataset.
    #[error("dataset construction failed")]
    Dataset {
        #[source]
        source: DatasetError,
    },
    /// The seed artifact could not be inserted into graph truth.
    #[error("seed insertion failed")]
    SeedInsertion {
        #[source]
        source: RunContextError,
    },
    /// A local/default run store could not be opened or inspected.
    #[error("run store failed during {operation}")]
    Store {
        operation: &'static str,
        #[source]
        source: StoreError,
    },
    /// A stored run could not be restored from checkpoint state.
    #[error("run resume failed during {operation}")]
    Resume {
        operation: &'static str,
        #[source]
        source: RunPersistenceError,
    },
    /// Durable execution cannot proceed because a live runtime has no stable behavior fingerprint.
    #[error("durable run requires an explicit {runtime} runtime fingerprint")]
    RuntimeFingerprintMissing {
        /// Runtime slot missing a stable fingerprint.
        runtime: &'static str,
    },
    /// Durable resume compatibility rejected the live run inputs.
    #[error(transparent)]
    ResumeCompatibility(Box<ResumeCompatibilityError>),
    /// Case content could not be fingerprinted for durable compatibility.
    #[error("case content fingerprinting failed")]
    CaseFingerprint {
        /// Serialization failure while deriving case identity.
        #[source]
        source: serde_json::Error,
    },
    /// Compatibility manifest storage failed.
    #[error("compatibility manifest failed during {operation}")]
    CompatibilityStore {
        operation: &'static str,
        #[source]
        source: std::io::Error,
    },
    /// Durable report storage failed.
    #[error("run report failed during {operation}")]
    ReportStore {
        operation: &'static str,
        #[source]
        source: std::io::Error,
    },
    /// Durable evaluation-cache storage failed.
    #[error("evaluation cache failed during {operation}")]
    EvaluationCache {
        operation: &'static str,
        #[source]
        source: EvaluationCacheStoreError,
    },
    /// A restored graph did not contain the seed candidate needed for reports.
    #[error("restored run graph does not contain a seed candidate")]
    MissingRestoredSeed,
    /// The engine or optimizer refused the run.
    #[error(transparent)]
    Optimizer(#[from] OptimizerError),
}

impl RuntimeKind {
    pub(crate) fn missing_error(self) -> OptimizeError {
        OptimizeError::RuntimeFingerprintMissing {
            runtime: self.as_str(),
        }
    }
}

impl From<DatasetError> for OptimizeError {
    fn from(source: DatasetError) -> Self {
        Self::Dataset { source }
    }
}

impl From<ResumeCompatibilityError> for OptimizeError {
    fn from(source: ResumeCompatibilityError) -> Self {
        Self::ResumeCompatibility(Box::new(source))
    }
}
