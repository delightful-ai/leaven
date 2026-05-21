# Firkin Git Workspace Backend

Status: planning reference / pre-implementation

Date: 2026-05-20

Recovery note, 2026-05-21: this file was restored from jj history after it was
missing from the active checkout. Treat it as a detailed design reference and
handoff denominator, not as automatically normative over newer code decisions,
the active goal text, or subsequent user direction. Reconcile differences
explicitly before claiming completion.

This document captures the design state for using Firkin product pods as the
Leaven sandbox backend for repo-shaped artifacts, especially EvoSkill-style
candidate programs. It is intentionally detailed because the design spans
artifact identity, Git object visibility, workspace lifecycle, reward-hacking
controls, Firkin pod layout, and Leaven crate boundaries.

This is not proof that the backend exists. It is the target shape to implement
against. Current code anchors are:

- `docs/specs/firkin_git_workspace_api_shape.md`: public API shape, no-duplicate
  rules, facade routing, and proof gates for this backend plan.
- `docs/specs/initial_library.md`: workspace, materializer, and agent runtime
  architecture.
- `docs/specs/agentic_stage_materialization.md`: `MaterializableArtifact`,
  `WorkspaceSlot`, and `WorkspaceFactoryContext` laws.
- `docs/specs/agentic_skill_optimization_primitives.md`: Git artifact shape,
  Git readback policy, and EvoSkill pressure requirements.
- `crates/leaven-workspace`: backend-neutral workspace substrate.
- `crates/leaven-workspace-git`: current host-local Git checkout backend.
- `crates/leaven-artifact-git`: current Git artifact vocabulary.
- `apple/containerization` Firkin checkout: current product-pod substrate.

## 1. Core Decision

For repo-shaped optimization, the codebase is the artifact. The container,
checkout, and workspace are disposable projections of that artifact.

```text
Git artifact = durable candidate state
Repo store    = durable Git object storage for artifact revisions
Pod mirror    = fast run-local Git mirror/cache inside one Firkin pod
Workspace     = disposable execution view of one candidate or proposal attempt
Container     = Firkin execution vessel for one workspace allocation
Product pod   = run-scoped resource island for many containers/workspaces
RunGraph      = Leaven's optimization causality and admission truth
```

The artifact/workspace lifecycle is tight during a stage call, but they are not
the same thing. A stage materializes an artifact into a workspace, runs an agent
or evaluator, reads back a typed change, and then destroys the workspace. The
artifact survives as immutable Git state plus Leaven graph state.

## 2. Non-Negotiable Boundary Split

### 2.1 Artifact

Artifact code owns candidate identity and typed changes.

For Git this means:

```text
Repo identity
Commit or tree identity
Subpath, when optimizing only part of a repo
Typed changes such as AdvanceRepo, AdvanceRepos, ApplyPatch, or UpdateVisibleRef
Diff summaries and ref lineage metadata
Validation of Git paths, refs, and object IDs
```

Artifact code must not:

```text
Run git commands
Allocate workspaces
Know Firkin, Docker, E2B, K8s, or local tempdir mechanics
Decide frontier admission
Use branch names or workspace paths as cache identity
Treat mutable refs as graph truth
```

### 2.2 Workspace

Workspace code owns disposable execution substrate.

For Firkin this means:

```text
Allocate a workspace inside a run-scoped product pod
Map WorkspacePath values into the current container's workspace root
Run commands
Read and write files
Set executable bits when supported
Clean up containers and workspace scratch roots
Expose declared backend context through WorkspaceFactoryContext
```

Workspace code must not:

```text
Know RunGraph internals
Decide candidate identity
Decide frontier admission
Parse agent proposals into graph mutations
Treat workspace mutation as graph mutation
```

### 2.3 Materializer and Parser

Materializers and parsers are the bridge.

```text
candidate artifact
  -> materializer writes workspace layout
  -> agent/evaluator runs in workspace
  -> parser/readback produces typed artifact change
  -> RunContext records proposal and applies change
```

For repo artifacts, materialization is usually checkout/projection. Readback is
usually commit/diff construction.

## 3. Workspace Means One Lease, Not One Pod

A Leaven `Workspace` is a lease for one stage call or one deliberate
cross-helper allocation. It is not a product pod.

The recommended Firkin shape is:

```text
one optimizer run
  -> one Firkin product pod
    -> many workspace allocations
      -> one disposable container/session per allocation
```

Inside a container, the agent should usually see:

```text
/workspace
```

It should not see:

```text
/workspaces/ws-123
/workspaces/ws-456
```

