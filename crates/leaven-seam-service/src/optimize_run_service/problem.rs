use leaven_core::{Artifact, ArtifactIdentity, CacheIdentity};
use leaven_kernel::{ContentId, Fingerprint};
use leaven_surface::{EditSurface, Part, PartAddress, SurfaceError, SurfaceFingerprint};
use serde::{Deserialize, Serialize};

/// Prompt artifact optimized behind `leaven/optimize.run`.
///
/// V1 only executes the `prompt` artifact type: a single template part. The
/// wire seed carries `{ "template": "..." }`; the host lowers it into this
/// type, runs GEPA over the one editable part, and re-encodes each frontier
/// candidate's template back into the wire artifact triple. Promotion to a
/// shared `leaven-artifact-prompt` crate is deliberate future work once a
/// second consumer exists; mirroring p8's `AimePrompt` keeps the host self
/// contained for now.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub(super) struct SeamPromptArtifact {
    template: String,
}

impl SeamPromptArtifact {
    pub(super) fn new(template: impl Into<String>) -> Self {
        Self {
            template: template.into(),
        }
    }

    /// Template text carried by the candidate prompt.
    pub(super) fn template(&self) -> &str {
        &self.template
    }
}

/// Artifact-native change for [`SeamPromptArtifact`]: a replacement template.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub(super) struct SeamPromptChange {
    template: String,
}

/// Apply error for [`SeamPromptArtifact`]. A template change always applies.
#[derive(Debug)]
pub(super) struct SeamPromptApplyError;

impl std::fmt::Display for SeamPromptApplyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("seam prompt change was invalid")
    }
}

impl std::error::Error for SeamPromptApplyError {}

impl Artifact for SeamPromptArtifact {
    type Change = SeamPromptChange;
    type ApplyError = SeamPromptApplyError;

    fn identity(&self) -> ArtifactIdentity {
        ArtifactIdentity::Content(ContentId::hash_bytes(self.template.as_bytes()))
    }

    fn cache_identity(&self) -> Option<CacheIdentity> {
        Some(CacheIdentity::Content(ContentId::hash_bytes(
            self.template.as_bytes(),
        )))
    }

    fn apply_change(&self, change: &Self::Change) -> Result<Self, Self::ApplyError> {
        Ok(Self {
            template: change.template.clone(),
        })
    }
}

/// The single-part edit surface over a [`SeamPromptArtifact`] template.
///
/// GEPA selects the `template` part, renders its current view into the
/// reflection prompt, and lowers a replacement edit through this surface back
/// into a [`SeamPromptChange`]. This mirrors p8's `AimePromptSurface`.
#[derive(Clone, Copy, Debug)]
pub(super) struct SeamPromptSurface;

impl EditSurface<SeamPromptArtifact> for SeamPromptSurface {
    type PartId = &'static str;
    type Address = PartAddress;
    type View<'a> = &'a str;
    type Edit = String;

    fn fingerprint(&self) -> SurfaceFingerprint {
        SurfaceFingerprint(Fingerprint::from_bytes([9; 32]))
    }

    fn parts<'a>(
        &self,
        artifact: &'a SeamPromptArtifact,
    ) -> Result<Vec<Part<Self::PartId, Self::Address, Self::View<'a>>>, SurfaceError> {
        Ok(vec![Part {
            id: "template",
            address: PartAddress("template".to_owned()),
            view: artifact.template.as_str(),
        }])
    }

    fn change_part(
        &self,
        _artifact: &SeamPromptArtifact,
        id: Self::PartId,
        edit: Self::Edit,
    ) -> Result<SeamPromptChange, SurfaceError> {
        if id != "template" {
            return Err(SurfaceError::UnknownPart);
        }
        Ok(SeamPromptChange { template: edit })
    }
}
