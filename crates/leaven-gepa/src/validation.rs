//! GEPA validation and train-batch policies.

use std::collections::BTreeMap;

use leaven_core::{EvaluationSet, PartitionId};
use leaven_kernel::{CandidateId, CaseId};
use serde::{Deserialize, Serialize};

#[doc(hidden)]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GepaRandom(crate::python_random::PythonRandom);

impl Default for GepaRandom {
    fn default() -> Self {
        Self::seeded(0)
    }
}

impl GepaRandom {
    pub(crate) fn seeded(seed: u64) -> Self {
        Self(crate::python_random::PythonRandom::seeded(seed))
    }

    pub(crate) fn randbelow(&mut self, upper: usize) -> usize {
        self.0.randbelow(upper)
    }

    pub(crate) fn shuffle<T>(&mut self, values: &mut [T]) {
        self.0.shuffle(values);
    }
}

/// Selects the train/search cases used for one GEPA feedback iteration.
pub trait BatchSampler {
    /// Return the evaluation set for one parent/child screening minibatch.
    fn sample_train(
        &mut self,
        train_partition: &PartitionId,
        train_cases: &[CaseId],
    ) -> Result<EvaluationSet, BatchSamplingError>;

    /// Return a minibatch using GEPA's shared reference RNG when supported.
    #[doc(hidden)]
    fn sample_train_with_gepa_rng(
        &mut self,
        train_partition: &PartitionId,
        train_cases: &[CaseId],
        _rng: &mut GepaRandom,
    ) -> Result<EvaluationSet, BatchSamplingError> {
        self.sample_train(train_partition, train_cases)
    }
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
    rng: GepaRandom,
    use_shared_rng: bool,
    shuffled_ids: Vec<CaseId>,
    epoch: Option<u64>,
    last_trainset_size: usize,
}

impl Default for EpochShuffled {
    fn default() -> Self {
        Self {
            minibatch_size: 3,
            cursor: 0,
            rng: GepaRandom::default(),
            use_shared_rng: true,
            shuffled_ids: Vec::new(),
            epoch: None,
            last_trainset_size: 0,
        }
    }
}

impl EpochShuffled {
    /// Build a deterministic sampler with a fixed minibatch size.
    #[must_use]
    pub fn new(minibatch_size: usize) -> Self {
        Self {
            minibatch_size,
            cursor: 0,
            rng: GepaRandom::seeded(0),
            use_shared_rng: true,
            shuffled_ids: Vec::new(),
            epoch: None,
            last_trainset_size: 0,
        }
    }

    /// Set the deterministic sampling seed.
    #[must_use]
    pub fn with_seed(mut self, seed: u64) -> Self {
        self.rng = GepaRandom::seeded(seed);
        self.use_shared_rng = false;
        self
    }

    fn refresh(&mut self, train_cases: &[CaseId]) {
        let mut rng = std::mem::take(&mut self.rng);
        self.refresh_with_rng(train_cases, &mut rng);
        self.rng = rng;
    }

    fn refresh_with_rng(&mut self, train_cases: &[CaseId], rng: &mut GepaRandom) {
        self.shuffled_ids = train_cases.to_vec();
        rng.shuffle(&mut self.shuffled_ids);
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

    fn sample_train_inner(
        &mut self,
        train_partition: &PartitionId,
        train_cases: &[CaseId],
        shared_rng: Option<&mut GepaRandom>,
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
            if self.use_shared_rng {
                if let Some(rng) = shared_rng {
                    self.refresh_with_rng(train_cases, rng);
                } else {
                    self.refresh(train_cases);
                }
            } else {
                self.refresh(train_cases);
            }
        }
        let base_idx = usize::try_from(base_idx % self.shuffled_ids.len() as u64)
            .expect("modulo result fits usize");
        let end_idx = base_idx + self.minibatch_size;
        let cases = self.shuffled_ids[base_idx..end_idx].to_vec();
        self.cursor = self.cursor.wrapping_add(1);
        Ok(EvaluationSet::Cases(cases))
    }
}

impl BatchSampler for EpochShuffled {
    fn sample_train(
        &mut self,
        train_partition: &PartitionId,
        train_cases: &[CaseId],
    ) -> Result<EvaluationSet, BatchSamplingError> {
        self.sample_train_inner(train_partition, train_cases, None)
    }

    fn sample_train_with_gepa_rng(
        &mut self,
        train_partition: &PartitionId,
        train_cases: &[CaseId],
        rng: &mut GepaRandom,
    ) -> Result<EvaluationSet, BatchSamplingError> {
        self.sample_train_inner(train_partition, train_cases, Some(rng))
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

#[cfg(test)]
mod tests {
    use leaven_core::{EvaluationSet, PartitionId};
    use leaven_kernel::CaseId;

    use super::{BatchSampler, BatchSamplingError, EpochShuffled, GepaRandom};

    struct DelegatingSampler;

    impl BatchSampler for DelegatingSampler {
        fn sample_train(
            &mut self,
            train_partition: &PartitionId,
            train_cases: &[CaseId],
        ) -> Result<EvaluationSet, BatchSamplingError> {
            assert_eq!(train_partition, &PartitionId::from("TRAIN"));
            Ok(EvaluationSet::Cases(train_cases.to_vec()))
        }
    }

    #[test]
    fn default_gepa_rng_sampler_hook_delegates_to_plain_sampling() {
        let partition = PartitionId::from("TRAIN");
        let cases = vec![CaseId::new(1), CaseId::new(2)];
        let mut sampler = DelegatingSampler;
        let mut rng = GepaRandom::seeded(0);

        match sampler
            .sample_train_with_gepa_rng(&partition, &cases, &mut rng)
            .unwrap()
        {
            EvaluationSet::Cases(sampled_cases) => assert_eq!(sampled_cases, cases),
            other => panic!("expected delegated cases, got {other:?}"),
        }
    }

    #[test]
    fn epoch_sampler_without_shared_rng_refreshes_with_owned_rng() {
        let partition = PartitionId::from("TRAIN");
        let cases = vec![CaseId::new(1), CaseId::new(2), CaseId::new(3)];
        let mut sampler = EpochShuffled::new(2);

        let sampled = sampler.sample_train(&partition, &cases).unwrap();

        match sampled {
            EvaluationSet::Cases(sampled_cases) => {
                assert_eq!(sampled_cases.len(), 2);
                assert!(sampled_cases.iter().all(|case| cases.contains(case)));
            }
            other => panic!("expected sampled cases, got {other:?}"),
        }
    }
}
