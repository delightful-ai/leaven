use bytes::Bytes;
use leaven_artifact_skill::{SkillBank, SkillBankChange};
use leaven_kernel::{CheckpointId, EvidenceRef, RunId};
use leaven_store::{CheckpointBytes, CheckpointStore};
use leaven_store_file::FileCheckpointStore;

use crate::data::EvoSkillCase;
use crate::error::Result;
use crate::evidence::CaseExecution;
use crate::proposal::SkillProposal;

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(tag = "phase")]
pub enum EvoSkillCheckpoint {
    BaselineComplete {
        run_id: RunId,
        cases: Vec<EvoSkillCase>,
        seed_bank: SkillBank,
        baseline_score: f64,
    },
    FailuresCollected {
        run_id: RunId,
        cases: Vec<EvoSkillCase>,
        seed_bank: SkillBank,
        baseline_score: f64,
        failures: Vec<CaseExecution>,
    },
    ProposalComplete {
        run_id: RunId,
        cases: Vec<EvoSkillCase>,
        seed_bank: SkillBank,
        baseline_score: f64,
        failures: Vec<CaseExecution>,
        proposal: SkillProposal,
        proposer_evidence: EvidenceRef,
    },
    CandidateBuilt {
        run_id: RunId,
        cases: Vec<EvoSkillCase>,
        seed_bank: SkillBank,
        baseline_score: f64,
        failures: Vec<CaseExecution>,
        proposal: SkillProposal,
        proposer_evidence: EvidenceRef,
        child_bank: SkillBank,
        change: SkillBankChange,
    },
    IterationComplete {
        run_id: RunId,
        baseline_score: f64,
        child_score: f64,
        admitted: bool,
        best_score: f64,
        best_bank: SkillBank,
    },
}

pub struct Checkpoints {
    store: FileCheckpointStore,
}

impl Checkpoints {
    pub fn open(store: FileCheckpointStore) -> Self {
        Self { store }
    }

    pub fn latest(&self) -> Result<Option<(CheckpointId, EvoSkillCheckpoint)>> {
        let Some(id) = self.store.latest()? else {
            return Ok(None);
        };
        let checkpoint = self.get(id)?;
        Ok(Some((id, checkpoint)))
    }

    pub fn save(&self, checkpoint: &EvoSkillCheckpoint) -> Result<CheckpointId> {
        let bytes = serde_json::to_vec_pretty(checkpoint)?;
        Ok(self.store.put(CheckpointBytes(Bytes::from(bytes)))?)
    }

    fn get(&self, id: CheckpointId) -> Result<EvoSkillCheckpoint> {
        let bytes = self.store.get(id)?;
        Ok(serde_json::from_slice(&bytes.0)?)
    }
}
