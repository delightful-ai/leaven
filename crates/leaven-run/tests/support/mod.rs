use leaven_core::{Artifact, ArtifactIdentity};
use leaven_kernel::{ContentId, Fingerprint};

pub const TEST_RUNNER_FINGERPRINT: Fingerprint = Fingerprint::from_bytes([7; 32]);
pub const TEST_SCORER_FINGERPRINT: Fingerprint = Fingerprint::from_bytes([8; 32]);

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TextArtifact(pub i32);

#[derive(Debug)]
pub struct TextArtifactError;

impl std::fmt::Display for TextArtifactError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("text artifact error")
    }
}

impl std::error::Error for TextArtifactError {}

impl Artifact for TextArtifact {
    type Change = i32;
    type ApplyError = TextArtifactError;

    fn identity(&self) -> ArtifactIdentity {
        ArtifactIdentity::Content(ContentId::hash_bytes(self.0.to_le_bytes()))
    }

    fn apply_change(&self, change: &Self::Change) -> Result<Self, Self::ApplyError> {
        Ok(Self(*change))
    }
}
