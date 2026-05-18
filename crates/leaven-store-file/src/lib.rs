//! File-backed store implementations.

mod atomic;
mod evidence;
mod store;

pub use evidence::{FileCheckpointStore, FileEvidenceStore, FileJsonCheckpointStore};
pub use store::FileStore;

pub mod prelude {
    //! Common file-store imports.

    pub use crate::{FileCheckpointStore, FileEvidenceStore, FileJsonCheckpointStore, FileStore};
}
