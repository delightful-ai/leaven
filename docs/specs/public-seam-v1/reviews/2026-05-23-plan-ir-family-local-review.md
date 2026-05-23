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

Fresh adversarial sub-agent sign-off:

- Reviewer: sub-agent `019e55f8-5be2-7c42-8ae9-02f8f756c2f1`
- Result: sign off. `ps1.plan.ir_family` may be marked proven.

Positive proof accepted:

- `crates/leaven-public-seam/tests/plan_document.rs::plan_ir_family_accepts_typed_let_call_write_documents` validates the typed Let/Call/Write document.
- `crates/leaven-public-seam/tests/plan_document.rs::plan_ir_family_lowers_and_executes_let_call_write_through_public_seam_owner` executes literal Let -> `lm_complete` Call -> `emit_run_event` Write through `PublicSeamPackage::execute_plan_document`.

Negative proof accepted:

- `crates/leaven-public-seam/tests/plan_document.rs::plan_ir_family_rejects_unknown_core_call_write_and_escape_hatch_ops` rejects unknown core, call, write, and top-level escape-hatch operations before execution.
- `crates/leaven-public-seam/tests/plan_document.rs::plan_ir_family_execution_rejects_dry_run_or_no_graph_write_fake_execution` rejects dry-run and no-graph-write fake execution before host calls.
- `crates/leaven-public-seam/tests/plan_document.rs::plan_ir_family_execution_rejects_known_variants_outside_representative_harness` rejects schema-valid but unsupported known variants at the representative harness boundary.

Fake-pass, topology, and public-maturity findings:

- `PublicSeamPackage::execute_plan_document` validates the active Plan schema, lowers through the seam harness, then validates the produced Plan Result, so the proof is not a generated-struct round trip.
- `PlanExecutionHost` is the explicit effect boundary; `leaven-public-seam` does not import engine graph internals or provider/runtime crates for this proof.
- `crates/leaven-public-seam/AGENTS.md` limits the route to advanced public-seam validation/classification and representative lowering/execution. It does not claim ACP/session delivery, provider runtime execution, cache behavior, graph mutation authority, full Plan IR coverage, evaluator runtime production, or runtime revision-read enforcement.

Fresh reviewer verification:

- `CARGO_INCREMENTAL=0 cargo test -p leaven-public-seam --test plan_document`
- `CARGO_INCREMENTAL=0 cargo test -p leaven-public-seam --test contract_package conformance_matrix_rows_are_unique_honest_and_reference_real_files -- --exact`
- `CARGO_INCREMENTAL=0 cargo test -p leaven --test topology_contract`

Residual non-blocking risks:

- `ps1.plan.revision_modes` and `ps1.plan.execution_modes` remain pending.
- This does not prove ACP/session/provider runtime behavior, cache/replay behavior, engine graph mutation, revision-mode runtime reads, or full execution coverage for every schema-valid Plan IR variant.

Execution-mode follow-up:

- `execute_plan_document` now honors `dry_run`, `require_cached`, and `replay` in addition to `execute`.
- `dry_run` validates and returns a schema-valid no-effect Plan Result without invoking call/write host effects.
- `require_cached` asks only the cache-specific LM hook and rejects cache misses before live LM/provider work.
- `replay` loads supplied receipts through the replay hook without invoking live call/write host effects.
- `ps1.plan.execution_modes` remains pending until this broader mode behavior receives its own adversarial sign-off.

Revision-mode follow-up:

- `execute_plan_document` now lowers schema-valid `graph_query` Let expressions through a public seam graph-read host hook.
- The graph-read request carries an explicit `PlanGraphReadScope` derived from `latest_at_start`, `at_revision`, or `since_revision` consistency.
- Read-only graph-query execution returns schema-valid `graph_set` Plan Result values and preserves `final_revision == base_revision`.
- `since_revision` event sources without the declared base revision are rejected during Plan document validation instead of falling back to latest.
- `ps1.plan.revision_modes` remains pending until this broader revision behavior receives its own adversarial sign-off.
