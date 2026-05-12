# agentic stage materialization — milestone plan, implementation-detail edition

Date: 2026-05-13  
Status: implementation-ready draft, framed as behavioral milestones  
Companions:

- `leaven_agentic_stage_materialization_goal_state_spec.md`
- `leaven_agentic_stage_materialization_prereq_work.md`
- May 12 design sketch: the A/B cut remains the governing premise.
- `docs/philosophy/goal_handoff.md`: the proxy-substitution failure mode this plan refuses.

## 0. purpose

The previous prerequisite plan identified the right phases, but several tasks still said things like “add `WorkspaceSlot`” or “add `StageReceipt`” without defining the actual structs, trait methods, error types, and test contracts. They also tracked work as a stack of PRs, which let “the PR landed” stand in for “the behavior holds.”

This document blows those work items up into an implementation map *and* reframes each phase as a behavioral milestone. The intended use is: an implementer should be able to open a milestone, create the listed modules, paste the target definitions, and know exactly which user- or system-visible behaviors must hold before the milestone can be claimed complete — and which nearby provable artifacts would be a misleading proxy for that completion.

### 0.1 milestone discipline (no workarounds)

Each milestone carries two short blocks:

```text
Done when (no workarounds): behaviors the system must exhibit at completion,
                            each tied to a named test, file, or runnable
                            assertion that exercises the production path.
Forbidden proxy proofs:     nearby artifacts that look like success but do not
                            satisfy the behavior — citing them as evidence
                            does not advance the milestone.
```

Acceptance rules that hold for every milestone:

```text
- "the PR landed" is not acceptance. A behavior either holds or it does not.
- "the file compiles" is not acceptance unless the milestone's behavior is
  about compilability (Milestone 4 is the only one).
- A test counts only if it exercises the production path. A hand-built shim
  that mirrors the production behavior is a forbidden proxy, not evidence.
- If a behavior would silently regress under a plausible future change, the
  milestone is not done; add the regression test.
- "Tests pass" without naming which test denominator was run is not
  acceptance. Each milestone lists the named tests that must be green.
- A workaround that ships behind a feature flag or `#[cfg(test)]` shim is
  not acceptance; it just hides the unmet behavior.
```

These rules are what makes the milestones load-bearing rather than decorative. If they ever conflict with a tempting shortcut, the rules win.

### 0.2 spirit over letter (read this first if you are implementing)

Every behavior in this plan is an attempt to encode a user-facing intent in code-shaped language. The encodings will be wrong in places — names will drift, file paths will move, signatures will change, and a few of the listed tests will turn out to be the wrong shape. When that happens, follow the spirit of the milestone, not the letter of the artifact list.

Concretely:

```text
- Type names, module paths, error variants, and test names in this plan are
  illustrative anchors, not contracts. If the codebase already has a better
  name that preserves the same behavior more honestly, prefer the better
  name and update the plan in the same change. Do not invent a new
  vocabulary just to match the plan literally when the existing vocabulary
  is fine.
- The "Done when" bullets describe behavioral intent. They are met when a
  reader of the diff could conclude the production path actually exhibits
  the behavior under adversarial input, not when the bullet count matches.
  A milestone with five green tests can still be incomplete if those tests
  exercise the happy path of a shim instead of the production path.
- The "Forbidden proxy proofs" are examples of plausible failure modes,
  not an exhaustive list. If the obvious move toward "completion" would be
  a worse proxy than anything listed, treat it as forbidden anyway. Each
  listed proxy is a representative; assume there are unlisted siblings.
- If a behavior in this plan conflicts with the governing specs in
  `docs/specs/` or with the philosophy docs in `docs/philosophy/`, the
  specs and philosophy win. This plan is downstream of them. Bring the
  conflict back into the plan in the same change rather than silently
  diverging.
- If a milestone seems to require a workaround — a feature flag, a parallel
  old/new path, a `#[cfg(test)]` shim, a TODO comment — stop. The right
  move is almost always to go back to the milestone's intent, ask which
  spec encodes it, and re-derive the implementation. The plan was written
  assuming hard cutovers; a workaround is evidence the intent has not been
  understood yet.
- If a forbidden proxy proof feels load-bearing for a reason this plan did
  not anticipate, escalate before claiming the milestone. Do not invoke
  the spirit clause as a quiet override.
```

If a milestone reads as a checklist that any plausible diff could pass, you are reading the letter and missing the spirit. Re-read the governing specs the milestone references and look for the user- or system-facing claim it actually encodes. The plan exists to make that claim mechanically defensible, not to substitute for it.

The north-star proof remains:

```text
GEPA/test optimizer builds typed stage request
  -> AgentBacked<ProposerSlot> builds AgentStagePlan<Req>
  -> materialize_stage_workspace writes BRIEF/focus/output + selected context
  -> FakeAgentRuntime writes output/proposal.json
  -> StageOutputParser parses ProposalBatch<P>
  -> RunContext::propose records the batch
  -> RunContext::apply_batch applies it
  -> StageReceipt proves what was visible/materialized/read back
```

## 1. current state, implementation-relevant

### 1.1 `leaven-workspace`

Current substrate has:

```rust
pub trait WorkspaceFactory: Send + Sync {
    fn allocate(
        &self,
        config: WorkspaceConfig,
    ) -> impl Future<Output = Result<Workspace, FactoryError>> + Send + '_;
}

pub struct Workspace {
    backend: Arc<Mutex<Box<dyn WorkspaceBackend>>>,
    local_mount: Option<PathBuf>,
}

pub struct WorkspaceView<'a> {
    backend: Arc<Mutex<Box<dyn WorkspaceBackend>>>,
    local_mount: Option<PathBuf>,
    prefix: WorkspacePath,
    marker: PhantomData<&'a mut ()>,
}
```

`WorkspaceView::subdir`, `write_file`, `read_file`, `list_files`, `set_executable`, `is_executable`, and `run_command` already provide most of the needed scoped IO behavior.

Missing for stage materialization:

```text
WorkspaceId
WorkspaceSlot
factory context
workspace/tree fingerprint helpers
receipt-friendly output/file metadata
```

### 1.2 `leaven-agent`

Current runtime-facing contract is good and should remain low-level:

```rust
pub trait AgentRuntime: Send + Sync {
    fn id(&self) -> AgentRuntimeId;
    fn fingerprint(&self) -> Fingerprint;
    fn capabilities(&self) -> AgentRuntimeCapabilities;

    fn run_session<'a>(
        &'a self,
        workspace: &'a mut WorkspaceView<'_>,
        request: AgentRunRequest,
        ctx: AgentRunContext<'a>,
    ) -> impl Future<Output = Result<Metered<AgentSession>, AgentRuntimeError>> + Send + 'a;
}
```

Runtime `OutputContract` is intentionally coarse:

```rust
pub enum leaven_agent::OutputContract {
    Files { paths: Vec<WorkspacePath> },
    JsonFile { path: WorkspacePath, schema: Option<JsonSchemaRef> },
    FinalMessage,
    WorkspaceDiff { roots: Vec<WorkspacePath> },
}
```

Do not replace this. Add a stage-level output contract above it and lower stage output contracts into runtime output contracts.

### 1.3 `leaven-engine`

Current proposer path is the right finalization path:

```rust
pub trait Proposer<P: OptimizationProblem>: Send + Sync {
    type Request: Send + Sync;

    fn id(&self) -> ProposerId;
    fn arity(&self) -> Arity;

    async fn propose(
        &self,
        request: Self::Request,
        ctx: ProposalContext<'_, P>,
    ) -> Result<Metered<ProposalBatch<P>>, ProposalError>;
}

impl<'a, P: OptimizationProblem> RunContext<'a, P> {
    pub async fn propose<T>(
        &mut self,
        proposer: &T,
        request: T::Request,
    ) -> Result<ProposalBatchReport, RunContextError>
    where
        T: Proposer<P>;
}
```

Current `ProposalContext` exposes broad graph access:

```rust
pub struct ProposalContext<'a, P: OptimizationProblem> {
    graph: RunGraphView<'a, P>,
    budget: BudgetHandle<'a>,
    read_scope: ReadScope,
}

impl<'a, P: OptimizationProblem> ProposalContext<'a, P> {
    pub fn graph(&self) -> &RunGraphView<'a, P>;
    pub fn read_scope(&self) -> &ReadScope;
    pub fn budget(&self) -> BudgetSnapshot;
    pub fn render_context(&mut self) -> RenderContext<'_, P>;
    pub fn materialize_context(&self) -> MaterializeContext<'a, P>;
}
```

Keep this initially. Add a preferred scoped source path for agent-backed stages; do not make the first PR a giant engine-context rewrite.

### 1.4 `leaven-agentic`

`AgentCase` / `CaseSuite` / `AgentWorkload` already solve candidate evaluation workload. Keep them A-shaped.

Current agentic proposer adapters exist:

```text
AgenticProposer
RepairingAgenticProposer
AgenticRunInput
ProposalParser
ProposalRepairPromptBuilder
```

They are transitional. Do not delete them before `AgentBacked<ProposerSlot>` exists and one example has migrated.

### 1.5 `leaven-gepa`

Current GEPA reflection path is still proxy-shaped:

```rust
pub trait SurfaceProposer<A, S>
where
    A: Artifact,
    S: EditSurface<A>,
{
    fn propose_edit(
        &mut self,
        artifact: &A,
        surface: &S,
        part: &S::PartId,
    ) -> Result<S::Edit, SurfaceError>;
}

pub struct ReflectiveMutation<E> {
    edit: E,
}
```

`ReflectiveMutation` always returns a cloned edit. It should be renamed/quarantined before the public docs imply it is real reflection.

Current GEPA records directly:

```rust
ctx.record_proposal_batch(...)?;
ctx.apply_batch(...)?;
```

The goal is for actual reflection to become an engine `Proposer<P>` call:

```rust
let report = ctx.propose(&self.reflector, request).await?;
let applied = ctx.apply_batch(report.batch_id)?;
```

## 2. Milestone 1 — docs boundary and A-shaped workload hardening

**Done when (no workarounds):**

- A reader can open `docs/specs/agentic_task_execution_substrate.md`, `agentic_stage_runtime.md`, `agentic_stage_materialization.md`, `gepa_optimizer_surface.md`, and `initial_library.md` and find consistent wording for the three jurisdictions: candidate-evaluation workload (`AgentCase`/`AgentWorkload`/`AgentCaseEvaluator`), optimizer-stage workspace (`AgentStagePlan`/`AgentBacked`/`StageReceipt`), and raw substrate (`Workspace`/`WorkspaceView`/`WorkspaceFactory`).
- `AgentWorkload::from_cases` builds a suite with a derived `all` partition, and the four named acceptance tests in `crates/leaven-agentic/tests/agentic_workload.rs` (derive-`all`, fingerprint-changes-on-hidden-target, reject-duplicate-ids, reject-partition-referencing-missing-case) are all green.
- A presenter-law test materializes a case with `CaseTarget::Hidden("SECRET...")` and the resulting `AgentRunRequest.instructions` text and workspace bytes provably do not contain the secret. The test fails if any future stock presenter leaks hidden target.

**Forbidden proxy proofs:**

- The presenter-law test materializes a case through a custom test presenter rather than every existing stock presenter, so future stock presenters that leak hidden target are unconstrained. The intent is that *any* stock presenter that ships in this repo cannot leak a hidden target; a test that pinpoints one presenter does not encode that.
- The hidden-target test only checks `instructions_text().contains(secret)` and not the workspace bytes (or vice versa). Hidden data leaks through whichever channel is unchecked, and the milestone passes anyway. The test must close every presenter-visible channel.
- The doc edits land but downstream code/specs still treat `AgentCase` as if it were stage-shaped (e.g., a stage-side helper that takes `AgentCase` and a workspace). The vocabulary boundary is then aspirational: the docs claim the jurisdictions are separated, the code does not.
- `AgentWorkload::from_cases` is implemented and tested but `from_parts` and `from_cases`/`from_parts` consistency are not — partial scope shipped as a complete milestone, leaving callers that need explicit partitions stranded.
- `workload_fingerprint_changes_when_hidden_target_changes` passes only because the workload fingerprint hashes the entire `CaseSuite` opaquely; it would also "change" if any unrelated field changed, which means it doesn't actually pin hidden-target sensitivity. The test should also assert the fingerprint is *stable* when nothing material changes.
- The "reader can find consistent wording" claim is satisfied by adding a glossary at the top of one doc and not propagating the vocabulary into the rest. Anyone reading a different spec first will still see the old conflated names.

### 2.1 update docs, no API churn yet

Files:

```text
docs/specs/agentic_task_execution_substrate.md
docs/specs/agentic_stage_runtime.md
docs/specs/agentic_stage_materialization.md
docs/specs/gepa_optimizer_surface.md
docs/specs/initial_library.md
```

Required wording:

```text
AgentCase / AgentWorkload / AgentCaseEvaluator are candidate-evaluation workload vocabulary.
AgentStagePlan / AgentBacked / StageReceipt are optimizer-stage workspace vocabulary.
Workspace / WorkspaceView / WorkspaceFactory are raw substrate.
```

Do not rename `AgentCase` in this PR.

### 2.2 add `AgentWorkload` convenience methods

File:

```text
crates/leaven-agentic/src/case.rs
```

Target replacement/extension:

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentWorkload {
    cases: CaseSuite,
}

impl AgentWorkload {
    /// Constructs a workload from an already validated suite.
    #[must_use]
    pub const fn new(cases: CaseSuite) -> Self {
        Self { cases }
    }

    /// Constructs a workload from cases, deriving the standard `all` partition.
    ///
    /// # Errors
    ///
    /// Returns [`AgenticAdapterError`] for duplicate ids or invalid suite data.
    pub fn from_cases(
        cases: impl IntoIterator<Item = AgentCase>,
    ) -> Result<Self, AgenticAdapterError> {
        Ok(Self {
            cases: CaseSuite::from_cases(cases)?,
        })
    }

    /// Constructs a workload from an explicit suite map and partitions.
    ///
    /// # Errors
    ///
    /// Returns [`AgenticAdapterError`] when any partition references a missing case.
    pub fn from_parts(
        cases: BTreeMap<CaseId, AgentCase>,
        partitions: CasePartitions,
    ) -> Result<Self, AgenticAdapterError> {
        Ok(Self {
            cases: CaseSuite::new(cases, partitions)?,
        })
    }

    /// Returns the workload case suite.
    #[must_use]
    pub const fn cases(&self) -> &CaseSuite {
        &self.cases
    }

    /// Returns named workload partitions.
    #[must_use]
    pub const fn partitions(&self) -> &CasePartitions {
        self.cases.partitions()
    }

    /// Returns the behavior fingerprint of this workload.
    #[must_use]
    pub const fn fingerprint(&self) -> Fingerprint {
        self.cases.fingerprint()
    }

    /// Returns whether the workload has no cases.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.cases.is_empty()
    }
}
```

Imports needed in `case.rs` already mostly exist:

```rust
use std::collections::{BTreeMap, BTreeSet};
use leaven_kernel::{CaseId, Fingerprint};
```

Acceptance tests:

```rust
#[test]
fn workload_from_cases_derives_all_partition() { ... }

#[test]
fn workload_fingerprint_changes_when_hidden_target_changes() { ... }

#[test]
fn workload_rejects_duplicate_case_ids() { ... }

#[test]
fn workload_rejects_partition_referencing_missing_case() { ... }
```

Test file:

```text
crates/leaven-agentic/tests/agentic_workload.rs
```

### 2.3 add hidden-target presenter law test

The law:

```text
CaseTarget::Hidden is scorer-visible but not candidate-visible through stock presenters.
```

If there is not yet a stock presenter that can prove this directly, add a dry-run fixture under tests. The test should materialize a case with hidden target and assert workspace files and `AgentRunRequest.instructions` do not contain the hidden target string.

Target test shape:

```rust
#[test]
fn hidden_target_is_not_presented_to_candidate() {
    let secret = "SECRET_TARGET_SHOULD_NOT_APPEAR";
    let case = AgentCase::text(
        CaseId::new(),
        "visible input",
        CaseTarget::Hidden(secret.to_owned()),
    );

    let presentation = dry_run_stock_presenter(&case).expect("presenter dry run");

    assert!(!presentation.instructions_text().contains(secret));
    assert!(!presentation.workspace_bytes().contains(secret.as_bytes()));
}
```

If the exact helper names differ, the required invariant is the important part. The test should fail if a future presenter writes hidden target data into the candidate-visible workspace.

## 3. Milestone 2 — kernel/workspace substrate definitions

This milestone adds the small mechanical affordances the stage layer needs. Keep optimizer vocabulary out of `leaven-workspace`.

**Done when (no workarounds):**

- `WorkspaceId`, `StageAttemptId`, `StageReceiptId`, and `StageQueryId` exist in `leaven-kernel`, serde-roundtrip, and two freshly allocated `Workspace`s have distinct `WorkspaceId`s.
- A factory can attach typed context via `Workspace::new_with_context(...)`, and `WorkspaceSlot::factory_context::<T>()` downcasts to that exact `T` on every slot derived from the workspace.
- A `WorkspaceSlot` rooted at `proposer/x` *cannot* write outside `proposer/x` via any slot method, including paths that would textually resolve outside (parent traversal); these are rejected, not silently scoped.
- `fingerprint_tree` returns the same `Fingerprint` for the same byte content regardless of the order `list_files` happens to return entries; rewriting one byte of one file changes the fingerprint; reading a missing file is `WorkspaceError`, not panic.
- The five named tests in `crates/leaven-workspace/tests/workspace_slot.rs` (`workspace_path_rejects_parent_traversal`, `slot_write_is_scoped_to_slot_root`, `nested_slot_write_is_scoped_to_nested_root`, `slot_list_files_returns_unscoped_paths`, `factory_context_downcasts_when_present`) and the four in `tests/fingerprint.rs` are all green.

**Forbidden proxy proofs:**

- Slot containment is "enforced" by checking that the joined path starts with the slot root as a string. A test passes for `proposer/x/file.txt` but never tries `proposer/x/../../etc/passwd`, `/absolute/path`, or paths whose normalization changes shape after symlink expansion. Containment must be tested adversarially, against the kinds of paths an agent under prompt injection would try to write — not the kinds an honest caller would.
- The path-order-independence test computes `fingerprint_tree` twice without changing anything between calls, sees the same result, and calls it good. The law that matters is "different `list_files` orderings produce identical fingerprints" — exercise it with a test backend that returns entries in two different orders, or shuffle a vector before re-fingerprinting.
- `WorkspaceId` distinctness is satisfied by `Uuid::new_v4()` and one test asserting two new `Workspace`s differ. The real risk is the id being captured into a long-lived structure (a receipt, an event, an in-memory cache) by reference and silently aliasing under cleanup; the test should also assert ids survive `Workspace::cleanup` for receipts that outlive the workspace.
- Factory context "downcasts to the right `T`" is tested by inserting a `JjRepoHandle` and downcasting to `JjRepoHandle`. The test never inserts an unrelated `T` and tries to downcast to `JjRepoHandle` (should be `None`), so a permissive downcast that always returns `Some` for the first non-empty context would pass.
- Fingerprint helpers are deterministic in tests because the local backend writes files synchronously. A future async/eventual-consistency backend (jj over network, lazy commit) silently breaks the law because no test exercises a backend whose `list_files` is non-deterministic between writes and reads. If the law is meant to hold across backends, the contract suite must run against more than one backend.
- "Slot writes are scoped" passes because every slot method internally calls the underlying `WorkspaceView` method, which already applies the prefix. A future slot method (e.g., `run_command` with a custom cwd) bypasses the prefix and the milestone is silently regressed because no test exercises every slot method's containment.

### 3.1 add workspace/stage ids to `leaven-kernel`

File:

```text
crates/leaven-kernel/src/ids.rs
```

Add UUID ids near existing `AgentSessionId` / `CheckpointId`:

```rust
uuid_id!(
    /// Identifier for one allocated workspace instance.
    ///
    /// Workspaces are ephemeral, but receipts and agent sessions need a durable
    /// handle to say which workspace a file/output belonged to.
    WorkspaceId
);

uuid_id!(
    /// Identifier for one optimizer-stage agent attempt.
    ///
    /// A stage attempt may succeed, fail to materialize, time out, or fail to
    /// parse output. This is distinct from proposal ids and apply-attempt ids.
    StageAttemptId
);

uuid_id!(
    /// Identifier for one stage materialization receipt.
    StageReceiptId
);

uuid_id!(
    /// Identifier for one lazy query made from a stage workspace.
    StageQueryId
);
```

