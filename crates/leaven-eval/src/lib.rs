//! Lowered evaluation data and report vocabulary.
//!
//! This crate does not execute evaluations. It owns the durable train,
//! validation, test, split-use, and report vocabulary that product builders
//! lower into engine case sets and final reports.

mod dataset;
mod error;
mod report;
mod split;
mod use_policy;

pub use dataset::{Case, Dataset, DatasetBuilder, LmCase, NoTarget};
pub use error::{DatasetError, DatasetSplitsError, SplitUsePolicyError};
pub use report::{
    CandidateEvaluationSummary, EvaluationReport, ReportScore, SplitReport, SplitUseSummary,
};
pub use split::{DatasetSplits, SplitPolicy, SplitRole};
pub use use_policy::{EvaluationUse, FinalTestPolicy, SplitUse, SplitUsePolicy};

pub mod prelude {
    //! Common lowered-eval imports.

    pub use crate::{
        CandidateEvaluationSummary, Case, Dataset, DatasetBuilder, DatasetSplits, EvaluationReport,
        EvaluationUse, FinalTestPolicy, LmCase, NoTarget, ReportScore, SplitPolicy, SplitReport,
        SplitRole, SplitUse, SplitUsePolicy, SplitUseSummary,
    };
}
