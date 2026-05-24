# ACP Backpressure Strategy Review

Date: 2026-05-24

Reviewer: Leibniz (`019e58f1-0f92-70f3-b7ae-89d288f2c994`)

Scope:
- `ps1.acp.transport_profile`
- `ps1.acp.lifecycle_backpressure`
- `ps1.public_routes.maturity_classified`

Reviewed claims:
- `AcpProfileDocument` parses the locked `flow_control.backpressure` strategy.
- `AcpWorkerSession` builds lifecycle state from a validated profile instead of an ad hoc public bounded constructor.
- `AcpSessionLifecycle` applies `pause_worker`, `drop_noncritical_updates`, and `disconnect` queue-overflow behavior at the contract layer.
- New public exports are advanced public seam contracts, not ordinary facade/prelude routes.
- ACP rows remain pending because this is contract-layer lifecycle evidence, not ACP process I/O, worker execution stop, or receipt/error production.

Initial finding:
- Important: `AcpSessionLifecycle::enqueue_progress` discarded non-enqueued `AcpProgressDisposition` values. Under `disconnect`, it cancelled the lifecycle but then returned the stale previously queued update as success.

Resolution:
- `enqueue_progress` now returns `Ok(&AcpSessionUpdate)` only for `AcpProgressDisposition::Enqueued`.
- `Disconnected(reason)` is converted into an error after preserving the cancellation side effect.
- The regression test covers the public `enqueue_progress` path under `disconnect` and asserts the queued update remains the original one.

Follow-up sign-off:
- No Critical findings.
- No Important findings after the fix.
- Minor stricter-test suggestion was applied.
- The reviewer approved committing this tranche as partial evidence with `ps1.acp.transport_profile` and `ps1.acp.lifecycle_backpressure` still pending.

Verification:
- `cargo fmt --check`
- `cargo test -p leaven-public-seam --test acp_profile acp_lifecycle_applies_profile_backpressure_strategies -- --nocapture`
- `cargo test -p leaven-public-seam --test acp_profile -- --nocapture`
- `cargo test -p leaven-public-seam --test contract_package public_seam_routes_reject_ordinary_facade_leaks -- --nocapture`
- `cargo test -p leaven-public-seam --test contract_package conformance_matrix_rows_are_unique_honest_and_reference_real_files -- --nocapture`
- `cargo clippy -p leaven-public-seam --tests -- -D warnings`
- `cargo test -p leaven --test topology_contract`
- `cargo test -p leaven-public-seam --tests`
