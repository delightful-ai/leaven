# Firkin/Git Workspace Implementation Goalcraft

Recovery note, 2026-05-21: this draft was restored from jj history with the
Firkin/Git workspace specs. Treat it as a historical goalcraft draft and intent
record, not as automatically normative over newer code decisions, the active
goal text, or subsequent user direction.

This is the ready-to-paste `/goal` draft for implementing the Firkin/Git
workspace backend plan. It points at the specs and handoff instead of restating
the whole design.

```text
/goal Destination: Implement the Firkin-backed Git workspace design in /Users/darin/src/personal/leaven against docs/specs/firkin_git_workspace_backend.md, docs/specs/firkin_git_workspace_api_shape.md, and docs/plans/2026-05-20-firkin-git-workspace-implementation/goal-handoff.yaml. Reference Firkin at /Users/darin/vendor/github.com/apple/containerization. End state: Leaven has a real typed Git program artifact path, Git projection/import/readback over WorkspaceView, and a Firkin workspace backend proof using product pods.

Context: the codebase is the artifact and workspace/pod/container are disposable projections. Preserve the no-duplicate public API rule: reuse leaven_core::Artifact, leaven_workspace::{Workspace, WorkspaceView, WorkspaceFactoryContext}, leaven_engine::Materializer, and leaven_stage::MaterializableArtifact. Do not introduce public WorkspaceLease, CodebaseArtifact, CodebaseChange, ContainerWorkspaceBackend, or a third MaterializationReport.

Scope: implement GitProgramArtifact/GitRepoArtifact/GitProgramChange in leaven-artifact-git with native one-or-many repos, immutable commit/tree identity, cache identity law, private fields, constructors/accessors, and no workspace/pod/candidate identity leakage. Implement Git projection/import/readback laws outside leaven-artifact-git, with local bare-repo tests for allowed refs only, hidden-only commits absent, no alternates/hardlink/local clone leakage, fsck before import, scratch/trusted ref separation, and durable store import before recording artifacts. Implement stage-owned Git materialization/readback over WorkspaceView using the artifact layout, handling committed changes, dirty worktrees, output bundle/patch proposals, no-op reads, and multi-repo changes. Add leaven-workspace-firkin only after updating topology contracts; it should implement WorkspaceFactory/WorkspaceBackend with typed Firkin context, product-pod allocation, per-workspace containers, file ops, command exec, executable-bit behavior or typed refusal, cleanup, and no Git artifact dependency. Expose Firkin through an optional leaven feature/alias such as workspace-firkin, not default features or leaven::prelude.

Preserve: no docs-only or skeleton-only success; no local-only git clone treated as product path; no full bare repo mounted as visibility isolation; no trusted refs, reward cache, hidden cases, or evaluator-only data visible to proposer workspaces; no scaffold exports through leaven::prelude; no jj or pod snapshot restore as required scope.

Verify: run focused tests as each slice lands, including cargo test -p leaven-artifact-git, Git projection/import tests, Git materializer/readback tests over leaven-workspace-local, cargo test -p leaven --test topology_contract, cargo test -p leaven-workspace --test workspace_view, cargo test -p leaven-workspace-firkin, a bounded live/signed Firkin product-pod proof if local Firkin is available, and final just check. If live Firkin is unavailable, complete all local/contract proof and mark live proof blocked, not proven.

Done/stop: done only when every handoff acceptance item is proven or explicitly blocked with evidence and final closeout compares implementation against the handoff proxy list. Stop before destructive Git/Firkin operations, broad public API promotion, or credential/runtime-dependent live claims that cannot be verified locally.
```
