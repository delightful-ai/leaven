//! GEPA validation and train-batch policies.

use std::collections::BTreeMap;

use leaven_core::{EvaluationSet, PartitionId};
use leaven_kernel::{CandidateId, CaseId};
use serde::{Deserialize, Serialize};

/// Selects the train/search cases used for one GEPA feedback iteration.
pub trait BatchSampler {
    /// Return the evaluation set for one parent/child screening minibatch.
    fn sample_train(
        &mut self,
        train_partition: &PartitionId,
        train_cases: &[CaseId],
    ) -> Result<EvaluationSet, BatchSamplingError>;
}

/// A train minibatch could not be sampled.
#[derive(Debug, thiserror::Error)]
pub enum BatchSamplingError {
    /// The train/search partition has no cases.
    #[error("GEPA cannot sample a train minibatch from an empty train partition {partition:?}")]
    EmptyTrainPartition {
        /// Partition that was sampled.
        partition: PartitionId,
    },
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
    cursor: u64,
    rng_state: u64,
    shuffled_ids: Vec<CaseId>,
    epoch: Option<u64>,
    last_trainset_size: usize,
}

impl Default for EpochShuffled {
    fn default() -> Self {
        Self {
            minibatch_size: 3,
            cursor: 0,
            rng_state: 0,
            shuffled_ids: Vec::new(),
            epoch: None,
            last_trainset_size: 0,
        }
    }
}

impl EpochShuffled {
    /// Build a deterministic sampler with a fixed minibatch size.
    #[must_use]
    pub const fn new(minibatch_size: usize) -> Self {
        Self {
            minibatch_size,
            cursor: 0,
            rng_state: 0,
            shuffled_ids: Vec::new(),
            epoch: None,
            last_trainset_size: 0,
        }
    }

    /// Set the deterministic sampling seed.
    #[must_use]
    pub const fn with_seed(mut self, seed: u64) -> Self {
        self.rng_state = seed;
        self
    }

    fn refresh(&mut self, train_cases: &[CaseId]) {
        self.shuffled_ids = train_cases.to_vec();
        shuffle_with_splitmix(&mut self.shuffled_ids, &mut self.rng_state);
        self.last_trainset_size = train_cases.len();

        let remainder = train_cases.len() % self.minibatch_size;
        let padding = if remainder == 0 {
            0
        } else {
            self.minibatch_size - remainder
        };
        let mut frequencies = BTreeMap::<CaseId, usize>::new();
        for case in &self.shuffled_ids {
            *frequencies.entry(*case).or_default() += 1;
        }
        for _ in 0..padding {
            let min_frequency = frequencies.values().copied().min().unwrap_or(0);
            let selected = self
                .shuffled_ids
                .iter()
                .rev()
                .copied()
                .find(|case| frequencies.get(case).copied().unwrap_or(0) == min_frequency)
                .expect("non-empty train cases were already checked");
            self.shuffled_ids.push(selected);
            *frequencies.entry(selected).or_default() += 1;
        }
    }
}

impl BatchSampler for EpochShuffled {
    fn sample_train(
        &mut self,
        train_partition: &PartitionId,
        train_cases: &[CaseId],
    ) -> Result<EvaluationSet, BatchSamplingError> {
        if train_cases.is_empty() {
            return Err(BatchSamplingError::EmptyTrainPartition {
                partition: train_partition.clone(),
            });
        }
        let base_idx = self.cursor.saturating_mul(self.minibatch_size as u64);
        let current_epoch = self
            .epoch
            .map_or(0, |_| base_idx / self.shuffled_ids.len().max(1) as u64);
        if self.shuffled_ids.is_empty()
            || train_cases.len() != self.last_trainset_size
            || self.epoch.is_some_and(|epoch| current_epoch > epoch)
        {
            self.epoch = Some(current_epoch);
            self.refresh(train_cases);
        }
        let base_idx = usize::try_from(base_idx % self.shuffled_ids.len() as u64)
            .expect("modulo result fits usize");
        let end_idx = base_idx + self.minibatch_size;
        let cases = self.shuffled_ids[base_idx..end_idx].to_vec();
        self.cursor = self.cursor.wrapping_add(1);
        Ok(EvaluationSet::Cases(cases))
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

fn shuffle_with_splitmix<T>(values: &mut [T], state: &mut u64) {
    for i in (1..values.len()).rev() {
        let j = bounded_index(splitmix64(state), i + 1);
        values.swap(i, j);
    }
}

fn bounded_index(value: u64, upper: usize) -> usize {
    let upper = u64::try_from(upper).expect("usize fits in u64");
    usize::try_from(value % upper).expect("bounded index fits in usize")
}

fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
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
