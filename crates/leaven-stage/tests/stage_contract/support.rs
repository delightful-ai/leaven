use leaven_core::{Artifact, ArtifactIdentity, CacheIdentity, Evidence, OptimizationProblem};
use leaven_engine::{BudgetLedger, RunGraph};
use leaven_kernel::{Budget, ContentId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextArtifact(pub String);

pub fn text_artifact(text: impl Into<String>) -> TextArtifact {
    TextArtifact(text.into())
}

impl Artifact for TextArtifact {
    type Change = String;
    type ApplyError = std::convert::Infallible;

    fn identity(&self) -> ArtifactIdentity {
        ArtifactIdentity::Content(ContentId::hash_bytes(self.0.as_bytes()))
    }

    fn cache_identity(&self) -> Option<CacheIdentity> {
        Some(CacheIdentity::Content(ContentId::hash_bytes(
            self.0.as_bytes(),
        )))
    }

    fn apply_change(&self, change: &Self::Change) -> Result<Self, Self::ApplyError> {
        Ok(Self(format!("{}{change}", self.0)))
    }
}

#[derive(Clone, Debug)]
pub struct TestEvidence;

impl Evidence for TestEvidence {}

pub struct TestProblem;

impl OptimizationProblem for TestProblem {
    type Artifact = TextArtifact;
    type Case = ();
    type Evidence = TestEvidence;
    type ProposalAnnotations = ();
}

pub fn graph_and_budget() -> (RunGraph<TestProblem>, BudgetLedger) {
    (
        RunGraph::new(leaven_kernel::RunId::new()),
        BudgetLedger::new(Budget::unlimited()),
    )
}
