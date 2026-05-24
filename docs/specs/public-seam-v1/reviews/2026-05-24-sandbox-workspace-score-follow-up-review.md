# Sandbox, WorkspaceRef, and Blob Score Follow-up Review

Scope: partial evidence tranche from `woslnyrl` through `tpmruxsu`, covering:

- `vxrllnlv`: sandbox command output projection into public-seam results.
- `mtpuxkyr`: WorkspaceRef object lifecycle facts.
- `otxuptwp`: audited blob-backed score output projection.
- `smoptuvz`: sandbox stream byte/SHA binding and exact object-ref release.
- `tpmruxsu`: rejection of bare-id release for object-ref handles.

Reviewers:

- Euler (`019e59e8-346a-7aa2-927d-1a0527b98945`)
- Laplace (`019e59f2-ee8f-7393-95dc-48c4a413a598`)

## Findings And Resolution

Euler found that `PlanSandboxExecOutcome::from_command_output` attached
caller-supplied stdout/stderr blob refs without proving they described the
captured `CommandOutput` bytes. `smoptuvz` resolved this for stream refs:
`validate_stream_blob_ref` compares declared byte length and SHA-256 to the
captured stdout/stderr bytes, and
`sandbox_exec_command_output_projection_rejects_unbound_stream_blob_refs`
rejects the fake pass.

Euler found that workspace object-ref lifecycle proof was narrower than the
docs because release state was keyed by workspace id. `smoptuvz` changed
release recording and replay to mark handles satisfying the requested
WorkspaceRef facts, and added
`workspace_lifecycle_does_not_collapse_same_id_distinct_object_refs`.

Laplace found one remaining object-ref collapse path: schema-valid bare string
WorkspaceRef requests still wildcarded object handles with `run` or
`snapshot_fingerprint`. `tpmruxsu` resolved this by requiring exact
id/run/snapshot equality in `WorkspaceRefFacts::satisfies_request`, and added
`workspace_lifecycle_rejects_bare_id_release_of_object_ref_handle`.

Laplace also noted that sandbox file artifact refs remain attachable without
content binding. That finding is not resolved in this tranche and keeps
`ps1.sandbox.exec_streaming` partial.

## Verdict

`ps1.workspace.handles_lifecycle`: partial evidence sign-off for object-ref
preservation, request-side enforcement, same-id distinct object refs, and
bare-id non-degradation. No row promotion.

`ps1.sandbox.exec_streaming`: partial evidence sign-off for stdout/stderr
`CommandOutput` stream binding only. File artifact content binding remains
open. No row promotion.

`ps1.evaluator.score_output`: partial evidence sign-off for audited blob-backed
score output projection from stored evidence through the `leaven-run` public
seam lowering. No row promotion.

No ACP, data-visibility, agent, capability, public-route maturity, or watch row
is promoted by this review. MCP-over-ACP remains outside V1 and watch runtime
remains deferred.

## Verification Evidence

The implementation was locally checked with:

- `cargo fmt --check`
- `cargo test -p leaven-public-seam --test sandbox_contract -- --nocapture`
- `cargo test -p leaven-public-seam --test workspace_ref_contract -- --nocapture`
- `cargo test -p leaven-public-seam --test plan_document -- --nocapture`
- `cargo test -p leaven-public-seam --test plan_document workspace -- --nocapture`
- `cargo test -p leaven-public-seam --test output_record -- --nocapture`
- `cargo test -p leaven-run --test public_seam -- --nocapture`
- `cargo test -p leaven-evidence --test command -- --nocapture`
- `cargo test -p leaven-public-seam --test contract_package conformance_matrix_rows_are_unique_honest_and_reference_real_files -- --exact --nocapture`
- `cargo clippy -p leaven-evidence -p leaven-public-seam -p leaven-run --tests -- -D warnings`
- `cargo clippy -p leaven-public-seam --tests -- -D warnings`
- `cargo test -p leaven --test topology_contract -- --nocapture`

`just check` was also attempted and currently fails before the full suite on
production line-count lint for existing large public-seam files:
`acp_profile.rs`, `plan_execution.rs`, `plan_execution/effects.rs`,
`package.rs`, `plan_execution/receipts.rs`, and `result.rs`.
