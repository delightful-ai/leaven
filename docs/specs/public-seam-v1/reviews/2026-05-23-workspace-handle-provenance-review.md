# Workspace Handle Provenance Partial Review

Date: 2026-05-23

Reviewer: Kierkegaard (`019e578f-c4a9-78d0-a143-2a75e9d5f84c`)

Range reviewed:

- Base: `wquspuln` / `58d3463c88f2` (`public-seam: record workspace query review signoff`)
- Head: `vyzuurvr` / `8531ad939688` (`public-seam: require materialized workspace provenance`)

Verdict: sign off for partial evidence only.

## Scope Reviewed

The reviewer inspected the public-seam live-workspace tranche covering:

- `workspace_materialize` host-returned workspace id and lifetime auditing.
- Internal `ExecutionState` provenance for materialized workspace handles.
- `workspace_release`, `workspace_query`, `agent_run`, and `sandbox_exec`
  refusal of literal forged workspace handles.
- Released-handle reuse denial after `workspace_release`.
- Matrix and `AGENTS.md` maturity language.

## Resolved Findings

- Literal `workspace_handle` forgery is resolved for the live representative
  harness. Only handles recorded from `workspace_materialize` in private
  execution state can satisfy `require_live_workspace`.
- Release lifetime substitution is resolved for live execution. The host return
  lifetime must match the live handle lifetime before a release handle is bound.
- No topology drift was found. The provenance state remains private to
  `leaven-public-seam` and does not move backend/provider details into cold
  crates.

## Remaining Non-Blocking Gaps

These gaps block row promotion but do not block retaining the tranche as partial
evidence while rows stay pending:

- `validate_plan_execution_result` validates wire preimages and receipt hashes,
  not live materialization provenance. It must not be cited as provenance proof.
- `WorkspaceRef` object refs still collapse to `id`; `run` and
  `snapshot_fingerprint` identity semantics remain unproven.
- `agent_run.workspace` is schema-optional while the representative harness
  requires it for live execution.
- Full agent/sandbox fake-pass closeout remains pending: these tests do not
  prove proposal parsing from agent sessions or real sandbox backend execution
  and streaming policy enforcement.

## Evidence

- `crates/leaven-public-seam/tests/plan_document.rs::workspace_handle_provenance_rejects_literal_forgery_and_released_reuse`
- `crates/leaven-public-seam/tests/plan_document.rs::workspace_release_rejects_unmaterialized_handles_and_host_path_substitutes`
- `crates/leaven-public-seam/src/plan_execution/effects.rs::LiveWorkspaceHandle`
- `crates/leaven-public-seam/src/plan_execution/effects.rs::require_live_workspace`
- `crates/leaven-public-seam/src/plan_execution.rs::ExecutionState`
