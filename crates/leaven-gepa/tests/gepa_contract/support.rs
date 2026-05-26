use std::marker::PhantomData;

use leaven_core::{Artifact, ArtifactIdentity, CacheIdentity, Evidence, OptimizationProblem};
use leaven_kernel::{ContentId, Fingerprint};
use leaven_surface::{EditSurface, Part, PartAddress, SurfaceError, SurfaceFingerprint};

#[derive(Clone, Debug)]
pub struct TestArtifact(pub String);

impl Artifact for TestArtifact {
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

pub struct TextProblem<E, C = ()>(PhantomData<(E, C)>);

impl<E, C> OptimizationProblem for TextProblem<E, C>
where
    E: Evidence,
    C: Send + Sync + 'static,
{
    type Artifact = TestArtifact;
    type Case = C;
    type Evidence = E;
    type ProposalAnnotations = ();
}

#[derive(Clone, Debug)]
pub struct WholeTextSurface;

impl EditSurface<TestArtifact> for WholeTextSurface {
    type PartId = &'static str;
    type Address = PartAddress;
    type View<'a> = &'a str;
    type Edit = String;

    fn fingerprint(&self) -> SurfaceFingerprint {
        SurfaceFingerprint(Fingerprint::from_bytes([4; 32]))
    }

    fn parts<'a>(
        &self,
        artifact: &'a TestArtifact,
    ) -> Result<Vec<Part<Self::PartId, Self::Address, Self::View<'a>>>, SurfaceError> {
        Ok(vec![Part {
            id: "text",
            address: PartAddress("text".to_owned()),
            view: artifact.0.as_str(),
        }])
    }

    fn change_part(
        &self,
        _artifact: &TestArtifact,
        id: Self::PartId,
        edit: Self::Edit,
    ) -> Result<<TestArtifact as Artifact>::Change, SurfaceError> {
        if id == "text" {
            Ok(edit)
        } else {
            Err(SurfaceError::UnknownPart)
        }
    }
}
