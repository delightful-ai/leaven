# Codex Goal Loop and JJ Tracked Proofs

> Status: initial implemented slice.  
> Date: 2026-05-12.  
> Governing specs: `docs/specs/agentic_stage_runtime.md`,
> `docs/specs/codex_cli_agent_runtime.md`.

Leaven's Codex goal loop is a composition of three separate concerns:

1. Pre-goal handoff prompts in `leaven-agentic`.
2. Explicit Codex CLI goal-mode enablement in `leaven-agent-codex-cli`.
3. Durable jj snapshot/evaluation vocabulary in `leaven-artifact-jj`.

The goal is not a toy `while true` example. The loop is only meaningful when it
keeps the original spec/audit denominator in view, plans the next best coherent
stage, executes through a persistent agent goal, snapshots intermediate jj
state, runs evaluations, and refuses proxy proofs.

## 1. Pre-goal Signatures

`leaven-agentic` owns three provider-neutral prompt contracts:

- `GoalSpecCheckSignature`: check the current workspace against the governing
  spec, audit notes, latest jj revision, and evaluation summary.
- `GoalStagePlanSignature`: choose the next best coherent stage when the goal is
  not yet satisfied.
- `GoalExecutionSignature`: hand one coherent stage to a persistent agent goal
  with verification commands, jj snapshot labels, and an honest closeout rule.

The pleasant API surface is `GoalLoop`. It is deliberately explicit: it builds
typed request wrappers, but it never runs a provider, reads a hidden file, or
creates jj snapshots by itself.

```rust
let goal_loop = GoalLoop::new(handoff, "complete the audit closer")
    .spec_path("docs/specs/codex_goal_loop.md")
    .audit_path("reviews/current")
    .latest_jj_revision("kotlyxqm 8a22de07")
    .evaluation_summary("coverage is green");

let check_request = goal_loop.spec_check_request();
let check_run = check_request.agent_run_request();
let check = check_request.parse_workspace_output(&workspace)?;

if check.needs_next_stage() {
    let plan_request = goal_loop
        .stage_plan_request(&check)
        .with_stage_budget("one coherent slice");
    let plan_run = plan_request.agent_run_request();
    let plan = plan_request.parse_workspace_output(&workspace)?;

    let execution_run = goal_loop.execution_request(&plan).agent_run_request();
}
```

The typed outputs are:

- `GoalSpecCheck` with `GoalSpecStatus::{Satisfied, NotSatisfied, Blocked}`.
- `GoalStagePlan` with objective, rationale, required changes, verification
  commands, jj snapshot labels, and stop condition.

Parsing failures surface as `AgenticParseError`; callers choose where runtime
outputs live by supplying the workspace view.

All three carry `GoalHandoff`, which preserves:

- original intent
- designed surface
- intent preservation
- misleading proxy proofs
- spec revisions before goal
- acceptance path
- proof denominator
- explicit non-goals
- handoff decision

## 2. Codex Runtime Boundary

`leaven-agent-codex-cli` remains a provider leaf. It only renders the Codex
command line:

```text
CodexCliConfig { goal_mode: CodexCliGoalMode::Enabled, .. }
  -> codex exec --enable goals ...
```

It must not learn Leaven candidates, proposals, assessments, jj snapshots, or
audit denominators. The runtime executes one `AgentRunRequest`; the stage above
it decides what that request means.

## 3. JJ Tracking Boundary

`leaven-artifact-jj` records run facts as data:

- `JjTrackedRun`
- `JjSnapshotRecord`
- `JjEvaluationRecord`
- `JjSnapshotPolicy`

It does not run `jj`, manage working copies, create changes, or interpret
workspace status. A runner or workspace layer can execute the commands and then
persist the resulting facts through these types.

## 4. Acceptance

This slice is accepted when:

- Codex CLI goal mode is opt-in and renders `--enable goals`.
- The pre-goal checklist is represented in typed Leaven signatures.
- Next-stage planning rejects misleading proxy proofs.
- Execution prompts require jj snapshots, verification commands, and honest
  blocked closeout.
- JJ tracked run data preserves the proof denominator, snapshot chain, and eval
  records.

Future work is the operational runner that executes those signatures in order
against a concrete workspace and stores the resulting `JjTrackedRun` artifacts.
