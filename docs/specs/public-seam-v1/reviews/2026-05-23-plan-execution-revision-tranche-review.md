# Public Seam V1 Plan Execution and Revision Tranche Review

Scope: broader public-seam Plan IR/result tranche after implementation commits:

- `somzrsol` / `fe8ce58f` — `public-seam: widen plan ir execution harness`
- `wmsrynlz` / `3cab08e4` — `public-seam: make graph reads emit query receipts`
- `kwynqsqs` / `898fa08b` — `public-seam: harden failed call charge linkage`

Rows reviewed:

- `ps1.plan.execution_modes`
- `ps1.plan.revision_modes`
- `ps1.receipts.audit_currency`
- `ps1.receipts.failed_costs`

Fresh evidence before review:

- `docs/specs/public-seam-v1/conformance-matrix.yaml`
- `docs/specs/public-seam-v1/01_plan_ir_spec_v0.3.md`
- `docs/specs/public-seam-v1/03_result_receipts_spec_v0.3.md`
- `docs/specs/public-seam-v1/notes/CONFORMANCE_TESTS_v0.3.md`
- `docs/specs/public-seam-v1/schemas/leaven.plan.v1.schema.json`
- `docs/specs/public-seam-v1/schemas/leaven.plan_result.v1.schema.json`
- `crates/leaven-public-seam/src/package.rs`
- `crates/leaven-public-seam/src/plan.rs`
- `crates/leaven-public-seam/src/plan_execution.rs`
- `crates/leaven-public-seam/src/result.rs`
- `crates/leaven-public-seam/src/lib.rs`
- `crates/leaven-public-seam/tests/plan_document.rs`
- `crates/leaven-public-seam/tests/plan_result.rs`
- `crates/leaven-public-seam/tests/contract_package.rs`
- `crates/leaven-public-seam/AGENTS.md`
- `docs/specs/public-seam-v1/reviews/2026-05-23-plan-ir-family-local-review.md`

Adversarial reviewer:

- Sub-agent `019e5611-b6a8-71e0-8574-2c239751a267`

Review result:

- `ps1.plan.execution_modes`: signed off. The row may be marked `proven` with current evidence.
- `ps1.plan.revision_modes`: signed off. The row may be marked `proven` with current evidence.
- `ps1.receipts.audit_currency`: blocked. The row must remain pending.
- `ps1.receipts.failed_costs`: blocked. The row must remain pending.

Signed-off rationale:

- `ps1.plan.execution_modes`: `execute_plan` branches on validated mode instead of ignoring serialized mode. `dry_run` returns a schema-valid no-effect result with `final_revision == base_revision` and no host call/write effects. `require_cached` uses `cached_lm_complete`, refuses misses, and rejects schema-valid `agent_run`/`sandbox_exec` without live host work. `replay` loads declared receipts through the replay hook without live call/write effects.
- `ps1.plan.revision_modes`: active schema consistency modes match the locked package. Implementation lowers them into `LatestAtStart`, `AtRevision`, and `SinceRevision` scopes. Tests assert the host receives the declared scopes and read-only results preserve `final_revision == base_revision`. Negative tests reject mismatched or missing `since_revision` event-source bases instead of falling back to latest.

Residual limits that must remain documented:

- The Plan execution-mode proof is representative public-seam mode behavior, not ACP delivery, provider runtime, engine RunGraph mutation proof, general cache backend, or full Plan IR runtime.
- The Plan revision-mode proof is active-schema validation and public-seam lowering into explicit `PlanGraphReadScope`, not an engine RunGraph read implementation.

Blocking findings for rows left pending:

- `ps1.receipts.audit_currency`: blocked. The row requires missing receipt, mismatched hash, or wrong operation kind to fail validation or replay. Current validation checks hash role prefixes, not that `result_hash` actually hashes the result value or `op_hash`/`request_hash` hashes the operation/request. `execute_plan` produces real JCS hashes, which is useful producer evidence, but the row's validation/replay negative remains unproven.
- `ps1.receipts.failed_costs`: blocked. Current positive proof is still a Plan Result fixture, not a controlled failing LM, agent, or sandbox call that incurs cost through execution. The validator also checks charge receipt presence and back-reference, but does not compare the failed call `cost` to referenced charge receipt costs, so a linked partial charge could still shrink cost provenance.

Required fixes before blocked rows can be promoted:

- `ps1.receipts.audit_currency`: add validation and negative tests for same-prefix but content-mismatched `result_hash` and `op_hash`/`request_hash`, or explicitly route that proof through replay if validation cannot recompute from available context.
- `ps1.receipts.failed_costs`: add an executable controlled failing paid call path or keep the row explicitly pending as runtime evidence. Also validate that linked charge receipt costs cover the failed call cost so cost cannot disappear or shrink while provenance links remain intact.

Verification reported before review:

- `cargo fmt --check`
- `CARGO_INCREMENTAL=0 cargo test -p leaven-public-seam --test plan_document`
- `CARGO_INCREMENTAL=0 cargo test -p leaven-public-seam --test plan_result`
- `CARGO_INCREMENTAL=0 cargo test -p leaven-public-seam --test contract_package conformance_matrix_rows_are_unique_honest_and_reference_real_files -- --exact`
- `CARGO_INCREMENTAL=0 cargo clippy -p leaven-public-seam --tests -- -D warnings`
- `CARGO_INCREMENTAL=0 cargo test -p leaven --test topology_contract`
