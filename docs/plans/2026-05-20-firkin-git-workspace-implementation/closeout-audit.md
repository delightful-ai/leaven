# Firkin Git Workspace Implementation Closeout

Status: implementation closeout audit
Date: 2026-05-21

This audit compares the active implementation against
`goal-handoff.yaml`, `docs/specs/firkin_git_workspace_backend.md`, and
`docs/specs/firkin_git_workspace_api_shape.md`.

Those restored specs remain design references and handoff denominator. They are
not treated as normative over the active goal text or the implementation
decisions made after recovery. The important closeout question is whether the
current Leaven codebase proves the intended artifact/materialization/workspace
behavior without introducing duplicate public API surfaces.

## Summary

The implementation has the intended local and contract shape:

- `leaven-artifact-git` owns a typed `GitProgramArtifact`/`GitRepoArtifact`
  product path with one-or-many repos, immutable Git commit/tree identity, typed
  changes, and no workspace/pod identity in artifact identity.
- `leaven-workspace-git` owns projection/import mechanics outside artifact
  identity code, including allowed-ref projection, hidden-object exclusion, no
  alternates, fsck-before-import, and scratch/trusted ref separation tests.
- `leaven-agentic-git` materializes and reads back Git program artifacts over
  `WorkspaceView`, including no-local-mount backends, dirty worktrees, committed
  changes, output bundles, output patches, no-op reads, and atomic multi-repo
  readback.
- `leaven-workspace-firkin` implements Leaven's existing `WorkspaceFactory` and
  `WorkspaceBackend` contracts with typed Firkin context and an optional
  `leaven::workspace_firkin` facade route.
- A Firkin product-pod e2e contract test proves two isolated no-local-mount
  workspaces in one product pod can run the Git materializer/readback/import
  path through the Firkin backend abstraction.

The implementation does not yet prove the signed/live Apple/VZ product-pod run
from Leaven. Firkin has product-pod APIs and a generic signed live runtime
script, but this repository does not yet contain an exact signed Leaven harness
that creates a live product pod and runs the Git e2e against it. That item is
therefore blocked/unproven, not claimed as complete live proof.

## Acceptance Items

### `git_program_artifact_laws`

Status: satisfied.

Requirement: `leaven-artifact-git` exposes validated Git program/repo artifact
and change shapes with native one-or-many repos, immutable commit/tree
identity, cache identity law, private fields, constructors/accessors, and no
workspace/pod/candidate identity leakage.

Implementation evidence:

- `crates/leaven-artifact-git/src/program.rs`
- `crates/leaven-artifact-git/src/change.rs`
- `crates/leaven-artifact-git/src/reference.rs`
- `crates/leaven-artifact-git/tests/git_program_artifact.rs`

Behavioral proof:

- `git_program_artifact_supports_single_and_multi_repo_identity`
- `git_program_change_advances_one_repo_without_touching_others`
- `git_program_change_advances_multiple_repos_atomically`
- `git_program_artifact_rejects_invalid_repo_keys_and_layout`

Notes:

- The implementation keeps the existing snapshot-like `GitArtifact` and adds
  `GitProgramArtifact` as the product commit/tree repo artifact path, matching
  the handoff's Option A.
- Multi-repo behavior is native to `GitProgramArtifact`; it is not modeled as a
  separate codebase wrapper or separate artifact family.

### `git_projection_import_laws`

Status: satisfied for local Git projection/import law.

Requirement: projection/import tests prove allowed refs only, hidden-only
commits absent, no alternates, no hardlink/local clone leakage, `git fsck`
before import, scratch/trusted ref separation, and durable store import before
artifact recording.

Implementation evidence:

- `crates/leaven-workspace-git/src/projection.rs`
- `crates/leaven-workspace-git/src/import.rs`
- `crates/leaven-workspace-git/tests/git_projection.rs`

Behavioral proof:

- `git_projection_contains_allowed_refs_without_hidden_objects_or_alternates`
- `git_commit_import_fscks_source_before_writing_durable_store`
- `git_commit_import_writes_child_revision_to_durable_store_after_validation`
- `git_commit_import_does_not_promote_source_scratch_or_trusted_refs`

Notes:

- Projection is the visibility boundary. The tests assert hidden refs are not
  present, hidden-only commits are not readable, and `objects/info/alternates`
  is absent from the projection.
- Proposal import writes the selected child into the durable store before the
  artifact-level readback returns a `GitProgramChange`.

### `workspace_materializer_readback`

Status: satisfied.

Requirement: Git materialization/readback runs over `WorkspaceView`, checks out
all repos from the artifact layout, handles committed changes, dirty worktrees,
output bundles, output patches, no-op reads, and returns typed
`GitProgramChange` values that apply through the artifact change path.

