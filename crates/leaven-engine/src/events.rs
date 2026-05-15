//! Run events.

use leaven_core::{CausalInputs, ProposalEffectKind};
use leaven_kernel::{
    AssessmentId, BudgetSnapshot, CandidateId, Cost, ErrorRecord, EvaluationRequestId, EvaluatorId,
    IterationId, PopulationId, ProposalBatchId, ProposalId, RunId, StageAttemptOutcome,
    StageAttemptReceiptRef, StageCallId, StageId, StageRole,
};

use crate::PopulationEvent;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct CausalInputsSummary {
    pub inputs: CausalInputs,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, serde::Serialize, serde::Deserialize)]
pub enum StopReason {
    OptimizerDone,
    BudgetReached,
    BudgetExceeded,
    StopperTriggered,
    External,
    Error,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ErrorPolicy {
    Continued,
    Retried,
    StoppedRun,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct EvaluationRequestSummary {
    pub candidate_count: usize,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum RunEvent {
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
        effect: ProposalEffectKind,
        causal: CausalInputs,
        informed_by_count: usize,
    },
    StageAttemptRecorded {
        stage_call_id: StageCallId,
        role: StageRole,
        receipt: StageAttemptReceiptRef,
        outcome: StageAttemptOutcome,
    },
    ApplySucceeded {
        proposal_id: ProposalId,
        candidate_id: CandidateId,
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
        cache: crate::CacheStatus,
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
}
