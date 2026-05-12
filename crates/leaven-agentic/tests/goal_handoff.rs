use leaven_agent::OutputContract;
use leaven_agentic::{
    GoalExecutionSignature, GoalHandoff, GoalHandoffDecision, GoalLoop, GoalSpecCheck,
    GoalSpecCheckRequest, GoalSpecCheckSignature, GoalSpecStatus, GoalStagePlan,
    GoalStagePlanRequest, GoalStagePlanSignature,
};

#[test]
fn goal_spec_check_signature_preserves_pre_goal_checklist() {
    let signature = GoalSpecCheckSignature::new(
        handoff(),
        "Complete the audit fixes without accepting placeholder proofs",
        ["docs/specs/initial_library.md"],
    )
    .with_audit_paths(["reviews/2026-05-11-fuckery-extermination-today"])
    .with_latest_jj_revision("kotlyxqm 24f02f97")
    .with_evaluation_summary("cargo test currently exposes missing public contracts");

    let instructions = signature.instructions();
    let task = instructions.task;

    assert!(task.contains("Original intent: build the real Leaven audit closer"));
    assert!(task.contains("Designed surface: Codex goal execution plus jj-tracked evals"));
    assert!(task.contains("Misleading proxy proofs:"));
    assert!(task.contains("toy loop"));
    assert!(task.contains(
        "Proof denominator: every cited audit finding is closed or explicitly re-scoped"
    ));
    assert!(task.contains("Latest jj revision: kotlyxqm 24f02f97"));
    assert!(task.contains("Return JSON with exactly these top-level fields"));
    assert!(task.contains("\"status\""));

    match signature.output_contract() {
        OutputContract::JsonFile { path, schema } => {
            assert_eq!(path.as_str(), ".leaven/goal/check.json");
            assert_eq!(schema.unwrap().name, "leaven_goal_spec_check_v1");
        }
        other => panic!("unexpected output contract: {other:?}"),
    }
}

#[test]
fn stage_plan_signature_forces_next_best_stage_instead_of_proxy_success() {
    let signature = GoalStagePlanSignature::new(
        handoff(),
        "Assessment says public jj tracking exists but Codex goal execution is missing",
    )
    .with_spec_paths(["docs/specs/codex_goal_loop.md"])
    .with_stage_budget("one coherent implementation slice");

    let instructions = signature.instructions();

    assert!(instructions.task.contains("Plan the next best stage"));
    assert!(
        instructions
            .task
            .contains("one coherent implementation slice")
    );
    assert!(
        instructions
            .task
            .contains("Do not plan a stage whose only proof is one of the misleading proxy proofs")
    );
    assert!(instructions.task.contains("\"objective\""));
    assert!(instructions.task.contains("\"verification_commands\""));

    match signature.output_contract() {
        OutputContract::JsonFile { path, schema } => {
            assert_eq!(path.as_str(), ".leaven/goal/next-stage.json");
            assert_eq!(schema.unwrap().name, "leaven_goal_stage_plan_v1");
        }
        other => panic!("unexpected output contract: {other:?}"),
    }
}

#[test]
fn execution_signature_tells_goal_mode_to_snapshot_verify_and_close_honestly() {
    let signature = GoalExecutionSignature::new(
        handoff(),
        "Implement the Codex goal adapter and jj run vocabulary",
    )
    .with_verification_commands([
        "cargo test -p leaven-agent-codex-cli",
        "cargo test -p leaven-artifact-jj",
    ])
    .with_jj_snapshot_labels(["pre-goal", "post-agent", "post-eval", "final"]);

    let instructions = signature.instructions();

    assert!(
        instructions
            .task
            .contains("Use persistent goal mode when the runtime exposes it")
    );
    assert!(instructions.task.contains("pre-goal"));
    assert!(instructions.task.contains("post-eval"));
    assert!(
        instructions
            .task
            .contains("cargo test -p leaven-agent-codex-cli")
    );
    assert!(
        instructions
            .task
            .contains("Only mark the goal complete after the proof denominator is satisfied")
    );
    assert!(
        instructions
            .task
            .contains("If blocked, stop with the blocker and the last jj snapshot")
    );

    assert!(matches!(
        signature.output_contract(),
        OutputContract::WorkspaceDiff { .. }
    ));
}

#[test]
fn handoff_prompts_cover_absent_optional_context_and_non_ready_decisions() {
    let mut revise = handoff();
    revise.decision = GoalHandoffDecision::ReviseSpecFirst;
    revise.misleading_proxy_proofs.clear();
    revise.spec_revisions_before_goal.clear();
    revise.explicit_non_goals.clear();

    let check = GoalSpecCheckSignature::new(
        revise,
        "repair the governing spec first",
        Vec::<String>::new(),
    );
    let check_task = check.instructions().task;

    assert!(check_task.contains("Decision: revise_spec_first"));
    assert!(check_task.contains("Governing spec paths:\n- none"));
    assert!(check_task.contains("Misleading proxy proofs:\n- none"));
    assert!(!check_task.contains("Latest jj revision:"));
    assert!(!check_task.contains("Evaluation summary:"));

    let mut reject = handoff();
    reject.decision = GoalHandoffDecision::RejectProxyGoal;
    let plan = GoalStagePlanSignature::new(reject, "proxy proof only").instructions();

    assert!(plan.task.contains("Decision: reject_proxy_goal"));
    assert!(plan.task.contains("Governing spec paths:\n- none"));
    assert!(!plan.task.contains("Stage budget:"));
}