The workspace ID is backend bookkeeping. The backend may use it to isolate pod
paths:

```text
/workspaces/<workspace-id>/root
/workspaces/<workspace-id>/tmp
/workspaces/<workspace-id>/output
```

but the container should mount or chroot the current workspace root as
`/workspace` so agent prompts and tool assumptions stay stable.

## 4. Run-Scoped Firkin Product Pod

The product pod is the resource island for a Leaven optimizer run.

It owns:

```text
Shared pod volumes
Prepared rootfs templates
Run-local Git mirrors and projected views
Build/tool caches
Workspace scratch roots
Sidecar services if needed
Container lifecycle for proposer/evaluator/builder roles
```

It does not own:

```text
Leaven RunGraph truth
Durable candidate identity
Durable score/admission truth
Hidden case truth
The only copy of any commit recorded in Leaven
```

Suggested pod filesystem:

```text
/run/leaven/
  run.json
  policy/
    visibility.json
    mount-policy.json
  repos/
    mirrors/
      <repo-key>.git
    views/
      <workspace-id>/
        <repo-key>.git
    imports/
      <workspace-id>/
        proposal.bundle
  workspaces/
    <workspace-id>/
      root/
      output/
      tmp/
  cache/
    cargo/
    npm/
    python/
    tool/
  evidence/
    proposer-visible/
    evaluator-only/
```

The exact guest prefix can change, but the roles should not collapse.

## 5. Git Object Stores And Visibility

### 5.1 The Security Fact

Git refs and tags are not a visibility boundary.

If an agent can read a bare repo's filesystem, assume it can inspect every
reachable object and many objects not currently named by visible refs. Packfiles,
alternates, reflogs, hardlinks, local clone optimizations, and shared object
directories can all defeat ref-level hiding.

Visibility must be implemented by projection:

```text
durable full store
  full candidate/object truth
  Leaven/backend only

run-local full mirror
  fast pod-local object cache
  Leaven/backend only

agent-visible projected repo
  only allowed refs and their reachable object closure
  mounted or fetched by one workspace
```

### 5.2 Projection Is The Visibility Boundary

For each workspace allocation, Leaven chooses an archive visibility policy and
creates a projected repository or bundle containing only the allowed object
closure.

The safe shapes are:

```text
bundle projection:
  git bundle create <bundle> <allowed-refspecs>
  clone/fetch that bundle inside workspace

fresh bare repo projection:
  git init --bare <view.git>
  git fetch --no-tags --no-local <source-url> <allowed-refspecs>
```

Use `file://` URLs or `--no-local` when fetching from a local source. Do not
use a local-path clone that hardlinks objects. Do not use alternates for
agent-visible projections.

Unsafe shapes:

```text
Mounting /repos/mirrors/<repo-key>.git read-only into an agent container
Relying on tags/branches to hide refs
Using alternates that point from a view repo into the full object store
Using hardlinked local clones as a visibility boundary
Putting hidden eval commits in the same object store exposed to agents
```

### 5.3 Archive Visibility Policy

The policy should be explicit and stage-configurable.

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

Semantics:

```text
None
  The workspace sees only its parent candidate checkout.

FrontierOnly
  The workspace sees current frontier candidate refs and optional public
  summaries. This is a conservative default for cross-pollination.

PublicCandidates
  The workspace sees all candidates marked proposer-visible by policy.

PublicCandidatesWithScores
  The workspace sees proposer-visible candidates plus public validation
  score summaries. It must not see hidden-case scores or hidden raw outputs.

PublicCandidatesWithDiffs
  The workspace sees proposer-visible candidates plus curated diff summaries
  or patch refs. This is useful for recombination but increases overfitting
  pressure.

ExplicitRefs
  The caller names the exact refs. This is for experiments and should still be
  checked against hidden-data policy.
```

Default should be `None` or `FrontierOnly`. EvoSkill-like runs may opt into
`PublicCandidatesWithScores` because cross-pollination is part of the search
strategy.

### 5.4 Agent Writeback

Agents should not push to trusted candidate or frontier refs.

Preferred writeback:

```text
/workspace/output/proposal.bundle
/workspace/output/proposal.patch
/workspace/output/proposal.json
```

or a controlled scratch ref namespace:

```text
refs/workspaces/<workspace-id>/proposal
refs/proposals/<proposal-id>
```

Leaven imports, validates, persists, scores, and promotes.

Trusted refs are Leaven-owned:

