//! Lowered evaluation refusal types.

use leaven_kernel::CaseId;
use thiserror::Error;

use crate::SplitRole;

/// Dataset construction failed.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum DatasetError {
    /// A caller tried to insert the same case id twice.
    #[error("duplicate case id: {0}")]
    DuplicateCase(CaseId),
}

/// Dataset split construction failed.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum DatasetSplitsError {
    /// A partition contains a case id not present in the dataset.
    #[error("split references unknown case: {0}")]
    UnknownCase(CaseId),
    /// A case appears in more than one partition while disjoint splits are required.
    #[error("case {case} appears in both {left:?} and {right:?}")]
    OverlappingCase {
        /// Duplicated case.
        case: CaseId,
        /// First role containing the case.
        left: SplitRole,
        /// Second role containing the case.
        right: SplitRole,
    },
    /// A required split role has no cases.
    #[error("required split {role:?} is empty")]
    EmptyRequiredSplit {
        /// Empty split role.
        role: SplitRole,
    },
}

/// Split-use policy construction failed.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum SplitUsePolicyError {
    /// `EvaluatorOnly` cannot be combined with optimizer-facing uses.
    #[error("EvaluatorOnly cannot be combined with optimizer-facing uses")]
    ContradictoryEvaluatorOnly,
}
