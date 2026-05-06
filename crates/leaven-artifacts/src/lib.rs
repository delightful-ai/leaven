//! leaven-artifacts crate skeleton.

pub mod dir {
    pub struct DirArtifact;
    pub struct DirChange;
    pub struct DirPathSurface;
    pub struct FsOp;
}
pub mod part_map {
    pub struct PartId;
    pub struct PartMapArtifact;
    pub struct PartMapChange;
    pub struct PartMapSurface;
}
pub mod text {
    pub struct TextArtifact;
    pub struct TextChange;
    pub struct TextSurface;
}
pub use dir::{DirArtifact, DirChange, DirPathSurface, FsOp};
pub use part_map::{PartId, PartMapArtifact, PartMapChange, PartMapSurface};
pub use text::{TextArtifact, TextChange, TextSurface};
pub mod prelude {
    pub use crate::{
        DirArtifact, DirChange, DirPathSurface, FsOp, PartId, PartMapArtifact, PartMapChange,
        PartMapSurface, TextArtifact, TextChange, TextSurface,
    };
}
