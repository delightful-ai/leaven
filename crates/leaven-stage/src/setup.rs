use leaven_kernel::{
    Cost, Fingerprint, MetadataBag, StageAttemptOutcome, StageAttemptReceiptId, StageCallId,
    StageRole, WorkspaceId,
};

use crate::receipt::WorkspaceSetupReceipt;
use crate::{QueryRecord, StageAttemptReceipt};

pub fn setup_stage_workspace() {}

pub struct StageAttemptReceiptBuilder {
    receipt_id: StageAttemptReceiptId,
    workspace_id: WorkspaceId,
    stage_call_id: StageCallId,
    role: StageRole,
    plan_fingerprint: Fingerprint,
    setup: WorkspaceSetupReceipt,
    queries: Vec<QueryRecord>,
    cost: Cost,
    metadata: MetadataBag,
}

impl StageAttemptReceiptBuilder {
    #[must_use]
    pub fn new(
        workspace_id: WorkspaceId,
        stage_call_id: StageCallId,
        role: StageRole,
        plan_fingerprint: Fingerprint,
    ) -> Self {
        Self {
            receipt_id: StageAttemptReceiptId::new(),
            workspace_id,
            stage_call_id,
            role,
            plan_fingerprint,
            setup: WorkspaceSetupReceipt::default(),
            queries: Vec::new(),
            cost: Cost::zero(),
            metadata: MetadataBag::new(),
        }
    }

    #[must_use]
    pub fn finish(self, outcome: StageAttemptOutcome) -> StageAttemptReceipt {
        StageAttemptReceipt {
            receipt_id: self.receipt_id,
            workspace_id: self.workspace_id,
            stage_call_id: self.stage_call_id,
            role: self.role,
            plan_fingerprint: self.plan_fingerprint,
            setup: self.setup,
            queries: self.queries,
            outputs: Vec::new(),
            parse: None,
            cost: self.cost,
            outcome,
            metadata: self.metadata,
        }
    }
}
