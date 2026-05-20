//! Lowered evaluation refusal types.

use leaven_kernel::CaseId;
use smol_str::SmolStr;
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
    /// A split constructor was asked to build the same role more than once.
    #[error("duplicate split role: {0:?}")]
    DuplicateSplitRole(SplitRole),
    /// A stratified split requested more cases than the strata contain.
    #[error("stratified split requested {requested} cases but only {available} are available")]
    InsufficientStratifiedCases {
        /// Total requested split cases.
        requested: usize,
        /// Total available source cases.
        available: usize,
    },
    /// One case appears in multiple strata.
    #[error("case {case} appears in both strata {left} and {right}")]
    DuplicateStratifiedCase {
        /// Duplicated case id.
        case: CaseId,
        /// First stratum containing the case.
        left: SmolStr,
        /// Second stratum containing the case.
        right: SmolStr,
    },
}

/// Split-use policy construction failed.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum SplitUsePolicyError {
    /// `EvaluatorOnly` cannot be combined with optimizer-facing uses.
    #[error("EvaluatorOnly cannot be combined with optimizer-facing uses")]
    ContradictoryEvaluatorOnly,
}

/// Evaluation sampler construction failed.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum SamplerError {
    /// The sampler has no category pools to draw from.
    #[error("sampler has no categories")]
    NoCategories,
    /// One category has no cases.
    #[error("category {0} has no cases")]
    EmptyCategory(SmolStr),
    /// One case appears in multiple category pools.
    #[error("case {case} appears in both categories {left} and {right}")]
    DuplicateCaseCategory {
        /// Duplicated case id.
        case: CaseId,
        /// First category containing the case.
        left: SmolStr,
        /// Second category containing the case.
        right: SmolStr,
    },
}