```text
refs/candidates/<candidate-id>
refs/frontier/<frontier-id>
refs/archive/<step>/<candidate-id>
```

## 6. Git Artifact Shape

The scalable artifact shape should be commit/tree based, not an in-memory file
snapshot. It should also make multi-repo native to the artifact value.

The live Leaven core already points this way. `OptimizationProblem::Artifact`
is one domain value for the run, and materialization is a lens over that value.
Therefore "this program has one repo" and "this program has three repos" should
be properties of the Git artifact itself, not separate artifact families.

Target product shape:

```rust
pub struct GitProgramArtifact {
    pub repos: BTreeMap<RepoKey, GitRepoArtifact>,
    pub layout: GitProgramLayout,
}

pub struct GitRepoArtifact {
    pub repo: RepoRef,
    pub revision: GitRevision,
    pub subpath: Option<RepoPath>,
    pub identity_mode: GitArtifactIdentityMode,
}

pub struct GitProgramLayout {
    pub entries: BTreeMap<RepoKey, WorkspacePath>,
}

pub enum GitArtifactIdentityMode {
    Commit,
    Tree,
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

pub enum GitRepoChange {
    AdvanceTo {
        expected_parent: GitRevision,
        child: GitRevision,
        summary: GitDiffSummary,
    },
    ApplyPatch {
        expected_parent: GitRevision,
        patch: GitPatch,
    },
    UpdateVisibleRef {
        key: GitRefKey,
        target: GitRefTarget,
        lineage: GitLineage,
    },
}
```

Current `crates/leaven-artifact-git` already owns Git paths, refs, object IDs,
lineage, changes, and diffs. Its current `GitArtifact` stores files plus refs,
which is useful for small fixtures and early proof but is not the final shape
for arbitrary growing repos.

The product path should record:

```text
RepoKey set
Per-repo RepoRef
Per-repo commit/tree object ID
Per-repo identity mode
Per-repo optional subpath
Artifact-owned workspace layout
Typed per-repo diff summaries
Ref lineage metadata where relevant
```

It should not record:

```text
Workspace path
Branch name as identity
Pod path
Container ID
Mutable local remote URL
CandidateId as cache identity
```

## 7. Native Multi-Repo Artifact State

The design must support multiple repos from the start. A single-repo assumption
will become a hidden product constraint.

Do not model this as a separate `CodebaseArtifact` layer unless a future
non-Git artifact family proves that "codebase" needs a generic abstraction.
For the Git product path, the artifact is already the codebase. Its repo map is
just part of the artifact state.

One repo is the degenerate case:

```text
GitProgramArtifact {
  repos: { "program" => GitRepoArtifact { revision: abc, ... } },
  layout: { "program" => "repos/program" },
}
```

Several repos are the same artifact shape with more entries:

```text
GitProgramArtifact {
  repos: {
    "agent-kit" => GitRepoArtifact { revision: abc, ... },
    "bench"     => GitRepoArtifact { revision: 111, ... },
    "harness"   => GitRepoArtifact { revision: 999, ... },
  },
  layout: {
    "agent-kit" => "repos/agent-kit",
    "bench"     => "repos/bench",
    "harness"   => "repos/harness",
  },
}
```

Example materialized layout:

```text
/workspace/
  repos/
    agent-kit/
    bench/
    harness/
  output/
    proposal.json
```

A proposal may mutate one repo:

```text
agent-kit: abc -> def
```

or several repos:

```text
agent-kit: abc -> def
bench:     111 -> 222
harness:   999 -> aaa
```

Leaven graph identity belongs to the whole `GitProgramArtifact`, while each
repo revision remains individually addressable for checkout, projection,
readback, diffing, and cache explanations.

The materializer should handle this by default:

```text
for each (repo_key, repo_artifact) in artifact.repos:
  checkout repo_artifact at artifact.layout.entries[repo_key]

for each repo touched during readback:
  produce a GitRepoChange

if one repo changed:
  return GitProgramChange::AdvanceRepo { repo, ... }

if multiple repos changed:
  return GitProgramChange::AdvanceRepos { repo_changes }
```

There should not be a separate "multi-repo mode" in the workspace backend.
Backends allocate and execute workspaces. Git materialization walks the artifact
layout.

## 8. Materialization Layouts

### 8.1 Local Host Backend

This is the dev/test shape. It does not prove sandboxing.

```text
host temp root:
  /tmp/leaven-local/<workspace-id>/
    repos/<repo-key>/
    output/
    tmp/
```

Materialization:

