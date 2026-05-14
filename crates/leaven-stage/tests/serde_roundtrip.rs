use leaven_kernel::{CandidateId, StageRole};
use leaven_stage::{
    AgentStagePlan, AllowedQuerySet, MediaType, OutputEntryId, StageDirective, StageOutputContract,
    StageQuery, StageQueryKind, StageQueryPolicy,
};
use leaven_workspace::WorkspacePath;

#[test]
fn stage_plan_roundtrips() {
    let plan = AgentStagePlan::new(
        StageRole::reflect(),
        serde_json::json!({ "candidate": "abc" }),
        StageDirective::new("Reflect", "write a proposal"),
        StageOutputContract::proposal_json(WorkspacePath::new("output/proposal.json").unwrap()),
    )
    .with_query_policy(StageQueryPolicy::bounded(
        AllowedQuerySet::only([StageQueryKind::Help, StageQueryKind::Candidate]),
        vec![StageQuery::Candidate {
            id: CandidateId::new(),
        }],
        Some(4),
        Some(1024),
    ));

    let encoded = serde_json::to_string(&plan).unwrap();
    let decoded: AgentStagePlan<serde_json::Value> = serde_json::from_str(&encoded).unwrap();

    assert_eq!(decoded.role, StageRole::reflect());
    assert!(decoded.query.allowed.contains(StageQueryKind::Candidate));
    assert_eq!(
        decoded.output.required[0].id,
        OutputEntryId::new_static("proposal")
    );
    assert_eq!(decoded.output.required[0].media_type, MediaType::Json);
}
