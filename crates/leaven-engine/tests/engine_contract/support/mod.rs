#![allow(dead_code)]

use leaven_core::{
    Artifact, ArtifactIdentity, CacheIdentity, Evidence, OptimizationProblem, Proposal,
    ProposalBatch, ProposalBatchSemantics,
};
use leaven_engine::{BudgetLedger, RunContext, RunGraph};
use leaven_kernel::{Budget, ContentId, Cost, MetadataBag, ProposalBatchId, RunId, StageId};
use thiserror::Error;

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct TextArtifact(pub String);

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum TextChange {
    Append(String),
    Fail,
}

#[derive(Debug, Error)]
#[error("text error")]
pub struct TextError;

impl Artifact for TextArtifact {
    type Change = TextChange;
    type ApplyError = TextError;

    fn identity(&self) -> ArtifactIdentity {
        if let Some(label) = self.0.strip_prefix("external:") {
            return ArtifactIdentity::External(label.to_owned());
        }
        ArtifactIdentity::Content(text_content_id(&self.0))
    }

    fn cache_identity(&self) -> Option<CacheIdentity> {
        if self.0.starts_with("external:") {
            return None;
        }
        Some(CacheIdentity::Content(text_content_id(&self.0)))
    }

    fn apply_change(&self, change: &Self::Change) -> Result<Self, Self::ApplyError> {
        match change {
            TextChange::Append(suffix) => Ok(Self(format!("{}{suffix}", self.0))),
            TextChange::Fail => Err(TextError),
        }
    }
}

fn text_content_id(text: &str) -> ContentId {
    let mut bytes = [0; 32];
    let raw = text.as_bytes();
    let len = raw.len().min(32);
    bytes[..len].copy_from_slice(&raw[..len]);
    ContentId::from_bytes(bytes)
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
