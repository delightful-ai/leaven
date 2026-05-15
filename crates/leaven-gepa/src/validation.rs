//! GEPA validation and train-batch policies.

use leaven_core::{EvaluationSet, PartitionId};
use leaven_kernel::CandidateId;
use serde::{Deserialize, Serialize};

/// Selects the train/search cases used for one GEPA feedback iteration.
pub trait BatchSampler {
    /// Return the evaluation set for one parent/child screening minibatch.
    fn sample_train(&mut self, train_partition: &PartitionId) -> EvaluationSet;
}

/// Private batch-sampler state that must survive GEPA checkpoint/restore.
pub trait CheckpointBatchSampler {
    /// Serializable sampler state.
    type State: Serialize + serde::de::DeserializeOwned;

    /// Captures sampler state.
    fn checkpoint_state(&self) -> Self::State;

    /// Restores sampler state.
    fn restore_state(&mut self, state: Self::State);
}

/// Deterministic epoch-style train minibatch sampler.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EpochShuffled {
    minibatch_size: usize,
    seed: u64,
    cursor: u64,
}

impl Default for EpochShuffled {
    fn default() -> Self {
        Self {
            minibatch_size: 3,
            seed: 0,
            cursor: 0,
        }
    }
}

impl EpochShuffled {
    /// Build a deterministic sampler with a fixed minibatch size.
    #[must_use]
    pub const fn new(minibatch_size: usize) -> Self {
        Self {
            minibatch_size,
            seed: 0,
            cursor: 0,
        }
    }

    /// Set the deterministic sampling seed.
    #[must_use]
    pub const fn with_seed(mut self, seed: u64) -> Self {
        self.seed = seed;
        self
    }
}

impl BatchSampler for EpochShuffled {
    fn sample_train(&mut self, train_partition: &PartitionId) -> EvaluationSet {
        let seed = self.seed.wrapping_add(self.cursor);
        self.cursor = self.cursor.wrapping_add(1);
        EvaluationSet::Sample {
            of: Box::new(EvaluationSet::Partition(train_partition.clone())),
            n: self.minibatch_size,
            seed,
        }
    }
}

impl CheckpointBatchSampler for EpochShuffled {
    type State = Self;

    fn checkpoint_state(&self) -> Self::State {
        self.clone()
    }

    fn restore_state(&mut self, state: Self::State) {
        *self = state;
    }
}

/// Decides which held-out validation request, if any, follows an accepted candidate.
pub trait ValidationPolicy {
    /// Return the evaluation set for an accepted candidate's validation pass.
    fn validation_set(&mut self, accepted: CandidateId) -> Option<EvaluationSet>;
}

/// Private validation-policy state that must survive GEPA checkpoint/restore.
pub trait CheckpointValidationPolicy {
    /// Serializable policy state.
    type State: Serialize + serde::de::DeserializeOwned;

    /// Captures policy state.
    fn checkpoint_state(&self) -> Self::State;

    /// Restores policy state.
    fn restore_state(&mut self, state: Self::State);
}

/// Validate accepted candidates on the full configured validation partition.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct FullValidation;

impl ValidationPolicy for FullValidation {
    fn validation_set(&mut self, _accepted: CandidateId) -> Option<EvaluationSet> {
        Some(EvaluationSet::Partition(PartitionId::from("VALIDATION")))
    }
}

impl CheckpointValidationPolicy for FullValidation {
    type State = ();

    fn checkpoint_state(&self) -> Self::State {}

    fn restore_state(&mut self, _state: Self::State) {}
}

/// Conservative default: screen on a minibatch and leave validation for final reporting.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MinibatchThenValidation;

impl ValidationPolicy for MinibatchThenValidation {
    fn validation_set(&mut self, _accepted: CandidateId) -> Option<EvaluationSet> {
        None
    }
}

impl CheckpointValidationPolicy for MinibatchThenValidation {
    type State = ();

    fn checkpoint_state(&self) -> Self::State {}

    fn restore_state(&mut self, _state: Self::State) {}
}