Implementation evidence:

- `crates/leaven-agentic-git/src/lib.rs`
- `crates/leaven-agentic-git/tests/git_program_materializer.rs`

Behavioral proof:

- `materializer_checks_out_multiple_repos_at_artifact_revisions`
- `readback_reports_no_change_for_clean_materialized_program`
- `readback_imports_committed_workspace_child_before_returning_change`
- `readback_imports_output_bundle_proposal_before_checkout_state`
- `readback_imports_output_patch_proposal_as_child_commit`
- `readback_imports_output_patch_without_local_mount`
- `readback_freezes_dirty_worktree_as_imported_child_commit`
- `readback_freezes_dirty_worktree_without_local_mount`
- `readback_returns_atomic_multi_repo_change_when_multiple_repos_move`

Notes:

- The no-local-mount tests are the important backend-seam proof. Readback no
  longer assumes a host path and instead uses the `WorkspaceView` file/command
  surface.
- Agents are not required to commit. Readback can preserve committed child
  state, import output proposals, or freeze dirty worktree state.

### `no_duplicate_public_apis`

Status: satisfied.

Requirement: reuse `Artifact`, `Workspace`, `WorkspaceView`,
`WorkspaceFactoryContext`, `leaven_engine::Materializer`, and
`leaven_stage::MaterializableArtifact`. Do not add public duplicate law
surfaces such as `WorkspaceLease`, `CodebaseArtifact`, `CodebaseChange`,
`ContainerWorkspaceBackend`, or another materialization report. Keep Firkin out
of default features and out of `leaven::prelude`.

Implementation evidence:

- `crates/leaven/src/lib.rs`
- `crates/leaven/src/prelude.rs`
- `crates/leaven/Cargo.toml`
- `crates/leaven/tests/public_surface_contract.rs`
- `crates/leaven/tests/topology_contract.rs`

Behavioral proof:

- `workspace-firkin = ["workspace", "dep:leaven-workspace-firkin"]`
- `leaven::workspace_firkin` is namespaced behind `#[cfg(feature =
  "workspace-firkin")]`
- `prelude.rs` does not export Firkin, Git workspace, or agentic Git scaffold.
- `public_surface_contract` enforces ordinary/import-route hygiene.

Notes:

- `leaven-workspace-firkin` reuses `WorkspaceFactory`,
  `WorkspaceBackend`, and `WorkspaceFactoryContext`; the typed
  `FirkinWorkspaceContext` is backend placement metadata, not artifact truth.

### `firkin_workspace_backend`

Status: satisfied for Leaven contract tests and Firkin facade API matching.

Requirement: `leaven-workspace-firkin` implements
`WorkspaceFactory`/`WorkspaceBackend` with product-pod allocation,
per-workspace containers, typed Firkin context, file read/write/list, command
execution, executable-bit behavior or typed refusal, cleanup, and no Git
artifact dependency. Its assumptions must match the Firkin checkout.

Implementation evidence:

- `crates/leaven-workspace-firkin/src/factory.rs`
- `crates/leaven-workspace-firkin/src/runtime.rs`
- `crates/leaven-workspace-firkin/src/placement.rs`
- `crates/leaven-workspace-firkin/src/adapter.rs`
- `crates/leaven-workspace-firkin/tests/firkin_workspace.rs`
- `crates/leaven-workspace-firkin/tests/firkin_runtime_adapter.rs`

Firkin checkout evidence:

- `/Users/darin/vendor/github.com/apple/containerization/crates/e2b-contract/src/runtime.rs`
  exposes product-pod adapter methods: `start_pod`, `stop_pod`,
  `add_pod_container`, `remove_pod_container`, and `wait_pod_container`.
- `/Users/darin/vendor/github.com/apple/containerization/crates/single-node/src/apple_vz.rs`
  implements product-pod add/remove/wait methods for the Apple/VZ single-node
  adapter.
- `/Users/darin/vendor/github.com/apple/containerization/scripts/run-signed-live-runtime-test.sh`
  signs and runs ignored Firkin live runtime tests, but it is generic Firkin
  runtime machinery rather than a Leaven-specific Git workspace harness.

Behavioral proof:

- `factory_allocates_container_in_product_pod_and_attaches_context`
- `backend_routes_file_and_command_operations_to_workspace_root`
- `backend_preserves_command_output_byte_limits`
- `executable_bit_operations_are_explicitly_unsupported`
- `guest_paths_reject_relative_and_parent_traversal_values`
- `runtime_adapter_uses_product_pod_containers_for_workspace_commands`
- `runtime_adapter_uses_product_pod_helpers_for_workspace_file_operations`
- `runtime_adapter_rejects_command_options_without_product_pod_support`

