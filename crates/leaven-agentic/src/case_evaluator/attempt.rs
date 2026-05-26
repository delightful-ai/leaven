use std::num::NonZeroUsize;

use leaven_kernel::{AgentSessionId, CandidateId, Cost, EvaluationSetId, RunId};
use leaven_workspace::WorkspacePath;

use crate::AgenticAdapterError;
use crate::case_record::{
    AgentCaseRetryRecord, AgentCaseRunError, AgentCaseRunRecord, FailedAgentCaseRun,
    ScoredAgentCaseRun,
};

pub(super) struct CaseAttemptFailure {
    pub(super) record: AgentCaseRunRecord,
    pub(super) source: AgenticAdapterError,
}

#[derive(Clone, Copy)]
pub(super) struct CaseAttemptScope {
    pub(super) run_id: RunId,
    pub(super) candidate: CandidateId,
    pub(super) case: leaven_kernel::CaseId,
    pub(super) partition: EvaluationSetId,
    pub(super) attempt: NonZeroUsize,
}

impl CaseAttemptScope {
    pub(super) fn failed(
        self,
        session: Option<AgentSessionId>,
        outputs: Vec<WorkspacePath>,
        error: AgentCaseRunError,
        cost: Cost,
    ) -> AgentCaseRunRecord {
        AgentCaseRunRecord::failed_attempt(FailedAgentCaseRun {
            run_id: self.run_id,
            candidate: self.candidate,
            case: self.case,
            partition: self.partition,
            attempt: self.attempt,
            session,
            outputs,
            error,
            cost,
        })
    }

    pub(super) fn scored(
        self,
        session: AgentSessionId,
        outputs: Vec<WorkspacePath>,
        retries: Vec<AgentCaseRetryRecord>,
        cost: Cost,
    ) -> AgentCaseRunRecord {
        AgentCaseRunRecord::scored_attempt(ScoredAgentCaseRun {
            run_id: self.run_id,
            candidate: self.candidate,
            case: self.case,
            partition: self.partition,
            attempt: self.attempt,
            session,
            outputs,
            retries,
            cost,
        })
    }
}

impl CaseAttemptFailure {
    pub(super) fn new(record: AgentCaseRunRecord, source: AgenticAdapterError) -> Self {
        Self { record, source }
    }
}
