//! Split-use policy.

use std::collections::{BTreeMap, BTreeSet};

use leaven_core::PartitionId;

use crate::SplitUsePolicyError;

/// One way a split may influence a run.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, serde::Serialize)]
pub enum EvaluationUse {
    /// May be rendered to proposers as feedback.
    ProposerFeedback,
    /// May influence parent selection.
    ParentSelection,
    /// May influence part selection.
    PartSelection,
    /// May decide whether a candidate is accepted.
    CandidateAcceptance,
    /// May update population/frontier state.
    PopulationObservation,
    /// May appear in reports.
    Report,
    /// Evaluator may see it, optimizer policy may not.
    EvaluatorOnly,
    /// Final held-out test reporting.
    FinalTest,
}

/// Use set for one split.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SplitUse {
    uses: BTreeSet<EvaluationUse>,
}

impl SplitUse {
    /// Builds a split-use set.
    pub fn new(uses: impl IntoIterator<Item = EvaluationUse>) -> Result<Self, SplitUsePolicyError> {
        let uses = uses.into_iter().collect::<BTreeSet<_>>();
        if uses.contains(&EvaluationUse::EvaluatorOnly) && uses.len() > 1 {
            return Err(SplitUsePolicyError::ContradictoryEvaluatorOnly);
        }
        Ok(Self { uses })
    }

    /// Report-only split use.
    pub fn report_only() -> Self {
        Self::new([EvaluationUse::Report]).expect("report-only split use is valid")
    }

    /// Train/search split use.
    pub fn optimizer_train() -> Self {
        Self::new([
            EvaluationUse::ProposerFeedback,
            EvaluationUse::ParentSelection,
            EvaluationUse::PartSelection,
            EvaluationUse::CandidateAcceptance,
            EvaluationUse::PopulationObservation,
            EvaluationUse::Report,
        ])
        .expect("train split use is valid")
    }

    /// Whether this split allows a use.
    #[must_use]
    pub fn allows(&self, use_: &EvaluationUse) -> bool {
        self.uses.contains(use_)
    }
}

/// Default policy for final test cases.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FinalTestPolicy {
    /// No final test is configured.
    Disabled,
    /// Final test may run only after optimization.
    FinalReportOnly,
    /// In-loop test use is explicitly allowed.
    ExplicitlyAllowedInLoop {
        /// Human reason for the exception.
        reason: String,
    },
}

/// Split-use policy by partition.
#[derive(Clone, Debug)]
pub struct SplitUsePolicy {
    uses: BTreeMap<PartitionId, SplitUse>,
    default: SplitUse,
    final_test: FinalTestPolicy,
}

impl SplitUsePolicy {
    /// Default GEPA policy: train drives search; validation/test report only.
    #[must_use]
    pub fn gepa_train_val_test() -> Self {
        Self {
            uses: BTreeMap::from([
                (PartitionId::from("TRAIN"), SplitUse::optimizer_train()),
                (PartitionId::from("VALIDATION"), SplitUse::report_only()),
                (
                    PartitionId::from("TEST"),
                    SplitUse::new([EvaluationUse::FinalTest, EvaluationUse::Report])
                        .expect("final test report use is valid"),
                ),
            ]),
            default: SplitUse::report_only(),
            final_test: FinalTestPolicy::FinalReportOnly,
        }
    }

    /// Uses for a partition.
    #[must_use]
    pub fn use_for(&self, partition: &PartitionId) -> &SplitUse {
        self.uses.get(partition).unwrap_or(&self.default)
    }

    /// Final-test policy.
    #[must_use]
    pub const fn final_test(&self) -> &FinalTestPolicy {
        &self.final_test
    }
}