```text
1. Create temp root.
2. Fetch/clone the required repo revisions from durable store.
3. Checkout parent revisions under repos/<repo-key>.
4. Optionally create projected archive repos under archive/<repo-key>.git.
5. Run agent/evaluator as a local process.
6. Read output and dirty repo state.
7. Create child commits in durable repo store.
8. Delete temp root.
```

This path is useful because it is cheap, deterministic, and easy to test. It
must be labeled as no-isolation.

### 8.2 Firkin Product Pod Backend

This is the target serious backend for local Apple/VZ sandboxing.

Run allocation:

```text
Leaven run starts
  -> FirkinPodWorkspaceFactory starts one product pod
  -> pod mounts shared pod-store
  -> pod prepares rootfs templates
  -> pod initializes run-local mirrors
```

Workspace allocation:

```text
stage calls factory.allocate()
  -> backend assigns workspace id
  -> backend creates /run/leaven/workspaces/<workspace-id>
  -> backend creates projected repos for selected visibility policy
  -> backend starts one container with /workspace mapped to that root
  -> Leaven receives Workspace handle
```

Inside container:

```text
/workspace/
  repos/
    <repo-key>/
  archive/
    <repo-key>.git
  input/
  output/
  tmp/
```

Readback:

```text
1. Parser inspects /workspace/output and repo working trees.
2. Backend or parser builds proposal bundle/patch/commit.
3. Leaven imports child commits into durable repo store.
4. Leaven records GitProgramChange values.
5. Workspace cleanup removes container and scratch root.
6. Product pod remains warm for the run.
```

### 8.3 Shared Remote Backend

This is a remote-control-plane variant, not a different artifact model.

```text
durable repo store:
  network Git remote, object store service, or run-dir-backed bare repos

workspace backend:
  could be Firkin, E2B, K8s, Docker, local

projection:
  created by Leaven before workspace allocation or by a backend-side helper
```

The remote should still expose projected refs/bundles to agents, not the full
trusted store.

### 8.4 Multiple Candidates In One Pod

Multiple candidates can be evaluated or proposed concurrently inside one pod,
but each workspace must get private scratch state.

```text
pod:
  /repos/mirrors/program.git          backend only
  /repos/views/ws-a/program.git       visible to workspace A
  /repos/views/ws-b/program.git       visible to workspace B
  /workspaces/ws-a/root               mounted as /workspace in container A
  /workspaces/ws-b/root               mounted as /workspace in container B
```

Candidate A should not write files that candidate B observes unless the run
policy explicitly provides a shared read-only archive projection.

## 9. Cross-Pollination

Candidate archive visibility is valuable for evolutionary search. It should be
an explicit policy, not an accidental consequence of a shared bare repo.

For cross-pollination:

```text
parent = candidate A
visible archive includes candidate B and candidate C
agent copies/merges ideas from B/C into a child of A
```

Leaven records:

```text
ProposalEffect::Change {
    target: A,
    change: GitProgramChange::AdvanceRepo {
        repo: "program",
        expected_parent: A.repo("program").rev,
        child,
        summary,
    },
}

CausalInputs::Single(A)
informed_by: [B, C]
```

This keeps the distinction:

```text
causal parent = artifact state actually mutated
informed_by   = candidate states used as references
```

Git may contain merge commits when the actual code state is a merge, but Leaven
causality is not inferred from Git ancestry alone.

## 10. Reward-Hacking Controls

The product pod increases power and therefore increases reward-hacking risk.
The answer is not to forbid candidate archive visibility. The answer is to
separate writable candidate state from protected evaluation truth and trusted
admission state.

### 10.1 Protected Facts

Agents must not be able to mutate or inspect:

```text
Hidden test cases
Answer keys
Scorer implementation when it reveals labels
Evaluator-only raw outputs
Durable Leaven RunGraph state
Durable repo store internals beyond projected visibility
Trusted candidate/frontier refs
Reward/evaluation cache internals
Other candidates' writable workspace roots
```

### 10.2 Allowed Facts

Depending on policy, agents may see:

```text
Their parent candidate repo checkout
Frontier candidate commits
Public candidate archive commits
Public validation score summaries
Curated diffs between visible candidates
Proposer-visible notes/evidence
```

### 10.3 Mount Policy

Use mount and projection policy, not convention.

Proposer container:

```text
rw  /workspace/repos/<candidate-repo>
rw  /workspace/output
rw  /workspace/tmp
ro  /workspace/archive/<projected-visible-repo>
ro  /workspace/input/proposer-visible
none hidden cases
none scorer internals
none durable store
none full pod mirror
```

