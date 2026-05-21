# Firkin Git Workspace API Shape

Status: API shape companion / pre-implementation

Date: 2026-05-20

Recovery note, 2026-05-21: this file was restored from jj history after it was
missing from the active checkout. Treat it as a detailed API-shape reference and
handoff denominator, not as automatically normative over newer code decisions,
the active goal text, or subsequent user direction. Reconcile differences
explicitly before claiming completion.

This is the public-API companion to
`docs/specs/firkin_git_workspace_backend.md`. The backend spec explains the
runtime model. This document pins the Rust API shape that should implement it
without duplicating existing Leaven public contracts.

The central rule:

```text
reuse the existing artifact, materializer, workspace, and facade surfaces
do not create parallel names for concepts Leaven already owns
```

## 1. Live API Facts

The current code already owns these concepts:

```text
leaven-core::Artifact
  whole domain value optimized by a run

leaven-core::Proposal / ProposalEffect
  graph mutation record; Create for fresh artifacts, Change for typed changes

leaven-engine::Materializer<P, T>
  stage-owned materializer over any value T into WorkspaceView

leaven-stage::MaterializableArtifact
  artifact-owned convenience materialization/readback trait

leaven-workspace::Workspace
  owning workspace lease and cleanup handle

leaven-workspace::WorkspaceView
  scoped file/command API over a workspace

leaven-workspace::WorkspaceFactory
  allocator of Workspace values

leaven-workspace::WorkspaceBackend
  backend implementation trait hidden behind Workspace

leaven-workspace::WorkspaceFactoryContext
  typed backend context attachment point

leaven-artifact-git
  Git artifact vocabulary: paths, refs, object IDs, lineage, changes, diffs

leaven-workspace-git
  concrete local Git checkout workspace backend
```

The Firkin/Git implementation must compose these surfaces instead of replacing
them.

## 2. Duplicate API Ban

Do not add these duplicate concepts:

```text
WorkspaceLease
  Use leaven_workspace::Workspace as the owning lease. If placement metadata is
  needed, attach a typed context through WorkspaceFactoryContext.

WorkspaceBackendRef
  Do not add a generic backend reference unless the live Workspace API needs it.
  Backend identity belongs in typed factory context for now.

CodebaseArtifact
  For Git, the artifact is already the codebase. Make one-or-many repos native
  to GitProgramArtifact instead of creating a generic codebase wrapper.

CodebaseChange
  Use GitProgramChange. It may contain one repo change or a BTreeMap of repo
  changes.

CodebaseLayout
  Use GitProgramLayout because the layout is specifically the Git artifact's
  workspace projection.

MaterializedCodebase
  Use GitMaterialization or GitMaterializedProgram if an owned record is needed.
  It is a materialization record, not a new artifact family.

ContainerWorkspaceBackend
  Concrete backend structs should be named by backend, for example
  FirkinWorkspaceBackend. Keep generic container traits private until a second
  backend needs the same public law.

RunResourceIsland
  Firkin run/pod placement belongs in leaven-workspace-firkin context types.
  Do not generalize it before K8s or another backend forces a shared API.
```

This is not a ban on helper structs. It is a ban on public duplicate law
surfaces.

## 3. Product Git Artifact API

`leaven-artifact-git` should own the product Git artifact shape because it owns
Git identity law and already exposes Git paths, refs, object IDs, lineage,
changes, and diffs.

Do not put the Git program artifact in `leaven-artifacts`. That crate currently
exposes broad standard placeholder names such as `DirArtifact`, `TextArtifact`,
and `PartMapArtifact`; it does not own Git repository identity law.

Target public surface:

```rust
pub struct GitProgramArtifact {
    repos: BTreeMap<RepoKey, GitRepoArtifact>,
    layout: GitProgramLayout,
}

pub struct GitRepoArtifact {
    repo: RepoRef,
    revision: GitRevision,
    subpath: Option<GitPath>,
    identity_mode: GitArtifactIdentityMode,
}

pub struct GitProgramLayout {
    entries: BTreeMap<RepoKey, WorkspacePath>,
}

pub enum GitProgramChange {
    AdvanceRepo {
        repo: RepoKey,
        expected_parent: GitRevision,
        child: GitRevision,
        summary: GitDiffSummary,
    },
    AdvanceRepos {
        repo_changes: BTreeMap<RepoKey, GitRepoChange>,
    },
    ApplyPatch {
        repo: RepoKey,
        expected_parent: GitRevision,
        patch: GitPatch,
    },
    UpdateVisibleRef {
        repo: RepoKey,
        key: GitRefKey,
        target: GitRefTarget,
        lineage: GitLineage,
    },
}
```

