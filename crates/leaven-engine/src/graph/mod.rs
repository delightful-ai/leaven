//! Append-only run graph.

pub mod indices;
pub mod query;
pub mod storage;
mod view;

pub use storage::{CandidateOrigin, RunGraph, RunGraphRestoreError, RunGraphSnapshot};
pub use view::{
    AssessmentQuery, AssessmentView, CandidateTree, CandidateView, EvaluationRequestView,
    FailureRef, Lineage, ProposalBatchView, ProposalView, RunGraphView,
};