Evaluator container:

```text
ro  candidate commit checkout
ro  evaluator harness
ro  hidden cases, if evaluator needs them
rw  evaluator scratch/output
none trusted frontier/admission write refs
```

Backend/control container or host-side Leaven process:

```text
rw durable repo store
rw run graph/checkpoints
rw trusted candidate/frontier refs
rw projection creation
```

### 10.4 Cache Policy

Shared caches are useful but dangerous.

Rules:

```text
Proposer-visible shared caches must not contain hidden evaluation truth.
Evaluation caches must be keyed by cache-safe artifact identity.
Evaluation cache keys must include evaluator version, case set, materializer
identity, runtime/tool identity, and relevant environment fingerprint.
Candidate containers should not write trusted reward caches directly.
Build caches may be shared when their contents cannot encode hidden labels.
When uncertain, namespace by workspace or artifact revision.
```

### 10.5 Ref Authority

Agents may write only scratch outputs or scratch refs.

Leaven writes:

```text
refs/candidates/*
refs/frontier/*
refs/archive/*
score/admission records
checkpoint state
```

Agents write:

```text
/workspace/output/*
refs/workspaces/<workspace-id>/*
refs/proposals/<proposal-id>/*
```

No agent-authored ref becomes trusted until Leaven imports and validates it.

## 11. Firkin Backend Requirements

The Leaven Firkin backend should be a new concrete workspace backend, likely:

```text
crates/leaven-workspace-firkin
```

It depends inward on:

```text
leaven-workspace
leaven-kernel
Firkin facade/client crates
```

It should not depend on:

```text
leaven-core
leaven-engine
leaven-artifact-git, unless a carefully named integration type requires it
leaven-population
optimizer crates
provider crates
```

### 11.1 Factory

Target type:

```rust
pub struct FirkinPodWorkspaceFactory {
    // run-scoped product pod owner or client
}
```

Responsibilities:

```text
Start or attach one product pod for the Leaven run.
Create shared pod directories and volumes.
Prepare rootfs templates for roles.
Maintain run-local repo mirrors and view roots.
Allocate one workspace/container per stage call.
Expose factory context with pod id, paths, repo view handles, and capability set.
Clean up abandoned workspaces and stop the pod at run end.
```

### 11.2 Backend

Target type:

```rust
struct FirkinWorkspaceBackend {
    pod_id: PodId,
    container_id: ContainerId,
    workspace_id: WorkspaceId,
    workspace_root: GuestPath,
}
```

Responsibilities:

```text
Map WorkspacePath to guest path below workspace_root.
Implement write_file/read_file/list_files through Firkin file APIs or helper.
Implement run_command through Firkin container exec/process APIs.
Implement cleanup by stopping/removing the container and workspace root.
Return local_mount() = None unless an explicit sync path exists.
Surface unsupported operations as typed WorkspaceError values.
```

### 11.3 Required Firkin Capabilities

Minimum:

```text
Product pod create/delete
Add/remove/wait pod container
Shared pod emptyDir or equivalent shared writable volume
Prepared rootfs templates or OCI rootfs materialization
Command execution with stdout/stderr capture
File write/read/list under workspace root
Executable-bit support or an honest unsupported path
Cleanup of per-container overlay/workspace state
```

Current Firkin facts from the Apple/VZ checkout:

```text
Product pods exist in runtime contract.
Pod create/add/delete/wait routes exist in the E2B server surface.
Pod rootfs sources include GuestPath, TemplateGuestPath, OciBundle, Ext4Image.
TemplateGuestPath uses a prepared lowerdir plus per-container overlay.
Apple/VZ advertises product-pods, pod-emptydir, pod-guest-path-rootfs,
pod-shared-rootfs-template, prepared-template-pods, pod-store-asif, and
pod-store-raw-size.
Apple/VZ still marks pod-snapshot-restore unsupported.
```

### 11.4 File API Gap

Leaven `WorkspaceBackend` requires file operations. If Firkin product pods do
not expose direct file operations for pod containers, the Leaven backend needs
one of:

```text
guest helper command for file read/write/list/chmod
sidecar service mounted into the pod
envd-compatible bridge
host-side pod-store mount/copy path
```

The backend must keep this hidden behind `WorkspaceBackend`; stages should not
branch on Firkin.

### 11.5 Snapshot Non-Requirement

Pod snapshot restore is not required for the first Leaven backend.

The first product path can be:

```text
run-scoped pod
reconstructable repo/materialization state
durable Leaven run/checkpoint store
durable Git repo store
explicit cleanup and restart reconciliation
```