The fields should start private with constructors and accessors. Public fields
would freeze validation too early. The existing `GitArtifact` has public
constructors and private fields; keep that style.

One repo is not a different type. It is:

```text
GitProgramArtifact.repos.len() == 1
```

Multi-repo is not a different type. It is:

```text
GitProgramArtifact.repos.len() > 1
```

## 4. Current GitArtifact Naming Decision

The current `GitArtifact` is an in-memory file/ref snapshot:

```text
files: BTreeMap<GitPath, Vec<u8>>
refs: BTreeMap<GitRefKey, GitRef>
```

That shape is useful for tests, small fixtures, and early EvoSkill scaffolding.
It is not the arbitrary-growing product repo artifact.

Implementation should pick one hard cutover path:

```text
Option A:
  keep GitArtifact as the current fixture/snapshot artifact
  add GitProgramArtifact for product commit/tree repo state

Option B:
  rename the current shape to GitSnapshotArtifact
  use GitArtifact for the product commit/tree repo state
```

Do not keep both `GitArtifact` and `GitProgramArtifact` as two equally
important product APIs. If both exist, their maturity and intended route must
be explicit in crate docs and facade exports.

Preferred first implementation path: Option A. It avoids a broad rename while
the product Git artifact is still being built. A later hard cutover can rename
the fixture shape once product behavior is proven.

## 5. Artifact Identity And Cache Identity

`GitProgramArtifact` should implement `leaven_core::Artifact`.

Identity law:

```text
artifact graph identity = ordered set of repo keys plus immutable repo revisions
```

Cache law:

```text
cache identity = only Some when every repo revision is immutable commit/tree
```

Use `CacheIdentity::ExternalContent` or a content fingerprint that encodes:

```text
repo key
repo store reference
object format
commit/tree object ID
subpath
identity mode
layout entry
```

Do not include:

```text
WorkspaceId
WorkspacePath outside artifact-owned layout
PodId
ContainerId
CandidateId
Mutable branch names
Mutable local remote paths
Archive visibility
Evaluator identity
```

Evaluator/cache keys can add evaluator and case identity at the engine layer.
The artifact cache identity should only identify evaluation-relevant artifact
state.

## 6. Workspace And Placement API

Do not add a new public `WorkspaceLease`.

The live owning lease is:

```rust
leaven_workspace::Workspace
```

Placement metadata should be a typed factory context value:

```rust
pub struct FirkinWorkspaceContext {
    pub run_id: RunId,
    pub pod_id: FirkinPodId,
    pub container_id: FirkinContainerId,
    pub workspace_id: WorkspaceId,
    pub workspace_root: WorkspacePath,
    pub capabilities: FirkinWorkspaceCapabilities,
}
```

This value is attached by:

```rust
Workspace::new_with_context(root, backend, context)
```

and read through:

```rust
workspace.factory_context::<FirkinWorkspaceContext>()
view.factory_context::<FirkinWorkspaceContext>()
slot.factory_context::<FirkinWorkspaceContext>()
```

The artifact must never read this context to compute identity. The context is
for logging, command routing, cleanup, diagnostics, and backend-specific proof.

## 7. Firkin Backend API

`leaven-workspace-firkin` should own:

```rust
pub struct FirkinWorkspaceFactory;
pub struct FirkinWorkspaceBackend;
pub struct FirkinWorkspaceContext;
pub struct FirkinWorkspaceCapabilities;
pub struct FirkinRunPodPolicy;
```

It should implement:

```rust
impl leaven_workspace::WorkspaceFactory for FirkinWorkspaceFactory
```

and hide:

```text
pod create/delete
container create/delete
container exec
file copy/helper/sidecar mechanics
guest path mapping
per-workspace scratch cleanup
run-pod lifecycle cleanup
```

It must not export generic container abstractions unless a second backend
requires the same law. `leaven-workspace-docker` and `leaven-workspace-k8s`
already own their concrete backend names; a generic container layer would be a
new public dependency decision, not a convenience helper.

## 8. Git Ops API

Git command execution should not live in `leaven-artifact-git`.

Introduce either:

