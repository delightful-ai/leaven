use leaven_kernel::{
    Cost, Fingerprint, MetadataBag, StageAttemptOutcome, StageAttemptReceiptId, StageCallId,
    StageRole, WorkspaceId,
};
use leaven_workspace::{WorkspacePath, WorkspaceSlot, fingerprint_file};

use crate::parser::ErasedStagePlan;
use crate::receipt::WorkspaceSetupReceipt;
use crate::tool::leaven_query_help;
use crate::{
    EntryAccess, EntryProjection, EntrySourceRef, OutputEntryReceipt, ParseReceipt, QueryRecord,
    StageAttemptReceipt, WorkspaceEntryReceipt, WorkspaceEntryRole, WorkspaceSetupError,
};

pub fn setup_stage_workspace(
    workspace: &mut WorkspaceSlot<'_>,
    plan: &ErasedStagePlan,
) -> Result<WorkspaceSetupReceipt, WorkspaceSetupError> {
    let mut receipt = WorkspaceSetupReceipt::default();
    let brief = render_brief(plan);
    write_plan_entry(
        workspace,
        &mut receipt,
        "BRIEF.md",
        brief.as_bytes(),
        WorkspaceEntryRole::brief(),
    )?;

    write_plan_entry(
        workspace,
        &mut receipt,
        "focus/stage_role.txt",
        plan.role.as_str().as_bytes(),
        WorkspaceEntryRole::stage_plan(),
    )?;

    let request_json = serde_json::to_vec_pretty(&plan.request_json)?;
    write_plan_entry(
        workspace,
        &mut receipt,
        "focus/request.json",
        &request_json,
        WorkspaceEntryRole::stage_plan(),
    )?;

    write_plan_entry(
        workspace,
        &mut receipt,
        "focus/instructions.md",
        plan.directive.instructions.as_bytes(),
        WorkspaceEntryRole::brief(),
    )?;

    let plan_json = serde_json::to_vec_pretty(plan)?;
    write_plan_entry(
        workspace,
        &mut receipt,
        ".leaven/stage-plan.json",
        &plan_json,
        WorkspaceEntryRole::stage_plan(),
    )?;

    let output_schema = serde_json::to_vec_pretty(&plan.output)?;
    write_plan_entry(
        workspace,
        &mut receipt,
        ".leaven/output_schema.json",
        &output_schema,
        WorkspaceEntryRole::stage_plan(),
    )?;

    let query_policy = serde_json::to_vec_pretty(&plan.query)?;
    write_plan_entry(
        workspace,
        &mut receipt,
        ".leaven/query_policy.json",
        &query_policy,
        WorkspaceEntryRole::stage_plan(),
    )?;

    write_plan_entry(
        workspace,
        &mut receipt,
        "output/.gitkeep",
        b"",
        WorkspaceEntryRole::output_skeleton(),
    )?;

    let tool_path = WorkspacePath::new("tools/leaven_query")?;
    workspace.write_file(&tool_path, leaven_query_tool_script().as_bytes())?;
    workspace.view_mut().set_executable(&tool_path, true)?;
    receipt.plan_entries.push(entry_receipt(
        workspace,
        tool_path,
        WorkspaceEntryRole::tool(),
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
    outputs: Vec<OutputEntryReceipt>,
    parse: Option<ParseReceipt>,
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
            outputs: Vec::new(),
            parse: None,
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
            outputs: self.outputs,
            parse: self.parse,
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

    pub fn push_output(&mut self, output: OutputEntryReceipt) {
        self.outputs.push(output);
    }

    pub fn set_parse(&mut self, parse: ParseReceipt) {
        self.cost = self.cost.clone().combine(&parse.cost);
        self.parse = Some(parse);
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

fn leaven_query_tool_script() -> String {
    format!("#!/bin/sh\ncat <<'EOF'\n{}EOF\n", leaven_query_help())
}

fn write_plan_entry(
    workspace: &mut WorkspaceSlot<'_>,
    receipt: &mut WorkspaceSetupReceipt,
    path: &str,
    bytes: &[u8],
    role: WorkspaceEntryRole,
) -> Result<(), WorkspaceSetupError> {
    let path = WorkspacePath::new(path)?;
    workspace.write_file(&path, bytes)?;
    receipt.plan_entries.push(entry_receipt(
        workspace,
        path,
        role,
        EntrySourceRef::Generated,
        EntryProjection::Generated,
    )?);
    Ok(())
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

#[cfg(test)]
mod tests {
    use futures::executor::block_on;
    use leaven_kernel::StageRole;
    use leaven_workspace::{WorkspaceConfig, WorkspaceFactory, WorkspacePath};
    use leaven_workspace_local::LocalWorkspaceFactory;

    use super::*;
    use crate::{
        AgentStagePlan, StageDirective, StageOutputContract, StageQueryPolicy,
        parser::ErasedStagePlan,
    };

    #[test]
    fn setup_stage_workspace_receipts_cover_every_generated_plan_entry() {
        block_on(async {
            let mut directive = StageDirective::new("Reflect", "Write the candidate edit.");
            directive.success_criteria.push("patch applies".to_owned());
            directive
                .cautions
                .push("do not inspect hidden labels".to_owned());
            let plan = AgentStagePlan::new(
                StageRole::reflect(),
                serde_json::json!({"candidate": "seed"}),
                directive,
                StageOutputContract::proposal_json(
                    WorkspacePath::new("output/proposal.json").unwrap(),
                ),
            )
            .with_query_policy(StageQueryPolicy::minimal());
            let erased = ErasedStagePlan::from_plan(&plan).unwrap();
            let mut workspace = LocalWorkspaceFactory::temp()
                .allocate(WorkspaceConfig::default())
                .await
                .unwrap();
            let mut slot = workspace.slot(WorkspacePath::root()).unwrap();

            let receipt = setup_stage_workspace(&mut slot, &erased).unwrap();

            assert_eq!(receipt.plan_entries.len(), 9);
            assert!(receipt.cost.is_zero());
            assert!(
                receipt
                    .plan_entries
                    .iter()
                    .all(|entry| entry.file.is_some())
            );
            assert!(receipt.plan_entries.iter().any(|entry| {
                entry.path == WorkspacePath::new("tools/leaven_query").unwrap()
                    && entry.role == WorkspaceEntryRole::tool()
            }));
            assert!(
                String::from_utf8(
                    slot.read_file(&WorkspacePath::new("BRIEF.md").unwrap())
                        .unwrap()
                )
                .unwrap()
                .contains("do not inspect hidden labels")
            );
            assert!(
                slot.view()
                    .is_executable(&WorkspacePath::new("tools/leaven_query").unwrap())
                    .unwrap()
            );
        });
    }
}
