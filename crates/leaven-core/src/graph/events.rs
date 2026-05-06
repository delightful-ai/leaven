//! `RunEvent` — the durable, structured event log.
//!
//! Every major operation emits exactly one event. The event stream is
//! a public debugging story: it must be readable independently and
//! must be ordered consistently with graph mutations. Callbacks see
//! events through [`crate::stage::callback::Callback`]; the run graph
//! also persists them for replay.

use crate::artifact::ContentId;
use crate::cost::{BudgetSnapshot, Cost};
use crate::error::ErrorRecord;
use crate::ids::{
    AssessmentId, CandidateId, EvaluationRequestId, EvaluatorId, IterationId, PopulationId,
    ProposalBatchId, ProposalId, RunId, StageId,
};
use crate::population::PopulationEvent;
use crate::problem::OptimizationProblem;
use crate::proposal::CausalInputs;

/// Cache hit/miss status for an evaluation. Forms part of the event
/// surface so observers can distinguish cached results from fresh
/// runs.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum CacheStatus {
    Hit,
    Miss,
    Bypassed,
}

/// Why a proposal application succeeded or failed, condensed for
/// event consumers. Full provenance lives in the proposal record.
#[derive(Clone, Debug)]
pub enum ProposalEffectSummary {
    Create,
    Change { target: CandidateId },
}

/// Why an iteration ended.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum StepStatus {
    Continue,
    NoProgress,
    Done,
}

/// Reasons a run can stop. The engine emits exactly one
/// `OptimizationStopping` then `OptimizationEnded`.
#[derive(Clone, Debug)]
pub enum StopReason {
    OptimizerDone,
    BudgetExceeded,
    StopperTriggered { name: String },
    External,
    Error,
}

/// Outer policy attached to an `Error` event: did the engine continue,
/// retry, or stop the run as a result?
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum ErrorPolicy {
    Continued,
    Retried,
    StoppedRun,
}

/// Compact summary of an evaluation request, for event consumers that
/// don't need the full resolved set.
#[derive(Clone, Debug)]
pub struct EvaluationRequestSummary {
    pub kind: EvaluationRequestKind,
    pub candidate_count: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum EvaluationRequestKind {
    Independent,
    Pairwise,
    Listwise,
}

/// All structured events the engine emits. Generic over the problem so
/// callbacks can pattern-match on typed payloads when needed.
#[derive(Clone, Debug)]
pub enum RunEvent<P: OptimizationProblem> {
    OptimizationStarted {
        run_id: RunId,
    },
    OptimizationStopping {
        reason: StopReason,
    },
    OptimizationEnded {
        run_id: RunId,
        best: Option<CandidateId>,
        budget: BudgetSnapshot,
    },

    IterationStarted {
        iteration: IterationId,
    },
    IterationEnded {
        iteration: IterationId,
        status: StepStatus,
    },

    ProposalBatchProduced {
        iteration: Option<IterationId>,
        batch_id: ProposalBatchId,
        proposer: StageId,
        proposal_count: usize,
    },
    ProposalRecorded {
        proposal_id: ProposalId,
        batch_id: ProposalBatchId,
        effect: ProposalEffectSummary,
        causal: CausalInputs,
        informed_by_count: usize,
    },

    ApplySucceeded {
        proposal_id: ProposalId,
        candidate_id: CandidateId,
        content_id: ContentId,
    },
    ApplyFailed {
        proposal_id: ProposalId,
        error: ErrorRecord,
    },

    EvaluationRequested {
        request_id: EvaluationRequestId,
        evaluator: EvaluatorId,
        request: EvaluationRequestSummary,
    },
    EvaluationCompleted {
        request_id: EvaluationRequestId,
        evaluator: EvaluatorId,
        assessment_ids: Vec<AssessmentId>,
        cost: Cost,
        cache: CacheStatus,
    },

    PopulationUpdated {
        population_id: PopulationId,
        events: Vec<PopulationEvent>,
    },

    BudgetCharged {
        stage: StageId,
        cost: Cost,
        remaining: BudgetSnapshot,
    },

    Error {
        stage: Option<StageId>,
        error: ErrorRecord,
        policy: ErrorPolicy,
    },

    /// Phantom usage of `P` so the generic parameter is meaningful
    /// even before optimizer-specific events are added.
    #[doc(hidden)]
    _Phantom(std::marker::PhantomData<P>),
}