#[test]
fn goal_loop_builds_typed_requests_and_decodes_outputs_without_hidden_runtime_calls() {
    let goal_loop = pleasant_goal_loop();
    let check_request = goal_loop.spec_check_request();
    assert_check_request_is_explicit(&check_request);

    let check = parsed_not_satisfied_check(&check_request);
    assert_eq!(check.status, GoalSpecStatus::NotSatisfied);
    assert!(check.needs_next_stage());
    assert!(check.summary().contains("operational runner"));

    let plan_request = goal_loop
        .stage_plan_request(&check)
        .with_stage_budget("one slice");
    assert!(
        plan_request
            .agent_run_request()
            .instructions
            .task
            .contains("one slice")
    );

    let plan = parsed_stage_plan(&plan_request);
    assert_eq!(plan.objective, "persist jj run records");

    let execution_run_request = goal_loop.execution_request(&plan).agent_run_request();
    assert_execution_task_carries_plan_details(&execution_run_request.instructions.task);
    assert!(matches!(
        execution_run_request.output_contract,
        OutputContract::WorkspaceDiff { .. }
    ));
}

#[test]
fn typed_goal_outputs_reject_invalid_json_and_make_satisfaction_explicit() {
    let request = GoalLoop::new(handoff(), "finish").spec_check_request();
    let error = request
        .parse_json_bytes(br#"{"status":"maybe"}"#)
        .unwrap_err();
    assert!(error.to_string().contains("goal output JSON"));

    let satisfied: GoalSpecCheck = request
        .parse_json_bytes(
            br#"{
                "status": "satisfied",
                "intent_preservation": "done",
                "satisfied_requirements": [],
                "missing_requirements": [],
                "proof_gaps": [],
                "misleading_proxy_proofs": [],
                "next_stage_hint": ""
            }"#,
        )
        .unwrap();
    assert!(satisfied.is_satisfied());
    assert!(!satisfied.needs_next_stage());

    let plan = GoalStagePlan {
        objective: "run final eval".to_owned(),
        why_this_stage: "prove denominator".to_owned(),
        required_changes: vec!["none".to_owned()],
        verification_commands: vec!["just check".to_owned()],
        jj_snapshot_labels: vec!["final".to_owned()],
        stop_condition: "green checks".to_owned(),
    };
    let execution = plan.execution_signature(handoff());
    assert!(
        execution
            .instructions()
            .task
            .contains("Stage objective: run final eval")
    );
}

fn handoff() -> GoalHandoff {
    GoalHandoff {
        original_intent: "build the real Leaven audit closer".to_owned(),
        designed_surface: "Codex goal execution plus jj-tracked evals".to_owned(),
        intent_preservation:
            "keeps the loop tied to the spec and audit denominator instead of a toy demo".to_owned(),
        misleading_proxy_proofs: vec![
            "toy loop".to_owned(),
            "green tests that do not cover the audit denominator".to_owned(),
        ],
        spec_revisions_before_goal: vec!["none".to_owned()],
        acceptance_path: vec![
            "check current work against the governing specs".to_owned(),
            "plan the next best stage".to_owned(),
            "run Codex goal execution with jj snapshots and evals".to_owned(),
        ],
        proof_denominator: "every cited audit finding is closed or explicitly re-scoped".to_owned(),
        explicit_non_goals: vec!["Firkin RLM backend".to_owned()],
        decision: GoalHandoffDecision::ReadyForGoal,
    }
}

fn pleasant_goal_loop() -> GoalLoop {
    GoalLoop::new(
        GoalHandoff::new(
            "close the real audit",
            "typed Codex goal loop surface",
            "keeps the stage tied to spec truth",
            "all findings are fixed or explicitly re-scoped",
        )
        .misleading_proxy_proof("toy example")
        .acceptance_step("check current workspace")
        .acceptance_step("plan next stage")
        .acceptance_step("execute with jj snapshots")
        .explicit_non_goal("Firkin backend"),
        "complete the audit closer",
    )
    .spec_path("docs/specs/codex_goal_loop.md")
    .audit_path("reviews/current")
    .latest_jj_revision("kotlyxqm 8a22de07")
    .evaluation_summary("coverage is green")
}

fn assert_check_request_is_explicit(check_request: &GoalSpecCheckRequest) {
    let run_request = check_request.agent_run_request();
    assert!(
        run_request
            .instructions
            .task
            .contains("complete the audit closer")
    );
    assert!(run_request.instructions.task.contains("reviews/current"));
    assert!(matches!(
        run_request.output_contract,
        OutputContract::JsonFile { .. }
    ));
}

fn parsed_not_satisfied_check(check_request: &GoalSpecCheckRequest) -> GoalSpecCheck {
    check_request
        .parse_json_bytes(
            br#"{
                "status": "not_satisfied",
                "intent_preservation": "still aligned",
                "satisfied_requirements": ["Codex goal flag exists"],
                "missing_requirements": ["operational runner"],
                "proof_gaps": ["no jj artifact persisted yet"],
                "misleading_proxy_proofs": ["toy example"],
                "next_stage_hint": "build the runner"
            }"#,
        )
        .unwrap()
}

fn parsed_stage_plan(plan_request: &GoalStagePlanRequest) -> GoalStagePlan {
    plan_request
        .parse_json_bytes(
            br#"{
                "objective": "persist jj run records",
                "why_this_stage": "data extraction is the blocker",
                "required_changes": ["add typed run records"],
                "verification_commands": ["cargo test -p leaven-artifact-jj"],
                "jj_snapshot_labels": ["pre-goal", "post-eval"],
                "stop_condition": "tests pass and records serialize"
            }"#,
        )
        .unwrap()
}

fn assert_execution_task_carries_plan_details(task: &str) {
    assert!(task.contains("persist jj run records"));
    assert!(task.contains("data extraction is the blocker"));
    assert!(task.contains("add typed run records"));
    assert!(task.contains("cargo test -p leaven-artifact-jj"));
    assert!(task.contains("tests pass and records serialize"));
}
