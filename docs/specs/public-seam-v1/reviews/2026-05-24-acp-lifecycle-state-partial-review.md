# ACP Lifecycle State Partial Review

Date: 2026-05-24T05:05:22Z
Reviewer: Wegener (`019e585d-5d91-7c42-a832-0943b862d6fa`)
Scope: `ps1.acp.lifecycle_backpressure` partial evidence for public-seam
lifecycle state.

## Reviewed Claim

`AcpSessionLifecycle` carries an explicit `AcpSessionState` and transitions from
`Running` to `Cancelled` through ACP lifecycle cancellation. Duplicate
cancellation preserves the first cancellation reason, and progress updates are
refused after cancellation.

The row remains pending. This is contract-layer partial evidence only.

## Findings

Initial review found no Critical or Important findings. One Minor wording issue
noted that `AcpSessionState::Running` claimed the worker session accepted
extension work, while the primitive only enforces progress-update acceptance.

Resolution: narrowed the `Running` variant documentation to progress updates.
Follow-up review confirmed the finding was resolved.

## Verdict

No blockers. This is allowed seam-level lifecycle state evidence, not ACP
transport/runtime behavior. It may be recorded as partial evidence with
`ps1.acp.lifecycle_backpressure` still pending. It does not justify promoting
the row.

## Verification

Main-agent verification for this tranche:

- `cargo fmt --check`
- `cargo test -p leaven-public-seam --test acp_profile acp_worker_session_uses_engine_client_worker_agent_inversion_and_bounded_updates -- --nocapture`
- `cargo clippy -p leaven-public-seam --test acp_profile -- -D warnings`
- `cargo test -p leaven-public-seam --test contract_package -- --nocapture`
- `cargo test -p leaven --test topology_contract`

