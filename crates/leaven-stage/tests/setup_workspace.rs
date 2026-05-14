use futures::executor::block_on;
use leaven_kernel::{Cost, Fingerprint, StageAttemptOutcome, StageCallId, StageRole, WorkspaceId};
use leaven_stage::{
    AgentStagePlan, QueryRecord, QueryRecordEffect, QueryTiming, StageAttemptReceiptBuilder,
    StageDirective, StageOutputContract, StageQuery, StageQueryPolicy, parser::ErasedStagePlan,
    setup_stage_workspace,
};
use leaven_workspace::{WorkspaceConfig, WorkspaceFactory, WorkspacePath};
use leaven_workspace_local::LocalWorkspaceFactory;

#[test]
fn setup_stage_workspace_writes_brief_plan_and_output_skeleton() {
    block_on(async {
        let mut directive = StageDirective::new("Reflect", "Write the patch.");
        directive
            .success_criteria
            .push("candidate applies".to_owned());
        directive
            .cautions
            .push("do not inspect hidden targets".to_owned());
        let plan = AgentStagePlan::new(
            StageRole::reflect(),
            serde_json::json!({"candidate": "abc"}),
            directive,
            StageOutputContract::proposal_json(WorkspacePath::new("output/proposal.json").unwrap()),
        )
        .with_query_policy(StageQueryPolicy::minimal());
        let erased = ErasedStagePlan::from_plan(&plan).unwrap();
        let mut workspace = LocalWorkspaceFactory::temp()
            .allocate(WorkspaceConfig::default())
            .await
            .unwrap();
        {
            let mut slot = workspace.slot(WorkspacePath::root()).unwrap();
            let receipt = setup_stage_workspace(&mut slot, &erased).unwrap();

            assert_eq!(receipt.plan_entries.len(), 3);
            assert!(
                String::from_utf8(
                    slot.read_file(&WorkspacePath::new("BRIEF.md").unwrap())
                        .unwrap()
                )
                .unwrap()
                .contains("do not inspect hidden targets")
            );
            assert!(
                slot.read_file(&WorkspacePath::new(".leaven/stage-plan.json").unwrap())
                    .is_ok()
            );
            assert_eq!(
                slot.read_file(&WorkspacePath::new("output/.gitkeep").unwrap())
                    .unwrap(),
                b""
            );
        }
        workspace.cleanup().await.unwrap();
    });
}

#[test]
fn stage_attempt_receipt_builder_combines_setup_query_and_extra_costs() {
    let mut builder = StageAttemptReceiptBuilder::new(
        WorkspaceId::new(),
        StageCallId::new(),
        StageRole::reflect(),
        Fingerprint::from_bytes([3; 32]),
    );
    let query = QueryRecord {
        query_id: leaven_kernel::StageQueryId::new(),
        timing: QueryTiming::AgentRequested,
        query: StageQuery::Help,
        effect: QueryRecordEffect::ReturnedSummary("ok".to_owned()),
        entries: Vec::new(),
        cost: Cost::metric_calls(2),
    };

    builder.push_query(query);
    builder.add_cost(&Cost::llm_calls(1));
    let receipt = builder.finish(StageAttemptOutcome::Completed);

    assert_eq!(receipt.queries.len(), 1);
    assert_eq!(receipt.cost.metric_calls, 2);
    assert_eq!(receipt.cost.llm_calls, 1);
    assert_eq!(receipt.outcome, StageAttemptOutcome::Completed);
}