File:

```text
crates/leaven-kernel/src/lib.rs
```

Add to public exports:

```rust
pub use ids::{
    // existing ...
    StageAttemptId, StageQueryId, StageReceiptId, WorkspaceId,
};
```

Acceptance tests:

```text
cargo test -p leaven-kernel ids
```

No behavioral tests beyond compile/serde roundtrip are needed.

### 3.2 add `WorkspaceId` and factory context to `Workspace`

File:

```text
crates/leaven-workspace/src/workspace.rs
```

Target struct:

```rust
use std::any::Any;
use leaven_kernel::WorkspaceId;

pub struct Workspace {
    id: WorkspaceId,
    backend: Arc<Mutex<Box<dyn WorkspaceBackend>>>,
    local_mount: Option<PathBuf>,
    factory_context: Option<Arc<dyn Any + Send + Sync>>,
}
```

Target constructors and accessors:

```rust
impl Workspace {
    #[must_use]
    pub fn new(_root: PathBuf, backend: Box<dyn WorkspaceBackend>) -> Self {
        Self::new_with_context(_root, backend, None)
    }

    #[must_use]
    pub fn new_with_context(
        _root: PathBuf,
        backend: Box<dyn WorkspaceBackend>,
        factory_context: Option<Arc<dyn Any + Send + Sync>>,
    ) -> Self {
        let local_mount = backend.local_mount().map(Path::to_path_buf);
        Self {
            id: WorkspaceId::new(),
            backend: Arc::new(Mutex::new(backend)),
            local_mount,
            factory_context,
        }
    }

    #[must_use]
    pub const fn id(&self) -> WorkspaceId {
        self.id
    }

    #[must_use]
    pub fn root(&self) -> WorkspacePath {
        WorkspacePath::root()
    }

    #[must_use]
    pub fn local_mount(&self) -> Option<&Path> {
        self.local_mount.as_deref()
    }

    #[must_use]
    pub fn factory_context<T>(&self) -> Option<&T>
    where
        T: Any + Send + Sync + 'static,
    {
        self.factory_context
            .as_deref()
            .and_then(|context| context.downcast_ref::<T>())
    }

    #[must_use]
    pub fn view(&mut self) -> WorkspaceView<'_> {
        WorkspaceView::from_backend(
            self.backend.clone(),
            self.local_mount.clone(),
            WorkspacePath::root(),
            self.factory_context.clone(),
            PhantomData,
        )
    }

    pub fn slot(&mut self, path: WorkspacePath) -> Result<WorkspaceSlot<'_>, WorkspaceError> {
        let root = path.clone();
        let view = self.view().subdir(path)?;
        Ok(WorkspaceSlot::new(root, view))
    }

    pub async fn cleanup(self) -> Result<(), WorkspaceError> {
        let backend = Arc::try_unwrap(self.backend)
            .map_err(|_| WorkspaceError::Cleanup("workspace views are still live".to_owned()))?
            .into_inner();
        backend.cleanup().await
    }
}
```

Notes:

- Do not add `context()` to `WorkspaceFactory` yet. Let factories choose `Workspace::new` or `Workspace::new_with_context`.
- `LocalWorkspaceFactory` remains unchanged because `Workspace::new` keeps the old signature.
- `JjWorkspaceFactory` later calls `Workspace::new_with_context(root, backend, Some(repo_handle))`.

### 3.3 extend `WorkspaceView` with factory context

File:

```text
crates/leaven-workspace/src/view.rs
```

Target struct:

```rust
use std::any::Any;

pub struct WorkspaceView<'a> {
    backend: Arc<Mutex<Box<dyn WorkspaceBackend>>>,
    local_mount: Option<PathBuf>,
    prefix: WorkspacePath,
    factory_context: Option<Arc<dyn Any + Send + Sync>>,
    marker: PhantomData<&'a mut ()>,
}
```

Target constructor/signature change:

```rust
impl<'a> WorkspaceView<'a> {
    #[must_use]
    pub(crate) fn from_backend(
        backend: Arc<Mutex<Box<dyn WorkspaceBackend>>>,
        local_mount: Option<PathBuf>,
        prefix: WorkspacePath,
        factory_context: Option<Arc<dyn Any + Send + Sync>>,
        marker: PhantomData<&'a mut ()>,
    ) -> Self {
        Self {
            backend,
            local_mount,
            prefix,
            factory_context,
            marker,
        }
    }

    pub fn subdir(&self, path: WorkspacePath) -> Result<Self, WorkspaceError> {
        Ok(Self {
            backend: self.backend.clone(),
            local_mount: self.local_mount.clone(),
            prefix: if self.prefix.as_str().is_empty() {
                path
            } else {
                self.prefix.join(path.as_str())?
            },
            factory_context: self.factory_context.clone(),
            marker: PhantomData,
        })
    }

    #[must_use]
    pub fn factory_context<T>(&self) -> Option<&T>
    where
        T: Any + Send + Sync + 'static,
    {
        self.factory_context
            .as_deref()
            .and_then(|context| context.downcast_ref::<T>())
    }
}
```

### 3.4 add `WorkspaceSlot`

File:

```text
crates/leaven-workspace/src/slot.rs
```

Full target definition:

```rust
//! Scoped workspace slots.

use std::any::Any;

use crate::{
    Command, CommandOutput, WorkspaceError, WorkspacePath, WorkspacePathError, WorkspaceView,
};

/// Scoped workspace view used by materializers.
///
/// A slot gives a materializer a rooted subdirectory rather than the whole
/// workspace. Paths passed to slot methods are relative to the slot root.
pub struct WorkspaceSlot<'a> {
    root: WorkspacePath,
    view: WorkspaceView<'a>,
}

impl<'a> WorkspaceSlot<'a> {
    #[must_use]
    pub(crate) const fn new(root: WorkspacePath, view: WorkspaceView<'a>) -> Self {
        Self { root, view }
    }

    /// Path of this slot root relative to the workspace root.
    #[must_use]
    pub const fn root(&self) -> &WorkspacePath {
        &self.root
    }

    /// Borrow the underlying scoped view.
    ///
    /// Prefer the slot methods when possible; this escape hatch exists for
    /// existing helpers that already accept `WorkspaceView`.
    pub fn view_mut(&mut self) -> &mut WorkspaceView<'a> {
        &mut self.view
    }

    /// Returns a nested slot below this slot.
    ///
    /// # Errors
    ///
    /// Returns [`WorkspaceError`] if `path` cannot be joined under the current
    /// slot root.
    pub fn subslot(&self, path: WorkspacePath) -> Result<WorkspaceSlot<'a>, WorkspaceError> {
        let root = self.root.join(path.as_str())?;
        let view = self.view.subdir(path)?;
        Ok(WorkspaceSlot { root, view })
    }

    /// Write a file at a path relative to this slot.
    pub fn write_file(
        &mut self,
        path: &WorkspacePath,
        bytes: &[u8],
    ) -> Result<(), WorkspaceError> {
        self.view.write_file(path, bytes)
    }

    /// Read a file at a path relative to this slot.
    pub fn read_file(&self, path: &WorkspacePath) -> Result<Vec<u8>, WorkspaceError> {
        self.view.read_file(path)
    }

    /// List files below a path relative to this slot.
    pub fn list_files(&self, path: &WorkspacePath) -> Result<Vec<WorkspacePath>, WorkspaceError> {
        self.view.list_files(path)
    }

    /// Mark a slot-relative path executable or not executable.
    pub fn set_executable(
        &mut self,
        path: &WorkspacePath,
        executable: bool,
    ) -> Result<(), WorkspaceError> {
        self.view.set_executable(path, executable)
    }

    /// Query whether a slot-relative path is executable.
    pub fn is_executable(&self, path: &WorkspacePath) -> Result<bool, WorkspaceError> {
        self.view.is_executable(path)
    }

    /// Run a command with cwd scoped to this slot unless the command sets a
    /// slot-relative cwd explicitly.
    pub fn run_command(&mut self, command: Command) -> Result<CommandOutput, WorkspaceError> {
        self.view.run_command(command)
    }

    /// Access declared factory context, if this workspace factory supplied one.
    #[must_use]
    pub fn factory_context<T>(&self) -> Option<&T>
    where
        T: Any + Send + Sync + 'static,
    {
        self.view.factory_context::<T>()
    }
}
```

File:

```text
crates/leaven-workspace/src/lib.rs
```

Add:

```rust
pub mod slot;
pub use slot::WorkspaceSlot;
```

Prelude add:

```rust
WorkspaceSlot,
```

Tests:

```text
crates/leaven-workspace/tests/workspace_slot.rs
```

Required cases:

```rust
#[test]
fn workspace_path_rejects_parent_traversal() { ... }

#[test]
fn slot_write_is_scoped_to_slot_root() { ... }

#[test]
fn nested_slot_write_is_scoped_to_nested_root() { ... }

#[test]
fn slot_list_files_returns_unscoped_paths() { ... }

#[test]
fn factory_context_downcasts_when_present() { ... }
```

### 3.5 add workspace fingerprint helpers

File:

```text
crates/leaven-workspace/src/fingerprint.rs
```

Full target definition:

```rust
//! Fingerprints for workspace files and trees.

use leaven_kernel::{Fingerprint, FingerprintBuilder};
use serde::{Deserialize, Serialize};

use crate::{WorkspaceError, WorkspacePath, WorkspaceView};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WorkspaceFileFingerprint {
    pub path: WorkspacePath,
    pub fingerprint: Fingerprint,
    pub bytes: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WorkspaceTreeFingerprint {
    pub root: WorkspacePath,
    pub fingerprint: Fingerprint,
    pub files: Vec<WorkspaceFileFingerprint>,
}

pub fn fingerprint_file(
    view: &WorkspaceView<'_>,
    path: &WorkspacePath,
) -> Result<WorkspaceFileFingerprint, WorkspaceError> {
    let bytes = view.read_file(path)?;
    let mut builder = FingerprintBuilder::new();
    builder
        .update(b"leaven.workspace.file.v1")
        .update(path.as_str().as_bytes())
        .update(&(bytes.len() as u64).to_le_bytes())
        .update(&bytes);
    Ok(WorkspaceFileFingerprint {
        path: path.clone(),
        fingerprint: builder.finish(),
        bytes: bytes.len() as u64,
    })
}

pub fn fingerprint_tree(
    view: &WorkspaceView<'_>,
    root: &WorkspacePath,
) -> Result<WorkspaceTreeFingerprint, WorkspaceError> {
    let mut files = view.list_files(root)?;
    files.sort();

    let mut file_fingerprints = Vec::with_capacity(files.len());
    let mut tree_builder = FingerprintBuilder::new();
    tree_builder.update(b"leaven.workspace.tree.v1");

    for path in files {
        let file = fingerprint_file(view, &path)?;
        tree_builder
            .update(file.path.as_str().as_bytes())
            .update(&file.bytes.to_le_bytes())
            .update(file.fingerprint.0);
        file_fingerprints.push(file);
    }

    Ok(WorkspaceTreeFingerprint {
        root: root.clone(),
        fingerprint: tree_builder.finish(),
        files: file_fingerprints,
    })
}
```

File:

```text
crates/leaven-workspace/src/lib.rs
```

Add:

```rust
pub mod fingerprint;
pub use fingerprint::{
    WorkspaceFileFingerprint, WorkspaceTreeFingerprint, fingerprint_file, fingerprint_tree,
};
```

Tests:

```text
crates/leaven-workspace/tests/fingerprint.rs
```

Required cases:

```rust
#[test]
fn same_file_bytes_same_fingerprint() { ... }

#[test]
fn changed_file_bytes_changes_fingerprint() { ... }

#[test]
fn tree_fingerprint_is_path_order_independent() { ... }

#[test]
fn missing_file_is_workspace_error() { ... }
```

## 4. Milestone 3 — engine event/sink prerequisites

This milestone keeps `leaven-stage` from becoming a second engine. It gives proposer-stage code a minimal way to leave stage-attempt breadcrumbs without pretending parse failure is apply failure.

**Done when (no workarounds):**

- A `Proposer<P>` impl can call `ctx.record_stage_attempt(summary)` mid-`propose` and `RunContext::propose` emits exactly one `RunEvent::StageAttempt { iteration, summary }` per recorded summary, in the order they were recorded, on the run event stream that downstream observers consume.
- The sink is drained and emitted *before* `RunContext::propose` returns its `Result`, on both the success and the error path. A proposer that records `Started`/`Materialized`/`Failed(OutputParse)` and then returns `Err(ProposalError)` produces all three events in the run log, not zero.
- For any `Proposer` that records `StageAttemptFailureKind::OutputParse` and returns `ProposalError`, `RunEvent::ApplyFailed` is *not* emitted anywhere in the run log for that stage. Parse failure is its own failure category and never silently routes through the apply pipeline.
- The three named tests in `crates/leaven-engine/tests/stage_attempt_events.rs` (`proposer_can_emit_stage_attempt_event_on_success`, `proposer_can_emit_stage_attempt_event_on_error`, `output_parse_failure_is_not_apply_failed`) are green and the third asserts `ApplyFailed` is *absent*, not just that some other event is present.

**Forbidden proxy proofs:**

- The sink drains via `?` short-circuit on success and via an explicit drain on error — but the explicit drain happens after the error has already been bubbled up the stack (e.g., inside an outer `match`), so events recorded by the proposer immediately before its error return are dropped. The test only inspects events on the success path because writing the error-path test required restructuring the harness; the milestone passes silently broken.
- The "no `ApplyFailed` for parse failure" test asserts `events.iter().filter(|e| matches!(e, RunEvent::StageAttempt { ... })).count() > 0` — true, but vacuous, because nothing in the test would have produced `ApplyFailed` regardless. The assertion that matters is `assert!(events.iter().all(|e| !matches!(e, RunEvent::ApplyFailed { .. })))` against a run that *would* have produced `ApplyFailed` if parse failure were misrouted.
- `StageAttemptSink` is plumbed into `ProposalContext`, but no existing proposer is migrated to use it. The milestone is "the mechanism exists" rather than "the mechanism is used"; downstream agents see the `record_stage_attempt` API and assume something else is responsible for emitting events.
- `StageAttemptFailureKind::OutputParse` is added but `ProposalError` continues to expose a generic `Parse(...)` variant that callers route through `ApplyFailed` for compatibility. The two failure surfaces both exist; the new one is decorative.
- Sink ordering is "preserved" because `Vec::push` and `mem::take` are FIFO in practice, but no test asserts ordering. A future change that swaps the sink for a hash-keyed map silently loses event order, and dependent UIs (replay tools, debug viewers) misrender stage history.
- The summary's `cost: Cost::zero()` is recorded for failure variants because "failed work has no cost." This loses the truth that a failed materialize or runtime call did consume budget; closeout reports show free failures and the budget enforcement that the engine relies on becomes a soft suggestion.

### 4.1 add generic stage-attempt events

File:

```text
crates/leaven-engine/src/events.rs
```

Add imports:

```rust
use leaven_kernel::{StageAttemptId, StageReceiptId, WorkspaceId};
```

Add these types near other event helper structs:

```rust
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum StageAttemptStatus {
    Started,
    Materialized,
    RuntimeStarted,
    RuntimeCompleted,
    OutputParsed,
    Failed(StageAttemptFailureKind),
    Completed,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum StageAttemptFailureKind {
    WorkspaceAllocate,
    Materialize,
    Runtime,
    RuntimeTimeout,
    OutputContract,
    OutputParse,
    Cleanup,
    StageAndCleanup,
    Budget,
    Other(String),
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct StageAttemptSummary {
    pub attempt_id: StageAttemptId,
    pub stage: StageId,
    pub role: Option<String>,
    pub workspace_id: Option<WorkspaceId>,
    pub receipt_id: Option<StageReceiptId>,
    pub status: StageAttemptStatus,
    pub cost: Cost,
}
```

Add `RunEvent` variant:

```rust
StageAttempt {
    iteration: Option<IterationId>,
    summary: StageAttemptSummary,
},
```

Full insertion point: after `IterationEnded` or before `ProposalBatchProduced`.

### 4.2 add a stage-attempt event sink to `ProposalContext`

File:

```text
crates/leaven-engine/src/context/proposal_context.rs
```

Add:

```rust
use std::sync::Arc;
use parking_lot::Mutex;
use crate::StageAttemptSummary;
```

Definition:

```rust
#[derive(Clone, Default)]
pub struct StageAttemptSink {
    inner: Arc<Mutex<Vec<StageAttemptSummary>>>,
}

impl StageAttemptSink {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&self, summary: StageAttemptSummary) {
        self.inner.lock().push(summary);
    }

    pub(crate) fn drain(&self) -> Vec<StageAttemptSummary> {
        std::mem::take(&mut *self.inner.lock())
    }
}
```

Update `ProposalContext`:

```rust
pub struct ProposalContext<'a, P: OptimizationProblem> {
    graph: RunGraphView<'a, P>,
    budget: BudgetHandle<'a>,
    read_scope: ReadScope,
    stage_attempt_sink: StageAttemptSink,
}
```

Update constructor:

```rust
pub(crate) fn new(
    graph: RunGraphView<'a, P>,
    budget: BudgetHandle<'a>,
    read_scope: ReadScope,
    stage_attempt_sink: StageAttemptSink,
) -> Self {
    Self {
        graph,
        budget,
        read_scope,
        stage_attempt_sink,
    }
}
```

Add accessors:

```rust
#[must_use]
pub fn stage_attempt_sink(&self) -> StageAttemptSink {
    self.stage_attempt_sink.clone()
}

pub fn record_stage_attempt(&self, summary: StageAttemptSummary) {
    self.stage_attempt_sink.push(summary);
}
```

### 4.3 drain sink in `RunContext::propose`

File:

```text
crates/leaven-engine/src/context/run_context.rs
```

Find `proposal_context(...)`. It likely currently returns `ProposalContext::new(graph, budget, read_scope)`. Change it to allocate a sink and expose a helper that returns both.

Target internal helper:

```rust
fn proposal_context_with_sink(
    &mut self,
    stage: StageId,
) -> (ProposalContext<'_, P>, StageAttemptSink) {
    let sink = StageAttemptSink::new();
    let ctx = ProposalContext::new(
        self.graph.view(self.read_scope.clone()),
        self.budget.sub_stage(stage),
        self.read_scope.clone(),
        sink.clone(),
    );
    (ctx, sink)
}
```

The exact `self.graph.view(...)` call may differ; preserve the current graph-view construction.

Update `propose`:

```rust
pub async fn propose<T>(
    &mut self,
    proposer: &T,
    request: T::Request,
) -> Result<ProposalBatchReport, RunContextError>
where
    T: Proposer<P>,
{
    let stage = StageId::from_proposer(proposer.id());
    let (proposal_ctx, sink) = self.proposal_context_with_sink(stage.clone());
    let metered = proposer
        .propose(request, proposal_ctx)
        .await
        .inspect_err(|err| {
            self.emit_stage_error(Some(stage.clone()), ErrorKind::Proposal, err);
        })?;

    for summary in sink.drain() {
        self.emit(RunEvent::StageAttempt {
            iteration: self.iteration,
            summary,
        });
    }

    self.record_proposal_batch(stage, metered.value, metered.cost)
}
```

Important: if `proposer.propose(...)` errors, the code above will not drain the sink. To preserve failed parse/materialize records, use an explicit match:

```rust
let result = proposer.propose(request, proposal_ctx).await;
for summary in sink.drain() {
    self.emit(RunEvent::StageAttempt {
        iteration: self.iteration,
        summary,
    });
}
let metered = result.inspect_err(|err| {
    self.emit_stage_error(Some(stage.clone()), ErrorKind::Proposal, err);
})?;
self.record_proposal_batch(stage, metered.value, metered.cost)
```

Acceptance tests:

```text
crates/leaven-engine/tests/stage_attempt_events.rs
```

Required cases:

