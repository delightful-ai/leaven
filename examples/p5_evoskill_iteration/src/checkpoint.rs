use leaven_artifact_skill::{SkillBank, SkillBankChange};
use leaven_kernel::{EvidenceRef, RunId};

use crate::data::EvoSkillCase;
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
