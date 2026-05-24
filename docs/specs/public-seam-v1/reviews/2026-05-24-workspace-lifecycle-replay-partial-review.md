# Workspace Lifecycle Replay Partial Review

Date: 2026-05-24
Reviewer: Halley (`019e58ca-e2d1-7ba0-b499-1346b56607e2`)
Scope: `ps1.workspace.handles_lifecycle` partial evidence only.

## Reviewed Change

- `workspace_materialize` replay validation now requires a `workspace_handle`
  result to carry `released: false` and the requested lifecycle lifetime before
  the handle is bound live.
- `workspace_release` replay validation now requires a `workspace_handle`
  result to carry `released: true` for the same live workspace/lifetime before
  downstream dependencies can observe the release.
- The negative test mutates valid execution results, rebinds call result hashes,
  and rejects schema-valid lifecycle forgeries.

## Initial Finding

Important: the first negative test set covered wrong materialize lifetime,
missing materialize `released`, release `released: false`, and missing release
`released`, but it did not cover the schema-valid materialize forgery where a
newly materialized handle is replayed with `released: true` and a valid rebound
result hash.

Resolution: added an explicit `released: true` materialize-result forgery case
to `plan_execution_result_rejects_workspace_lifecycle_state_forgery_with_valid_hashes`.

## Sign-Off

Critical: none.
Important: none after the follow-up fix.
Minor: none.

The reviewer signed off this tranche as partial evidence only for
`ps1.workspace.handles_lifecycle`.

Semantic basis:

- The implementation remains public-seam replay validation, not workspace
  backend/runtime behavior.
- Locked schema semantics were not changed; `released` remains schema-optional,
  while replay validation requires it for successful workspace lifecycle call
  results.
- The conformance matrix remains pending and records only partial evidence.
- The public maturity note stays scoped to seam lifecycle proof and does not
  claim full backend/artifact/snapshot closeout.

## Verification Evidence

- `cargo fmt --check`
- `cargo test -p leaven-public-seam --test plan_document plan_execution_result_rejects_workspace_lifecycle_state_forgery_with_valid_hashes -- --nocapture`
- `cargo test -p leaven-public-seam --test plan_document workspace_ -- --nocapture`
- `cargo test -p leaven-public-seam --test contract_package -- --nocapture`
- `cargo clippy -p leaven-public-seam --tests -- -D warnings`
- `cargo test -p leaven --test topology_contract`