```rust
#[tokio::test]
async fn proposer_can_emit_stage_attempt_event_on_success() { ... }

#[tokio::test]
async fn proposer_can_emit_stage_attempt_event_on_error() { ... }

#[tokio::test]
async fn output_parse_failure_is_not_apply_failed() { ... }
```

The third test can use a dummy proposer that records `StageAttemptFailureKind::OutputParse` and returns `ProposalError`. Assert no `RunEvent::ApplyFailed` exists.

## 5. Milestone 4 — create `leaven-stage` crate skeleton

(This milestone spans sections 5 and 6 — the skeleton/manifest here, the concrete data definitions in section 6. The behavioral block below covers both halves; do not declare Milestone 4 done until section 6's types are in place and exercised.)

**Done when (no workarounds):**

- `cargo check -p leaven-stage` compiles the crate in isolation. `cargo metadata --format-version 1 -p leaven-stage --no-deps | jq '.packages[].dependencies[].name'` lists no dependency named `leaven-gepa` or `leaven-agentic`, directly or transitively. The crate sits below them in the dependency DAG, not beside them.
- Every type listed in `lib.rs` (ids, role, media, plan, output, receipt, source, errors) round-trips through `serde_json` for at least one realistic value. The roundtrip is byte-identical for canonical encodings (deterministic field order) and structurally identical otherwise — round-tripping is not satisfied by "deserializing the serialized output succeeds" alone; the value compared back must equal the original.
- `StageOutputContract::validate` rejects every required or optional entry whose path does not live under `output/`, including paths that *start* with `output/` but escape it (`output/../foo`, `output//../foo`). The validator is invoked everywhere a contract enters the system; an invalid contract cannot reach `materialize_stage_workspace`.
- `OutputRole`, `MaterializationRole`, and `StageRole` carry typed variants for every concept named in the governing specs. The `Other(String)` variants exist as escape hatches for genuinely unanticipated cases, not as the primary path used by the stage layer's own callers.
- `ParseFailurePolicy::Strict` and `ParseFailurePolicy::RecordAttempt` both compile and are exercised by at least one test (or are explicitly marked unused-and-deferred in the docs of `agent_backed.rs` so a future implementer knows the behavioral gap).

**Forbidden proxy proofs:**

- `cargo check -p leaven-stage` passes because `[dev-dependencies]` pulls in `leaven-engine` which transitively pulls in `leaven-gepa` (or `leaven-agentic`). The "no dependency" claim only holds at `[dependencies]`; the crate's tests still tangle the graph and any downstream crate that consumes `leaven-stage`'s tests inherits the tangle. Run the metadata check on the full dependency tree, not just direct deps.
- Serde roundtrip "passes" because the offending fields are wrapped in `#[serde(skip)]` (or `#[serde(default)]` with a fallback that hides drift). The roundtrip then trivially succeeds because the troublesome data was thrown away. If a field is hard to serialize, write the harder serializer or move the field out of the serializable type — do not redact it from the roundtrip.
- `StageOutputContract::validate` rejects paths via `path.as_str().starts_with("output/")`. `output/../escape.txt` passes because it textually starts with `output/`. The validator must operate on canonicalized `WorkspacePath` segments, not on raw strings, and the tests must include an attempted escape.
- `OutputRole::Other(String)` becomes the primary variant used by every Leaven-internal stage caller because it's the path of least resistance. The typed variants exist but never fire; downstream consumers cannot pattern-match on stage roles meaningfully. The milestone is structurally complete and operationally untyped.
- The crate compiles in isolation but `pub use` re-exports types from `leaven-engine` that downstream consumers would have to import anyway. The "no gepa/agentic dependency" claim is true literally and false in spirit because every consumer of `leaven-stage` ends up importing `leaven-engine` items via the stage prelude, which means the stage crate has not actually achieved a useful boundary.
- `[dev-dependencies]` includes `leaven-workspace-local` and the integration tests run against a hardcoded `LocalWorkspaceFactory`. The crate is "factory-agnostic" in production but every test pins one factory, so the abstraction breaks the moment a second factory exists.

### 5.1 workspace `Cargo.toml`

Root `Cargo.toml`, add member:

```toml
"crates/leaven-stage",
```

Add workspace dependency:

```toml
leaven-stage = { path = "crates/leaven-stage", version = "0.0.0" }
```

### 5.2 crate `Cargo.toml`

File:

```text
crates/leaven-stage/Cargo.toml
```

Full file:

```toml
[package]
name = "leaven-stage"
description = "Optimizer-stage agent workspace materialization for leaven."
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
repository.workspace = true

[lints]
workspace = true

[dependencies]
leaven-agent = { workspace = true }
leaven-core = { workspace = true }
leaven-engine = { workspace = true }
leaven-kernel = { workspace = true }
leaven-store = { workspace = true }
leaven-workspace = { workspace = true }
futures = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
smol_str = { workspace = true }
thiserror = { workspace = true }

[dev-dependencies]
leaven-agent = { workspace = true }
leaven-store-inline = { workspace = true }
leaven-workspace-local = { workspace = true }
```

### 5.3 module layout

Files:

```text
crates/leaven-stage/src/lib.rs
crates/leaven-stage/src/id.rs
crates/leaven-stage/src/error.rs
crates/leaven-stage/src/role.rs
crates/leaven-stage/src/media.rs
crates/leaven-stage/src/output.rs
crates/leaven-stage/src/plan.rs
crates/leaven-stage/src/receipt.rs
crates/leaven-stage/src/source.rs
crates/leaven-stage/src/materialize.rs
crates/leaven-stage/src/parser.rs
crates/leaven-stage/src/bootstrap.rs
crates/leaven-stage/src/artifact.rs
crates/leaven-stage/src/agent_backed.rs
crates/leaven-stage/src/slots.rs
```

`lib.rs`:

```rust
//! Optimizer-stage agent workspace materialization.
//!
//! This crate is for Leaven-owned optimizer stages, not user task-package
//! harnesses. It depends on `leaven-engine`; `leaven-engine` must not depend on
//! this crate.

pub mod agent_backed;
pub mod artifact;
pub mod bootstrap;
pub mod error;
pub mod id;
pub mod materialize;
pub mod media;
pub mod output;
pub mod parser;
pub mod plan;
pub mod receipt;
pub mod role;
pub mod slots;
pub mod source;

pub use agent_backed::{AgentBacked, AgentBackedConfig, AgentBackedPolicy, ParseFailurePolicy};
pub use artifact::{
    ArtifactMaterializationError, ArtifactReadbackError, MaterializableArtifact,
    ReconstructibleArtifact,
};
pub use bootstrap::{AgentStageBootstrap, StageBootstrapContext};
pub use error::{StageError, StageMaterializeError, StageParseError};
pub use id::{MaterializationEntryId, OutputEntryId};
pub use materialize::{materialize_stage_workspace, StageMaterializationInput};
pub use media::MediaType;
pub use output::{
    OutputRole, StageOutputContract, StageOutputEntry, StageOutputSchema,
};
pub use parser::{StageOutputParseInput, StageOutputParser};
pub use plan::{
    AccessMode, AgentStagePlan, GeneratedContent, MaterializationEntry,
    MaterializationRole, MaterializationTarget, QueryPolicy, StageDirective,
};
pub use receipt::{
    MaterializedEntryReceipt, OutputEntryReceipt, ParseReceipt, ParseStatus,
    QueryRecord, ReadScopeDigest, StageReceipt, StageReceiptStatus,
    StageSourceRef, VisibilityViolation,
};
pub use role::StageRole;
pub use slots::ProposerSlot;
pub use source::{AssessmentSnapshot, ScopedStageSource, StageSourceError};

pub mod prelude {
    pub use crate::{
        AgentBacked, AgentBackedConfig, AgentBackedPolicy, AgentStageBootstrap,
        AgentStagePlan, MaterializableArtifact, OutputRole, ParseFailurePolicy,
        ProposerSlot, StageDirective, StageOutputContract, StageOutputEntry,
        StageOutputParser, StageRole,
    };
}
```

## 6. Milestone 4 (continued) — concrete `leaven-stage` data definitions

The behavioral block for Milestone 4 is at section 5. This section gives the concrete type bodies that must be present before Milestone 4 can be claimed complete.

### 6.1 ids

File:

```text
crates/leaven-stage/src/id.rs
```

Full definition:

```rust
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MaterializationEntryId(SmolStr);

impl MaterializationEntryId {
    pub fn new(value: impl Into<SmolStr>) -> Result<Self, crate::StageError> {
        let value = value.into();
        if value.is_empty() {
            return Err(crate::StageError::InvalidId("materialization entry id"));
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn new_static(value: &'static str) -> Self {
        Self::new(SmolStr::new_static(value)).expect("static id must be non-empty")
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct OutputEntryId(SmolStr);

impl OutputEntryId {
    pub fn new(value: impl Into<SmolStr>) -> Result<Self, crate::StageError> {
        let value = value.into();
        if value.is_empty() {
            return Err(crate::StageError::InvalidId("output entry id"));
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn new_static(value: &'static str) -> Self {
        Self::new(SmolStr::new_static(value)).expect("static id must be non-empty")
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}
```

### 6.2 errors

File:

```text
crates/leaven-stage/src/error.rs
```

Full definition:

```rust
use leaven_agent::AgentRuntimeError;
use leaven_engine::ProposalError;
use leaven_workspace::{FactoryError, WorkspaceError, WorkspacePathError};

#[derive(Debug, thiserror::Error)]
pub enum StageError {
    #[error("invalid {0}")]
    InvalidId(&'static str),

    #[error("stage plan is invalid: {0}")]
    InvalidPlan(String),

    #[error(transparent)]
    Materialize(#[from] StageMaterializeError),

    #[error(transparent)]
    Parse(#[from] StageParseError),
}

#[derive(Debug, thiserror::Error)]
pub enum StageMaterializeError {
    #[error(transparent)]
    Workspace(#[from] WorkspaceError),

    #[error(transparent)]
    Path(#[from] WorkspacePathError),

    #[error("workspace allocation failed")]
    Allocate(#[from] FactoryError),

    #[error("source lookup failed: {0}")]
    Source(String),

    #[error("artifact materialization failed: {0}")]
    Artifact(String),

    #[error("serialization failed: {0}")]
    Serialize(#[from] serde_json::Error),

    #[error("materialization budget exceeded: {0}")]
    Budget(String),
}

#[derive(Debug, thiserror::Error)]
pub enum StageParseError {
    #[error(transparent)]
    Workspace(#[from] WorkspaceError),

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[error("required output missing: {0}")]
    MissingRequiredOutput(String),

    #[error("malformed output at {path}: {message}")]
    Malformed { path: String, message: String },

    #[error("stage output parse failed: {0}")]
    Message(String),

    #[error("stage output parse failed: {message}")]
    WithSource {
        message: String,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync + 'static>,
    },
}

impl StageParseError {
    #[must_use]
    pub fn with_source(
        message: impl Into<String>,
        source: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        Self::WithSource {
            message: message.into(),
            source: Box::new(source),
        }
    }
}

impl From<StageParseError> for ProposalError {
    fn from(source: StageParseError) -> Self {
        ProposalError::with_source("agent-backed stage output parse failed", source)
    }
}

impl From<AgentRuntimeError> for StageError {
    fn from(source: AgentRuntimeError) -> Self {
        StageError::InvalidPlan(format!("agent runtime failed before stage boundary: {source}"))
    }
}
```

Note: the `From<AgentRuntimeError>` impl is intentionally weak. In `AgentBacked`, runtime failures should usually become `ProposalError::with_source(...)` directly so the failure kind can be recorded first.

### 6.3 role

File:

```text
crates/leaven-stage/src/role.rs
```

Full definition:

```rust
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StageRole(SmolStr);

impl StageRole {
    pub fn new(value: impl Into<SmolStr>) -> Result<Self, crate::StageError> {
        let value = value.into();
        if value.is_empty() {
            return Err(crate::StageError::InvalidId("stage role"));
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn new_static(value: &'static str) -> Self {
        Self::new(SmolStr::new_static(value)).expect("static stage role must be non-empty")
    }

    #[must_use]
    pub fn reflect() -> Self {
        Self::new_static("reflect")
    }

    #[must_use]
    pub fn select_parent() -> Self {
        Self::new_static("select_parent")
    }

    #[must_use]
    pub fn select_part() -> Self {
        Self::new_static("select_part")
    }

    #[must_use]
    pub fn sample_batch() -> Self {
        Self::new_static("sample_batch")
    }

    #[must_use]
    pub fn accept() -> Self {
        Self::new_static("accept")
    }

    #[must_use]
    pub fn merge() -> Self {
        Self::new_static("merge")
    }

    #[must_use]
    pub fn resolve_conflicts() -> Self {
        Self::new_static("resolve_conflicts")
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl std::fmt::Display for StageRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}
```

Law:

```text
StageRole is metadata. It is not parser dispatch authority.
```

### 6.4 media type

File:

```text
crates/leaven-stage/src/media.rs
```

Full definition:

```rust
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MediaType(SmolStr);

impl MediaType {
    pub fn new(value: impl Into<SmolStr>) -> Result<Self, crate::StageError> {
        let value = value.into();
        if value.is_empty() {
            return Err(crate::StageError::InvalidId("media type"));
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn text_plain() -> Self {
        Self::new(SmolStr::new_static("text/plain")).expect("static media type")
    }

    #[must_use]
    pub fn markdown() -> Self {
        Self::new(SmolStr::new_static("text/markdown")).expect("static media type")
    }

    #[must_use]
    pub fn json() -> Self {
        Self::new(SmolStr::new_static("application/json")).expect("static media type")
    }

    #[must_use]
    pub fn octet_stream() -> Self {
        Self::new(SmolStr::new_static("application/octet-stream")).expect("static media type")
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}
```

### 6.5 output contract

File:

```text
crates/leaven-stage/src/output.rs
```

Full definition:

```rust
use leaven_agent::JsonSchemaRef;
use leaven_workspace::WorkspacePath;
use serde::{Deserialize, Serialize};

use crate::{MediaType, OutputEntryId, StageError};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct StageOutputContract {
    pub required: Vec<StageOutputEntry>,
    pub optional: Vec<StageOutputEntry>,
}

impl StageOutputContract {
    #[must_use]
    pub fn new(required: Vec<StageOutputEntry>) -> Self {
        Self {
            required,
            optional: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_optional(mut self, optional: Vec<StageOutputEntry>) -> Self {
        self.optional = optional;
        self
    }

    pub fn validate(&self) -> Result<(), StageError> {
        if self.required.is_empty() {
            return Err(StageError::InvalidPlan(
                "stage output contract must require at least one output".to_owned(),
            ));
        }
        for entry in self.required.iter().chain(self.optional.iter()) {
            if !entry.path.as_str().starts_with("output/") {
                return Err(StageError::InvalidPlan(format!(
                    "stage output `{}` must live under output/; got `{}`",
                    entry.id.as_str(),
                    entry.path.as_str()
                )));
            }
        }
        Ok(())
    }

    #[must_use]
    pub fn all_entries(&self) -> impl Iterator<Item = &StageOutputEntry> {
        self.required.iter().chain(self.optional.iter())
    }

    #[must_use]
    pub fn to_agent_contract(&self) -> leaven_agent::OutputContract {
        if self.required.len() == 1 && self.optional.is_empty() {
            let entry = &self.required[0];
            if entry.media_type == MediaType::json() {
                return leaven_agent::OutputContract::JsonFile {
                    path: entry.path.clone(),
                    schema: entry.schema.as_ref().and_then(|schema| schema.json_schema.clone()),
                };
            }
        }
        leaven_agent::OutputContract::Files {
            paths: self.required.iter().map(|entry| entry.path.clone()).collect(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct StageOutputEntry {
    pub id: OutputEntryId,
    pub path: WorkspacePath,
    pub role: OutputRole,
    pub media_type: MediaType,
    pub schema: Option<StageOutputSchema>,
    pub description: Option<String>,
}

impl StageOutputEntry {
    pub fn json(
        id: OutputEntryId,
        path: WorkspacePath,
        role: OutputRole,
        schema: Option<StageOutputSchema>,
    ) -> Self {
        Self {
            id,
            path,
            role,
            media_type: MediaType::json(),
            schema,
            description: None,
        }
    }

    pub fn markdown(id: OutputEntryId, path: WorkspacePath, role: OutputRole) -> Self {
        Self {
            id,
            path,
            role,
            media_type: MediaType::markdown(),
            schema: None,
            description: None,
        }
    }

    #[must_use]
    pub fn described(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum OutputRole {
    ProposalBatch,
    CandidateSelection,
    PartSelection,
    AcceptanceDecision,
    MergeDecision,
    RepairDecision,
    Notes,
    Transcript,
    Diagnostics,
    Other(String),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct StageOutputSchema {
    pub name: String,
    pub json_schema: Option<JsonSchemaRef>,
    pub prose: Option<String>,
}
```

Important: no parser ref appears here. The typed parser belongs to `AgentBacked`.

### 6.6 plan

File:

```text
crates/leaven-stage/src/plan.rs
```

Full definition:

```rust
use leaven_kernel::{AssessmentId, CandidateId, EvidenceRef, ProposalId};
use leaven_workspace::WorkspacePath;
use serde::{Deserialize, Serialize};

use crate::{
    MaterializationEntryId, MediaType, StageOutputContract, StageRole,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentStagePlan<Req> {
    pub role: StageRole,
    pub request: Req,
    pub directive: StageDirective,
    pub output: StageOutputContract,
    pub eager: Vec<MaterializationEntry>,
    pub query: QueryPolicy,
}

impl<Req> AgentStagePlan<Req> {
    #[must_use]
    pub fn new(
        role: StageRole,
        request: Req,
        directive: StageDirective,
        output: StageOutputContract,
    ) -> Self {
        Self {
            role,
            request,
            directive,
            output,
            eager: Vec::new(),
            query: QueryPolicy::default(),
        }
    }

    #[must_use]
    pub fn with_eager(mut self, eager: Vec<MaterializationEntry>) -> Self {
        self.eager = eager;
        self
    }

    #[must_use]
    pub fn with_query_policy(mut self, query: QueryPolicy) -> Self {
        self.query = query;
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct StageDirective {
    pub instructions: String,
    pub system: Option<String>,
    pub brief_title: Option<String>,
}

impl StageDirective {
    #[must_use]
    pub fn new(instructions: impl Into<String>) -> Self {
        Self {
            instructions: instructions.into(),
            system: None,
            brief_title: None,
        }
    }

    #[must_use]
    pub fn with_system(mut self, system: impl Into<String>) -> Self {
        self.system = Some(system.into());
        self
    }

    #[must_use]
    pub fn with_brief_title(mut self, title: impl Into<String>) -> Self {
        self.brief_title = Some(title.into());
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct QueryPolicy {
    pub enabled: bool,
    pub max_queries: usize,
    pub max_materialized_bytes: u64,
    pub allow_candidate_artifact: bool,
    pub allow_assessment: bool,
    pub allow_lineage: bool,
}

impl Default for QueryPolicy {
    fn default() -> Self {
        Self {
            enabled: false,
            max_queries: 0,
            max_materialized_bytes: 0,
            allow_candidate_artifact: false,
            allow_assessment: false,
            allow_lineage: false,
        }
    }
}

impl QueryPolicy {
    #[must_use]
    pub fn minimal_enabled() -> Self {
        Self {
            enabled: true,
            max_queries: 16,
            max_materialized_bytes: 8 * 1024 * 1024,
            allow_candidate_artifact: true,
            allow_assessment: true,
            allow_lineage: true,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MaterializationEntry {
    pub id: MaterializationEntryId,
    pub role: MaterializationRole,
    pub target: MaterializationTarget,
    pub access: AccessMode,
    pub path: WorkspacePath,
    pub media_type: Option<MediaType>,
    pub source: MaterializationSource,
}

impl MaterializationEntry {
    pub fn generated_text(
        id: MaterializationEntryId,
        role: MaterializationRole,
        path: WorkspacePath,
        text: impl Into<String>,
        media_type: MediaType,
    ) -> Self {
        Self {
            id,
            role,
            target: MaterializationTarget::AgentWorkspace,
            access: AccessMode::ReadOnly,
            path,
            media_type: Some(media_type),
            source: MaterializationSource::Generated(GeneratedContent::Utf8(text.into())),
        }
    }

    pub fn candidate_artifact(
        id: MaterializationEntryId,
        candidate: CandidateId,
        path: WorkspacePath,
    ) -> Self {
        Self {
            id,
            role: MaterializationRole::CandidateArtifact,
            target: MaterializationTarget::AgentWorkspace,
            access: AccessMode::ReadOnly,
            path,
            media_type: None,
            source: MaterializationSource::CandidateArtifact { candidate },
        }
    }

    pub fn assessment_record(
        id: MaterializationEntryId,
        assessment: AssessmentId,
        path: WorkspacePath,
    ) -> Self {
        Self {
            id,
            role: MaterializationRole::AssessmentRecord,
            target: MaterializationTarget::AgentWorkspace,
            access: AccessMode::ReadOnly,
            path,
            media_type: Some(MediaType::json()),
            source: MaterializationSource::AssessmentRecord { assessment },
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum MaterializationRole {
    Brief,
    FocusRequest,
    FocusInstructions,
    CandidateArtifact,
    SelectedPart,
    SelectedFeedback,
    AssessmentRecord,
    EvidenceSummary,
    TraceExcerpt,
    LineageSummary,
    GraphIndex,
    Tool,
    OutputSchema,
    RuntimeConfig,
    Other(String),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum MaterializationTarget {
    AgentWorkspace,
    ReceiptOnly,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum AccessMode {
    ReadOnly,
    ReadWrite,
    Execute,
    OutputOnly,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum MaterializationSource {
    Generated(GeneratedContent),
    CandidateArtifact { candidate: CandidateId },
    AssessmentRecord { assessment: AssessmentId },
    EvidenceRef { evidence: EvidenceRef },
    LineageSummary { candidate: CandidateId, depth: usize },
    ProposalRecord { proposal: ProposalId },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum GeneratedContent {
    Utf8(String),
    Json(serde_json::Value),
    Bytes(Vec<u8>),
}
```

Initial implementation should support these sources:

```text
Generated
CandidateArtifact
AssessmentRecord
LineageSummary
```

`EvidenceRef` can be receipt-only until evidence rendering policy is explicit.

### 6.7 receipt

File:

```text
crates/leaven-stage/src/receipt.rs
```

Full definition:

```rust
use leaven_core::{ExternalRef, InfoRef};
use leaven_engine::{EvidenceVisibility, ReadScope};
use leaven_kernel::{
    AssessmentId, CandidateId, Cost, EvaluationRequestId, EvidenceRef, Fingerprint,
    ProposalId, StageAttemptId, StageQueryId, StageReceiptId, WorkspaceId,
};
use leaven_workspace::{WorkspaceFileFingerprint, WorkspacePath};
use serde::{Deserialize, Serialize};

use crate::{
    AccessMode, MaterializationEntryId, MaterializationRole, MaterializationTarget,
    OutputEntryId, OutputRole, StageRole,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StageReceipt {
    pub id: StageReceiptId,
    pub attempt_id: StageAttemptId,
    pub workspace_id: WorkspaceId,
    pub role: StageRole,
    pub status: StageReceiptStatus,
    pub read_scope: ReadScopeDigest,
    pub eager_materialization: Vec<MaterializedEntryReceipt>,
    pub lazy_materialization: Vec<MaterializedEntryReceipt>,
    pub queries: Vec<QueryRecord>,
    pub outputs: Vec<OutputEntryReceipt>,
    pub parse: Option<ParseReceipt>,
    pub visibility_violations: Vec<VisibilityViolation>,
    pub cost: Cost,
}

impl StageReceipt {
    #[must_use]
    pub fn new(
        attempt_id: StageAttemptId,
        workspace_id: WorkspaceId,
        role: StageRole,
        read_scope: ReadScopeDigest,
    ) -> Self {
        Self {
            id: StageReceiptId::new(),
            attempt_id,
            workspace_id,
            role,
            status: StageReceiptStatus::Started,
            read_scope,
            eager_materialization: Vec::new(),
            lazy_materialization: Vec::new(),
            queries: Vec::new(),
            outputs: Vec::new(),
            parse: None,
            visibility_violations: Vec::new(),
            cost: Cost::zero(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum StageReceiptStatus {
    Started,
    Materialized,
    RuntimeCompleted,
    Parsed,
    Failed { reason: String },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ReadScopeDigest {
    pub hidden_partition_count: usize,
    pub hidden_partition_fingerprint: Fingerprint,
    pub visible_evidence: EvidenceVisibility,
}

impl ReadScopeDigest {
    #[must_use]
    pub fn from_read_scope(read_scope: &ReadScope) -> Self {
        let mut builder = leaven_kernel::FingerprintBuilder::new();
        builder.update(b"leaven.stage.read-scope.v1");
        for partition in &read_scope.hidden_partitions {
            builder.update(partition.as_str().as_bytes());
        }
        Self {
            hidden_partition_count: read_scope.hidden_partitions.len(),
            hidden_partition_fingerprint: builder.finish(),
            visible_evidence: read_scope.visible_evidence,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MaterializedEntryReceipt {
    pub id: MaterializationEntryId,
    pub role: MaterializationRole,
    pub path: Option<WorkspacePath>,
    pub target: MaterializationTarget,
    pub access: AccessMode,
    pub source: StageSourceRef,
    pub file: Option<WorkspaceFileFingerprint>,
    pub produced_by_query: Option<StageQueryId>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum StageSourceRef {
    Generated { label: String },
    Candidate(CandidateId),
    CandidateArtifact(CandidateId),
    Assessment(AssessmentId),
    Evidence(EvidenceRef),
    Proposal(ProposalId),
    EvaluationRequest(EvaluationRequestId),
    Info(InfoRef),
    External(ExternalRef),
    FactoryContext { type_name: String },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QueryRecord {
    pub id: StageQueryId,
    pub command: String,
    pub status: QueryStatus,
    pub materialized: Vec<MaterializationEntryId>,
    pub cost: Cost,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum QueryStatus {
    Succeeded,
    NotVisible,
    NotFound,
    DeniedByPolicy,
    Failed { reason: String },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OutputEntryReceipt {
    pub id: OutputEntryId,
    pub role: OutputRole,
    pub path: WorkspacePath,
    pub file: Option<WorkspaceFileFingerprint>,
    pub parse_status: ParseStatus,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ParseReceipt {
    pub status: ParseStatus,
    pub diagnostics: Vec<ParseDiagnostic>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ParseStatus {
    NotParsed,
    Parsed,
    MissingRequiredOutput,
    Malformed,
    PolicyDenied,
    Failed,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ParseDiagnostic {
    pub severity: DiagnosticSeverity,
    pub message: String,
    pub path: Option<WorkspacePath>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum DiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct VisibilityViolation {
    pub source: StageSourceRef,
    pub reason: String,
}
```

Note: `PartitionId::as_str()` exists in `leaven-core`? If not, implement the digest by serializing the hidden partition set into JSON bytes. The important point is deterministic digest, not the exact helper.

## 7. Milestone 5 — scoped source and materialization implementation

**Done when (no workarounds):**

- Given a valid `AgentStagePlan`, `materialize_stage_workspace` writes `BRIEF.md`, `focus/stage_role.txt`, `focus/request.json`, `focus/instructions.md`, and `.leaven/output_contract.json` under the workspace root, and *only* those files (plus declared eager entries). Each is recorded in `receipt.eager_materialization` with a `WorkspaceFileFingerprint` computed from the bytes actually persisted, not from the bytes we intended to write.
- A `MaterializationSource::CandidateArtifact { candidate }` for a candidate that is not visible under the current `ReadScope` fails with `StageMaterializeError::Source` *before* any byte hits the workspace. The workspace is left empty (or with only the pre-existing generated files), the receipt records no entry for the invisible candidate, and the failure preserves enough context to identify the candidate id and the read-scope reason.
- An invalid output contract — empty `required`, a path outside `output/`, a path containing parent traversal — is rejected by `StageOutputContract::validate` before any file write occurs. The error names the offending entry id and path, not just "invalid plan."
- A receipt produced by a successful materialize, serialized to JSON, then deserialized, equals the original. Receipts are the durable proof of what the agent saw; they must survive serde without information loss.
- The four named tests in `crates/leaven-stage/tests/materialize_minimal.rs` are green, and each adversarially probes one invariant rather than just exercising the happy path.

**Forbidden proxy proofs:**

- The materializer writes the four required files and the test asserts they exist — but the test does not assert they are the *only* files written. A future change adds a `.leaven/debug.log` or a partial intermediate write, the test silently passes, and receipts no longer match what's actually on disk. The test must enumerate the post-materialize file set and compare it to the expected set.
- Fingerprints are computed from the `bytes: &[u8]` argument to `write_file`, not from a re-read after the write. If the workspace backend is lossy (truncation, encoding normalization, CRLF rewriting on Windows), the fingerprint records what we intended to persist, not what an agent will see. The fingerprint must be computed from `view.read_file(path)?` after the write.
- "Invisible candidate fails" is satisfied by a test where the candidate id is one that has never existed in the graph at all. That exercises the not-found path, not the visibility path. The test must use a candidate that exists *but is hidden by the current read scope*, and the milestone fails if the same candidate becomes visible by widening the scope.
- The output-contract validator runs but only inside `materialize_stage_workspace`; bootstraps that build plans and inspect them before calling materialize see invalid contracts as valid. The validator must run at every entry point — bootstrap return, materialize entry, and any future plan-rewriting helper — so a downstream agent cannot route around it.
- `render_brief` is asserted to produce non-empty markdown and the test stops there. The brief is the agent's first read; if it omits the workspace layout, the output contract, or the "hidden data is not available" line, the agent will improvise. The test must assert the brief contains the specific anchors an agent relies on, not just that it is non-empty.
- Generated files like `focus/request.json` are written via `serde_json::to_vec(&request)` (compact). The agent then can't diff its inputs across runs because formatting is non-canonical. Use `to_vec_pretty` (as the plan body shows) and assert canonical formatting in tests, otherwise replay/debug tools degrade silently.
- The receipt records the eager materialization correctly but the materializer mutates `receipt` by pushing to `eager_materialization` after the file write, with no rollback if a later entry fails. A failure mid-materialization leaves a partial workspace AND a partial receipt that claims more than what's on disk. Either materialize transactionally or record the receipt only after all writes succeed.

### 7.1 scoped source

File:

```text
crates/leaven-stage/src/source.rs
```

Full definition:

```rust
use leaven_core::{AssessmentTarget, OptimizationProblem};
use leaven_engine::{AssessmentView, ReadScope, RunGraphView};
use leaven_kernel::{
    AssessmentId, CandidateId, EvaluationRequestId, EvaluatorId, EvidenceRef, Timestamp,
};
use leaven_store::EvidenceStore;
use serde::{Deserialize, Serialize};

#[derive(Clone)]
pub struct ScopedStageSource<'a, P: OptimizationProblem> {
    graph: RunGraphView<'a, P>,
    read_scope: ReadScope,
    evidence_store: Option<&'a dyn EvidenceStore<P::Evidence>>,
}

impl<'a, P: OptimizationProblem> ScopedStageSource<'a, P> {
    #[must_use]
    pub fn new(
        graph: RunGraphView<'a, P>,
        read_scope: ReadScope,
        evidence_store: Option<&'a dyn EvidenceStore<P::Evidence>>,
    ) -> Self {
        Self {
            graph,
            read_scope,
            evidence_store,
        }
    }

    #[must_use]
    pub fn graph(&self) -> RunGraphView<'a, P> {
        self.graph.clone()
    }

    #[must_use]
    pub const fn read_scope(&self) -> &ReadScope {
        &self.read_scope
    }

    #[must_use]
    pub fn artifact(&self, candidate: CandidateId) -> Result<&'a P::Artifact, StageSourceError> {
        self.graph
            .artifact(candidate)
            .ok_or(StageSourceError::CandidateNotFound(candidate))
    }

    #[must_use]
    pub fn assessment_snapshot(
        &self,
        assessment: AssessmentId,
    ) -> Result<AssessmentSnapshot, StageSourceError> {
        let view = self
            .graph
            .assessment(assessment)
            .ok_or(StageSourceError::AssessmentNotVisibleOrMissing(assessment))?;
        Ok(AssessmentSnapshot::from_view(&view))
    }

    pub fn evidence(&self, evidence: &EvidenceRef) -> Result<P::Evidence, StageSourceError> {
        let Some(store) = self.evidence_store else {
            return Err(StageSourceError::EvidenceStoreUnavailable);
        };
        if self.read_scope.visible_evidence == leaven_engine::EvidenceVisibility::None {
            return Err(StageSourceError::EvidenceNotVisible(evidence.clone()));
        }
        store
            .get(evidence)
            .map_err(|source| StageSourceError::EvidenceLoad {
                evidence: evidence.clone(),
                source,
            })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AssessmentSnapshot {
    pub id: AssessmentId,
    pub request_id: EvaluationRequestId,
    pub evidence_ref: EvidenceRef,
    pub evaluator: EvaluatorId,
    pub target: AssessmentTarget,
    pub independent_candidate: Option<CandidateId>,
    pub pairwise_candidates: Option<(CandidateId, CandidateId)>,
    pub listwise_candidates: Option<Vec<CandidateId>>,
    pub created_at: Timestamp,
}

impl AssessmentSnapshot {
    #[must_use]
    pub fn from_view(view: &AssessmentView<'_>) -> Self {
        Self {
            id: view.id(),
            request_id: view.request_id(),
            evidence_ref: view.evidence_ref().clone(),
            evaluator: view.evaluator().clone(),
            target: view.target().clone(),
            independent_candidate: view.independent_candidate(),
            pairwise_candidates: view.pairwise_candidates(),
            listwise_candidates: view.listwise_candidates().map(<[CandidateId]>::to_vec),
            created_at: view.created_at(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum StageSourceError {
    #[error("candidate not found: {0}")]
    CandidateNotFound(CandidateId),

    #[error("assessment not visible or missing: {0}")]
    AssessmentNotVisibleOrMissing(AssessmentId),

    #[error("evidence store unavailable")]
    EvidenceStoreUnavailable,

    #[error("evidence not visible: {0}")]
    EvidenceNotVisible(EvidenceRef),

    #[error("evidence load failed for {evidence}: {source}")]
    EvidenceLoad {
        evidence: EvidenceRef,
        #[source]
        source: leaven_store::StoreError,
    },
}
```

Engine addition to expose source from proposal context:

File:

```text
crates/leaven-engine/src/context/proposal_context.rs
```

Add a non-stage-specific helper first:

```rust
impl<'a, P: OptimizationProblem> ProposalContext<'a, P> {
    #[must_use]
    pub fn graph_clone(&self) -> RunGraphView<'a, P> {
        self.graph.clone()
    }

    #[must_use]
    pub fn read_scope_clone(&self) -> ReadScope {
        self.read_scope.clone()
    }
}
```

Then `leaven-stage` can build `ScopedStageSource` without `leaven-engine` depending on `leaven-stage`.

### 7.2 materialization input

File:

```text
crates/leaven-stage/src/materialize.rs
```

Full input definition:

```rust
use leaven_core::OptimizationProblem;
use leaven_kernel::{Cost, Metered, StageAttemptId};
use leaven_workspace::{
    fingerprint_file, WorkspacePath, WorkspaceView,
};
use serde::Serialize;

use crate::{
    AccessMode, AgentStagePlan, GeneratedContent, MaterializableArtifact,
    MaterializationEntry, MaterializationRole, MaterializationSource,
    MaterializationTarget, MaterializedEntryReceipt, ParseStatus,
    ReadScopeDigest, ScopedStageSource, StageMaterializeError, StageReceipt,
    StageReceiptStatus, StageSourceRef,
};

pub struct StageMaterializationInput<'a, P, Req>
where
    P: OptimizationProblem,
{
    pub attempt_id: StageAttemptId,
    pub workspace_id: leaven_kernel::WorkspaceId,
    pub workspace: &'a mut WorkspaceView<'a>,
    pub source: ScopedStageSource<'a, P>,
    pub plan: &'a AgentStagePlan<Req>,
}
```

If `WorkspaceView<'a>` causes borrow pain in actual compilation, weaken the field to:

```rust
pub workspace: &'a mut WorkspaceView<'_>,
```

That is probably what will compile with the existing `AgentRuntime` style.

### 7.3 materialization function

Same file:

```rust
pub async fn materialize_stage_workspace<P, Req>(
    input: StageMaterializationInput<'_, P, Req>,
) -> Result<Metered<StageReceipt>, StageMaterializeError>
where
    P: OptimizationProblem,
    P::Artifact: MaterializableArtifact,
    Req: Serialize,
{
    input.plan.output.validate().map_err(|error| {
        StageMaterializeError::Source(format!("invalid output contract: {error}"))
    })?;

    let mut receipt = StageReceipt::new(
        input.attempt_id,
        input.workspace_id,
        input.plan.role.clone(),
        ReadScopeDigest::from_read_scope(input.source.read_scope()),
    );

    write_generated_stage_files(input.workspace, input.plan, &mut receipt)?;

    for entry in &input.plan.eager {
        materialize_entry(input.workspace, &input.source, entry, &mut receipt).await?;
    }

    write_output_schema(input.workspace, input.plan, &mut receipt)?;

    receipt.status = StageReceiptStatus::Materialized;
    Ok(Metered::new(receipt, Cost::zero()))
}
```

Helper signatures:

```rust
fn write_generated_stage_files<Req>(
    workspace: &mut WorkspaceView<'_>,
    plan: &AgentStagePlan<Req>,
    receipt: &mut StageReceipt,
) -> Result<(), StageMaterializeError>
where
    Req: Serialize;

async fn materialize_entry<P>(
    workspace: &mut WorkspaceView<'_>,
    source: &ScopedStageSource<'_, P>,
    entry: &MaterializationEntry,
    receipt: &mut StageReceipt,
) -> Result<(), StageMaterializeError>
where
    P: OptimizationProblem,
    P::Artifact: MaterializableArtifact;

fn write_output_schema<Req>(
    workspace: &mut WorkspaceView<'_>,
    plan: &AgentStagePlan<Req>,
    receipt: &mut StageReceipt,
) -> Result<(), StageMaterializeError>
where
    Req: Serialize;
```

### 7.4 generated files implementation

```rust
fn write_generated_stage_files<Req>(
    workspace: &mut WorkspaceView<'_>,
    plan: &AgentStagePlan<Req>,
    receipt: &mut StageReceipt,
) -> Result<(), StageMaterializeError>
where
    Req: Serialize,
{
    let brief = render_brief(plan)?;
    write_receipted_file(
        workspace,
        receipt,
        MaterializedEntryReceipt {
            id: crate::MaterializationEntryId::new_static("brief"),
            role: MaterializationRole::Brief,
            path: Some(WorkspacePath::new("BRIEF.md")?),
            target: MaterializationTarget::AgentWorkspace,
            access: AccessMode::ReadOnly,
            source: StageSourceRef::Generated {
                label: "BRIEF.md".to_owned(),
            },
            file: None,
            produced_by_query: None,
        },
        brief.as_bytes(),
    )?;

    write_receipted_file(
        workspace,
        receipt,
        MaterializedEntryReceipt {
            id: crate::MaterializationEntryId::new_static("focus.stage_role"),
            role: MaterializationRole::FocusRequest,
            path: Some(WorkspacePath::new("focus/stage_role.txt")?),
            target: MaterializationTarget::AgentWorkspace,
            access: AccessMode::ReadOnly,
            source: StageSourceRef::Generated {
                label: "stage_role".to_owned(),
            },
            file: None,
            produced_by_query: None,
        },
        plan.role.as_str().as_bytes(),
    )?;

    let request_json = serde_json::to_vec_pretty(&plan.request)?;
    write_receipted_file(
        workspace,
        receipt,
        MaterializedEntryReceipt {
            id: crate::MaterializationEntryId::new_static("focus.request"),
            role: MaterializationRole::FocusRequest,
            path: Some(WorkspacePath::new("focus/request.json")?),
            target: MaterializationTarget::AgentWorkspace,
            access: AccessMode::ReadOnly,
            source: StageSourceRef::Generated {
                label: "request".to_owned(),
            },
            file: None,
            produced_by_query: None,
        },
        &request_json,
    )?;

    write_receipted_file(
        workspace,
        receipt,
        MaterializedEntryReceipt {
            id: crate::MaterializationEntryId::new_static("focus.instructions"),
            role: MaterializationRole::FocusInstructions,
            path: Some(WorkspacePath::new("focus/instructions.md")?),
            target: MaterializationTarget::AgentWorkspace,
            access: AccessMode::ReadOnly,
            source: StageSourceRef::Generated {
                label: "instructions".to_owned(),
            },
            file: None,
            produced_by_query: None,
        },
        plan.directive.instructions.as_bytes(),
    )?;

    Ok(())
}
```

`write_receipted_file`:

```rust
fn write_receipted_file(
    workspace: &mut WorkspaceView<'_>,
    receipt: &mut StageReceipt,
    mut entry: MaterializedEntryReceipt,
    bytes: &[u8],
) -> Result<(), StageMaterializeError> {
    let Some(path) = entry.path.clone() else {
        return Err(StageMaterializeError::Source(
            "cannot write receipt-only entry as file".to_owned(),
        ));
    };
    workspace.write_file(&path, bytes)?;
    entry.file = Some(fingerprint_file(workspace, &path)?);
    receipt.eager_materialization.push(entry);
    Ok(())
}
```

`render_brief`:

```rust
fn render_brief<Req>(plan: &AgentStagePlan<Req>) -> Result<String, StageMaterializeError> {
    let title = plan
        .directive
        .brief_title
        .as_deref()
        .unwrap_or("Leaven optimizer stage workspace");

    let mut out = String::new();
    out.push_str("# ");
    out.push_str(title);
    out.push_str("\n\n");
    out.push_str("stage role: `");
    out.push_str(plan.role.as_str());
    out.push_str("`\n\n");
    out.push_str("## instructions\n\n");
    out.push_str(&plan.directive.instructions);
    out.push_str("\n\n");
    out.push_str("## workspace layout\n\n");
    out.push_str("- `focus/request.json`: typed request for this call.\n");
    out.push_str("- `focus/instructions.md`: stage-specific instructions.\n");
    out.push_str("- `output/`: write required outputs here.\n");
    out.push_str("- `.leaven/`: machine-readable plan/schema/receipt files.\n");
    out.push_str("\n## required outputs\n\n");
    for entry in &plan.output.required {
        out.push_str("- `");
        out.push_str(entry.path.as_str());
        out.push_str("` (`");
        out.push_str(entry.id.as_str());
        out.push_str("`, role `");
        out.push_str(&format!("{:?}", entry.role));
        out.push_str("`)\n");
        if let Some(description) = &entry.description {
            out.push_str("  - ");
            out.push_str(description);
            out.push('\n');
        }
    }
    out.push_str("\nOnly use files and tools inside this workspace. Hidden data is not available.\n");
    Ok(out)
}
```

### 7.5 materialize selected entry implementation

```rust
async fn materialize_entry<P>(
    workspace: &mut WorkspaceView<'_>,
    source: &ScopedStageSource<'_, P>,
    entry: &MaterializationEntry,
    receipt: &mut StageReceipt,
) -> Result<(), StageMaterializeError>
where
    P: OptimizationProblem,
    P::Artifact: MaterializableArtifact,
{
    match &entry.source {
        MaterializationSource::Generated(content) => {
            let bytes = match content {
                GeneratedContent::Utf8(text) => text.as_bytes().to_vec(),
                GeneratedContent::Json(value) => serde_json::to_vec_pretty(value)?,
                GeneratedContent::Bytes(bytes) => bytes.clone(),
            };
            write_receipted_file(
                workspace,
                receipt,
                entry_receipt_for(entry, StageSourceRef::Generated {
                    label: entry.id.as_str().to_owned(),
                }),
                &bytes,
            )?;
        }
        MaterializationSource::CandidateArtifact { candidate } => {
            let artifact = source
                .artifact(*candidate)
                .map_err(|error| StageMaterializeError::Source(error.to_string()))?;
            let mut slot = workspace.subdir(entry.path.clone())?;
            let mut artifact_slot = leaven_workspace::WorkspaceSlot::new(
                entry.path.clone(),
                slot,
            );
            artifact
                .write_to(&mut artifact_slot)
                .await
                .map_err(|error| StageMaterializeError::Artifact(error.to_string()))?;

            let tree = leaven_workspace::fingerprint_tree(workspace, &entry.path)?;
            receipt.eager_materialization.push(MaterializedEntryReceipt {
                id: entry.id.clone(),
                role: entry.role.clone(),
                path: Some(entry.path.clone()),
                target: entry.target.clone(),
                access: entry.access.clone(),
                source: StageSourceRef::CandidateArtifact(*candidate),
                file: Some(leaven_workspace::WorkspaceFileFingerprint {
                    path: entry.path.clone(),
                    fingerprint: tree.fingerprint,
                    bytes: tree.files.iter().map(|file| file.bytes).sum(),
                }),
                produced_by_query: None,
            });
        }
        MaterializationSource::AssessmentRecord { assessment } => {
            let snapshot = source
                .assessment_snapshot(*assessment)
                .map_err(|error| StageMaterializeError::Source(error.to_string()))?;
            let bytes = serde_json::to_vec_pretty(&snapshot)?;
            write_receipted_file(
                workspace,
                receipt,
                entry_receipt_for(entry, StageSourceRef::Assessment(*assessment)),
                &bytes,
            )?;
        }
        MaterializationSource::LineageSummary { candidate, depth } => {
            let graph = source.graph();
            let lineage = graph.lineage(*candidate);
            let summary = format_lineage_summary(*candidate, *depth, lineage);
            write_receipted_file(
                workspace,
                receipt,
                entry_receipt_for(entry, StageSourceRef::Candidate(*candidate)),
                summary.as_bytes(),
            )?;
        }
        MaterializationSource::EvidenceRef { evidence } => {
            receipt.eager_materialization.push(MaterializedEntryReceipt {
                id: entry.id.clone(),
                role: entry.role.clone(),
                path: None,
                target: MaterializationTarget::ReceiptOnly,
                access: AccessMode::ReadOnly,
                source: StageSourceRef::Evidence(evidence.clone()),
                file: None,
                produced_by_query: None,
            });
        }
        MaterializationSource::ProposalRecord { proposal } => {
            receipt.eager_materialization.push(MaterializedEntryReceipt {
                id: entry.id.clone(),
                role: entry.role.clone(),
                path: None,
                target: MaterializationTarget::ReceiptOnly,
                access: AccessMode::ReadOnly,
                source: StageSourceRef::Proposal(*proposal),
                file: None,
                produced_by_query: None,
            });
        }
    }
    Ok(())
}

