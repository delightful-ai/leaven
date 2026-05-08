//! File-backed store implementations.

mod evidence;
mod store;

pub use evidence::{FileCheckpointStore, FileEvidenceStore};
pub use store::FileStore;

pub mod prelude {
    //! Common file-store imports.

    pub use crate::{FileCheckpointStore, FileEvidenceStore, FileStore};
}
