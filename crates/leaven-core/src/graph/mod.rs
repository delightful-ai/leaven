//! The append-only run graph: storage, views, indices, queries, and
//! events.
//!
//! [`storage::RunGraph`] is the source-of-truth structure. All
//! mutations go through `RunContext` (see [`crate::context`]); strategy
//! authors interact with read-scoped [`view::RunGraphView`]s.

pub mod events;
pub mod indices;
pub mod query;
pub mod storage;
pub mod view;

pub use events::RunEvent;
pub use storage::RunGraph;
pub use view::RunGraphView;
