//! Evidence grouped by evaluation case.

use std::collections::BTreeMap;

use leaven_core::Evidence;
use leaven_kernel::CaseId;
use serde::{Deserialize, Serialize};

/// Evidence for one case in a resolved evaluation set.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CaseOutcome<E: Evidence> {
    case: CaseId,
    evidence: E,
}

impl<E: Evidence> CaseOutcome<E> {
    /// Build one case outcome.
    #[must_use]
    pub const fn new(case: CaseId, evidence: E) -> Self {
        Self { case, evidence }
    }

    /// Case this outcome describes.
    #[must_use]
    pub const fn case(&self) -> CaseId {
        self.case
    }

    /// Evidence for this case.
    #[must_use]
    pub const fn evidence(&self) -> &E {
        &self.evidence
    }
}

/// Sparse per-case evidence.
///
/// Missing case evidence is represented by absence. Duplicate case ids are
/// canonicalized deterministically: the last outcome for a case wins, and the
/// stored outcomes are ordered by [`CaseId`].
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CasewiseEvidence<E: Evidence> {
    outcomes: Vec<CaseOutcome<E>>,
}

impl<E: Evidence> CasewiseEvidence<E> {
    /// Build sparse casewise evidence.
    #[must_use]
    pub fn new(outcomes: Vec<CaseOutcome<E>>) -> Self {
        let mut by_case = BTreeMap::new();
        for outcome in outcomes {
            by_case.insert(outcome.case, outcome.evidence);
        }
        Self {
            outcomes: by_case
                .into_iter()
                .map(|(case, evidence)| CaseOutcome { case, evidence })
                .collect(),
        }
    }

    /// Case outcomes in deterministic case-id order.
    #[must_use]
    pub fn outcomes(&self) -> &[CaseOutcome<E>] {
        &self.outcomes
    }

    /// Evidence for one case, when present.
    #[must_use]
    pub fn get(&self, case: CaseId) -> Option<&E> {
        self.outcomes
            .binary_search_by_key(&case, CaseOutcome::case)
            .ok()
            .map(|index| self.outcomes[index].evidence())
    }
}

impl<E: Evidence> Evidence for CasewiseEvidence<E> {}
