use std::convert::Infallible;

use leaven::plumbing::ContentId;
use leaven::prelude::{Artifact, ArtifactIdentity};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextArtifact(pub String);

impl Artifact for TextArtifact {
    type Change = String;
    type ApplyError = Infallible;

    fn identity(&self) -> ArtifactIdentity {
        ArtifactIdentity::Content(ContentId::hash_bytes(self.0.as_bytes()))
    }

    fn apply_change(&self, change: &Self::Change) -> Result<Self, Self::ApplyError> {
        Ok(Self(change.clone()))
    }
}
