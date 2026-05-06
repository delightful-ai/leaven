#![allow(dead_code)]

use leaven_core::{
    Artifact, ArtifactIdentity, Evidence, OptimizationProblem, Proposal, ProposalBatch,
    ProposalBatchSemantics,
};
use leaven_engine::{BudgetLedger, RunContext, RunGraph};
use leaven_kernel::{Budget, ContentId, Cost, MetadataBag, ProposalBatchId, RunId, StageId};
use thiserror::Error;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextArtifact(pub String);

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TextChange {
    Append(&'static str),
    Fail,
}

#[derive(Debug, Error)]
#[error("text error")]
pub struct TextError;

impl Artifact for TextArtifact {
    type Change = TextChange;
    type ApplyError = TextError;

    fn identity(&self) -> ArtifactIdentity {
        let mut bytes = [0; 32];
        let raw = self.0.as_bytes();
        let len = raw.len().min(32);
        bytes[..len].copy_from_slice(&raw[..len]);
        ArtifactIdentity::Content(ContentId::from_bytes(bytes))
    }

    fn apply_change(&self, change: &Self::Change) -> Result<Self, Self::ApplyError> {
        match change {
            TextChange::Append(suffix) => Ok(Self(format!("{}{suffix}", self.0))),
            TextChange::Fail => Err(TextError),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TestEvidence {
    pub score: f64,
}

impl Evidence for TestEvidence {}

pub struct TestProblem;

impl OptimizationProblem for TestProblem {
    type Artifact = TextArtifact;
    type Case = &'static str;
    type Evidence = TestEvidence;
    type ProposalAnnotations = ();
}

pub fn graph_and_budget() -> (RunGraph<TestProblem>, BudgetLedger) {
    (
        RunGraph::new(RunId::new()),
        BudgetLedger::new(Budget::unlimited()),
    )
}

pub fn record_one(
    ctx: &mut RunContext<'_, TestProblem>,
    proposal: Proposal<TestProblem>,
) -> ProposalBatchId {
    ctx.record_proposal_batch(
        StageId::custom("test"),
        ProposalBatch {
            proposals: vec![proposal],
            semantics: ProposalBatchSemantics::Alternatives,
            metadata: MetadataBag::new(),
        },
        Cost::zero(),
    )
    .unwrap()
    .batch_id
}