fn entry_receipt_for(
    entry: &MaterializationEntry,
    source: StageSourceRef,
) -> MaterializedEntryReceipt {
    MaterializedEntryReceipt {
        id: entry.id.clone(),
        role: entry.role.clone(),
        path: Some(entry.path.clone()),
        target: entry.target.clone(),
        access: entry.access.clone(),
        source,
        file: None,
        produced_by_query: None,
    }
}
```

The `WorkspaceSlot::new` constructor above was `pub(crate)`, so either:

1. make it `pub fn new(root, view)`; or
2. add `WorkspaceView::into_slot(root)`.

Prefer option 2 if you want fewer public constructors:

```rust
impl<'a> WorkspaceView<'a> {
    pub fn into_slot(self, root: WorkspacePath) -> WorkspaceSlot<'a> {
        WorkspaceSlot::new(root, self)
    }
}
```

### 7.6 output schema writer

```rust
fn write_output_schema<Req>(
    workspace: &mut WorkspaceView<'_>,
    plan: &AgentStagePlan<Req>,
    receipt: &mut StageReceipt,
) -> Result<(), StageMaterializeError>
where
    Req: Serialize,
{
    let bytes = serde_json::to_vec_pretty(&plan.output)?;
    write_receipted_file(
        workspace,
        receipt,
        MaterializedEntryReceipt {
            id: crate::MaterializationEntryId::new_static("leaven.output_contract"),
            role: MaterializationRole::OutputSchema,
            path: Some(WorkspacePath::new(".leaven/output_contract.json")?),
            target: MaterializationTarget::AgentWorkspace,
            access: AccessMode::ReadOnly,
            source: StageSourceRef::Generated {
                label: "output_contract".to_owned(),
            },
            file: None,
            produced_by_query: None,
        },
        &bytes,
    )?;
    Ok(())
}
```

Acceptance tests:

```text
crates/leaven-stage/tests/materialize_minimal.rs
```

Required cases:

```rust
#[tokio::test]
async fn materializes_brief_focus_request_and_output_contract() { ... }

