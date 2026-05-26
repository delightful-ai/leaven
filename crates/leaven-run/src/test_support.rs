use leaven_core::{Artifact, ArtifactIdentity, Evidence, OptimizationProblem};
use leaven_kernel::ContentId;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct TestArtifact;

#[derive(Debug)]
pub(crate) struct TestArtifactError;

impl std::fmt::Display for TestArtifactError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("test artifact error")
    }
}

impl std::error::Error for TestArtifactError {}

impl Artifact for TestArtifact {
    type Change = ();
    type ApplyError = TestArtifactError;

    fn identity(&self) -> ArtifactIdentity {
        ArtifactIdentity::Content(ContentId::from_bytes([1; 32]))
    }

    fn apply_change(&self, _change: &Self::Change) -> Result<Self, Self::ApplyError> {
        Ok(self.clone())
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct TestEvidence;

impl Evidence for TestEvidence {}

pub(crate) struct TestProblem;

impl OptimizationProblem for TestProblem {
    type Artifact = TestArtifact;
    type Case = ();
    type Evidence = TestEvidence;
    type ProposalAnnotations = ();
}