```text
crates/leaven-git-ops
```

or, for the first narrow slice:

```text
a private module in the stage/integration crate
```

Move to a crate when at least two callers need the same Git operation laws.

The public shape, when it becomes a crate:

```rust
pub trait GitOps: Send + Sync {
    fn ensure_revision_present(
        &self,
        store: &RepoStoreRef,
        revision: &GitRevision,
    ) -> impl Future<Output = Result<(), GitOpsError>> + Send;

    fn create_projection(
        &self,
        workspace: &mut WorkspaceView<'_>,
        request: GitProjectionRequest,
    ) -> impl Future<Output = Result<GitProjectionReceipt, GitOpsError>> + Send;

    fn checkout_program(
        &self,
        workspace: &mut WorkspaceView<'_>,
        artifact: &GitProgramArtifact,
    ) -> impl Future<Output = Result<GitCheckoutReceipt, GitOpsError>> + Send;

    fn read_back_program(
        &self,
        workspace: &mut WorkspaceView<'_>,
        request: GitReadbackRequest,
    ) -> impl Future<Output = Result<GitReadbackResult, GitOpsError>> + Send;
}
```

The trait consumes `WorkspaceView`, not a backend-specific lease. That keeps the
same Git materialization code usable with local, Firkin, Docker, K8s, and E2B
backends.

Git ops owns:

```text
CLI argv construction
refspec validation
bundle projection
fresh bare fetch projection
no-alternates checks
no-hardlink/no-local clone rules
fsck validation
proposal import
durable store sync
```

Git ops refuses:

```text
artifact identity law
workspace allocation
optimizer parent selection
frontier admission
case/target visibility policy
provider protocol
```

## 9. Materializer API

Prefer stage-owned materialization for the first product slice:

```rust
pub struct GitProgramMaterializer<G> {
    git: G,
    readback_policy: GitReadbackPolicy,
}
```

Implement against the existing engine trait:

```rust
impl<P, G> leaven_engine::Materializer<P, GitProgramArtifact>
    for GitProgramMaterializer<G>
where
    P: OptimizationProblem<Artifact = GitProgramArtifact>,
    G: GitOps,
{
    ...
}
```

Use `leaven-stage::MaterializableArtifact` only when we deliberately want
artifact-owned convenience materialization. That trait already exists and
should not be duplicated as `GitMaterializable`, `WorkspaceArtifact`,
`MaterializedArtifact`, or another wrapper law.

The current `leaven-artifact-jj` implements `MaterializableArtifact` inside the
artifact crate. Treat that as a scaffold precedent, not as proof that product
Git should add stage/workspace dependencies to `leaven-artifact-git`.

## 10. Materialization Report Names

There are already two public `MaterializationReport` names:

```text
leaven_engine::MaterializationReport
  used by leaven_engine::Materializer
  reports files_written, bytes_written, truncations

leaven_stage::MaterializationReport
  used by leaven_stage::MaterializableArtifact
  reports workspace entries, diagnostics, cost
```

Do not add a third generic `MaterializationReport`.

Git-specific receipts should be named for what they prove:

```text
GitCheckoutReceipt
GitProjectionReceipt
GitImportReceipt
GitReadbackReceipt
FirkinWorkspaceReceipt
```

If a Git materializer implements `leaven_engine::Materializer`, it returns the
engine report and stores Git-specific details in Git receipts owned by the
materializer/readback result. If a later wrapper implements
`leaven_stage::MaterializableArtifact`, it returns the stage report.

## 11. Materialization Record API

If an owned record is needed between materialize, agent execution, and readback,
name it as a record, not a lease and not an artifact:

```rust
pub struct GitMaterialization {
    artifact: GitProgramArtifact,
    repos: BTreeMap<RepoKey, MaterializedGitRepo>,
    archive: Option<GitArchiveProjection>,
    checkout_receipt: GitCheckoutReceipt,
}

pub struct MaterializedGitRepo {
    repo: RepoKey,
    path: WorkspacePath,
    parent: GitRepoArtifact,
    initial_state: ObservedGitState,
}
```

Do not store `Workspace` inside this record unless the record itself owns
cleanup. In the preferred shape, the stage owns `Workspace`, passes
`WorkspaceView` to materializers/agents/readback, and cleanup remains explicit
through the existing `Workspace` API.

## 12. Readback API

Readback should return a typed artifact change plus audit observations:

