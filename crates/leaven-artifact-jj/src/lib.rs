//! JJ-backed artifact vocabulary for materialized file snapshots.

pub mod artifact {
    use std::collections::BTreeMap;

    use leaven_core::{Artifact, ArtifactIdentity, CacheIdentity};
    use leaven_kernel::{ContentId, FingerprintBuilder};
    use leaven_stage::{
        ArtifactReadbackError, MaterializableArtifact, MaterializationReport, WorkspaceSetupError,
    };
    use leaven_workspace::{WorkspacePath, WorkspaceSlot};

    use crate::JjChange;

    #[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
    pub struct JjArtifact {
        files: BTreeMap<WorkspacePath, Vec<u8>>,
    }

    impl JjArtifact {
        #[must_use]
        pub fn new(files: BTreeMap<WorkspacePath, Vec<u8>>) -> Self {
            Self { files }
        }

        #[must_use]
        pub fn files(&self) -> &BTreeMap<WorkspacePath, Vec<u8>> {
            &self.files
        }
    }

    impl Artifact for JjArtifact {
        type Change = JjChange;
        type ApplyError = std::convert::Infallible;

        fn identity(&self) -> ArtifactIdentity {
            ArtifactIdentity::Content(content_id(self))
        }

        fn cache_identity(&self) -> Option<CacheIdentity> {
            Some(CacheIdentity::Content(content_id(self)))
        }

        fn apply_change(&self, _change: &Self::Change) -> Result<Self, Self::ApplyError> {
            Ok(self.clone())
        }
    }

    impl MaterializableArtifact for JjArtifact {
        async fn write_to(
            &self,
            slot: &mut WorkspaceSlot<'_>,
        ) -> Result<MaterializationReport, WorkspaceSetupError> {
            for (path, bytes) in &self.files {
                slot.write_file(path, bytes)?;
            }
            Ok(MaterializationReport::default())
        }

        async fn read_back_change(
            &self,
            slot: &WorkspaceSlot<'_>,
        ) -> Result<Option<Self::Change>, ArtifactReadbackError> {
            let path = WorkspacePath::new(".leaven/jj/change.patch")
                .map_err(|err| ArtifactReadbackError::InvalidArtifact(err.to_string()))?;
            match slot.read_file(&path) {
                Ok(bytes) => Ok(Some(JjChange::Patch(String::from_utf8(bytes).map_err(
                    |err| ArtifactReadbackError::InvalidArtifact(err.to_string()),
                )?))),
                Err(_) => Ok(None),
            }
        }
    }

    fn content_id(artifact: &JjArtifact) -> ContentId {
        let mut builder = FingerprintBuilder::new();
        builder.update(b"leaven.artifact-jj.v1");
        for (path, bytes) in artifact.files() {
            builder
                .update(path.as_str().as_bytes())
                .update((bytes.len() as u64).to_le_bytes())
                .update(bytes);
        }
        ContentId::from_bytes(builder.finish().0)
    }
}
pub mod change {
    #[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
    pub enum JjChange {
        Patch(String),
    }
}
pub mod error {
    #[derive(Debug, thiserror::Error)]
    pub enum JjArtifactError {
        #[error("jj artifact failed")]
        Message,
    }
}
pub use artifact::JjArtifact;
pub use change::JjChange;
pub use error::JjArtifactError;
