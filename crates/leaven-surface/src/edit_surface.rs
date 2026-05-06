//! Edit surface trait.

use leaven_core::Artifact;

use crate::{Part, SurfaceError};

pub trait EditSurface<A: Artifact>: Send + Sync {
    type PartId: Eq + std::hash::Hash + Clone + Send + Sync + 'static;
    type Address: Eq + std::hash::Hash + Clone + Send + Sync + 'static;
    type View<'a>: Send + Sync
    where
        A: 'a;
    type Edit: Clone + Send + Sync + 'static;

    fn fingerprint(&self) -> SurfaceFingerprint;

    #[allow(clippy::type_complexity)]
    fn parts<'a>(
        &self,
        artifact: &'a A,
    ) -> Result<Vec<Part<Self::PartId, Self::Address, Self::View<'a>>>, SurfaceError>;

    fn change_part(
        &self,
        artifact: &A,
        id: Self::PartId,
        edit: Self::Edit,
    ) -> Result<A::Change, SurfaceError>;
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct SurfaceFingerprint(pub leaven_kernel::Fingerprint);