```rust
pub struct GitReadbackResult {
    parent: GitProgramArtifact,
    change: Option<GitProgramChange>,
    child: Option<GitProgramArtifact>,
    repo_readbacks: BTreeMap<RepoKey, GitRepoReadback>,
    observations: GitAgentRunObservation,
    imported: Vec<ImportedGitProposal>,
    warnings: Vec<GitReadbackWarning>,
}
```

`child` must only be `Some` after the imported objects are present in the
durable repo store. The run graph should record:

```text
ProposalEffect::Change { target, change }
```

and rely on `Artifact::apply_change` to produce the child artifact. Readback
may compute the child as proof, but graph insertion should still go through the
existing artifact law.

## 13. Visibility API

Archive visibility is optimizer/materializer policy, not workspace backend
policy:

```rust
pub enum GitArchiveVisibility {
    None,
    FrontierOnly,
    PublicCandidates,
    PublicCandidatesWithScores,
    PublicCandidatesWithDiffs,
    ExplicitRefs(Vec<GitRefKey>),
}
```

Firkin enforces mounts and execution isolation. Git projection enforces object
visibility. The two are composed by the materializer.

Do not put archive visibility in:

```text
WorkspaceConfig
FirkinWorkspaceContext
GitProgramArtifact identity
WorkspaceBackend
```

## 14. Facade And Prelude Route

Initial public routing should be conservative:

```text
leaven_artifact_git
  advanced public API for Git artifact law

leaven_workspace_firkin
  advanced public API for the Firkin backend

leaven::workspace_firkin
  optional crate alias behind a workspace-firkin feature

leaven::stdlib::artifacts
  may re-export GitProgramArtifact behind the git feature once behavior is real

leaven::prelude
  should not export GitProgramArtifact, GitOps, FirkinWorkspaceFactory, or
  GitReadbackResult in the first implementation slice
```

The ordinary prelude should stay focused on defining problems, artifacts,
surfaces, scorers, and `optimize`. Backend and Git-specific surfaces are
advanced imports.

## 15. Public Maturity Labels

Each new public item in this family should be classified before export:

```text
ordinary public contract
  stable enough for prelude/default docs

advanced public contract
  stable enough for named crate/module imports

explicit scaffold
  public for early integration or tests, documented as scaffold

private implementation detail
  not re-exported from lib.rs
```

Suggested first labels:

```text
GitProgramArtifact                  advanced public contract
GitRepoArtifact                     advanced public contract
GitProgramChange                    advanced public contract
GitRepoChange                       advanced public contract
GitProgramLayout                    advanced public contract
GitArchiveVisibility                advanced public contract
GitReadbackPolicy                   advanced public contract
GitReadbackResult                   explicit scaffold until import tests land
GitOps                              explicit scaffold until two callers exist
FirkinWorkspaceFactory              explicit scaffold until live proof lands
FirkinWorkspaceContext              advanced public contract
FirkinWorkspaceBackend              private or crate-visible implementation
```

Do not promote scaffold through `leaven::prelude`.

## 16. Topology Contract Changes

When implementation starts, expect topology updates:

```text
add crates/leaven-workspace-firkin
  deps: leaven-kernel, leaven-workspace, Firkin client/facade crate

maybe add crates/leaven-git-ops
  deps: leaven-artifact-git, leaven-kernel, leaven-workspace

update crates/leaven/tests/topology_contract.rs
  workspace members
  expected crates
  dependency allowlist
```

Do not add `leaven-workspace-firkin` to `leaven::prelude`. Add it as an
optional `leaven::workspace_firkin` crate alias only after the crate compiles
and has a clear proof gate.

## 17. Proof Gates

API-shape proof:

```text
cargo test -p leaven --test topology_contract
cargo test -p leaven-artifact-git
cargo test -p leaven-workspace --test workspace_view
git diff --check
```

Git projection proof:

```text
FrontierOnly projection has allowed refs only
hidden-only commits are absent
.git/objects/info/alternates is absent
local hardlink clone paths are not used
git fsck passes before import
```

Firkin proof:

```text
one run-scoped product pod
two workspace allocations in separate containers
stable /workspace root in each container
private scratch roots
shared read-only archive projection only when policy allows it
proposal commit imports into durable store
cleanup leaves no running workspace containers
```

The API is not complete because it compiles. It is complete when these proofs
show the public names are carrying the intended law.