Snapshot-backed resume would be a later performance feature. It must not be
the only way to recover candidate artifacts.

## 12. Git Workspace And Artifact Changes

### 12.1 Current `leaven-workspace-git`

Current `leaven-workspace-git` is useful as a host-local clone backend and test
fixture. It can clone, checkout, run commands, capture tracked files/refs,
restore refs, and delete refs.

It should not become the conceptual center for Firkin.

The better abstraction is:

```text
Git materialization/readback over any WorkspaceBackend
```

Then:

```text
LocalWorkspaceFactory + Git materializer/readback
  -> local Git execution

FirkinPodWorkspaceFactory + Git materializer/readback
  -> pod/container Git execution
```

### 12.2 Orphan Rule Problem

`MaterializableArtifact` lives in `leaven-stage`. `GitArtifact` lives in
`leaven-artifact-git`.

A third crate cannot implement a foreign trait for a foreign type. Therefore a
Git materialization implementation must choose one of:

```text
1. Implement MaterializableArtifact for GitArtifact inside leaven-artifact-git.
   This adds stage/workspace dependencies to leaven-artifact-git.

2. Introduce a local wrapper type in an integration crate:
   WorkspaceGitProgramArtifact(GitProgramArtifact)
   The wrapper implements MaterializableArtifact.

3. Avoid trait impl and use stage-owned Git materializer/parser fields.
   This aligns with initial_library.md's stage-owned materializer default.
```

Preferred first slice: option 3 for explicit composition, plus a real artifact
shape in `leaven-artifact-git`. Add wrapper types only when repeated call sites
need them.

### 12.3 New Shared Git Operational Layer

Avoid putting command execution into `leaven-artifact-git`.

Possible crate:

```text
crates/leaven-git-ops
```

or a focused module inside an agentic integration crate if there is only one
caller initially.

It would own:

```text
Git CLI command construction
Projection repo creation
Bundle creation/import
Refspec validation
fsck/import validation
Commit readback helpers
Durable store sync
```

It must not own:

```text
Artifact identity law
Workspace allocation
Optimizer policy
Frontier admission
Hidden-data policy decisions
```

Name should be chosen carefully. Do not create `common`, `utils`, or `shared`.

## 13. Git CLI Versus gix/libgit2

Initial implementation should use Git CLI.

Reasons:

```text
Git already implements correct pack, fetch, bundle, and object closure rules.
Projection by bundle/fetch is exactly the desired operation.
CLI behavior is easy to smoke-test against real repositories.
It avoids prematurely reimplementing Git security-sensitive closure logic.
```

CLI discipline:

```text
Use std::process::Command, not shell strings.
Validate ref names and repo keys before passing them.
Set HOME and Git config env explicitly.
Use GIT_CONFIG_NOSYSTEM=1 and an explicit empty global config where possible.
Use file:// or --no-local for local projection fetches.
Reject alternates in agent-visible repos.
Run git fsck before importing proposals.
Use temp directories and atomic rename for projected views.
Capture stdout/stderr and command status in typed errors.
```

Move to `gix` later when:

```text
We need in-process pack streaming.
We need better structured Git object errors.
We need to remove guest dependency on a Git binary.
We need fine-grained object filtering with library-level tests.
```

`libgit2` is viable but brings a C dependency and does not eliminate the need
to prove behavior against Git CLI semantics. Prefer CLI first, `gix` second.

## 14. Durable Repo Store

The durable repo store is the source of truth for commit IDs that Leaven records
as artifact identities.

Possible locations:

```text
run directory:
  .leaven/runs/<run-id>/repos/<repo-key>.git

global Leaven store:
  .leaven/repos/<repo-key>.git

external network remote:
  ssh://, https://, or object-store-backed Git service
```

For first implementation, a run-directory store is easiest:

```text
.leaven/runs/<run-id>/
  graph/
  evidence/
  repos/
    <repo-key>.git
  repo-views/
    <workspace-id>/
  imports/
    <workspace-id>/
```

If the product pod contains mirrors, they are caches:

```text
pod /repos/mirrors/<repo-key>.git
  fast cache, not graph truth

run dir .leaven/runs/<run-id>/repos/<repo-key>.git
  durable source of truth for recorded artifact revisions
```

Before Leaven records a child artifact, the child commit must be present in the
durable store.

## 15. End-To-End Flows

### 15.1 Proposer Mutation

