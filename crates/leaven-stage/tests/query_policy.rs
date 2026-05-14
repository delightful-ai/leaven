use leaven_stage::{AllowedQuerySet, StageQuery, StageQueryKind, StageQueryPolicy};

#[test]
fn prewarm_queries_count_as_queries() {
    let policy = StageQueryPolicy::bounded(
        AllowedQuerySet::only([StageQueryKind::Help]),
        vec![StageQuery::Help],
        Some(1),
        Some(0),
    );

    assert_eq!(policy.prewarm.len(), 1);
    assert!(policy.allowed.contains(policy.prewarm[0].kind()));
    assert_eq!(policy.max_queries, Some(1));
}
