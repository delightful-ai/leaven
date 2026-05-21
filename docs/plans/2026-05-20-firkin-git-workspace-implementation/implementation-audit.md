# Firkin/Git Workspace Implementation Audit

Date: 2026-05-21

Status: current-state audit after recovering the missing Firkin/Git spec bundle
from jj history.

## Scope

This audit compares the active implementation against:

- `docs/plans/2026-05-20-firkin-git-workspace-implementation/goal-handoff.yaml`
- `docs/specs/firkin_git_workspace_backend.md`
- `docs/specs/firkin_git_workspace_api_shape.md`
- current code on the `evoskill-replication` stack

The restored specs are detailed reference material and a useful handoff
denominator. They are not automatically normative over later decisions. Where
current code or conversation made a better call, keep the better call and note
the reconciliation explicitly.

## Executive Result

The current implementation is not a throwaway, but it is not complete enough to
claim the full goal.

Keep:

- `GitProgramArtifact`, `GitRepoArtifact`, `GitProgramLayout`, and
  `GitProgramChange` as the product Git artifact direction.
- The local Git projection/import tests as the start of the Git visibility law.
- The `leaven-agentic-git` materializer/readback tests as useful local proof.
- `leaven-workspace-firkin` as the concrete backend crate and optional facade
  route.

Overhaul before completion:

- Git readback cannot depend on `WorkspaceView::local_mount()` if the same path
  must run inside Firkin, because the Firkin backend intentionally has no local
  mount.
- Git projection/import needs a stronger law surface than the current
  `leaven-workspace-git` helper functions, including explicit no-local/hardlink
  projection behavior and scratch/trusted ref separation.
- The Firkin adapter proves product-pod container calls against a fake
  `RuntimeAdapter`, but not a live or signed one-product-pod/two-workspace
  Git e2e.

## Acceptance Audit

| Handoff item | Current status | Evidence | Gap / decision |
| --- | --- | --- | --- |
| `git_program_artifact_laws` | Mostly satisfied, still thin | `crates/leaven-artifact-git/src/program.rs` defines private-field `RepoKey`, `RepoRef`, `GitRevision`, `GitRepoArtifact`, `GitProgramLayout`, `GitProgramArtifact`, `GitRepoChange`, and `GitProgramChange`; `GitProgramArtifact` implements `Artifact`; tests cover single and multi repo identity, layout validation, and one/many repo advance. | Keep the shape. Gaps relative to the restored specs: no diff summaries on changes, no patch/ref change variants yet, no explicit test named around excluding workspace/pod/candidate IDs from identity beyond the type shape. |
| `git_projection_import_laws` | Partial | `crates/leaven-workspace-git/src/projection.rs` creates a fresh bare projection and fetches only allowed refs; `tests/git_projection.rs` proves allowed refs exist, hidden refs fail, hidden-only commit lookup fails, no alternates file exists, projection fsck passes, import fscks source before durable write, and durable store contains imported commit after validation. | The projection fetch uses a raw local source path, not an explicit `file://` URL or documented no-local/no-hardlink mode. There is no scratch/trusted ref authority test. This likely wants a named Git ops layer or stricter module before relying on it for product isolation. |
| `workspace_materializer_readback` | Partial, with one blocking architecture gap | `crates/leaven-agentic-git/src/lib.rs` implements `GitProgramMaterializer` over `WorkspaceView`, and tests prove multi-repo checkout, no-op readback, committed child import, dirty worktree freezing, output bundle import, output patch import, and atomic multi-repo readback. | Readback/import uses `WorkspaceView::local_mount()` to find checkout and output paths. That works for `leaven-workspace-local` but does not work for Firkin, whose backend returns no local mount. This is the largest mismatch with the goal. |
| `no_duplicate_public_apis` | Mostly satisfied | The implementation reuses `Artifact`, `Workspace`, `WorkspaceView`, `WorkspaceFactoryContext`, and `leaven_engine::Materializer`. `crates/leaven/src/lib.rs` exposes Firkin only as `leaven::workspace_firkin` behind `workspace-firkin`; it is not in the prelude. No public `WorkspaceLease`, `CodebaseArtifact`, `CodebaseChange`, `ContainerWorkspaceBackend`, or third generic `MaterializationReport` was introduced. | `FirkinWorkspaceRuntime` is a public adapter trait and should be treated as explicit scaffold/law surface. If it stays public, document its maturity and why the crate can condemn wrong implementations. |
| `firkin_workspace_backend` | Partial but useful | `leaven-workspace-firkin` implements `WorkspaceFactory`/`WorkspaceBackend`; fake-runtime tests cover context, file read/write/list, command routing, output limits, executable-bit refusal, and cleanup. The optional `firkin-facade` adapter wraps Firkin `RuntimeAdapter` by adding anchor/helper pod containers and has feature-gated tests for command, file helpers, cleanup, and unsupported stdin/user overrides. | It attaches to an existing product pod; it does not own start/stop pod lifecycle. It has no live Apple/VZ proof. Context lacks some planned fields such as workspace id/capabilities/run id, though the current slimmer context may be the better choice. |
| `firkin_git_e2e` | Missing | No current evidence creates one product pod, allocates two workspaces, materializes projected Git state, performs mutation/readback/import, proves allowed archive sharing only, and cleans up. | This cannot honestly pass until readback no longer requires local mounts or until the Firkin backend provides a real controlled file/sync/import path. |
| `repo_standards_hold` | Partial | The restored specs now exist with recovery notes; topology includes `leaven-workspace-firkin`; the active graph is separated from unrelated parquet work. | Need rerun full required gates after the next implementation pass: `cargo test -p leaven-artifact-git`, projection/import tests, materializer/readback tests, `cargo test -p leaven-workspace --test workspace_view`, Firkin backend tests with feature gate, topology contract, and final `just check`. |