```text
1. Optimizer selects parent candidate A.
2. Visibility policy selects archive refs B, C, ...
3. Leaven ensures parent and visible refs exist in durable repo store.
4. Firkin factory allocates workspace/container.
5. Git projector creates workspace-visible archive repo.
6. Materializer checks out each repo in parent A under the artifact layout.
7. Agent mutates files and writes output/proposal metadata.
8. Parser/readback creates child commit D or proposal bundle.
9. Leaven imports D into durable repo store.
10. Parser returns GitProgramChange::AdvanceRepo or AdvanceRepos.
11. RunContext records ProposalEffect::Change.
12. Evaluation/admission later decides whether D becomes frontier.
13. Workspace container and scratch root are cleaned up.
```

### 15.2 Evaluator Run

```text
1. Evaluator receives resolved candidate set and case set.
2. Factory allocates evaluator workspace/container.
3. Materializer checks out candidate commit read-only.
4. Evaluator-only materializer writes hidden cases and scorer assets.
5. Agent/runtime/test command runs.
6. Evaluator reads result artifacts and emits Assessment.
7. Hidden raw outputs stay evaluator-only unless policy projects summaries.
8. Workspace cleanup runs.
```

### 15.3 Cross-Pollination

```text
1. Parent A selected.
2. Archive visibility exposes B and C.
3. Agent reads B/C diffs or repos.
4. Agent applies useful ideas to A's checkout.
5. Child D is recorded as a child of A.
6. B/C are recorded as informed_by, not causal parents, unless the optimizer
   deliberately creates a merge proposal.
```

### 15.4 Multi-Repo Mutation

```text
1. GitProgramArtifact has repos agent-kit, bench, harness.
2. Materializer checks out all three.
3. Agent modifies agent-kit and harness.
4. Readback produces GitRepoChange values for two repos.
5. GitProgramChange::AdvanceRepos applies to produce a new GitProgramArtifact.
6. Evaluation cache identity covers the whole Git program state.
```

## 16. Reward-Hacking Threat Model

| Threat | Example | Control |
|---|---|---|
| Hidden answer leak | Proposer reads test labels from pod volume | Hidden cases are evaluator-only and never in proposer projection |
| Scorer mutation | Agent edits evaluator to always pass | Scorer/harness mounted read-only or absent from proposer workspace |
| Ref forgery | Agent updates `refs/frontier/winner` | Agents write only scratch refs/output; Leaven owns trusted refs |
| Cache poisoning | Agent writes reward cache entries | Reward cache is Leaven/evaluator-owned, keyed and not proposer-writable |
| Cross-workspace interference | Candidate A writes file read by candidate B | Per-workspace roots; shared volumes read-only or mediated |
| Object store leak | Agent reads full bare repo packfiles | Agent sees only projected repos/bundles with object closure filtering |
| Score overfitting | Agent sees hidden raw eval outputs | Public summaries are policy-filtered; hidden raw outputs stay hidden |
| Toolchain poisoning | Agent modifies shared compiler/tool cache | Caches are content-addressed, read-only, or namespaced by trust level |

The product pod is not itself the trust boundary. The trust boundary is the
combination of:

```text
projection
mount policy
workspace path containment
role-specific containers
RunContext-only graph mutation
Leaven-owned durable repo import/admission
```

## 17. Crate Placement Sketch

This is a planning sketch, not a required final crate list.

```text
leaven-artifact-git
  owns:
    RepoRef, RepoKey, GitProgramLayout
    GitRevision, GitRepoArtifact, GitProgramArtifact
    GitRepoChange, GitProgramChange
    GitRefKey, GitLineage, GitDiffSummary
  refuses:
    command execution, workspace allocation, Firkin, optimizer policy

leaven-workspace-firkin
  owns:
    FirkinPodWorkspaceFactory
    FirkinWorkspaceBackend
    Firkin workspace context/capability records
  refuses:
    Git artifact identity, optimizer policy, graph mutation

leaven-git-ops or stage-specific git module
  owns:
    Git CLI projection/import helpers
    bundle/fetch/fsck mechanics
    durable store sync helpers
  refuses:
    artifact identity law, workspace allocation, frontier policy

leaven-agentic or shape-specific adapter crate
  owns:
    stage-owned materializer/parser composition
    proposal parsing into typed ProposalBatch
  refuses:
    provider-specific protocol and Firkin-specific backend mechanics

optimizer/example/EvoSkill crate
  owns:
    archive visibility choice
    parent selection
    validation scoring
    frontier admission policy
    paper-specific metadata and prompts
  refuses:
    generic Git projection mechanics and backend execution details
```