#[tokio::test]
async fn output_paths_must_live_under_output_dir() { ... }

#[tokio::test]
async fn receipt_records_file_fingerprints() { ... }

#[tokio::test]
async fn candidate_artifact_entry_requires_visible_candidate() { ... }
```

## 8. Milestone 6 — artifact materialization tier

This trait should live in `leaven-stage`, not `leaven-core`, because it depends on `leaven-workspace`. This preserves cold-core purity.

**Done when (no workarounds):**

- A `TextArtifact` written into a `WorkspaceSlot` by `write_to`, then read back from the unchanged slot by `read_back_change`, returns `Ok(None)`. The slot is observably equivalent to its pre-write state from the artifact's perspective.
- A `TextArtifact` written into a slot whose `artifact.txt` is then rewritten with different bytes reads back as `Ok(Some(ReplaceText { text: new_text }))`. The change is reconstructed from the slot, not derived from a side channel.
- A slot whose `artifact.txt` is invalid UTF-8 (or whose required file is missing entirely) makes `read_back_change` return `Err(ArtifactReadbackError)`. It never returns `Ok(None)` for an unreadable or absent artifact — the absence of a change must be distinguishable from the absence of evidence.
- `write_to` does not produce any file or side effect outside the provided `WorkspaceSlot`. Running `write_to` twice into different slots produces two independent on-disk artifacts; the second does not alter the first.
- The three named tests in `crates/leaven-stage/tests/artifact_text.rs` (or equivalent file) cover unchanged-readback, changed-readback, and invalid-input-readback. The trait docstring on `MaterializableArtifact` codifies all five laws listed at the end of section 8.1.

**Forbidden proxy proofs:**

- `read_back_change` returns `Ok(None)` for unchanged input — and *also* returns `Ok(None)` when the artifact file is missing entirely, because the implementation does `slot.read_file(path).ok().filter(...)`. A caller cannot then distinguish "no change" from "the agent never wrote anything." This collapses two genuinely different facts into one and makes downstream optimizer decisions wrong.
- The invalid-UTF-8 test asserts `Err(...)` and stops there. The error surface leaks the raw bytes into the error message, which then flows into receipts. Receipts now contain whatever bytes the agent wrote, which may include hidden data from a compromised stage. The error must redact the bytes and reference them by fingerprint instead.
- `write_to` is tested by writing one artifact into one slot and checking the file exists. The slot-containment law (`write_to` does not write outside the slot) is never adversarially tested — a future artifact impl that uses `std::fs` directly to write to a host path would pass. The contract test must use a slot that would observably be polluted if `write_to` escaped.
- `ReconstructibleArtifact::parse_from` is left as `todo!()` because TextArtifact "doesn't need it" — but the milestone is about validating the contract shape, not about TextArtifact specifically. If `parse_from` cannot be implemented for the toy artifact, the trait is mis-designed for the real artifacts that will need it.
- The trait laws are documented in a comment but the test file does not have a corresponding contract suite that can be re-used to validate future `MaterializableArtifact` impls. The next implementer (jj, Inspect, whatever) writes their own ad-hoc tests, the laws drift, and the abstraction stops being uniform.
- `write_to` calls `slot.write_file` for the happy path but, when the artifact is multi-file, recurses into `slot.subslot(...)` and the recursion is not exercised. A future multi-file artifact silently breaks the nesting invariant because no test pushes through the recursion.

### 8.1 trait definitions

File:

```text
crates/leaven-stage/src/artifact.rs
```

Full definition:

```rust
use leaven_core::Artifact;
use leaven_engine::{MaterializationReport, MaterializeError};
use leaven_workspace::{WorkspaceError, WorkspaceSlot};

#[allow(async_fn_in_trait)]
pub trait MaterializableArtifact: Artifact {
    /// Writes this artifact into the provided workspace slot.
    ///
    /// The implementation may create files/directories inside the slot and may
    /// use declared factory context. It must not write outside the slot except
    /// through declared factory capabilities recorded by the caller.
    async fn write_to(
        &self,
        slot: &mut WorkspaceSlot<'_>,
    ) -> Result<MaterializationReport, ArtifactMaterializationError>;

    /// Reads back a change relative to `self` from a workspace slot.
    ///
    /// `Ok(None)` means the slot is valid but unchanged. Invalid workspace
    /// content must return an error, not `None`.
    async fn read_back_change(
        &self,
        slot: &WorkspaceSlot<'_>,
    ) -> Result<Option<Self::Change>, ArtifactReadbackError>;
}