## Detailed Findings

### 1. The Firkin adapter is worth keeping, but it is not the Git e2e backend yet

The `leaven-workspace-firkin` crate is well placed: it depends on
`leaven-workspace` and Firkin e2b/type crates behind an optional feature, and it
does not depend on Git artifact crates or optimizer crates.

The adapter currently proves this useful contract:

- Leaven workspace allocation maps to one product-pod anchor container.
- Command/file/list operations run through captured helper containers sharing
  the workspace volume.
- Cleanup removes the workspace container.
- Unsupported stdin/user override behavior is typed and tested.

That is a good backend slice. It should not be discarded.

It does not prove the full handoff because:

- Product pod creation/lifecycle is not owned here.
- There is no live/signed Firkin proof.
- There is no composition with Git projection/materialization/readback.

### 2. The current Git readback shape is local-only at the import boundary

`GitProgramMaterializer` runs Git commands through `WorkspaceView`, which is
the right direction. But `GitProgramReadback` then crosses out of the
workspace abstraction:

- `checkout_host_path()` requires `workspace.local_mount()`.
- `output_proposal_path()` requires `workspace.local_mount()`.
- committed child import reads from a host-visible checkout path.
- proposal bundle/patch discovery reads host paths under `output/`.

This is exactly the seam the restored specs warned about: local clone behavior
cannot be treated as the product path. The local proof is still valuable, but
the product implementation needs one of these hard-cut shapes:

- make Git readback use only `WorkspaceView` plus explicit output bundle/patch
  bytes, then import through a backend-neutral transfer path;
- add a deliberate workspace export/readback capability to `leaven-workspace`;
- or split local readback and product readback into clearly named adapters,
  with the Firkin path using helper containers or a controlled transfer service.

Do not claim Firkin compatibility until this is resolved.

### 3. Projection/import tests prove the right idea but not the full visibility law

The current projection tests are good as a first law:

- allowed refs exist;
- hidden refs are absent;
- hidden-only commits are absent;
- alternates are absent;
- fsck runs before import;
- durable store contains imported commits only after validation.

Missing pieces:

- explicit no-hardlink/no-local projection proof;
- explicit use of `file://` or another documented safe transport for local
  sources;
- scratch ref input versus trusted ref output tests;
- archive visibility policy tests such as `None` and `FrontierOnly`;
- no proposer access to hidden evaluator data/reward caches.

This likely becomes the next implementation slice before more Firkin work.

### 4. The restored specs and current code disagree on a few details, and that is okay

Examples:

- The specs sketch a richer `FirkinWorkspaceContext` with workspace id,
  capabilities, and run id. Current code keeps pod/container/root/image. That
  may be better until a caller needs the extra fields.
- The specs call the product Git value `GitProgramArtifact`. The user has
  correctly challenged whether "program" is the right long-term name. Current
  code keeps the name; do not broaden the rename until the behavior is proven.
- The specs contemplate `leaven-git-ops`; current code starts with focused
  functionality in `leaven-workspace-git` and `leaven-agentic-git`. That is
  acceptable only while the law surface stays small. Projection/import is now
  large enough that a named Git ops boundary is becoming justified.

## Recommendation

Do not do a total rewrite of everything.

Do a targeted overhaul in this order:

1. Keep the restored docs and current artifact/Firkin slices.
2. Add a first-class audit/visibility regression slice for Git projection and
   import: no-local/hardlink proof, scratch/trusted refs, and explicit
   visibility policy.
3. Replace or split `GitProgramReadback` so product readback does not require
   `local_mount()`.
4. Compose the backend-neutral Git path with `leaven-workspace-firkin` in a
   fake-runtime product-pod e2e.
5. Only then attempt the bounded live/signed Firkin product-pod proof.

That gives us the benefit of the work already landed without pretending the
current implementation satisfies the full restored handoff.