Notes:

- Executable bit operations are explicit typed refusals. This is acceptable for
  the current Firkin adapter surface and prevents silent partial support.
- The facade adapter rejects stdin and user overrides because Firkin product-pod
  container requests do not currently support those command options.
- `leaven-workspace-firkin` does not depend on `leaven-artifact-git`.

### `firkin_git_e2e`

Status: contract e2e satisfied; signed/live Apple/VZ proof blocked/unproven.

Requirement: a live or signed Firkin proof creates one product pod, allocates
two isolated workspaces, materializes projected Git state, runs at least one
mutation/readback/import, proves allowed sharing only, and cleans up
containers/trusted refs.

Implementation evidence:

- `crates/leaven-workspace-firkin/tests/firkin_git_e2e.rs`

Behavioral proof:

- `firkin_product_pod_materializes_and_reads_back_isolated_git_workspaces`

What the e2e proves:

- A single `FirkinProductPodId` hosts two workspace allocations.
- Each allocation gets a distinct container id and workspace root.
- Both workspaces expose `local_mount() == None`.
- The Git program materializer writes the same parent artifact into both
  workspace views.
- Mutating workspace A and running readback imports a child commit into the
  durable store.
- Workspace B still sees the parent content, proving workspace isolation across
  the same product pod.
- Cleanup removes both workspace containers.

Live proof blocker:

- Leaven does not yet include an ignored/signed live test that wires a live
  Apple/VZ `RuntimeAdapter` into `FirkinRuntimeAdapterRuntime`, starts a real
  product pod, and runs this Git e2e path.
- Firkin provides the lower-level product-pod APIs and a generic signed live
  test runner, so this is a missing Leaven harness rather than a missing
  conceptual substrate.

### `repo_standards_hold`

Status: satisfied.

Focused verification already run successfully during this stack:

```bash
cargo fmt --check
python3 scripts/lint-line-count.py
git diff --check
cargo check -p leaven-population -p leaven-gepa
cargo test -p leaven-population -- --nocapture
cargo test -p leaven-gepa --profile coverage -- --nocapture
cargo clippy -p leaven-population -p leaven-gepa --all-targets -- -D warnings
cargo test -p leaven-artifact-git -- --nocapture
cargo test -p leaven-workspace-git -- --nocapture
cargo test -p leaven-agentic-git -- --nocapture
cargo test -p leaven-workspace-firkin -- --nocapture
cargo test -p leaven-workspace --test workspace_view -- --nocapture
cargo test -p leaven --test topology_contract -- --nocapture
cargo check -p leaven --features workspace-firkin
CARGO_TARGET_DIR=/tmp/leaven-firkin-target CARGO_BUILD_JOBS=1 cargo test -p leaven-workspace-firkin --features firkin-facade --test firkin_runtime_adapter -- --nocapture
CARGO_TARGET_DIR=/tmp/leaven-firkin-target CARGO_BUILD_JOBS=1 cargo clippy -p leaven-workspace-firkin --features firkin-facade --all-targets -- -D warnings
just check
```

Final `just check` evidence:

- `cargo fmt --check` passed.
- `python3 scripts/lint-line-count.py` passed with existing oversized-module
  warnings only.
- Workspace clippy passed with milestone/example exclusions.
- `python3 scripts/test-suite-sla.py --sla-seconds 30` passed: 813 tests, 0
  failed, 1 ignored doctest, 21.28s under the 30s SLA.
- `python3 scripts/coverage-gate.py` passed on the default coverage lane:
  89.02% line coverage over a floor of 89.01%, and 87.32% branch coverage over
  a floor of 87.31%.

Known verification caveat:

- `cargo test -p leaven-gepa --test agent_stage_routing` in the default
  dev profile reproducibly hits a nightly Cranelift rustc internal compiler
  error on this machine:
  `unexpected DefKind in AliasTy: LifetimeParam`.
- The same GEPA tests passed under the repo `coverage` profile, which uses the
  LLVM backend. The current evidence points to a compiler/backend ICE rather
  than a Leaven behavior failure.

## Residual Work

The remaining implementation-grade follow-up is an ignored signed live Leaven
test harness:

1. Build or obtain a prepared/live Firkin product-pod runtime image suitable
   for running `git` and the Leaven workspace helper commands.
2. Start a real Apple/VZ Firkin product pod.
3. Construct `FirkinRuntimeAdapterRuntime` from the live runtime adapter.
4. Reuse the existing Git fixture/materializer/readback e2e against the live
   workspace factory.
5. Run it through Firkin's signed live test runner and record the command in
   this plan directory.

Until that harness exists and passes, Leaven has contract/e2e proof for the
backend shape, not live Apple/VZ proof.
