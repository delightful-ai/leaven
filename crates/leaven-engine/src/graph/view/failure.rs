use leaven_kernel::{ApplyAttemptId, ErrorRecord, ProposalId, Timestamp};

use crate::graph::storage::{ApplyAttemptOutcome, ApplyAttemptRecord};

pub struct FailureRef<'g> {
    pub(super) record: &'g ApplyAttemptRecord,
}

impl FailureRef<'_> {
    #[must_use]
    pub fn id(&self) -> ApplyAttemptId {
        self.record.id
    }

    #[must_use]
    pub fn proposal_id(&self) -> ProposalId {
        self.record.proposal_id
    }

    #[must_use]
    pub fn error(&self) -> &ErrorRecord {
        match &self.record.outcome {
            ApplyAttemptOutcome::Failure { error } => error,
            ApplyAttemptOutcome::Success { .. } => {
                unreachable!("FailureRef is only constructed from failed apply attempts")
            }
        }
    }

    #[must_use]
    pub fn created_at(&self) -> Timestamp {
        self.record.created_at
    }
}
