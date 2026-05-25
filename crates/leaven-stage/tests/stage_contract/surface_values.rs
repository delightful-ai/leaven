use leaven_kernel::{BudgetSnapshot, StageCallId, StageRole};
use leaven_stage::{
    AllowedQuerySet, Diagnostic, DiagnosticSeverity, MediaType, QueryEffect, QueryResult,
    QueryTiming, StageQuery, StageQueryKind,
};

#[test]
fn stage_value_helpers_cover_roles_queries_media_and_diagnostics() {
    assert_eq!(StageRole::new("reflect").unwrap().as_str(), "reflect");
    assert!(StageRole::new("bad role").is_err());
    assert_eq!(StageRole::reflect().as_str(), "reflect");

    let default_queries = AllowedQuerySet::reflection_default();
    for kind in [
        StageQueryKind::Help,
        StageQueryKind::Candidate,
        StageQueryKind::Assessment,
        StageQueryKind::Lineage,
        StageQueryKind::Diff,
    ] {
        assert!(default_queries.contains(kind));
    }
    assert!(!default_queries.contains(StageQueryKind::Evidence));
    assert_eq!(
        StageQuery::Assessment {
            id: leaven_kernel::AssessmentId::new(),
        }
        .kind(),
        StageQueryKind::Assessment
    );

    assert_eq!(MediaType::Json.as_str(), "application/json");
    assert_eq!(MediaType::Markdown.as_str(), "text/markdown");
    assert_eq!(MediaType::Text.as_str(), "text/plain");
    assert_eq!(MediaType::Diff.as_str(), "text/x-diff");
    assert_eq!(MediaType::Binary.as_str(), "application/octet-stream");
    assert_eq!(
        MediaType::Custom("application/x-test".to_owned()).as_str(),
        "application/x-test"
    );

    let diagnostic = Diagnostic::error("stage failed");
    assert_eq!(diagnostic.severity, DiagnosticSeverity::Error);
    assert_eq!(diagnostic.message, "stage failed");

    let query_id = leaven_kernel::StageQueryId::new();
    let record = QueryResult {
        query_id,
        timing: QueryTiming::AgentRequested,
        query: StageQuery::Help,
        effect: QueryEffect::NotVisible("hidden".to_owned()),
        entries: Vec::new(),
        cost: leaven_kernel::Cost::zero(),
    }
    .into_record();
    assert_eq!(record.query_id, query_id);
}

#[test]
fn agent_stage_call_context_exposes_explicit_scope_values() {
    let stage_call_id = StageCallId::new();
    let read_scope = leaven_engine::ReadScope::default();
    let budget = BudgetSnapshot::default();
    let ctx =
        leaven_stage::AgentStageCallContext::new(stage_call_id, read_scope.clone(), budget.clone());

    assert_eq!(ctx.stage_call_id(), stage_call_id);
    assert_eq!(
        ctx.read_scope().hidden_partitions,
        read_scope.hidden_partitions
    );
    assert_eq!(ctx.budget_snapshot(), budget);
}
