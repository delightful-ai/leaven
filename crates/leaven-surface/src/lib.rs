//! Explicit edit/read surfaces over artifacts.
//!
//! An artifact is intrinsic. A surface is a chosen projection over an artifact.

pub mod address;
pub mod edit_surface;
pub mod error;
pub mod part;
pub mod path_surface;
pub mod selection;

pub use address::PartAddress;
pub use edit_surface::{EditSurface, SurfaceFingerprint};
pub use error::SurfaceError;
pub use part::{Part, PartKind, PartView};
pub use path_surface::{PathAddress, PathPartId, PathSurfaceConfig};
pub use selection::PartSelection;

pub mod prelude {
    //! Common surface imports.

    pub use crate::{
        EditSurface, Part, PartAddress, PartKind, PartSelection, PartView, SurfaceError,
        SurfaceFingerprint,
    };
}
