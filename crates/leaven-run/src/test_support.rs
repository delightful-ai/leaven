use leaven_core::{Artifact, ArtifactIdentity, Evidence, OptimizationProblem};
use leaven_kernel::ContentId;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct TestArtifact;

#[derive(Debug)]
pub struct TestArtifactError;

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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IntArtifact(pub i32);

impl Artifact for IntArtifact {
    type Change = i32;
    type ApplyError = TestArtifactError;

    fn identity(&self) -> ArtifactIdentity {
        ArtifactIdentity::Content(ContentId::hash_bytes(self.0.to_le_bytes()))
    }

    fn apply_change(&self, change: &Self::Change) -> Result<Self, Self::ApplyError> {
        Ok(Self(*change))
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct TestEvidence;

impl Evidence for TestEvidence {}

pub struct TestProblem;

impl OptimizationProblem for TestProblem {
    type Artifact = TestArtifact;
    type Case = ();
    type Evidence = TestEvidence;
    type ProposalAnnotations = ();
}
