//! Persistence contracts for Leaven.
//!
//! This crate defines storage traits. Concrete storage backends live in backend
//! crates. This crate does not know `RunGraph`.

pub mod blob;
pub mod checkpoint;
pub mod error;
pub mod evidence;

pub use blob::{BlobStore, BlobWrite};
pub use checkpoint::{CheckpointBytes, CheckpointStore};
pub use error::StoreError;
pub use evidence::EvidenceStore;
pub use leaven_core::Evidence;

pub mod prelude {
    //! Common store imports.

    pub use crate::{
        BlobStore, BlobWrite, CheckpointBytes, CheckpointStore, Evidence, EvidenceStore, StoreError,
    };
}
