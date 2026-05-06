//! Append-only run graph.

pub mod indices;
pub mod query;
pub mod storage;
mod view;

pub use storage::{CandidateOrigin, RunGraph};
pub use view::{
    AssessmentQuery, AssessmentView, CandidateTree, CandidateView, FailureRef, Lineage,
    ProposalBatchView, ProposalView, RunGraphView,
};
