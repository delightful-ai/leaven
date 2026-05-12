use leaven_artifact_jj::{
    JjEvaluationRecord, JjSnapshotKind, JjSnapshotPolicy, JjSnapshotRecord, JjTrackedRun,
};

#[test]
fn jj_tracked_run_records_intermediate_snapshots_and_eval_denominator() {
    let mut run = JjTrackedRun::for_goal(
        "leaven-audit-closeout",
        "all audit findings are closed or explicitly re-scoped",
    );
    run.spec_paths
        .push("docs/specs/codex_goal_loop.md".to_owned());

    let initial = run.record_snapshot(
        JjSnapshotRecord::new(JjSnapshotKind::Initial, "pre-goal")
            .with_change_id("kotlyxqm")
            .with_commit_id("24f02f97")
            .with_operation_id("op-1")
            .with_description("before Codex goal loop"),
    );
    let post_eval = run.record_snapshot(
        JjSnapshotRecord::new(JjSnapshotKind::PostEvaluation, "post-eval")
            .with_change_id("ppprmwnq")
            .with_commit_id("aabbccdd"),
    );

    run.record_evaluation(
        JjEvaluationRecord::new(
            "agent-codex-cli tests",
            "cargo test -p leaven-agent-codex-cli",
            post_eval,
        )
        .with_exit_code(0)
        .with_stdout_path(".leaven/evals/agent-codex-cli.stdout"),
    );

    assert_eq!(initial, 0);
    assert_eq!(post_eval, 1);
    assert_eq!(run.snapshots.len(), 2);
    assert_eq!(run.evaluations.len(), 1);
    assert_eq!(run.latest_snapshot().unwrap().label, "post-eval");
    assert_eq!(run.evaluations[0].snapshot_index, post_eval);
    assert!(run.evaluations[0].passed());

    let serialized = serde_json::to_string(&run).unwrap();
    assert!(serialized.contains("proof_denominator"));
    assert!(serialized.contains("post-evaluation"));
}

#[test]
fn jj_goal_loop_snapshot_policy_names_required_points() {
    let policy = JjSnapshotPolicy::goal_loop();

    assert!(policy.required);
    assert_eq!(policy.points[0].kind, JjSnapshotKind::Initial);
    assert!(
        policy.points.iter().any(|point| {
            point.kind == JjSnapshotKind::PostAgent && point.label == "post-agent"
        })
    );
    assert!(policy.points.iter().any(|point| {
        point.kind == JjSnapshotKind::PostEvaluation && point.label == "post-eval"
    }));
    assert!(
        policy
            .points
            .iter()
            .any(|point| { point.kind == JjSnapshotKind::Final && point.label == "final" })
    );
}

#[test]
fn jj_records_dirty_blocked_snapshots_and_failed_eval_logs() {
    let mut run = JjTrackedRun::for_goal("blocked-goal", "runtime can explain the blocker");
    let blocked = run.record_snapshot(
        JjSnapshotRecord::new(JjSnapshotKind::Blocked, "blocked")
            .with_dirty(true)
            .with_description("verification could not run"),
    );
    let failed = JjEvaluationRecord::new("coverage", "just coverage", blocked)
        .with_exit_code(1)
        .with_stderr_path(".leaven/evals/coverage.stderr");

    assert!(!failed.passed());
    assert_eq!(
        failed.stderr_path.as_deref(),
        Some(".leaven/evals/coverage.stderr")
    );
    run.record_evaluation(failed);

    let latest = run.latest_snapshot().unwrap();
    assert!(latest.dirty);
    assert_eq!(latest.kind, JjSnapshotKind::Blocked);
}