#[allow(async_fn_in_trait)]
pub trait ReconstructibleArtifact: MaterializableArtifact {
    /// Reconstructs a complete artifact value from a slot.
    ///
    /// This is stronger than reading back a change and is not required for
    /// large external artifacts such as jj/git worktrees.
    async fn parse_from(slot: &WorkspaceSlot<'_>) -> Result<Self, ArtifactReadbackError>
    where
        Self: Sized;
}

#[derive(Debug, thiserror::Error)]
pub enum ArtifactMaterializationError {
    #[error(transparent)]
    Workspace(#[from] WorkspaceError),

    #[error(transparent)]
    EngineMaterialize(#[from] MaterializeError),

    #[error("artifact materialization failed: {0}")]
    Message(String),

    #[error("artifact materialization failed: {message}")]
    WithSource {
        message: String,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync + 'static>,
    },
}

impl ArtifactMaterializationError {
    #[must_use]
    pub fn with_source(
        message: impl Into<String>,
        source: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        Self::WithSource {
            message: message.into(),
            source: Box::new(source),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ArtifactReadbackError {
    #[error(transparent)]
    Workspace(#[from] WorkspaceError),

    #[error("artifact readback failed: {0}")]
    Message(String),

    #[error("artifact readback failed: {message}")]
    WithSource {
        message: String,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync + 'static>,
    },
}

impl ArtifactReadbackError {
    #[must_use]
    pub fn with_source(
        message: impl Into<String>,
        source: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        Self::WithSource {
            message: message.into(),
            source: Box::new(source),
        }
    }
}
```

Trait laws to document in this file:

```text
write_to must only create agent-visible workspace state inside the provided slot, unless the side effect goes through declared factory context.
read_back_change on an unchanged, valid slot returns Ok(None).
read_back_change on invalid slot content fails.
read_back_change must not depend on ambient host paths outside declared slot/factory context.
ReconstructibleArtifact::parse_from is optional and stronger than MaterializableArtifact.
```

### 8.2 tiny proof artifact

Test helper type:

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
struct TextArtifact {
    text: String,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
struct ReplaceText {
    text: String,
}

#[derive(Debug, thiserror::Error)]
#[error("text artifact error: {0}")]
struct TextArtifactError(String);

impl leaven_core::Artifact for TextArtifact {
    type Change = ReplaceText;
    type ApplyError = TextArtifactError;

    fn identity(&self) -> leaven_core::ArtifactIdentity {
        let mut builder = leaven_kernel::FingerprintBuilder::new();
        builder.update(b"text-artifact.v1").update(self.text.as_bytes());
        leaven_core::ArtifactIdentity::External(format!("text:{:?}", builder.finish()))
    }

    fn apply_change(&self, change: &Self::Change) -> Result<Self, Self::ApplyError> {
        Ok(Self {
            text: change.text.clone(),
        })
    }
}

impl MaterializableArtifact for TextArtifact {
    async fn write_to(
        &self,
        slot: &mut WorkspaceSlot<'_>,
    ) -> Result<MaterializationReport, ArtifactMaterializationError> {
        let path = WorkspacePath::new("artifact.txt")?;
        slot.write_file(&path, self.text.as_bytes())?;
        Ok(MaterializationReport {
            files_written: 1,
            bytes_written: self.text.len() as u64,
            truncations: Vec::new(),
        })
    }

    async fn read_back_change(
        &self,
        slot: &WorkspaceSlot<'_>,
    ) -> Result<Option<Self::Change>, ArtifactReadbackError> {
        let path = WorkspacePath::new("artifact.txt")?;
        let bytes = slot.read_file(&path)?;
        let text = String::from_utf8(bytes)
            .map_err(|error| ArtifactReadbackError::with_source("artifact was not utf8", error))?;
        if text == self.text {
            Ok(None)
        } else {
            Ok(Some(ReplaceText { text }))
        }
    }
}
```

Tests:

```rust
#[tokio::test]
async fn text_artifact_unchanged_slot_reads_back_none() { ... }

#[tokio::test]
async fn text_artifact_changed_slot_reads_back_replace_text() { ... }

#[tokio::test]
async fn text_artifact_invalid_utf8_fails_readback() { ... }
```

## 9. Milestone 7 — bootstrap and parser contracts

**Done when (no workarounds):**

- `AgentStageBootstrap<P, Req>` and `StageOutputParser<P, Req, Out>` are implementable for at least two distinct stage roles (e.g., `reflect` and `select_parent`, or `reflect` and `accept`) without altering the trait signatures. The traits are stable enough that a downstream crate can implement them once and reuse the impl across stages.
- A bootstrap impl can produce an `AgentStagePlan` whose plan, when fed to `materialize_stage_workspace` *in the same test*, yields a workspace that a paired parser impl can successfully read. The bootstrap-materialize-parser chain is exercised end-to-end, not just per-trait.
- A `JsonProposalBatchParser` (or a test-shaped equivalent) reads its target output from the path declared in `plan.output.required`, not from a hardcoded `output/proposal.json`. Two different bootstraps that pick different paths are both parseable by the same parser without modification.
- A missing required output produces `StageParseError::MissingRequiredOutput { id, path }` naming the specific entry. A present-but-empty output produces `StageParseError::Malformed { path, message }` — these are distinct failures with distinct receipts, not folded into a generic parse error.
- A malformed required output produces a `StageParseError` whose source preserves the underlying `serde_json::Error` (or the bytes-level reason) so that downstream retry/repair policies can inspect *what* failed, not just *that* it failed.

**Forbidden proxy proofs:**

- The parser reads `output/proposal.json` literally. A bootstrap that declares its output at `output/decision.json` produces a workspace the parser cannot read. The milestone "compiles for two stages" but the production code path silently misroutes for any stage whose output path differs from the example. Parsers must consult `plan.output` to find their target paths.
- The trait shape is "implementable" but the only test impl unused every borrowed input (`request: &Req`, `receipt: &StageReceipt`, `graph: RunGraphView`). The lifetime constraints those bindings carry are never exercised, and a downstream impl that needs to actually use them discovers the trait does not compile for their use case. The contract suite must include an impl that meaningfully uses every input.
- `MissingRequiredOutput` is returned only when the file is literally absent. A present-but-zero-byte file becomes a generic JSON error because `serde_json::from_slice("")` produces `Error("EOF")`. Two adjacent failure modes get one bucket and downstream retry logic cannot distinguish "the agent never wrote" from "the agent wrote garbage."
- `JsonProposalBatchParser` requires serde bounds on `ProposalBatch<P>` that force every consumer to derive `Serialize`/`Deserialize` on their problem types. The "common parser helper" then becomes a tax on every problem, and most consumers write a custom parser anyway. Either commit to the bounds and assert that every Leaven-shipped problem satisfies them, or scope the helper as test-only and document it as such.
- The bootstrap returns a plan whose output contract's `required` is non-empty but whose entries reference paths that `materialize_stage_workspace` would reject. No test runs the bootstrap output through materialize before parser — the bootstrap and parser tests are decoupled, and a bootstrap that produces an invalid plan looks fine until Milestone 8 wires the chain together.
- The parser error wraps `serde_json::Error` only at the outermost layer; once it's a `StageParseError::Json(...)`, the original error's line/column information is lost because the `Display` impl truncates it. Repair policies that want to feed the error back to an agent for self-correction can't, and the "feedback channel" property degrades to "something went wrong."

### 9.1 bootstrap context and trait

File:

```text
crates/leaven-stage/src/bootstrap.rs
```

Full definition:

```rust
use leaven_core::OptimizationProblem;
use leaven_engine::{ReadScope, RunGraphView};
use leaven_kernel::BudgetSnapshot;

use crate::{AgentStagePlan, StageError};

pub struct StageBootstrapContext<'a, P: OptimizationProblem> {
    pub graph: RunGraphView<'a, P>,
    pub read_scope: ReadScope,
    pub budget: BudgetSnapshot,
}

impl<'a, P: OptimizationProblem> StageBootstrapContext<'a, P> {
    #[must_use]
    pub fn new(
        graph: RunGraphView<'a, P>,
        read_scope: ReadScope,
        budget: BudgetSnapshot,
    ) -> Self {
        Self {
            graph,
            read_scope,
            budget,
        }
    }
}

#[allow(async_fn_in_trait)]
pub trait AgentStageBootstrap<P, Req>: Send + Sync
where
    P: OptimizationProblem,
    Req: Send + Sync,
{
    async fn bootstrap(
        &self,
        request: Req,
        ctx: StageBootstrapContext<'_, P>,
    ) -> Result<AgentStagePlan<Req>, StageError>;
}
```

Optional function adapter for ergonomic closures:

```rust
pub struct BootstrapFn<F>(pub F);

impl<F> BootstrapFn<F> {
    #[must_use]
    pub const fn new(f: F) -> Self {
        Self(f)
    }
}
```

A blanket impl for arbitrary async closures is annoying without boxing. Do not block on it. Users can define a tiny struct for first proof.

### 9.2 parser contract

File:

```text
crates/leaven-stage/src/parser.rs
```

Full definition:

```rust
use leaven_agent::AgentSession;
use leaven_core::OptimizationProblem;
use leaven_engine::RunGraphView;
use leaven_kernel::Metered;
use leaven_workspace::WorkspaceView;

use crate::{AgentStagePlan, StageParseError, StageReceipt};

pub struct StageOutputParseInput<'a, P, Req>
where
    P: OptimizationProblem,
{
    pub workspace: &'a mut WorkspaceView<'a>,
    pub session: &'a AgentSession,
    pub request: &'a Req,
    pub plan: &'a AgentStagePlan<Req>,
    pub receipt: &'a StageReceipt,
    pub graph: RunGraphView<'a, P>,
}

#[allow(async_fn_in_trait)]
pub trait StageOutputParser<P, Req, Out>: Send + Sync
where
    P: OptimizationProblem,
    Req: Send + Sync,
    Out: Send + Sync,
{
    async fn parse(
        &self,
        input: StageOutputParseInput<'_, P, Req>,
    ) -> Result<Metered<Out>, StageParseError>;
}
```

If the `WorkspaceView<'a>` lifetime is too tight, use:

```rust
pub workspace: &'a mut WorkspaceView<'_>,
```

### 9.3 common JSON parser helper for proposal batches

Optional but useful for first proof:

```rust
pub struct JsonProposalBatchParser;

impl<P, Req> StageOutputParser<P, Req, ProposalBatch<P>> for JsonProposalBatchParser
where
    P: OptimizationProblem,
    P::Artifact: serde::Serialize + serde::de::DeserializeOwned,
    <P::Artifact as leaven_core::Artifact>::Change: serde::Serialize + serde::de::DeserializeOwned,
    P::ProposalAnnotations: Default + serde::Serialize + serde::de::DeserializeOwned,
    Req: Send + Sync,
{
    async fn parse(
        &self,
        input: StageOutputParseInput<'_, P, Req>,
    ) -> Result<Metered<ProposalBatch<P>>, StageParseError> {
        let path = leaven_workspace::WorkspacePath::new("output/proposal.json")?;
        let bytes = input.workspace.read_file(&path)?;
        let batch = serde_json::from_slice::<ProposalBatch<P>>(&bytes)?;
        Ok(Metered::new(batch, leaven_kernel::Cost::zero()))
    }
}
```

This requires adding serde derives/bounds to `ProposalBatch<P>` if not already usable. If that is too much, write a test-only parser over a test-specific raw JSON format instead.

## 10. Milestone 8 — `AgentBacked<ProposerSlot>`

**Done when (no workarounds):**

- A test wires `AgentBacked::<ProposerSlot, LocalWorkspaceFactory, FakeAgentRuntime, _, _, Req, ProposalBatch<P>>::new(...)` and passes it to `RunContext::propose(&backed, request)`. The full chain — bootstrap → `workspace_factory.allocate` → `materialize_stage_workspace` → `agent_runtime.run_session` → `output_parser.parse` — runs end to end and returns `Metered<ProposalBatch<P>>`. The run event stream contains, *in order*, `StageAttempt { status: Started }`, `Materialized`, `RuntimeCompleted`, `OutputParsed`, with matching `attempt_id` across all four events.
- The `FakeAgentRuntime` is scripted with bytes the test fixture *does not* hand directly to `RunContext::apply_batch`. The only path from "fake runtime wrote bytes" to "apply_batch saw a batch" is through `materialize → run_session → parser`. A regression that bypasses the parser (e.g., a shortcut that returns a hardcoded batch) fails the test because the agent-written bytes are not the same as the optimizer's hardcoded ones.
- `RunContext::apply_batch(batch_id)` applies the returned batch and produces a new candidate whose artifact reflects the agent's written content. The test asserts the new candidate's bytes, not just that `apply_batch` returned `Ok`.
- Workspace cleanup runs on both the success path and every failure path (bootstrap error, allocate error, materialize error, runtime error, parser error, runtime panic). A test that injects a parser error confirms `workspace.cleanup()` was called before `propose` returned. If cleanup fails after a successful stage, the resulting error is `StageAndCleanupError` and both the stage value and the cleanup error are observable in the receipt or event log.
- A test that scripts the fake runtime to write malformed `output/proposal.json` produces `StageAttemptFailureKind::OutputParse` in the event stream and *no* `RunEvent::ApplyFailed` for that stage. The same test then continues — `RunContext` is still usable for a subsequent `propose` call — so a single parse failure does not poison the optimizer.

**Forbidden proxy proofs:**

- The integration test scripts `FakeAgentRuntime` with bytes the test also passes (directly or indirectly) to a comparison helper or assertion. The "agent produced this" claim collapses to "our test fixture produced this," and a regression where the parser ignores workspace content and returns a hardcoded batch passes silently. The fake runtime's bytes must be derived independently and the only assertion route must go through the workspace.
- Cleanup is called on success but the failure path uses `?` to bubble up before reaching `workspace.cleanup()`. Tests cover only the success cleanup; failure tests leave the workspace allocated. In production the optimizer leaks workspaces under parser/runtime failure and someone discovers it via disk pressure weeks later.
- The four `StageAttempt` events are asserted to *exist* in `events.iter().filter(...)` but their order is not checked. A future change that emits `RuntimeCompleted` before `Materialized` (because of a refactor that hoists the runtime call) passes the test, and any UI that replays stage history misrenders the run.
- The malformed-JSON test asserts `OutputParse` is present and `ApplyFailed` is absent — but in the test's context, `ApplyFailed` would never fire regardless because no batch reached apply. The absence assertion is vacuous. The test must construct a scenario where, if `OutputParse` were mis-routed through `ApplyFailed`, an `ApplyFailed` event would observably appear.
- `StageAndCleanupError` is defined but the cleanup-after-success-failure path is not tested. A future change makes `workspace.cleanup()` infallible (returns `()`), the variant becomes dead, and nobody notices until a real cleanup failure shows up in production with no observability.
- `AgentBackedPolicy::max_parse_retries` is set to 0 by default and the test only exercises 0 retries. A non-zero retry count is added later; the retry loop is implemented to re-call the parser without re-running the runtime, so retries are vacuous re-reads of the same bad bytes. The milestone passes because `max_parse_retries` is "supported" syntactically, but the semantic is unproven.
- The test uses `FakeAgentRuntime` with one scripted action — the runtime's session-level capabilities (multi-step, mid-session reads, tool use) are not exercised. A future runtime that actually runs an agent across multiple turns breaks invariants no test pinned. The integration test should script at least one multi-action session so the chain is proven under non-trivial runtime usage.
- `agent_runtime.run_session(&mut view, ...)` borrows the view mutably for the runtime's lifetime, then the parser re-borrows it. The test happens to compile because the lifetime relationships work for `FakeAgentRuntime` specifically, but a runtime that holds the view across an await point (which real runtimes will) breaks the borrow checker. The milestone "compiles" but a real runtime cannot drop in.

### 10.1 slot marker

File:

```text
crates/leaven-stage/src/slots.rs
```

Full definition:

```rust
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash)]
pub struct ProposerSlot;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash)]
pub struct ParentSelectorSlot;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash)]
pub struct PartSelectorSlot;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash)]
pub struct MergeSlot;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash)]
pub struct AcceptanceSlot;
```

Only implement `ProposerSlot` in the first PR. The others exist only as markers if useful; otherwise omit them until needed.

### 10.2 policy/config/adapter struct

File:

```text
crates/leaven-stage/src/agent_backed.rs
```

Full definition:

```rust
use std::marker::PhantomData;
use std::time::Duration;

use leaven_agent::{
    AgentInstructions, AgentRunContext, AgentRunRequest, AgentRuntime, AgentSession,
};
use leaven_core::{OptimizationProblem, ProposalBatch};
use leaven_engine::{
    Arity, ProposalContext, ProposalError, Proposer, StageAttemptFailureKind,
    StageAttemptStatus, StageAttemptSummary,
};
use leaven_kernel::{AgentSessionId, Cost, Metered, ProposerId, StageAttemptId};
use leaven_workspace::{WithWorkspaceError, WorkspaceConfig, WorkspaceFactory};
use serde::{de::DeserializeOwned, Serialize};

use crate::{
    materialize_stage_workspace, AgentStageBootstrap, ArtifactMaterializationError,
    MaterializableArtifact, ParseFailurePolicy, ScopedStageSource, StageBootstrapContext,
    StageMaterializationInput, StageOutputParseInput, StageOutputParser,
    StageReceiptStatus,
};

pub struct AgentBacked<S, Factory, Runtime, Bootstrap, Parser, Req, Out> {
    pub config: AgentBackedConfig,
    pub workspace_factory: Factory,
    pub agent_runtime: Runtime,
    pub bootstrap: Bootstrap,
    pub output_parser: Parser,
    pub policy: AgentBackedPolicy,
    marker: PhantomData<(S, Req, Out)>,
}

impl<S, Factory, Runtime, Bootstrap, Parser, Req, Out>
    AgentBacked<S, Factory, Runtime, Bootstrap, Parser, Req, Out>
{
    #[must_use]
    pub fn new(
        config: AgentBackedConfig,
        workspace_factory: Factory,
        agent_runtime: Runtime,
        bootstrap: Bootstrap,
        output_parser: Parser,
        policy: AgentBackedPolicy,
    ) -> Self {
        Self {
            config,
            workspace_factory,
            agent_runtime,
            bootstrap,
            output_parser,
            policy,
            marker: PhantomData,
        }
    }
}

#[derive(Clone, Debug)]
pub struct AgentBackedConfig {
    pub id: ProposerId,
    pub arity: Arity,
    pub workspace: WorkspaceConfig,
}

impl AgentBackedConfig {
    #[must_use]
    pub fn new(id: ProposerId) -> Self {
        Self {
            id,
            arity: Arity::Single,
            workspace: WorkspaceConfig::default(),
        }
    }

    #[must_use]
    pub const fn with_arity(mut self, arity: Arity) -> Self {
        self.arity = arity;
        self
    }

    #[must_use]
    pub fn with_workspace(mut self, workspace: WorkspaceConfig) -> Self {
        self.workspace = workspace;
        self
    }
}

#[derive(Clone, Debug)]
pub struct AgentBackedPolicy {
    pub max_parse_retries: usize,
    pub parse_failure: ParseFailurePolicy,
    pub timeout: Option<Duration>,
    pub record_receipt_file: bool,
}

impl Default for AgentBackedPolicy {
    fn default() -> Self {
        Self {
            max_parse_retries: 0,
            parse_failure: ParseFailurePolicy::Strict,
            timeout: None,
            record_receipt_file: true,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParseFailurePolicy {
    Strict,
    RecordAttempt,
}
```

Note: if this file imports `ParseFailurePolicy` from crate and also defines it, remove it from the import list. The code block shows final shape, not exact compiler-final import hygiene.

### 10.3 build runtime request helper

```rust
fn build_agent_run_request<Req>(
    plan: &crate::AgentStagePlan<Req>,
    policy: &AgentBackedPolicy,
) -> AgentRunRequest {
    let mut request = AgentRunRequest::new(
        AgentInstructions {
            system: plan.directive.system.clone(),
            task: "Read BRIEF.md, perform the stage task, and write the required output files under output/.".to_owned(),
            context: vec![leaven_agent::AgentContextRef {
                label: "brief".to_owned(),
                path: leaven_workspace::WorkspacePath::new("BRIEF.md")
                    .expect("static path"),
                media_type: Some("text/markdown".to_owned()),
            }],
        },
        plan.output.to_agent_contract(),
    );
    request.cwd = leaven_workspace::WorkspacePath::root();
    request.limits.timeout = policy.timeout;
    request
}
```

### 10.4 event helper

```rust
fn emit_attempt(
    ctx: &ProposalContext<'_, impl OptimizationProblem>,
    attempt_id: StageAttemptId,
    stage: leaven_kernel::StageId,
    role: Option<String>,
    workspace_id: Option<leaven_kernel::WorkspaceId>,
    receipt_id: Option<leaven_kernel::StageReceiptId>,
    status: StageAttemptStatus,
    cost: Cost,
) {
    ctx.record_stage_attempt(StageAttemptSummary {
        attempt_id,
        stage,
        role,
        workspace_id,
        receipt_id,
        status,
        cost,
    });
}
```

Because `impl Trait` in argument position with `ProposalContext` may be awkward here, implement this inline or make it generic:

```rust
fn emit_attempt<P: OptimizationProblem>(
    ctx: &ProposalContext<'_, P>,
    // same fields...
) { ... }
```

### 10.5 `Proposer` impl

```rust
impl<P, Factory, Runtime, Bootstrap, Parser, Req>
    Proposer<P>
    for AgentBacked<
        crate::ProposerSlot,
        Factory,
        Runtime,
        Bootstrap,
        Parser,
        Req,
        ProposalBatch<P>,
    >
where
    P: OptimizationProblem,
    P::Artifact: MaterializableArtifact,
    Factory: WorkspaceFactory,
    Runtime: AgentRuntime,
    Bootstrap: AgentStageBootstrap<P, Req>,
    Parser: StageOutputParser<P, Req, ProposalBatch<P>>,
    Req: Serialize + DeserializeOwned + Send + Sync,
{
    type Request = Req;

    fn id(&self) -> ProposerId {
        self.config.id.clone()
    }

    fn arity(&self) -> Arity {
        self.config.arity
    }

    async fn propose(
        &self,
        request: Self::Request,
        ctx: ProposalContext<'_, P>,
    ) -> Result<Metered<ProposalBatch<P>>, ProposalError> {
        let attempt_id = StageAttemptId::new();
        let stage = leaven_kernel::StageId::from_proposer(self.id());

        ctx.record_stage_attempt(StageAttemptSummary {
            attempt_id,
            stage: stage.clone(),
            role: None,
            workspace_id: None,
            receipt_id: None,
            status: StageAttemptStatus::Started,
            cost: Cost::zero(),
        });

        let bootstrap_ctx = StageBootstrapContext::new(
            ctx.graph_clone(),
            ctx.read_scope_clone(),
            ctx.budget(),
        );
        let plan = self
            .bootstrap
            .bootstrap(request, bootstrap_ctx)
            .await
            .map_err(|source| ProposalError::with_source("agent-backed bootstrap failed", source))?;

        let mut workspace = self
            .workspace_factory
            .allocate(self.config.workspace.clone())
            .await
            .map_err(|source| ProposalError::with_source("agent-backed workspace allocation failed", source))?;
        let workspace_id = workspace.id();

        let stage_result = async {
            let mut view = workspace.view();
            let source = ScopedStageSource::new(
                ctx.graph_clone(),
                ctx.read_scope_clone(),
                None,
            );

            let materialized = materialize_stage_workspace(StageMaterializationInput {
                attempt_id,
                workspace_id,
                workspace: &mut view,
                source,
                plan: &plan,
            })
            .await
            .map_err(|source| ProposalError::with_source("agent-backed materialization failed", source))?;

            let mut receipt = materialized.value;
            ctx.record_stage_attempt(StageAttemptSummary {
                attempt_id,
                stage: stage.clone(),
                role: Some(plan.role.as_str().to_owned()),
                workspace_id: Some(workspace_id),
                receipt_id: Some(receipt.id),
                status: StageAttemptStatus::Materialized,
                cost: materialized.cost.clone(),
            });

            let budget = ctx.budget();
            let session = self
                .agent_runtime
                .run_session(
                    &mut view,
                    build_agent_run_request(&plan, &self.policy),
                    AgentRunContext::new(AgentSessionId::new(), &budget),
                )
                .await
                .map_err(|source| ProposalError::with_source("agent-backed runtime failed", source))?;

            ctx.record_stage_attempt(StageAttemptSummary {
                attempt_id,
                stage: stage.clone(),
                role: Some(plan.role.as_str().to_owned()),
                workspace_id: Some(workspace_id),
                receipt_id: Some(receipt.id),
                status: StageAttemptStatus::RuntimeCompleted,
                cost: session.cost.clone(),
            });

            receipt.status = StageReceiptStatus::RuntimeCompleted;
            let parsed = self
                .output_parser
                .parse(StageOutputParseInput {
                    workspace: &mut view,
                    session: &session.value,
                    request: &plan.request,
                    plan: &plan,
                    receipt: &receipt,
                    graph: ctx.graph_clone(),
                })
                .await;

            match parsed {
                Ok(parsed) => {
                    receipt.status = StageReceiptStatus::Parsed;
                    ctx.record_stage_attempt(StageAttemptSummary {
                        attempt_id,
                        stage: stage.clone(),
                        role: Some(plan.role.as_str().to_owned()),
                        workspace_id: Some(workspace_id),
                        receipt_id: Some(receipt.id),
                        status: StageAttemptStatus::OutputParsed,
                        cost: parsed.cost.clone(),
                    });
                    Ok(parsed)
                }
                Err(parse_error) => {
                    ctx.record_stage_attempt(StageAttemptSummary {
                        attempt_id,
                        stage: stage.clone(),
                        role: Some(plan.role.as_str().to_owned()),
                        workspace_id: Some(workspace_id),
                        receipt_id: Some(receipt.id),
                        status: StageAttemptStatus::Failed(
                            StageAttemptFailureKind::OutputParse,
                        ),
                        cost: Cost::zero(),
                    });

                    match self.policy.parse_failure {
                        ParseFailurePolicy::Strict | ParseFailurePolicy::RecordAttempt => {
                            Err(ProposalError::with_source(
                                "agent-backed output parse failed",
                                parse_error,
                            ))
                        }
                    }
                }
            }
        }
        .await;

        let cleanup_result = workspace.cleanup().await;
        match (stage_result, cleanup_result) {
            (Ok(value), Ok(())) => Ok(value),
            (Ok(_), Err(cleanup)) => Err(ProposalError::with_source(
                "agent-backed cleanup failed after successful stage",
                cleanup,
            )),
            (Err(stage), Ok(())) => Err(stage),
            (Err(stage), Err(cleanup)) => Err(ProposalError::with_source(
                "agent-backed stage failed and cleanup also failed",
                StageAndCleanupError {
                    stage: Box::new(stage),
                    cleanup,
                },
            )),
        }
    }
}

#[derive(Debug, thiserror::Error)]
#[error("stage failed and cleanup failed: stage={stage}; cleanup={cleanup}")]
struct StageAndCleanupError {
    stage: Box<ProposalError>,
    cleanup: leaven_workspace::WorkspaceError,
}
```

Implementation notes:

- The exact lifetime of `StageMaterializationInput.workspace` may need adjustment to `&mut WorkspaceView<'_>`.
- `ParseFailurePolicy::RecordAttempt` currently still returns an error. That is acceptable for the first implementation because the stage attempt is recorded and no `ApplyFailed` is emitted. A later retry/continue policy can decide what “continue” means per optimizer.
- Do not implement parent/part/merge slots in this PR.

Acceptance test:

```text
crates/leaven-stage/tests/agent_backed_proposer.rs
```

Required scenario:

```rust
#[tokio::test]
async fn fake_runtime_writes_proposal_json_and_parser_returns_batch() { ... }
```

Assertions:

```text
- BRIEF.md exists
- focus/request.json exists
- output/proposal.json was required
- fake runtime wrote proposal.json
- parser returned ProposalBatch<P>
- RunContext::propose recorded ProposalBatchProduced
- RunContext::apply_batch can apply it
- StageAttempt events exist for Started, Materialized, RuntimeCompleted, OutputParsed
```

## 11. Milestone 9 — GEPA request/feedback prerequisites

Do this after Milestone 8 (`AgentBacked<ProposerSlot>`) is complete, so GEPA has a real target to route through.

**Done when (no workarounds):**

- `FixedSurfaceEdit<E>` is the canonical public name for the deterministic edit fixture. `ReflectiveMutation<E>` exists only as a `#[deprecated]` type alias for `FixedSurfaceEdit<E>`. Public examples, docs, prelude, and downstream tests use `FixedSurfaceEdit`; `ReflectiveMutation` is no longer the name a new reader of the crate would encounter first.
- `ReflectRequest<PartId>`, `SelectedFeedback`, and `CaseFeedbackSummary` exist in `leaven-gepa::reflection` with serde derives that roundtrip cleanly for at least one realistic `PartId` (`String` is fine for the smoke test; a richer test should also exercise an enum-shaped `PartId`).
- `ParentAssessmentFeedback::select_feedback(req, ctx)`, given a parent whose `parent_assessment` is visible under the current `ReadScope`, returns a `SelectedFeedback` whose:
  - `assessment_refs` includes `req.parent_assessment` and nothing the test did not author,
  - `evidence_refs` includes the `EvidenceRef` carried by that assessment, looked up via the actual `ctx.graph`,
  - `provenance_refs` includes `InfoRef::Candidate(req.parent)`,
  - and `informed_by_refs()` is the concatenation/dedup of the above, not a hardcoded list.
- A parent whose assessment is hidden under the current `ReadScope` produces `FeedbackSelectionError::AssessmentNotVisible(id)`. The error path names the assessment id and is observably distinct from the missing-assessment path (the latter would be a different error variant or a different read-scope state).
- The deprecation of `ReflectiveMutation` is enforced: `cargo check` with `-D deprecated` (or the workspace's existing deprecation lints) fails if any code in `leaven-gepa` or its tests still uses `ReflectiveMutation`. Aliases exist for downstream consumers, not for the crate's own code.

**Forbidden proxy proofs:**

- `ReflectiveMutation` is renamed to `FixedSurfaceEdit` and the alias is added — but the crate's own examples, its prelude, and the docs in `docs/specs/gepa_optimizer_surface.md` continue to use the old name. New readers encounter the deprecated name first and assume it is the canonical surface; the rename is cosmetic. The deprecation lint must be on for the gepa crate, not silenced by `#[allow(deprecated)]`.
- `ParentAssessmentFeedback::select_feedback` returns a `SelectedFeedback` populated by hardcoded constructor calls (`SelectedFeedback { assessment_refs: vec![request.parent_assessment], ... }`) without ever reading `ctx.graph`. The test passes because the hardcoded value matches the test fixture. A future selector that needs to derive refs from the actual assessment (e.g., to include the evidence ref) cannot, because the abstraction was never load-bearing — it always returned what the request told it to.
- `informed_by_refs()` is implemented as `self.provenance_refs.clone()` and the test asserts it contains the parent candidate. The assessment ref is never folded in, so a downstream consumer that calls `informed_by_refs()` to attribute provenance sees an incomplete picture. The method must include every load-bearing ref by construction, not just the ones the test happened to check.
- `SelectedFeedback` serde-roundtrips with `PartId = String` and that is all that is tested. A real GEPA `PartId` is `S::PartId` which can be an enum or a tuple; the trait bounds in `ReflectRequest<PartId>` don't actually require `Serialize`/`Deserialize` to hold, and the roundtrip fails when a real surface is plugged in. Add a roundtrip test against a non-`String` `PartId` that mirrors a real edit surface.
- The hidden-assessment failure path uses an assessment that does not exist in the graph at all. That exercises the missing path, not the visible-but-hidden path. The test must use an assessment that exists in the graph but is excluded by `ReadScope.hidden_partitions`.
- `FeedbackSelectionError` is added to the public error surface but the existing `ReflectionError` (or whichever GEPA-level error type the optimizer surfaces) does not have a `From<FeedbackSelectionError>` impl. The optimizer code path that calls `select_feedback` then wraps the error in a generic `OptimizerError::Message(format!("{e}"))`, losing the structured failure. The error plumbing must preserve the variant up to the optimizer boundary.

### 11.1 rename/quarantine fixed edit fixture

File:

```text
crates/leaven-gepa/src/proposer.rs
```

Replace public name:

```rust
/// Deterministic fixed surface-edit fixture.
#[derive(Clone, Debug)]
pub struct FixedSurfaceEdit<E> {
    edit: E,
}

impl<E> FixedSurfaceEdit<E> {
    #[must_use]
    pub const fn new(edit: E) -> Self {
        Self { edit }
    }
}

impl<A, S> SurfaceProposer<A, S> for FixedSurfaceEdit<S::Edit>
where
    A: Artifact,
    S: EditSurface<A>,
{
    fn propose_edit(
        &mut self,
        _artifact: &A,
        _surface: &S,
        _part: &S::PartId,
    ) -> Result<S::Edit, SurfaceError> {
        Ok(self.edit.clone())
    }
}

#[deprecated(note = "use FixedSurfaceEdit; ReflectiveMutation is reserved for real reflection")]
pub type ReflectiveMutation<E> = FixedSurfaceEdit<E>;
```

Update exports/prelude to prefer `FixedSurfaceEdit`.

### 11.2 GEPA selected feedback/request types

File:

```text
crates/leaven-gepa/src/reflection.rs
```

Add module export in `lib.rs`:

```rust
pub mod reflection;
pub use reflection::{
    CaseFeedbackSummary, FeedbackSelectionRequest, FeedbackSelector,
    ReflectionObjective, ReflectRequest, SelectedFeedback,
};
```

Full definitions:

```rust
use leaven_core::{InfoRef, OptimizationProblem};
use leaven_engine::RunGraphView;
use leaven_kernel::{AssessmentId, CandidateId, CaseId, EvidenceRef};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReflectRequest<PartId> {
    pub parent: CandidateId,
    pub selected_part: PartId,
    pub feedback: SelectedFeedback,
    pub objective: ReflectionObjective,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SelectedFeedback {
    pub assessment_refs: Vec<AssessmentId>,
    pub evidence_refs: Vec<EvidenceRef>,
    pub case_summaries: Vec<CaseFeedbackSummary>,
    pub provenance_refs: Vec<InfoRef>,
}

impl SelectedFeedback {
    #[must_use]
    pub fn informed_by_refs(&self) -> Vec<InfoRef> {
        let mut refs = self.provenance_refs.clone();
        refs.extend(self.assessment_refs.iter().copied().map(InfoRef::Assessment));
        refs
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CaseFeedbackSummary {
    pub case_id: Option<CaseId>,
    pub assessment: AssessmentId,
    pub score: Option<f64>,
    pub summary: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ReflectionObjective {
    ImproveSelectedPart,
    FixValidationError,
    ImproveFailingCases,
    ExploreAlternative,
    Other(String),
}

#[derive(Clone, Debug)]
pub struct FeedbackSelectionRequest<PartId> {
    pub parent: CandidateId,
    pub selected_part: PartId,
    pub parent_assessment: AssessmentId,
}

pub struct FeedbackSelectionContext<'a, P: OptimizationProblem> {
    pub graph: RunGraphView<'a, P>,
}

#[allow(async_fn_in_trait)]
pub trait FeedbackSelector<P, PartId>: Send + Sync
where
    P: OptimizationProblem,
    PartId: Clone + Send + Sync,
{
    async fn select_feedback(
        &mut self,
        request: FeedbackSelectionRequest<PartId>,
        ctx: FeedbackSelectionContext<'_, P>,
    ) -> Result<SelectedFeedback, FeedbackSelectionError>;
}

#[derive(Debug, thiserror::Error)]
pub enum FeedbackSelectionError {
    #[error("selected parent assessment is not visible or missing: {0}")]
    AssessmentNotVisible(AssessmentId),

    #[error("feedback selection failed: {0}")]
    Message(String),
}
```

### 11.3 default feedback selector

```rust
#[derive(Clone, Debug, Default)]
pub struct ParentAssessmentFeedback;

impl<P, PartId> FeedbackSelector<P, PartId> for ParentAssessmentFeedback
where
    P: OptimizationProblem,
    PartId: Clone + Send + Sync,
{
    async fn select_feedback(
        &mut self,
        request: FeedbackSelectionRequest<PartId>,
        ctx: FeedbackSelectionContext<'_, P>,
    ) -> Result<SelectedFeedback, FeedbackSelectionError> {
        let assessment = ctx
            .graph
            .assessment(request.parent_assessment)
            .ok_or(FeedbackSelectionError::AssessmentNotVisible(
                request.parent_assessment,
            ))?;
        Ok(SelectedFeedback {
            assessment_refs: vec![assessment.id()],
            evidence_refs: vec![assessment.evidence_ref().clone()],
            case_summaries: Vec::new(),
            provenance_refs: vec![InfoRef::Candidate(request.parent)],
        })
    }
}
```

### 11.4 GEPA proposer adapter for surface edits

If GEPA’s reflector still wants to output surface edits before native `ProposalBatch<P>`, add an adapter that implements engine `Proposer<P>`.

```rust
pub struct SurfaceEditProposerAdapter<S, Reflect, FeedbackSel> {
    pub surface: S,
    pub reflector: Reflect,
    pub feedback_selector: FeedbackSel,
    pub id: leaven_kernel::ProposerId,
}
```

Preferred long-term: use native `AgentBacked<ProposerSlot, ..., ReflectRequest<S::PartId>, ProposalBatch<P>>` directly.

For the first GEPA migration, the native proposer route is cleaner:

```rust
Reflect: Proposer<P, Request = ReflectRequest<S::PartId>>
```

GEPA `propose_candidate` becomes async:

```rust
async fn propose_candidate<P>(
    &mut self,
    ctx: &mut RunContext<'_, P>,
    parent: CandidateId,
    parent_assessment: AssessmentId,
) -> Result<Option<CandidateId>, OptimizerError>
where
    P: OptimizationProblem,
    P::ProposalAnnotations: Default,
    S: EditSurface<P::Artifact>,
    S::PartId: Clone + Send + Sync + serde::Serialize + serde::de::DeserializeOwned,
    Reflect: Proposer<P, Request = ReflectRequest<S::PartId>>,
    PartSel: PartSelector<P::Artifact, S>,
{
    let artifact = ctx
        .graph()
        .artifact(parent)
        .ok_or_else(|| OptimizerError::Message(format!(
            "selected parent {parent} is missing from graph"
        )))?
        .clone();

    let part = self
        .part_selector
        .select_part(&artifact, &self.surface)
        .map_err(|source| OptimizerError::with_source("GEPA part selection failed", source))?;

    let selected_feedback = self
        .feedback_selector
        .select_feedback(
            FeedbackSelectionRequest {
                parent,
                selected_part: part.clone(),
                parent_assessment,
            },
            FeedbackSelectionContext { graph: ctx.graph() },
        )
        .await
        .map_err(|source| OptimizerError::with_source("GEPA feedback selection failed", source))?;

    let request = ReflectRequest {
        parent,
        selected_part: part,
        feedback: selected_feedback,
        objective: ReflectionObjective::ImproveSelectedPart,
    };

    let batch = ctx
        .propose(&self.reflector, request)
        .await
        .map_err(|source| OptimizerError::with_source("GEPA reflection failed", source))?;

    let applied = ctx
        .apply_batch(batch.batch_id)
        .map_err(|source| OptimizerError::with_source("GEPA proposal application failed", source))?;

    Ok(applied.successful_candidates().next())
}
```

This requires adding `feedback_selector` to `Gepa` generics. If that is too much for the first migration, make the default selection inline, then extract the selector after the proof passes.

## 12. Milestone 10 — fake-runtime integrated proof

**Done when (no workarounds):**

- A single end-to-end integration test exercises the full GEPA-shaped flow: build a `ReflectRequest`, route it through `AgentBacked<ProposerSlot>` whose runtime is `FakeAgentRuntime` scripted to write `output/proposal.json`, return through `RunContext::propose`, and have `RunContext::apply_batch` produce a *new candidate* whose artifact bytes match what the fake runtime was scripted to write — *not* what the optimizer hardcoded.
- The new candidate's `informed_by` list contains exactly the refs that `ParentAssessmentFeedback::select_feedback` produced for the parent: the parent candidate `InfoRef`, the parent assessment id, and any evidence refs that the assessment carried. Asserted by id, not by length.
- A second test variant scripts the fake runtime to write malformed `output/proposal.json`. The test asserts:
  - the stage event log contains `StageAttemptFailureKind::OutputParse`,
  - no `RunEvent::ApplyFailed` is emitted for that stage,
  - the parent candidate is unchanged in the graph (no partial apply),
  - `RunContext::propose` is still callable for a subsequent attempt (the optimizer is not poisoned).
- The test uses real `ProposalBatch<TextProblem>` construction through the parser, not a hand-built fixture. Reading `output/proposal.json` from the workspace, calling the parser, and getting a batch are exercised by the test; if any one link were severed (parser stubbed, runtime bypassed, materializer skipped), the test would fail.
- The test problem (`TextProblem` or whatever stand-in) has at least two distinct candidate states differing in `text`, and the test asserts the post-`apply_batch` artifact equals the script-written text. Equality, not "contains" or "length matches."

**Forbidden proxy proofs:**

- The fake runtime is scripted with `"good answer"` and the test asserts the new candidate has text `"good answer"` — but `"good answer"` is also a hardcoded constant in the test that's compared by reference, so a bug where the parser returns a default `ProposalBatch` and `apply_batch` happens to leave the artifact at its default (which happens to equal `"good answer"` in the test fixture) silently passes. Use a randomly generated string per test run, or compare via fingerprint, so the only way the assertion holds is through the workspace.
- `informed_by` is asserted via `assert!(!informed_by.is_empty())` or `assert!(informed_by.len() == n)`. The presence of the specific parent-assessment id is not checked, so a future change that includes the *wrong* refs (e.g., a different candidate's assessment) passes the length check. Assert by id-set equality.
- The malformed-JSON test asserts `OutputParse` is present in events and stops. The test never tries to call `RunContext::propose` again afterward, so a regression that leaves `RunContext` in a poisoned state — unable to allocate a workspace, or holding a budget lock — silently breaks recovery semantics. The test must run a *second* successful propose after the malformed failure.
- The test scripts a single-shot `FakeAgentRuntime::WriteFile` action. Real GEPA reflection sessions are multi-step: the agent reads, writes scratch files, reads them back, then commits the output. A future change that holds workspace state between actions (or that requires the runtime to flush before parse) breaks invariants this test never exercised. Add at least one multi-action script.
- "Apply produced a new candidate" is asserted by `graph.candidates().count() > before` — a regression that produces a *cloned* candidate with the wrong artifact (or that produces the new candidate but doesn't wire it into the lineage) passes the count check. Assert lineage: the new candidate's parent is the original, and its artifact bytes are the scripted bytes.
- The test claims "GEPA reflection works through `RunContext::propose`" but uses a `TextProblem` stand-in, not the actual `Gepa` optimizer driving the flow. The proof is "the materialize-runtime-parse chain works for *something* that looks like a reflector"; the actual `Gepa::propose_candidate` path remains untested until a later milestone, but Milestone 10 is claimed complete. Either the test must drive `Gepa` itself, or the milestone's claim must be narrowed in the doc to "the chain works for a reflector-shaped proposer."
- The test runs in a single test process where the FakeAgentRuntime, the parser, and the bootstrap all share a `static` test fixture (e.g., shared lazy data). Concurrent test runs interfere; the milestone "passes" in CI's serial test mode but flakes locally. Make the fixture per-test.



### 12.1 test problem

Define a local test problem in `crates/leaven-stage/tests/agent_backed_gepa_like.rs` or `crates/leaven-gepa/tests/agent_backed_reflection.rs`.

```rust
#[derive(Clone, Debug)]
struct TextProblem;

impl OptimizationProblem for TextProblem {
    type Artifact = TextArtifact;
    type Case = ();
    type Evidence = TestEvidence;
    type ProposalAnnotations = ();
}

#[derive(Clone, Debug)]
struct TestEvidence {
    feedback: String,
}

impl leaven_core::Evidence for TestEvidence {}
```

### 12.2 raw proposal JSON format for parser

To avoid requiring full serde on `ProposalBatch<P>` in the first test, use a tiny raw format:

```rust
#[derive(serde::Deserialize)]
struct RawTextProposal {
    target: CandidateId,
    replacement: String,
}

struct RawTextProposalParser;

impl StageOutputParser<TextProblem, ReflectRequest<String>, ProposalBatch<TextProblem>>
    for RawTextProposalParser
{
    async fn parse(
        &self,
        input: StageOutputParseInput<'_, TextProblem, ReflectRequest<String>>,
    ) -> Result<Metered<ProposalBatch<TextProblem>>, StageParseError> {
        let path = WorkspacePath::new("output/proposal.json")?;
        let bytes = input.workspace.read_file(&path)?;
        let raw: RawTextProposal = serde_json::from_slice(&bytes)?;
        let proposal = Proposal::mutate(
            raw.target,
            ReplaceText { text: raw.replacement },
        )
        .informed_by(input.request.feedback.informed_by_refs())
        .build();
        Ok(Metered::new(
            ProposalBatch {
                proposals: vec![proposal],
                semantics: ProposalBatchSemantics::Alternatives,
                metadata: MetadataBag::new(),
            },
            Cost::zero(),
        ))
    }
}
```

### 12.3 bootstrap for reflection

```rust
struct TextReflectBootstrap;

impl AgentStageBootstrap<TextProblem, ReflectRequest<String>> for TextReflectBootstrap {
    async fn bootstrap(
        &self,
        request: ReflectRequest<String>,
        _ctx: StageBootstrapContext<'_, TextProblem>,
    ) -> Result<AgentStagePlan<ReflectRequest<String>>, StageError> {
        let output = StageOutputContract::new(vec![StageOutputEntry::json(
            OutputEntryId::new_static("proposal"),
            WorkspacePath::new("output/proposal.json")?,
            OutputRole::ProposalBatch,
            None,
        )]);

        Ok(AgentStagePlan::new(
            StageRole::reflect(),
            request,
            StageDirective::new(
                "Read focus/request.json and selected feedback. Write a JSON proposal to output/proposal.json.",
            ),
            output,
        ))
    }
}
```

### 12.4 fake runtime actions

```rust
let fake_runtime = FakeAgentRuntime::new(vec![FakeAgentAction::WriteFile {
    path: WorkspacePath::new("output/proposal.json")?,
    bytes: serde_json::to_vec(&serde_json::json!({
        "target": parent,
        "replacement": "good answer"
    }))?,
}]);
```

Assertions:

```text
- output controls the proposal; optimizer does not hardcode replacement.
- `RunContext::propose` records `ProposalBatchProduced`.
- `RunContext::apply_batch` creates a new candidate with text `good answer`.
- proposal `informed_by` includes the parent assessment.
- stage attempt events include started/materialized/runtime_completed/output_parsed.
- malformed JSON records output-parse failure and no ApplyFailed event.
```

## 13. Milestone 11 — lazy `leaven_query` minimal implementation

Do not block the first proof on this. Add it after eager materialization and `AgentBacked<ProposerSlot>` work (Milestones 5 and 8).

**Done when (no workarounds):**

- A stage agent inside a materialized workspace can run `leaven_query candidate <visible_id>` (via `slot.run_command(...)` or the equivalent stage-internal API). The command materializes the candidate's artifact under `graph/candidate/<id>/...` in the workspace, returns success on stdout, and records a `QueryRecord { status: Succeeded, materialized: vec![...], cost: ... }` in `receipt.lazy_materialization`.
- `leaven_query assessment <hidden_id>` against an id that *exists in the graph* but is hidden by the current `ReadScope` returns exit code mapped to `QueryStatus::NotVisible`, materializes nothing, and records the not-visible status in the receipt. The path that handles "exists-but-hidden" is distinct in code from the path that handles "does-not-exist."
- `QueryPolicy` caps are enforced *before* the materialization. A query that would exceed `max_queries` returns `QueryStatus::DeniedByPolicy` with no workspace write and no artifact load. A query whose target size would exceed the remaining `max_materialized_bytes` returns `DeniedByPolicy` *before* the bytes are read into memory — bound enforcement at the entry point, not after the fact.
- `leaven_query help` returns a deterministic, agent-readable help text listing every supported `LeavenQueryCommand` variant. Adding a new variant requires updating the help; the test asserts every documented variant appears in help output.
- The receipt's `lazy_materialization` entries carry the same `MaterializationEntryId` / `MaterializationRole` / `WorkspaceFileFingerprint` shape as eager entries, so downstream auditors do not need a separate code path to interpret lazy vs eager materialization.

**Forbidden proxy proofs:**

- The query executor looks up candidates via `graph.artifact(id)` directly and never consults `read_scope.hidden_partitions`. The "hidden assessment" test happens to use an id that doesn't exist in the graph at all, so the not-found path masquerades as the not-visible path. A hidden candidate that *does* exist in the graph would leak through. The test must construct a graph where the target id exists but is excluded by read scope, and the query must return `NotVisible`, not the artifact.
- Caps are enforced via a post-hoc check (`if total_bytes > cap { return Err(Budget) }`) *after* the artifact was materialized into memory. A single oversized artifact blows the cap before the check fires. Caps must be projected from cheap metadata (file size, byte count from a directory listing) and enforced before the load.
- `leaven_query help` returns hardcoded text that lists the variants the test author remembered. A new variant is added later, help is not updated, and an agent that reads help to decide which queries to run is silently behind. Help must be derived from the enum (via a derive macro, a match on every variant, or a documented `LeavenQueryCommand::variants()` helper) so adding a variant without updating help is a compile error or test failure.
- The parser for `leaven_query <args>` is permissive — unknown commands silently fall through to `Help`, or unknown flags are ignored. A prompt-injected agent that issues `leaven_query candidate <id> --escape-host /etc` gets help instead of a denial, and the receipt records nothing meaningful. The parser must reject unknown commands and unknown flags with a documented `QueryStatus::Failed { reason }`.
- The query executor returns `QueryStatus::Succeeded` for any read that the underlying graph accepts. A `Lineage { depth: usize::MAX }` query that would walk the entire candidate set silently succeeds — until it OOMs. Caps must apply to derived materialization (lineage depth, list page size) as well as direct artifact reads.
- `QueryRecord.cost` is recorded as `Cost::zero()` because "reads are free." This loses the truth that a large lazy materialization consumed real time and disk; budget reports now show free queries and an attacker who controls the agent can exfiltrate by repeated zero-cost queries. Record real cost.
- The query path uses a shell command (`leaven_query candidate ...`) that is parsed inside the stage workspace via a small CLI. The CLI implementation accepts `--from-file <path>` for inputs, and the path is resolved relative to the workspace; nothing prevents `../` traversal, so the agent can read host files. The query CLI must inherit the slot-containment rules; any path-shaped argument must go through `WorkspacePath::new` with rejection of traversal.



### 13.1 command enum

File:

```text
crates/leaven-stage/src/query.rs
```

```rust
use leaven_kernel::{AssessmentId, CandidateId};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum LeavenQueryCommand {
    Help,
    Candidate { id: CandidateId },
    Assessment { id: AssessmentId, with_evidence: bool },
    Lineage { id: CandidateId, depth: usize },
    ListCandidates { page: usize, page_size: usize },
}
```

### 13.2 executor

```rust
pub struct LeavenQueryExecutor<'a, P: OptimizationProblem> {
    pub source: ScopedStageSource<'a, P>,
    pub policy: QueryPolicy,
}

impl<'a, P: OptimizationProblem> LeavenQueryExecutor<'a, P>
where
    P::Artifact: MaterializableArtifact,
{
    pub async fn execute(
        &self,
        command: LeavenQueryCommand,
        workspace: &mut WorkspaceView<'_>,
        receipt: &mut StageReceipt,
    ) -> Result<QueryRecord, StageMaterializeError> {
        // enforce policy, materialize to graph/... paths, update receipt.lazy_materialization
        todo!()
    }
}
```

Initial acceptance:

```text
help works
candidate <id> materializes artifact if allowed
assessment <id> materializes assessment snapshot if visible
hidden assessment returns QueryStatus::NotVisible
list candidates requires pagination
query count/byte caps are enforced
```

## 14. Milestone 12 — jj spike prerequisites

Do not start jj before the `TextArtifact` proof (Milestone 6).

**Done when (no workarounds):**

- `JjWorkspaceFactory::allocate` produces a `Workspace` whose `factory_context::<Arc<JjRepoHandle>>()` returns the exact handle the factory was constructed with. Two factories constructed with two distinct repo handles produce workspaces whose contexts downcast to those distinct handles; there is no global jj-repo state.
- `JjCodebase` implements `MaterializableArtifact`. `write_to` produces a working jj workspace inside the provided slot using only commands rooted at the slot (resolved via the factory context, never via the process's `cwd` or `$HOME` or any env var). Running `write_to` from a process whose `cwd` is `/tmp/elsewhere` produces the same jj workspace as running it from inside the repo.
- `JjCodebase::read_back_change` on an unchanged slot returns `Ok(None)`. On a slot whose working copy has advanced to a new jj change, it returns `Ok(Some(JjAdvance { new_change_id }))` where `new_change_id` is the actual jj-reported change id (obtained by running `jj log -r @ -T change_id` inside the slot, not synthesized from a counter). `new_change_id != self.change_id` always for the changed case.
- `Artifact::apply_change(&self, change)` produces a new `JjCodebase` whose `change_id == change.new_change_id` and whose `cache_identity` reflects the *commit* (content), not the change (handle). Two apply chains that end at the same commit produce equal `cache_identity` values.
- The jj integration test is gated on jj being available at a pinned version (or via a feature flag), and the gate is *detected* at test time, not silently skipped. A CI runner without jj produces a clear "skipped because jj N.N.N not found" message, not green-by-omission.

**Forbidden proxy proofs:**

- `JjCodebase::write_to` shells out to `jj` using the process's current `cwd`, which in the CI test happens to be inside the test workspace. The test passes; the same code run from `/tmp` would target a different repo (or none) and would silently materialize the wrong state. The slot-containment law requires *every* jj invocation to pass an explicit `--repository` / `--working-dir` argument derived from the slot, never relying on ambient cwd.
- `read_back_change` returns `Some(JjAdvance { new_change_id: self.change_id.next() })` — a synthetic id, not the actual jj-reported id. The test passes because the assertion is "the new id differs from the old," but downstream code that uses `new_change_id` to look up the commit in the repo fails because the synthetic id does not exist. Read the id from jj, not from a counter.
- The factory context's `Arc<JjRepoHandle>` is constructed from a globally cached `JjRepoHandle::current()` that reads `~/.jjconfig` or walks up from the process's cwd. The "no ambient host paths" claim is violated through the side channel; two factories that *appear* distinct end up pointing at the same repo because the cache returned the same handle. Factory context must be passed in explicitly to `JjWorkspaceFactory::new`, with no static fallback.
- `cache_identity` is implemented as `ArtifactIdentity::External(format!("jj-change:{}", self.change_id))` — keyed on the change (working-copy handle), not the commit (immutable content). Two artifacts at semantically-identical states but with different change ids miss the cache. The cache identity must be keyed on the commit hash retrieved from jj, not on the change id.
- The integration test is gated on jj availability but the gate is `if !jj_available { return Ok(()); }` — a no-op pass. CI without jj reports green, the milestone is "complete," and no one notices jj coverage has not actually run. Use a `#[ignore]` annotation that requires explicit opt-in or a hard error when jj is expected but missing.
- `JjCodebase::write_to` succeeds even when the underlying jj command emits warnings to stderr that the test ignores. A warning about a corrupted working copy is silently dropped; the slot looks valid, the receipt records a successful materialization, and `read_back_change` returns plausible results because the corruption only manifests on the next operation. Either treat any non-empty stderr as a failure or parse stderr explicitly and route warnings into the receipt diagnostics.
- The "two factories with two handles" test uses two repos that are clones of the same upstream — so even if the factory context were swapped, the produced workspaces would look identical for the test's purposes. The test must use repos with materially different content (different commits, different change ids) so a wrong-handle bug is observable.



Target `leaven-artifact-jj` types:

```rust
#[derive(Clone, Debug)]
pub struct JjRepoHandle {
    root: std::path::PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, serde::Serialize, serde::Deserialize)]
pub struct JjChangeId(String);

#[derive(Clone, Debug, Eq, PartialEq, Hash, serde::Serialize, serde::Deserialize)]
pub struct JjCommitId(String);

#[derive(Clone, Debug)]
pub struct JjCodebase {
    pub change_id: JjChangeId,
    pub repo: std::sync::Arc<JjRepoHandle>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct JjAdvance {
    pub new_change_id: JjChangeId,
}
```

Artifact impl:

```rust
impl Artifact for JjCodebase {
    type Change = JjAdvance;
    type ApplyError = JjArtifactError;

    fn identity(&self) -> ArtifactIdentity {
        ArtifactIdentity::External(format!("jj-change:{}", self.change_id.as_str()))
    }

    fn cache_identity(&self) -> Option<CacheIdentity> {
        let commit = self.repo.commit_for(&self.change_id).ok()?;
        Some(CacheIdentity::ExternalContent(format!("jj-commit:{}", commit.as_str())))
    }

    fn apply_change(&self, change: &Self::Change) -> Result<Self, Self::ApplyError> {
        self.repo.ensure_change_exists(&change.new_change_id)?;
        Ok(Self {
            change_id: change.new_change_id.clone(),
            repo: self.repo.clone(),
        })
    }
}
```

Materializable impl:

```rust
impl MaterializableArtifact for JjCodebase {
    async fn write_to(
        &self,
        slot: &mut WorkspaceSlot<'_>,
    ) -> Result<MaterializationReport, ArtifactMaterializationError> {
        let repo = slot
            .factory_context::<JjRepoHandle>()
            .ok_or_else(|| ArtifactMaterializationError::Message(
                "jj workspace factory context missing JjRepoHandle".to_owned(),
            ))?;
        // run jj workspace add / checkout through slot.run_command(...)
        todo!()
    }

    async fn read_back_change(
        &self,
        slot: &WorkspaceSlot<'_>,
    ) -> Result<Option<Self::Change>, ArtifactReadbackError> {
        // run jj log -r @ -T change_id inside slot
        let new_change_id = todo!();
        if new_change_id == self.change_id {
            Ok(None)
        } else {
            Ok(Some(JjAdvance { new_change_id }))
        }
    }
}
```

Factory context:

```rust
pub struct JjWorkspaceFactory {
    repo: std::sync::Arc<JjRepoHandle>,
    inner: leaven_workspace_local::LocalWorkspaceFactory,
}

impl WorkspaceFactory for JjWorkspaceFactory {
    async fn allocate(&self, config: WorkspaceConfig) -> Result<Workspace, FactoryError> {
        let root = self.inner.allocate_root_only(config).await?; // or inline local allocation
        Ok(Workspace::new_with_context(
            root.clone(),
            Box::new(LocalWorkspaceBackend::new(root)),
            Some(self.repo.clone()),
        ))
    }
}
```

If `LocalWorkspaceBackend` is private, either expose a low-level constructor from `leaven-workspace-local` or implement jj factory inside `leaven-workspace-git/jj` with its own backend wrapper. Do not make jj depend on ambient host paths.

## 15. dependency order, with behavioral gates

Each gate restates the *minimum* behavior the milestone must exhibit before the next milestone can rely on it. Gates are summaries; the load-bearing acceptance lives in each milestone's "Done when" block. If a gate appears to hold but the milestone's "Done when" doesn't, the gate is the wrong line to read.

```text
1. Milestone 1 — docs boundary + AgentWorkload convenience methods
   gate: a presenter-law test fails if any stock presenter leaks hidden
         target into candidate-visible workspace bytes OR instructions,
         AND AgentWorkload rejects duplicate ids and missing-partition
         references.

2. Milestone 2 — kernel ids + Workspace id/context/slot/fingerprint
   gate: a slot rooted at proposer/x cannot write to ../../etc (tested
         adversarially), tree fingerprint is path-order-independent
         (tested by shuffling input order), factory context downcasts
         to T only when T was actually inserted.

3. Milestone 3 — engine StageAttempt event sink
   gate: a proposer that records OutputParse failure and returns
         ProposalError produces all recorded stage-attempt events in
         the run log AND ApplyFailed is observably absent in a run
         where, if parse failure were misrouted, it would have fired.

4. Milestone 4 — leaven-stage skeleton + data structs
   gate: cargo metadata on the full dependency tree shows no
         leaven-gepa or leaven-agentic in leaven-stage's transitive
         deps, AND every public type round-trips through serde with
         value equality, AND output-path validation rejects parent
         traversal (not just non-output/ prefixes).

5. Milestone 5 — materialize_stage_workspace minimal writer
   gate: materialize writes exactly the four generated files plus
         declared eager entries (no others), fingerprints are computed
         from re-reads not from input bytes, an invisible-but-existing
         candidate fails before any byte is written.

6. Milestone 6 — MaterializableArtifact + TextArtifact proof
   gate: unchanged slot -> Ok(None), changed slot -> Ok(Some(Change))
         with the new content, missing or invalid slot -> Err — and
         "Ok(None)" can never mean "no artifact present."

7. Milestone 7 — AgentStageBootstrap + StageOutputParser
   gate: the same parser impl reads outputs from two bootstraps that
         declare different paths in plan.output, AND MissingRequiredOutput
         is distinct from Malformed which is distinct from
         underlying-source errors (each carries enough context for
         feedback-to-agent).

8. Milestone 8 — AgentBacked<ProposerSlot>
   gate: a FakeAgentRuntime scripted with randomly-generated bytes
         produces a ProposalBatch through RunContext::propose whose
         applied candidate has those exact bytes, AND cleanup runs on
         every failure path, AND parse failure leaves RunContext
         usable for the next propose.

9. Milestone 9 — GEPA ReflectRequest + FeedbackSelector
   gate: ParentAssessmentFeedback derives refs by reading ctx.graph
         (not by echoing the request), informed_by_refs() includes
         every load-bearing ref, and ReflectiveMutation is deprecated
         and unused inside leaven-gepa.

10. Milestone 10 — GEPA reflection through RunContext::propose
    gate: a GEPA-shaped end-to-end test routes through
          AgentBacked<ProposerSlot> with no direct
          record_proposal_batch in the reflection path AND the new
          candidate's informed_by id-set equals the expected set.

11. Milestone 11 — leaven_query minimal
    gate: a candidate that exists in the graph but is hidden by
          read scope is NotVisible (not NotFound, not Succeeded),
          caps are enforced before materialization, query path
          inherits slot-containment for path-shaped arguments.

12. Milestone 12 — jj spike
    gate: write_to / read_back_change resolve every jj command via
          the factory-context-supplied repo handle (no process cwd,
          no $HOME, no env), and cache_identity is keyed on the
          commit hash, not the change id.
```

The order is a hard topological dependency, not a suggestion: each milestone assumes the gate of every milestone above it holds. If a downstream milestone fails because an upstream gate quietly regressed, fix the upstream gate before patching the downstream code.

## 16. public deprecation surface

Do not remove these immediately:

```text
AgenticProposer
RepairingAgenticProposer
AgenticRunInput
ProposalParser
ProposalRepairPromptBuilder
```

Add docs:

```rust
/// Transitional pre-stage-materialization adapter.
///
/// New optimizer-stage agent integrations should prefer
/// `leaven_stage::AgentBacked` once the stage crate is available.
```

Rename/quarantine:

```text
ReflectiveMutation -> FixedSurfaceEdit, deprecated alias retained temporarily
SystemAwareMerge -> scaffold/demo unless behavior implemented
WorstEvidencePart -> scaffold/demo unless behavior implemented
```

## 17. what not to implement in these prereq milestones

Do not:

```text
- delete AgentCase or AgentCaseEvaluator
- make AgentCase a dependency of AgentStagePlan
- implement all AgentBacked slots at once
- build a Harbor/Inspect task compiler
- dispatch parser behavior from StageRole strings
- put parser refs on normal OutputEntry
- rely on prompt-only hiding for hidden data
- make jj the first proof
- run live AIME before fake reflection proves real proposal production
```

## 18. promotion checklist

The goal-state spec can move from "goal-state" to "accepted" only when *every* item below holds *and* each item's evidence references a named test, file, or runnable assertion from the corresponding milestone. A checked box without that evidence pointer is not acceptance.

```text
[ ] docs distinguish AgentCase workload from AgentStage workspace, and the
    vocabulary is consistent across every spec in docs/specs/ that touches
    the agentic surface (not just one canonical doc)
[ ] AgentCase hidden-target presenter law passes against every stock
    presenter, not a single test presenter; AgentWorkload partition/id
    rejection paths are exercised
[ ] leaven-stage's full transitive dependency tree (not just direct deps)
    contains neither leaven-gepa nor leaven-agentic
[ ] WorkspaceSlot containment is asserted adversarially (parent traversal,
    absolute paths, symlink shapes), and factory context downcast rejects
    wrong-type T
[ ] StageAttempt events record success and failure paths; ApplyFailed is
    observably absent in a run where misrouting parse failure would have
    fired it
[ ] materialize_stage_workspace writes ONLY the declared files, fingerprints
    are computed from re-reads, and the materializer rejects invisible
    candidates before writing anything
[ ] StageOutputParser parses a ProposalBatch-like output from FakeAgentRuntime
    by consulting plan.output, not a hardcoded path, AND its error variants
    are distinguishable enough to drive repair feedback
[ ] AgentBacked<ProposerSlot> runs through RunContext::propose end-to-end,
    cleans up the workspace on every failure path, and leaves RunContext
    usable after parse failure
[ ] TextArtifact proof applies a fake-agent-produced edit where the test
    cannot tell whether the edit came from the agent or from a hardcoded
    fixture — the only path is through the workspace
[ ] parse failure emits OutputParse stage failure and no ApplyFailed, AND
    the same RunContext successfully proposes again after the failure
[ ] hidden partition does not materialize eagerly even for candidates that
    exist in the graph but are excluded by ReadScope
[ ] hidden partition cannot be fetched lazily once leaven_query lands; caps
    are enforced before materialization and the query CLI inherits slot
    containment
[ ] GEPA reflection routes through RunContext::propose with no direct
    record_proposal_batch call remaining in the reflection path
[ ] FixedSurfaceEdit is the canonical public name; ReflectiveMutation is
    deprecated and unused inside leaven-gepa (deprecation lint on, not
    silenced)
[ ] one example migration demonstrates the new abstraction is simpler than
    the transitional adapters (bespoke layout / parser / repair scaffold)
    OR the example honestly documents where the new abstraction is not yet
    simpler and why
[ ] no workaround, feature flag, parallel old/new path, or cfg(test) shim
    is required for any checked item — each item is exhibited by the
    production code path
```

If any item cannot be checked without a workaround, the goal-state spec is not yet acceptable; revise the spec or finish the work, do not relax the bar.

## 19. final architectural note

The important simplification is not “one more crate.” It is the jurisdictional split:

```text
AgentCase owns candidate-evaluation workload shape.
AgentStagePlan owns optimizer-stage deliberation shape.
Workspace owns file/command substrate.
RunContext owns graph truth.
```

This plan tries to make that jurisdiction mechanically enforceable. The public story stays small, but the private constitution has enough typed records to make leakage, parse failure, provenance, and workspace readback auditable.

