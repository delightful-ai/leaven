use leaven_core::{Artifact, ArtifactIdentity};
use leaven_kernel::ContentId;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TestArtifact(pub String);

impl Artifact for TestArtifact {
    type Change = String;
    type ApplyError = TestApplyError;

    fn identity(&self) -> ArtifactIdentity {
        let byte = u8::try_from(self.0.len()).unwrap_or(u8::MAX);
        ArtifactIdentity::Content(ContentId::from_bytes([byte; 32]))
    }

    fn apply_change(&self, change: &Self::Change) -> Result<Self, Self::ApplyError> {
        Ok(Self(change.clone()))
    }
}

#[derive(Debug, thiserror::Error)]
#[error("test apply failed")]
pub struct TestApplyError;
