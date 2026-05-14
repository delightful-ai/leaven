use leaven_kernel::{
    Cost, Fingerprint, MetadataBag, StageAttemptOutcome, StageAttemptReceiptId, StageCallId,
    StageRole, WorkspaceId,
};
use leaven_workspace::{WorkspacePath, WorkspaceSlot, fingerprint_file};

use crate::parser::ErasedStagePlan;
use crate::receipt::WorkspaceSetupReceipt;
use crate::{
    EntryAccess, EntryProjection, EntrySourceRef, QueryRecord, StageAttemptReceipt,
    WorkspaceEntryReceipt, WorkspaceEntryRole, WorkspaceSetupError,
};

pub fn setup_stage_workspace(
    workspace: &mut WorkspaceSlot<'_>,
    plan: &ErasedStagePlan,
) -> Result<WorkspaceSetupReceipt, WorkspaceSetupError> {
    let mut receipt = WorkspaceSetupReceipt::default();
    let brief = render_brief(plan);
    let brief_path = WorkspacePath::new("BRIEF.md")?;
    workspace.write_file(&brief_path, brief.as_bytes())?;
    receipt.plan_entries.push(entry_receipt(
        workspace,
        brief_path,
        WorkspaceEntryRole::brief(),
        EntrySourceRef::Generated,
        EntryProjection::Generated,
    )?);

    let plan_path = WorkspacePath::new(".leaven/stage-plan.json")?;
    let plan_json = serde_json::to_vec_pretty(plan)?;
    workspace.write_file(&plan_path, &plan_json)?;
    receipt.plan_entries.push(entry_receipt(
        workspace,
        plan_path,
        WorkspaceEntryRole::stage_plan(),
        EntrySourceRef::Generated,
        EntryProjection::Generated,
    )?);

    let keep_path = WorkspacePath::new("output/.gitkeep")?;
    workspace.write_file(&keep_path, b"")?;
    receipt.plan_entries.push(entry_receipt(
        workspace,
        keep_path,
        WorkspaceEntryRole::output_skeleton(),
        EntrySourceRef::Generated,
        EntryProjection::Generated,
    )?);
    Ok(receipt)
}

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

    pub fn set_setup(&mut self, setup: WorkspaceSetupReceipt) {
        self.cost = self.cost.clone().combine(&setup.cost);
        self.setup = setup;
    }

    pub fn push_query(&mut self, query: QueryRecord) {
        self.cost = self.cost.clone().combine(&query.cost);
        self.queries.push(query);
    }

    pub fn add_cost(&mut self, cost: &Cost) {
        self.cost = self.cost.clone().combine(cost);
    }
}

fn render_brief(plan: &ErasedStagePlan) -> String {
    let mut brief = format!(
        "# {}\n\n{}\n\n",
        plan.directive.title, plan.directive.instructions
    );
    if !plan.directive.success_criteria.is_empty() {
        brief.push_str("## Success Criteria\n");
        for criterion in &plan.directive.success_criteria {
            brief.push_str("- ");
            brief.push_str(criterion);
            brief.push('\n');
        }
    }
    if !plan.directive.cautions.is_empty() {
        brief.push_str("\n## Cautions\n");
        for caution in &plan.directive.cautions {
            brief.push_str("- ");
            brief.push_str(caution);
            brief.push('\n');
        }
    }
    brief
}

fn entry_receipt(
    workspace: &WorkspaceSlot<'_>,
    path: WorkspacePath,
    role: WorkspaceEntryRole,
    source: EntrySourceRef,
    projection: EntryProjection,
) -> Result<WorkspaceEntryReceipt, WorkspaceSetupError> {
    let file = fingerprint_file(workspace.view(), &path)?;
    Ok(WorkspaceEntryReceipt {
        id: leaven_kernel::WorkspaceEntryId::new(),
        path,
        role,
        source,
        projection,
        access: EntryAccess::InputReadOnly,
        fingerprint: file.fingerprint,
        bytes: Some(file.bytes),
        file: Some(file),
        produced_by_query: None,
        metadata: MetadataBag::new(),
    })
}
