# Plan IR Family Local Adversarial Pass

Scope: `ps1.plan.ir_family` evidence staged in `leaven-public-seam`.

Reviewer: Codex local review in active goal thread `019e5525-e070-7be3-adee-781067a24e84`.

Result: no sign-off. This is a blocker-finding and fix note only; the row remains pending until the required adversarial sub-agent review signs off.

Findings:

- The staged Plan execution harness proved that a schema-valid Let/Call/Write document could lower through `PublicSeamPackage::execute_plan_document`, but the first execution fixture used `mode: execute` with `commit: no_graph_writes`. That made a write-host call possible under a no-write policy and would have been a fake pass for both Plan IR family evidence and execution-mode semantics.
- The harness also had no negative that a dry-run plan cannot reach the host. That left room for an implementation that serializes `mode` while ignoring it.

Resolution:

- `execute_plan_document` now rejects non-`execute` modes before lowering.
- `execute_plan_document` now rejects write ops under `no_graph_writes` before host calls.
- `plan_ir_family_lowers_and_executes_let_call_write_through_public_seam_owner` now uses `graph_writes_atomic` with stale rejection for the representative write execution path.
- `plan_ir_family_execution_rejects_dry_run_or_no_graph_write_fake_execution` proves both fake execution cases leave the host untouched.

Residual limits:

- This still does not sign off `ps1.plan.ir_family`.
- The representative harness does not prove full Plan IR coverage, graph-query execution, cache/replay behavior, ACP delivery, provider runtime execution, or RunContext graph mutation authority.