## 18. Implementation Ladder

1. Document and test Git projection mechanics with local bare repos.
   - Prove `FrontierOnly` projection does not include hidden refs.
   - Prove no alternates/hardlinks are used in view repos.
   - Prove proposal bundles import only after `git fsck`.

2. Harden `leaven-artifact-git` around commit/tree identity.
   - Add `GitProgramArtifact` or evolve `GitArtifact`.
   - Make one-or-many repos native to that artifact shape.
   - Add `GitProgramChange::{AdvanceRepo, AdvanceRepos}`.
   - Keep current file-snapshot artifact as fixture support or clearly label it.

3. Add durable repo-store helpers.
   - Run-dir-backed bare repo store.
   - Import/export bundle support.
   - Multi-repo `RepoKey` handling.

4. Add stage-owned Git materializer/readback over `WorkspaceView`.
   - Local backend first.
   - No Firkin dependency yet.
   - Prove materialize -> no change returns no-op/None.
   - Prove dirty workspace -> child commit.

5. Add `leaven-workspace-firkin` scaffold.
   - Factory starts/attaches run-scoped product pod.
   - Backend allocates one container as one workspace.
   - File/command/cleanup operations implemented or honestly unsupported.

6. Compose Git materializer with Firkin backend.
   - Parent checkout under `/workspace/repos/<repo-key>`.
   - Archive projection under `/workspace/archive`.
   - Readback bundle/import.

7. Add reward-hacking regression tests.
   - Hidden refs absent from projections.
   - Proposer cannot write trusted refs.
   - Evaluator-only files absent from proposer workspace.
   - Cache identity excludes CandidateId and includes revision/runtime/materializer.

8. Build one fake-runtime EvoSkill-shaped iteration.
   - Real Git artifact.
   - Real local projection.
   - Fake agent mutation.
   - Real frontier admission.

9. Run the same through Firkin with a signed live smoke.
   - One product pod.
   - Multiple workspace containers.
   - Shared pod mirror and per-workspace projections.
   - Cleanup proof.

## 19. Verification Expectations

Doc-only changes:

```text
Referenced paths resolve.
Markdown is internally consistent.
No implementation completion is claimed.
```

Git projection tests:

```text
Projected repo contains allowed refs.
Projected repo lacks hidden refs.
Projected repo has no alternates.
Projected object closure does not include hidden-only commits.
Proposal import runs fsck.
```

Workspace backend tests:

```text
WorkspacePath containment.
run_command cwd and env semantics.
stdout/stderr truncation.
cleanup removes container/scratch root.
local_mount() is None unless a real mount exists.
factory context is explicit and typed.
```

Firkin live proof:

```text
Create one product pod.
Allocate two workspaces as separate containers.
Both see stable /workspace roots.
They cannot see each other's writable scratch roots.
They can read allowed shared archive projections.
At least one proposal commit imports to durable store.
Pod cleanup leaves no running containers or trusted refs from agent scratch.
```

## 20. Open Questions

1. Should the product Git artifact be named `GitProgramArtifact`, or should it
   become the replacement meaning of `GitArtifact` while the current file
   snapshot type is renamed to make its fixture role explicit?

2. Does the first Firkin backend use direct Firkin file APIs, an envd bridge, or
   a small Leaven guest helper for file operations?

3. Is the first durable repo store run-directory scoped, global Leaven scoped,
   or external remote scoped?

4. What is the default `GitArchiveVisibility` for ordinary agentic codebase
   optimization: `None` or `FrontierOnly`?

5. How should public score summaries be represented so they are useful for
   cross-pollination but cannot leak hidden test truth?

6. Should Leaven own a Git projection service abstraction, or should each
   materializer call Git ops directly until a second backend needs the same
   behavior?

## 21. Current Position

The preferred direction is:

```text
one Firkin product pod per optimizer run
one workspace allocation per stage call
one container/session per workspace allocation
workspace root mounted as /workspace inside the container
run-local pod mirrors for speed
projected repos/bundles for agent-visible candidate archive
durable run-dir or store-backed bare repos as source of truth
Git artifact identity based on immutable commit/tree IDs
Leaven graph as causality/admission truth
Leaven-owned imports/promotions for trusted refs
explicit archive visibility policy for cross-pollination
role-specific mount policy to prevent reward hacking
```

This lets EvoSkill-style systems use the full power of the Git DAG for
mutation, branching, recombination, and archive inspection, while preserving
Leaven's boundaries around hidden evaluation truth, cache identity, and graph
mutation authority.
