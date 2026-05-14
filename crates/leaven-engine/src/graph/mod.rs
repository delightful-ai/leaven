//! Append-only run graph.

pub mod indices;
pub mod query;
mod scoped_view;
pub mod storage;
mod view;

pub use scoped_view::ScopedRunGraphView;
pub use storage::{CandidateOrigin, RunGraph, RunGraphRestoreError, RunGraphSnapshot};
pub use view::{
    AssessmentQuery, AssessmentView, CandidateTree, CandidateView, EvaluationRequestView,
    FailureRef, Lineage, ProposalBatchView, ProposalView, RunGraphView,
};
