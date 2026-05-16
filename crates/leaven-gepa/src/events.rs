//! GEPA phase event summaries.

use leaven_kernel::CandidateId;
use serde::{Deserialize, Serialize};

use crate::GepaCandidateIndex;

/// Non-fatal GEPA proposal skip reason.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum GepaSkipReason {
    /// The reflective dataset builder produced no examples.
    NoReflectiveExamples,
    /// All selected parent rows were already perfect.
    AllScoresPerfect,
}

/// Structured GEPA phase event summary for reports/tests.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum GepaEventSummary {
    /// Profile was resolved.
    ProfileResolved,
    /// Seed validation started.
    SeedValidationStarted { candidate: CandidateId },
    /// Seed validation completed.
    SeedValidationCompleted {
        candidate_index: GepaCandidateIndex,
        score: String,
    },
    /// One GEPA iteration started.
    IterationStarted { iteration: usize },
    /// Parent was selected for mutation.
    ParentSelected { candidate_index: GepaCandidateIndex },
    /// Train minibatch was sampled.
    TrainMinibatchSampled,
    /// Parent evaluation completed.
    ParentEvaluated { metric_calls_delta: u64 },
    /// Proposal was skipped before provider work.
    ProposalSkipped { reason: GepaSkipReason },
    /// Reflective examples were built.
    ReflectiveDatasetBuilt { records: usize },
    /// Child candidate was built.
    ChildBuilt { candidate: CandidateId },
    /// Child evaluation completed.
    ChildEvaluated { metric_calls_delta: u64 },
    /// Proposal was accepted by the train-screening policy.
    ProposalAccepted { child: CandidateId },
    /// Proposal was rejected by the train-screening policy.
    ProposalRejected,
    /// Accepted candidate validation completed.
    AcceptedValidationCompleted { candidate_index: GepaCandidateIndex },
    /// Validation frontier was updated.
    FrontierUpdated,
    /// GEPA reached the end of optimizer execution.
    OptimizationEnded { best: Option<GepaCandidateIndex> },
}
