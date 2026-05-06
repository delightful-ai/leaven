//! Inline in-memory store implementations.

mod evidence;
mod store;

pub use evidence::InlineEvidenceStore;
pub use store::InlineStore;

pub mod prelude {
    //! Common inline store imports.

    pub use crate::{InlineEvidenceStore, InlineStore};
}
