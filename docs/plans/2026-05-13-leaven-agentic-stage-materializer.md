# agentic stage materialization — milestone plan v0.4, implementation-detail edition

Date: 2026-05-13  
Status: implementation-ready draft, framed as behavioral milestones

## Current prerequisite status

Status updated after the `agentic-stage: satisfy workspace-wide completion gate`
jj slice. The durable spec route is now
`docs/specs/agentic_stage_materialization.md`; misspelled duplicate spec paths
are not authoritative.

This plan is complete enough to launch the next focused implementation goal for
the stage materialization spec. The remaining gaps below are deliberate
scaffolding and must not be reported as implemented behavior until their own
tests land.

### Proven prerequisite substrate

- `leaven-agentic` keeps `AgentCase`, `CaseSuite`, `AgentWorkload`, and
  `AgentCaseEvaluator` in the candidate-evaluation workload layer; optimizer
  stage workspaces route through `leaven-stage` instead.
- `leaven-workspace` has workspace ids, typed factory context, slot/root
  containment, command cwd scoping, and fingerprint helpers.
- `leaven-engine` has scoped stage handoff and one receipt-backed
  `StageAttemptRecorded` event path through `RunContext::propose`.
- `leaven-stage` exists with the user/adapter/receipt surface:
  `AgentStagePlan`, `AgentBacked`, `StageReadAuthority`,
  `StageAttemptReceipt`, `StageQueryPolicy`, `setup_stage_workspace`,
  output contracts, parser traits, receipt store, query parsing, and
  `MaterializableArtifact`.
- `AgentBacked<ProposerSlot<_>>` has a fake-runtime proof that writes
  `output/proposal.json`, parses it into `ProposalBatch`, records a stage
  attempt, and applies the batch through `RunContext::apply_batch`.
- GEPA has stage-request/bootstrap vocabulary in `crates/leaven-gepa/src/agent_stage.rs`,
  but the full optimizer switch is not part of this prerequisite tranche.
- `leaven-artifact-jj` has deterministic materializable-file scaffold tests,
  but it is not live `jj` checkout/apply behavior.

### Remaining scaffold, not a blocker for starting the next goal

- Early `AgentBacked` setup/runtime/parse failures are surfaced and persisted as
  failed attempt receipts; serialization/allocation failures still happen before
  a workspace receipt can exist.
- `setup_stage_workspace` writes `BRIEF.md`, `.leaven/stage-plan.json`, the
  output skeleton, and an executable `tools/leaven_query` help shim.
- Output receipts exist as types, and the fake-runtime path records present
  outputs plus parse status/files-read; richer runtime-integrated
  agent-requested query execution is still follow-on work.
- GEPA still has fixed-edit reflection scaffold; the real optimizer switch to
  agent-backed stage reflection remains a separate implementation slice.
- JJ materialization reads and writes deterministic workspace files; live jj
  command execution, change ids, and apply semantics remain follow-on work.

## 0. purpose

This plan turns the goal-state agentic stage materialization spec into implementation milestones. It is intentionally code-shaped: every milestone names files, target structs, traits, methods, error types, and tests. The desired outcome is that an implementer can open a milestone, create the listed modules, paste/adapt the target definitions, and know exactly which observable behaviors must hold before the milestone is complete.

The implementation target is the v0.4 architecture:

```text
A. candidate evaluation workload
   AgentCase / CaseSuite / AgentWorkload / AgentCaseEvaluator

B. optimizer agentic stage workspace
   AgentStagePlan / AgentBacked / StageAttemptReceipt / StageReadAuthority

C. raw workspace substrate
   Workspace / WorkspaceView / WorkspaceFactory / WorkspacePath / Command
```

The important v0.4 correction is that workspace setup and graph/evidence queries are separate:

```text
workspace setup
  Plan-derived files: BRIEF.md, focus/, output skeleton, .leaven/, optional tools.

query
  Scoped graph/evidence/artifact reads that may write entries into the workspace.
  Prewarm queries and agent-requested queries are the same operation with different timing.

agent output
  Files required or allowed by StageOutputContract.
```

The north-star proof:

```text
optimizer builds typed stage request
  -> AgentBacked<ProposerSlot> asks bootstrap for AgentStagePlan<Req>
  -> setup_stage_workspace writes plan-derived setup files
  -> StageQueryPolicy.prewarm runs through StageReadAuthority
  -> FakeAgentRuntime writes output/proposal.json
  -> StageOutputParser parses ProposalBatch<P>
  -> RunContext::propose records the batch
  -> RunContext::apply_batch applies it
  -> StageAttemptReceipt proves setup/query/output/parse facts
```

## 0.1 milestone discipline: no workarounds

Each milestone carries two blocks:

```text
Done when (no workarounds): behaviors the system must exhibit at completion,
                            each tied to named tests or runnable assertions.

Forbidden proxy proofs:     nearby artifacts that look like success but do not
                            satisfy the behavior.
```

Rules that hold for every milestone:

```text
- "the PR landed" is not acceptance.
- "the file compiles" is not acceptance unless the milestone is about compiling.
- A test counts only if it exercises the production path.
- A shim that mirrors production behavior is not evidence.
- Every user-visible or system-visible claim needs a named test or runnable assertion.
- If a behavior would silently regress under a plausible future change, add the regression test.
- Feature flags, cfg(test)-only bypasses, parallel old/new paths, and TODO comments do not satisfy a milestone.
```

## 0.2 spirit over letter

The concrete names below are anchors, not sacred spelling. If the codebase already has a better name that preserves the same behavior, prefer the better name and update the plan in the same change. But do not keep stale names that reintroduce old ontology.

In particular, the following old terms should not survive in new stage APIs except in changelog/migration text:

```text
EagerMaterializationPolicy
QueryPolicy
MaterializationEntry
MaterializationRole
MaterializationSource
MaterializationTarget
MaterializedEntryReceipt
StageReceipt
ReadScopeDigest
ScopedStageSource
materialize_stage_workspace
AccessMode
AgentBacked<..., Req, Out>
```

Use the v0.4 terms instead:

```text
StageQueryPolicy
AllowedQuerySet
StageQuery
WorkspaceEntry
WorkspaceEntryRole
EntrySource
EntryProjection
EntryAccess
WorkspaceEntryReceipt
StageAttemptReceipt
StageReadAuthority
setup_stage_workspace
SlotMarker<P>::Output
AgentBacked<Slot, Runtime, Bootstrap, Parser>
```

---

# 1. current state, implementation-relevant

## 1.1 `leaven-workspace`

Current substrate has the needed foundation:

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

Already useful:

```text
WorkspaceView::subdir
WorkspaceView::write_file
WorkspaceView::read_file
WorkspaceView::list_files
WorkspaceView::set_executable
WorkspaceView::is_executable
WorkspaceView::run_command
Workspace::cleanup(self)
WorkspacePath rejects raw escaping paths
```

Missing:

```text
WorkspaceId
WorkspaceFactoryContext typed registry
WorkspaceSlot
workspace file/tree fingerprint helpers
receipt-friendly file metadata
```

## 1.2 `leaven-agent`

The runtime-facing contract should remain low-level:

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

Runtime `OutputContract` remains coarse. Stage output contracts lower into it.

## 1.3 `leaven-engine`

`RunContext::propose` is the preferred finalization path:

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
```

Current `ProposalContext` exposes broad graph access. The v0.4 path adds a narrower stage boundary:

```rust
StageEngineContext<'a, P>
ScopedRunGraphView<'a, P>
StageReadAuthority<'a, P>
```

The stage layer should not receive unscoped `RunGraphView` directly.

## 1.4 `leaven-agentic`

`AgentCase`, `CaseSuite`, `AgentWorkload`, and `AgentCaseEvaluator` remain the candidate-evaluation workload layer. Do not delete them. Do not make optimizer-stage workspaces depend on them.

## 1.5 `leaven-gepa`

Current GEPA reflection is still proxy-shaped:

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

`ReflectiveMutation` is a fixed-edit fixture and should become `FixedSurfaceEdit`. Real reflection should go through an engine `Proposer<P>` path.

---

# 2. milestone 1 — docs boundary and A-shaped workload hardening

## Done when, no workarounds

```text
- AgentCase / AgentWorkload / AgentCaseEvaluator are documented as candidate-evaluation workload.
- AgentStagePlan / AgentBacked / StageAttemptReceipt are documented as optimizer-stage workspace.
- Workspace / WorkspaceView / WorkspaceFactory are documented as raw substrate.
- No public docs imply AgentCase is required by AgentStagePlan or AgentBacked.
- AgentWorkload::from_cases and from_parts exist and preserve suite validation.
- Hidden CaseTarget cannot leak through any stock presenter into agent-visible instructions or workspace bytes.
```

Named tests:

```text
crates/leaven-agentic/tests/agentic_workload.rs::workload_from_cases_derives_all_partition
crates/leaven-agentic/tests/agentic_workload.rs::workload_fingerprint_changes_when_hidden_target_changes
crates/leaven-agentic/tests/agentic_workload.rs::workload_rejects_duplicate_case_ids
crates/leaven-agentic/tests/agentic_workload.rs::workload_rejects_partition_referencing_missing_case
crates/leaven-agentic/tests/presenter_visibility.rs::hidden_target_is_not_presented_to_candidate
```

## Forbidden proxy proofs

```text
- A hidden-target test checks instructions but not workspace bytes, or workspace bytes but not instructions.
- The docs have one glossary but downstream specs still use AgentCase as a stage workspace term.
- from_cases exists, but from_parts and explicit partition validation are missing.
- fingerprint tests only assert "changed" and never assert stability under no material change.
```

## 2.1 update docs, no API churn yet

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
AgentStagePlan / AgentBacked / StageAttemptReceipt are optimizer-stage workspace vocabulary.
Workspace / WorkspaceView / WorkspaceFactory are raw substrate.
```

## 2.2 add AgentWorkload convenience methods

File:

```text
crates/leaven-agentic/src/case.rs
```

Target extension:

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentWorkload {
    cases: CaseSuite,
}

impl AgentWorkload {
    #[must_use]
    pub const fn new(cases: CaseSuite) -> Self {
        Self { cases }
    }

    pub fn from_cases(
        cases: impl IntoIterator<Item = AgentCase>,
    ) -> Result<Self, AgenticAdapterError> {
        Ok(Self {
            cases: CaseSuite::from_cases(cases)?,
        })
    }

    pub fn from_parts(
        cases: BTreeMap<CaseId, AgentCase>,
        partitions: CasePartitions,
    ) -> Result<Self, AgenticAdapterError> {
        Ok(Self {
            cases: CaseSuite::new(cases, partitions)?,
        })
    }

    #[must_use]
    pub const fn cases(&self) -> &CaseSuite {
        &self.cases
    }

    #[must_use]
    pub const fn partitions(&self) -> &CasePartitions {
        self.cases.partitions()
    }

    #[must_use]
    pub const fn fingerprint(&self) -> Fingerprint {
        self.cases.fingerprint()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.cases.is_empty()
    }
}
```

## 2.3 hidden-target presenter law

The law:

```text
CaseTarget::Hidden is scorer-visible but not candidate-visible.
```

Test shape:

```rust
#[test]
fn hidden_target_is_not_presented_to_candidate() {
    let secret = "SECRET_TARGET_SHOULD_NOT_APPEAR";
    let case = AgentCase::text(
        CaseId::new(),
        "visible input",
        CaseTarget::Hidden(secret.to_owned()),
    );

    let presentation = dry_run_all_stock_presenters(&case).expect("presenter dry run");

    for rendered in presentation {
        assert!(!rendered.instructions_text().contains(secret));
        assert!(!rendered.workspace_bytes().windows(secret.len()).any(|w| w == secret.as_bytes()));
    }
}
```

---

# 3. milestone 2 — shared ids, workspace context, slots, fingerprints

## Done when, no workarounds

```text
- WorkspaceId, StageCallId, StageAttemptReceiptId, StageQueryId, WorkspaceEntryId exist.
- Workspace has a stable id visible to receipts after cleanup.
- WorkspaceFactoryContext is a typed registry, not a single Option<Any>.
- WorkspaceSlot cannot write/read/run outside its scoped root.
- fingerprint_tree is deterministic regardless of list_files order.
```

Named tests:

```text
crates/leaven-kernel/tests/stage_ids.rs::stage_ids_serde_roundtrip
crates/leaven-workspace/tests/workspace_slot.rs::workspace_path_rejects_parent_traversal
crates/leaven-workspace/tests/workspace_slot.rs::slot_write_is_scoped_to_slot_root
crates/leaven-workspace/tests/workspace_slot.rs::nested_slot_write_is_scoped_to_nested_root
crates/leaven-workspace/tests/workspace_slot.rs::slot_command_cwd_is_scoped
crates/leaven-workspace/tests/workspace_slot.rs::factory_context_downcasts_when_present
crates/leaven-workspace/tests/workspace_slot.rs::factory_context_rejects_wrong_type
crates/leaven-workspace/tests/fingerprint.rs::tree_fingerprint_is_path_order_independent
```

## Forbidden proxy proofs

```text
- Slot containment is checked by string prefix only.
- Tree fingerprint is tested twice with same backend order rather than shuffled order.
- WorkspaceFactoryContext stores only one Any and therefore cannot hold jj repo + backend metadata together.
- Wrong-type downcast is untested.
- run_command can set a cwd outside the slot.
```

## 3.1 add ids to `leaven-kernel`

File:

```text
crates/leaven-kernel/src/ids.rs
```

Target definitions:

```rust
uuid_id!(
    /// Identifier for one allocated workspace instance.
    WorkspaceId
);

uuid_id!(
    /// Identifier for one optimizer stage call.
    StageCallId
);

uuid_id!(
    /// Identifier for a durable receipt produced by one stage attempt.
    StageAttemptReceiptId
);

uuid_id!(
    /// Identifier for one query handled by StageReadAuthority.
    StageQueryId
);

uuid_id!(
    /// Identifier for one workspace entry written by setup or query.
    WorkspaceEntryId
);
```

Export them from `crates/leaven-kernel/src/lib.rs`.

## 3.2 add WorkspaceFactoryContext

File:

```text
crates/leaven-workspace/src/context.rs
```

Target definition:

```rust
use std::any::{Any, TypeId};
use std::collections::BTreeMap;
use std::sync::Arc;

#[derive(Clone, Default)]
pub struct WorkspaceFactoryContext {
    entries: Arc<BTreeMap<TypeId, Arc<dyn Any + Send + Sync>>>,
}

impl WorkspaceFactoryContext {
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn builder() -> WorkspaceFactoryContextBuilder {
        WorkspaceFactoryContextBuilder::default()
    }

    pub fn get<T>(&self) -> Result<Arc<T>, WorkspaceFactoryContextError>
    where
        T: Any + Send + Sync + 'static,
    {
        let type_id = TypeId::of::<T>();
        let Some(value) = self.entries.get(&type_id) else {
            return Err(WorkspaceFactoryContextError::Missing {
                type_name: std::any::type_name::<T>(),
            });
        };
        value.clone().downcast::<T>().map_err(|_| WorkspaceFactoryContextError::TypeMismatch {
            type_name: std::any::type_name::<T>(),
        })
    }
}

#[derive(Default)]
pub struct WorkspaceFactoryContextBuilder {
    entries: BTreeMap<TypeId, Arc<dyn Any + Send + Sync>>,
}

impl WorkspaceFactoryContextBuilder {
    pub fn insert<T>(&mut self, value: Arc<T>) -> Result<(), WorkspaceFactoryContextError>
    where
        T: Any + Send + Sync + 'static,
    {
        let type_id = TypeId::of::<T>();
        if self.entries.contains_key(&type_id) {
            return Err(WorkspaceFactoryContextError::Duplicate {
                type_name: std::any::type_name::<T>(),
            });
        }
        self.entries.insert(type_id, value);
        Ok(())
    }

    #[must_use]
    pub fn build(self) -> WorkspaceFactoryContext {
        WorkspaceFactoryContext {
            entries: Arc::new(self.entries),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum WorkspaceFactoryContextError {
    #[error("workspace factory context missing value of type {type_name}")]
    Missing { type_name: &'static str },

    #[error("workspace factory context already has a value of type {type_name}")]
    Duplicate { type_name: &'static str },

    #[error("workspace factory context value had wrong type for {type_name}")]
    TypeMismatch { type_name: &'static str },
}
```

Export:

```rust
pub mod context;
pub use context::{WorkspaceFactoryContext, WorkspaceFactoryContextBuilder, WorkspaceFactoryContextError};
```

## 3.3 thread context through Workspace and WorkspaceView

File:

```text
crates/leaven-workspace/src/workspace.rs
```

Target struct:

```rust
pub struct Workspace {
    id: WorkspaceId,
    backend: Arc<Mutex<Box<dyn WorkspaceBackend>>>,
    local_mount: Option<PathBuf>,
    factory_context: WorkspaceFactoryContext,
}
```

Target methods:

```rust
impl Workspace {
    #[must_use]
    pub fn new(root: PathBuf, backend: Box<dyn WorkspaceBackend>) -> Self {
        Self::new_with_context(root, backend, WorkspaceFactoryContext::empty())
    }

    #[must_use]
    pub fn new_with_context(
        _root: PathBuf,
        backend: Box<dyn WorkspaceBackend>,
        factory_context: WorkspaceFactoryContext,
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

    pub fn factory_context<T>(&self) -> Result<Arc<T>, WorkspaceFactoryContextError>
    where
        T: Any + Send + Sync + 'static,
    {
        self.factory_context.get::<T>()
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

    pub fn slot(&mut self, root: WorkspacePath) -> Result<WorkspaceSlot<'_>, WorkspaceError> {
        let view = self.view().subdir(root.clone())?;
        Ok(WorkspaceSlot::new(root, view))
    }
}
```

File:

```text
crates/leaven-workspace/src/view.rs
```

Add `factory_context: WorkspaceFactoryContext` and accessors:

```rust
impl<'a> WorkspaceView<'a> {
    pub(crate) fn from_backend(
        backend: Arc<Mutex<Box<dyn WorkspaceBackend>>>,
        local_mount: Option<PathBuf>,
        prefix: WorkspacePath,
        factory_context: WorkspaceFactoryContext,
        marker: PhantomData<&'a mut ()>,
    ) -> Self { ... }

    pub fn factory_context<T>(&self) -> Result<Arc<T>, WorkspaceFactoryContextError>
    where
        T: Any + Send + Sync + 'static,
    {
        self.factory_context.get::<T>()
    }
}
```

`subdir` must clone the same context.

## 3.4 add WorkspaceSlot

File:

```text
crates/leaven-workspace/src/slot.rs
```

Target definition:

```rust
use std::any::Any;
use std::sync::Arc;

use crate::{
    Command, CommandOutput, WorkspaceError, WorkspaceFactoryContextError, WorkspacePath,
    WorkspaceView,
};

pub struct WorkspaceSlot<'a> {
    root: WorkspacePath,
    view: WorkspaceView<'a>,
}

impl<'a> WorkspaceSlot<'a> {
    #[must_use]
    pub fn new(root: WorkspacePath, view: WorkspaceView<'a>) -> Self {
        Self { root, view }
    }

    #[must_use]
    pub const fn root(&self) -> &WorkspacePath {
        &self.root
    }

    #[must_use]
    pub fn view(&self) -> &WorkspaceView<'a> {
        &self.view
    }

    pub fn view_mut(&mut self) -> &mut WorkspaceView<'a> {
        &mut self.view
    }

    pub fn subslot(&self, path: WorkspacePath) -> Result<WorkspaceSlot<'a>, WorkspaceError> {
        let root = self.root.join(path.as_str())?;
        let view = self.view.subdir(path)?;
        Ok(Self { root, view })
    }

    pub fn write_file(
        &mut self,
        path: &WorkspacePath,
        bytes: &[u8],
    ) -> Result<(), WorkspaceError> {
        self.view.write_file(path, bytes)
    }

    pub fn read_file(&self, path: &WorkspacePath) -> Result<Vec<u8>, WorkspaceError> {
        self.view.read_file(path)
    }

    pub fn list_files(&self, path: &WorkspacePath) -> Result<Vec<WorkspacePath>, WorkspaceError> {
        self.view.list_files(path)
    }

    pub fn run_command(&mut self, mut command: Command) -> Result<CommandOutput, WorkspaceError> {
        if command.cwd().is_none() {
            command = command.cwd(WorkspacePath::root());
        }
        self.view.run_command(command)
    }

    pub fn factory_context<T>(&self) -> Result<Arc<T>, WorkspaceFactoryContextError>
    where
        T: Any + Send + Sync + 'static,
    {
        self.view.factory_context::<T>()
    }
}
```

If `Command::cwd` is not currently available or has another shape, add a command API that forces cwd to be relative to the slot. Do not allow an absolute host cwd through `WorkspaceSlot`.

## 3.5 add fingerprint helpers

File:

```text
crates/leaven-workspace/src/fingerprint.rs
```

Target definition:

```rust
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
    let mut paths = view.list_files(root)?;
    paths.sort();

    let mut files = Vec::with_capacity(paths.len());
    let mut builder = FingerprintBuilder::new();
    builder.update(b"leaven.workspace.tree.v1");

    for path in paths {
        let file = fingerprint_file(view, &path)?;
        builder
            .update(file.path.as_str().as_bytes())
            .update(&file.bytes.to_le_bytes())
            .update(file.fingerprint.0);
        files.push(file);
    }

    Ok(WorkspaceTreeFingerprint {
        root: root.clone(),
        fingerprint: builder.finish(),
        files,
    })
}
```

---

# 4. milestone 3 — engine scoped stage boundary and single stage-attempt event

## Done when, no workarounds

```text
- Engine can construct StageEngineContext from ProposalContext without exposing unscoped RunGraphView to leaven-stage.
- StageEngineContext contains ScopedRunGraphView, ReadScope, BudgetHandle/BudgetSnapshot, stage_call_id, and optional evidence store.
- RunEvent has one generic StageAttemptRecorded event.
- A proposer can record a stage attempt receipt ref/outcome from inside propose.
- Parse failure records StageAttemptOutcome::Failed(OutputParse) and no ApplyFailed.
- Stage events are emitted on success and error paths before RunContext::propose returns.
```

Named tests:

```text
crates/leaven-engine/tests/stage_attempt_events.rs::stage_attempt_recorded_on_success
crates/leaven-engine/tests/stage_attempt_events.rs::stage_attempt_recorded_on_proposer_error
crates/leaven-engine/tests/stage_attempt_events.rs::output_parse_failure_is_not_apply_failed
crates/leaven-engine/tests/stage_scope.rs::stage_engine_context_does_not_expose_unscoped_graph
```

## Forbidden proxy proofs

```text
- ProposalContext::graph_clone is passed to leaven-stage and called "scoped" by convention.
- Multiple lifecycle events are emitted instead of one receipt-backed event.
- The error path returns before draining stage-attempt events.
- OutputParse test is vacuous because the run never could have emitted ApplyFailed.
```

## 4.1 shared stage event vocabulary

To avoid dependency cycles, put small event-facing types in `leaven-kernel`.

File:

```text
crates/leaven-kernel/src/stage.rs
```

Target definitions:

```rust
use smol_str::SmolStr;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StageRole(SmolStr);

impl StageRole {
    pub fn new(value: impl Into<SmolStr>) -> Result<Self, StageRoleError> {
        let value = value.into();
        if value.is_empty() || value.contains('/') || value.chars().any(char::is_whitespace) {
            return Err(StageRoleError { value: value.to_string() });
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn new_static(value: &'static str) -> Self {
        Self(SmolStr::new_static(value))
    }

    #[must_use]
    pub fn reflect() -> Self { Self::new_static("reflect") }
    #[must_use]
    pub fn select_parent() -> Self { Self::new_static("select_parent") }
    #[must_use]
    pub fn select_part() -> Self { Self::new_static("select_part") }
    #[must_use]
    pub fn sample_batch() -> Self { Self::new_static("sample_batch") }
    #[must_use]
    pub fn accept() -> Self { Self::new_static("accept") }
    #[must_use]
    pub fn merge() -> Self { Self::new_static("merge") }
    #[must_use]
    pub fn resolve_conflicts() -> Self { Self::new_static("resolve_conflicts") }

    #[must_use]
    pub fn as_str(&self) -> &str { self.0.as_str() }
}

#[derive(Clone, Debug, thiserror::Error)]
#[error("invalid stage role `{value}`")]
pub struct StageRoleError {
    pub value: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StageAttemptReceiptRef {
    pub id: StageAttemptReceiptId,
    pub fingerprint: Option<Fingerprint>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum StageAttemptOutcome {
    Completed,
    Failed(StageAttemptFailure),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum StageAttemptFailure {
    WorkspaceAllocate,
    WorkspaceSetup,
    Query,
    RuntimeTimeout,
    Runtime,
    OutputContract,
    OutputParse,
    Cleanup,
    StageAndCleanup {
        stage: Box<StageAttemptFailure>,
        cleanup: Box<StageAttemptFailure>,
    },
    Budget,
    Other(String),
}
```

Export from kernel.

## 4.2 engine event

File:

```text
crates/leaven-engine/src/events.rs
```

Add:

```rust
pub enum RunEvent {
    // existing variants...

    StageAttemptRecorded {
        stage_call_id: StageCallId,
        role: StageRole,
        receipt: StageAttemptReceiptRef,
        outcome: StageAttemptOutcome,
    },
}
```

Do not add separate `AgentStageStarted`, `AgentStageMaterialized`, `AgentStageCompleted`, etc. The receipt contains that detail.

## 4.3 stage-attempt sink on ProposalContext

File:

```text
crates/leaven-engine/src/context/proposal_context.rs
```

Target shape:

```rust
#[derive(Clone, Default)]
pub struct StageAttemptEventSink {
    inner: Arc<Mutex<Vec<PendingStageAttemptEvent>>>,
}

#[derive(Clone, Debug)]
pub struct PendingStageAttemptEvent {
    pub stage_call_id: StageCallId,
    pub role: StageRole,
    pub receipt: StageAttemptReceiptRef,
    pub outcome: StageAttemptOutcome,
}

impl StageAttemptEventSink {
    #[must_use]
    pub fn new() -> Self { Self::default() }

    pub fn push(&self, event: PendingStageAttemptEvent) {
        self.inner.lock().push(event);
    }

    pub(crate) fn drain(&self) -> Vec<PendingStageAttemptEvent> {
        std::mem::take(&mut *self.inner.lock())
    }
}

pub struct ProposalContext<'a, P: OptimizationProblem> {
    graph: RunGraphView<'a, P>,
    budget: BudgetHandle<'a>,
    read_scope: ReadScope,
    stage_call_id: StageCallId,
    stage_attempt_sink: StageAttemptEventSink,
}

impl<'a, P: OptimizationProblem> ProposalContext<'a, P> {
    pub fn record_stage_attempt(
        &self,
        role: StageRole,
        receipt: StageAttemptReceiptRef,
        outcome: StageAttemptOutcome,
    ) {
        self.stage_attempt_sink.push(PendingStageAttemptEvent {
            stage_call_id: self.stage_call_id,
            role,
            receipt,
            outcome,
        });
    }

    #[must_use]
    pub const fn stage_call_id(&self) -> StageCallId {
        self.stage_call_id
    }

    pub(crate) fn stage_attempt_sink(&self) -> StageAttemptEventSink {
        self.stage_attempt_sink.clone()
    }
}
```

## 4.4 scoped engine handoff

File:

```text
crates/leaven-engine/src/graph/scoped_view.rs
```

Target type:

```rust
pub struct ScopedRunGraphView<'a, P: OptimizationProblem> {
    graph: RunGraphView<'a, P>,
    read_scope: ReadScope,
}

impl<'a, P: OptimizationProblem> ScopedRunGraphView<'a, P> {
    pub(crate) fn new(graph: RunGraphView<'a, P>, read_scope: ReadScope) -> Self {
        Self { graph, read_scope }
    }

    pub fn candidate(&self, id: CandidateId) -> Option<CandidateView<'a, P>> {
        self.graph.candidate(id)
    }

    pub fn artifact(&self, id: CandidateId) -> Option<&'a P::Artifact> {
        self.graph.artifact(id)
    }

    pub fn assessment(&self, id: AssessmentId) -> Option<AssessmentView<'a>> {
        self.graph.assessment_visible_under(id, &self.read_scope)
    }

    pub fn assessments_for_candidate(
        &self,
        id: CandidateId,
        limit: Option<usize>,
    ) -> Vec<AssessmentView<'a>> {
        self.graph
            .assessments(id)
            .filter_visible_under(&self.read_scope)
            .take(limit.unwrap_or(usize::MAX))
            .collect()
    }

    pub fn list_candidates(
        &self,
        frontier_only: bool,
        page: PageRequest,
    ) -> Vec<CandidateSummary> {
        // candidate existence is visible in v0.4; assessment/evidence visibility is scoped.
        todo!()
    }

    #[must_use]
    pub const fn read_scope(&self) -> &ReadScope {
        &self.read_scope
    }
}
```

If current engine cannot implement `assessment_visible_under`, add the smallest helper that applies existing `RunGraphView` filtering. The important property is that `leaven-stage` cannot call the unfiltered assessment/evidence path.

## 4.5 drain sink in RunContext::propose

File:

```text
crates/leaven-engine/src/context/run_context.rs
```

Target behavior:

```rust
let sink = StageAttemptEventSink::new();
let stage_call_id = StageCallId::new();
let proposal_ctx = ProposalContext::new(
    graph_view,
    budget_handle,
    read_scope,
    stage_call_id,
    sink.clone(),
);

let result = proposer.propose(request, proposal_ctx).await;

for pending in sink.drain() {
    self.emit(RunEvent::StageAttemptRecorded {
        stage_call_id: pending.stage_call_id,
        role: pending.role,
        receipt: pending.receipt,
        outcome: pending.outcome,
    });
}

let metered = result?;
// continue existing proposal recording path
```

Drain before returning on both success and error.

---

# 5. milestone 4 — create `leaven-stage` crate skeleton

## Done when, no workarounds

```text
- cargo check -p leaven-stage passes.
- cargo metadata proves leaven-stage has no dependency on leaven-gepa or leaven-agentic.
- lib.rs exports are tiered: USER, ADAPTER, RECEIPT.
- Every public type in the stage crate has at least one serde roundtrip test where applicable.
- No old v0.3 materialization names are public exports.
```

Named tests:

```text
crates/leaven-stage/tests/serde_roundtrip.rs
crates/leaven-stage/tests/dependency_shape.rs
```

## Forbidden proxy proofs

```text
- leaven-stage avoids leaven-gepa in dependencies but pulls it in via dev-dependencies.
- serde tests pass because fields are skipped.
- lib.rs re-exports internals in the prelude, making user surface look huge.
```

## 5.1 Cargo.toml

File:

```text
crates/leaven-stage/Cargo.toml
```

Target:

```toml
[package]
name = "leaven-stage"
description = "Optimizer-stage agent workspace setup and query support for Leaven."
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

## 5.2 module layout

Files:

```text
crates/leaven-stage/src/lib.rs
crates/leaven-stage/src/agent_backed.rs
crates/leaven-stage/src/artifact.rs
crates/leaven-stage/src/bootstrap.rs
crates/leaven-stage/src/entry.rs
crates/leaven-stage/src/error.rs
crates/leaven-stage/src/id.rs
crates/leaven-stage/src/media.rs
crates/leaven-stage/src/output.rs
crates/leaven-stage/src/parser.rs
crates/leaven-stage/src/plan.rs
crates/leaven-stage/src/query.rs
crates/leaven-stage/src/read_authority.rs
crates/leaven-stage/src/receipt.rs
crates/leaven-stage/src/receipt_store.rs
crates/leaven-stage/src/setup.rs
crates/leaven-stage/src/slots.rs
crates/leaven-stage/src/tool.rs
```

`lib.rs`:

```rust
//! Optimizer-owned agentic stage workspace setup and query support.
//!
//! This crate is B-shaped: it helps Leaven's own optimizer stages give an
//! agent a bounded workspace and read back a typed decision. It is not a user
//! task-package framework.

pub mod agent_backed;
pub mod artifact;
pub mod bootstrap;
pub mod entry;
pub mod error;
pub mod id;
pub mod media;
pub mod output;
pub mod parser;
pub mod plan;
pub mod query;
pub mod read_authority;
pub mod receipt;
pub mod receipt_store;
pub mod setup;
pub mod slots;
pub mod tool;

// USER surface.
pub use agent_backed::{AgentBacked, AgentBackedPolicy, ParseFailurePolicy, ReceiptSinkPolicy};
pub use artifact::{MaterializableArtifact, MaterializationReport, ReconstructibleArtifact};
pub use bootstrap::AgentStageBootstrap;
pub use output::{OutputEntry, OutputEntryId, OutputRole, OutputSchema, StageOutputContract};
pub use parser::StageOutputParser;
pub use plan::{AgentStageCallContext, AgentStagePlan, StageDirective};
pub use query::{AllowedQuerySet, StageQuery, StageQueryKind, StageQueryPolicy};
pub use slots::{ProposerSlot, SlotMarker};
pub use leaven_kernel::StageRole;

// ADAPTER surface.
pub use entry::{EntryAccess, EntryProjection, EntrySource, Placement, WorkspaceEntry, WorkspaceEntryRole};
pub use read_authority::{QueryEffect, QueryResult, QueryTiming, StageReadAuthority};
pub use setup::{setup_stage_workspace, StageAttemptReceiptBuilder, WorkspaceSetupReceipt};

// RECEIPT/debug surface.
pub use receipt::{
    EntrySourceRef, OutputEntryReceipt, OutputEntryStatus, ParseReceipt, ParseStatus,
    QueryRecord, QueryRecordEffect, StageAttemptReceipt, WorkspaceEntryReceipt,
};

pub mod prelude {
    pub use crate::{
        AgentBacked, AgentBackedPolicy, AgentStageBootstrap, AgentStagePlan,
        MaterializableArtifact, OutputEntry, OutputRole, ProposerSlot, SlotMarker,
        StageDirective, StageOutputContract, StageOutputParser, StageQueryPolicy,
    };
    pub use leaven_kernel::StageRole;
}
```

Do not export `StageReadAuthority`, `StageAttemptReceipt`, or receipt internals in the prelude.

---

# 6. milestone 5 — USER surface definitions

## Done when, no workarounds

```text
- StageDirective, AgentStagePlan, StageQueryPolicy, StageOutputContract, SlotMarker, AgentBackedPolicy compile.
- StageQueryPolicy contains allowed + prewarm, not eager/lazy split.
- StageOutputContract validates output paths and rejects traversal.
- OutputRole and WorkspaceEntryRole use open-tag pattern.
- AgentBacked has four type parameters and derives output from SlotMarker.
```

Named tests:

```text
crates/leaven-stage/tests/output_contract.rs::output_paths_must_be_under_output
crates/leaven-stage/tests/output_contract.rs::output_paths_reject_parent_traversal
crates/leaven-stage/tests/query_policy.rs::prewarm_queries_count_as_queries
crates/leaven-stage/tests/slot_marker.rs::proposer_slot_output_is_proposal_batch
```

## 6.1 media type

File:

```text
crates/leaven-stage/src/media.rs
```

Target:

```rust
#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, serde::Serialize, serde::Deserialize)]
pub enum MediaType {
    Json,
    Markdown,
    Text,
    Diff,
    Binary,
    Custom(SmolStr),
}

impl MediaType {
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            Self::Json => "application/json",
            Self::Markdown => "text/markdown",
            Self::Text => "text/plain",
            Self::Diff => "text/x-diff",
            Self::Binary => "application/octet-stream",
            Self::Custom(value) => value.as_str(),
        }
    }
}
```

## 6.2 output contract

File:

```text
crates/leaven-stage/src/output.rs
```

Target:

```rust
use leaven_workspace::WorkspacePath;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

use crate::{MediaType, StageOutputContractError};

#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct OutputEntryId(SmolStr);

impl OutputEntryId {
    pub fn new(value: impl Into<SmolStr>) -> Result<Self, StageOutputContractError> {
        let value = value.into();
        if value.is_empty() || value.contains('/') || value.chars().any(char::is_whitespace) {
            return Err(StageOutputContractError::InvalidEntryId(value.to_string()));
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn new_static(value: &'static str) -> Self {
        Self(SmolStr::new_static(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct OutputRole(SmolStr);

impl OutputRole {
    pub fn new(value: impl Into<SmolStr>) -> Result<Self, StageOutputContractError> {
        let value = value.into();
        if value.is_empty() || value.contains('/') || value.chars().any(char::is_whitespace) {
            return Err(StageOutputContractError::InvalidOutputRole(value.to_string()));
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn new_static(value: &'static str) -> Self {
        Self(SmolStr::new_static(value))
    }

    #[must_use]
    pub fn proposal_json() -> Self { Self::new_static("proposal_json") }
    #[must_use]
    pub fn candidate_selection() -> Self { Self::new_static("candidate_selection") }
    #[must_use]
    pub fn part_selection() -> Self { Self::new_static("part_selection") }
    #[must_use]
    pub fn merge_plan() -> Self { Self::new_static("merge_plan") }
    #[must_use]
    pub fn acceptance_decision() -> Self { Self::new_static("acceptance_decision") }
    #[must_use]
    pub fn notes() -> Self { Self::new_static("notes") }
    #[must_use]
    pub fn patch() -> Self { Self::new_static("patch") }
    #[must_use]
    pub fn workspace_diff() -> Self { Self::new_static("workspace_diff") }

    #[must_use]
    pub fn as_str(&self) -> &str { self.0.as_str() }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OutputSchema {
    pub media_type: MediaType,
    pub schema_text: String,
    pub schema_fingerprint: Option<Fingerprint>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OutputEntry {
    pub id: OutputEntryId,
    pub path: WorkspacePath,
    pub role: OutputRole,
    pub media_type: MediaType,
    pub max_bytes: Option<u64>,
}

impl OutputEntry {
    #[must_use]
    pub fn new(
        id: OutputEntryId,
        path: WorkspacePath,
        role: OutputRole,
        media_type: MediaType,
    ) -> Self {
        Self { id, path, role, media_type, max_bytes: None }
    }

    #[must_use]
    pub fn with_max_bytes(mut self, max_bytes: u64) -> Self {
        self.max_bytes = Some(max_bytes);
        self
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StageOutputContract {
    pub required: Vec<OutputEntry>,
    pub optional: Vec<OutputEntry>,
    pub schema: Option<OutputSchema>,
}

impl StageOutputContract {
    #[must_use]
    pub fn new(required: Vec<OutputEntry>) -> Self {
        Self { required, optional: Vec::new(), schema: None }
    }

    #[must_use]
    pub fn proposal_json(path: WorkspacePath) -> Self {
        Self::new(vec![OutputEntry::new(
            OutputEntryId::new_static("proposal"),
            path,
            OutputRole::proposal_json(),
            MediaType::Json,
        )])
    }

    #[must_use]
    pub fn with_optional(mut self, entry: OutputEntry) -> Self {
        self.optional.push(entry);
        self
    }

    #[must_use]
    pub fn with_schema(mut self, schema: OutputSchema) -> Self {
        self.schema = Some(schema);
        self
    }

    pub fn validate(&self) -> Result<(), StageOutputContractError> {
        if self.required.is_empty() {
            return Err(StageOutputContractError::NoRequiredOutputs);
        }
        for entry in self.required.iter().chain(self.optional.iter()) {
            validate_output_path(&entry.path).map_err(|source| {
                StageOutputContractError::InvalidOutputPath {
                    id: entry.id.clone(),
                    path: entry.path.clone(),
                    source,
                }
            })?;
        }
        Ok(())
    }

    #[must_use]
    pub fn all_entries(&self) -> impl Iterator<Item = &OutputEntry> {
        self.required.iter().chain(self.optional.iter())
    }

    #[must_use]
    pub fn to_agent_output_contract(&self) -> leaven_agent::OutputContract {
        if self.required.len() == 1 && self.optional.is_empty() {
            let entry = &self.required[0];
            if entry.media_type == MediaType::Json {
                return leaven_agent::OutputContract::JsonFile {
                    path: entry.path.clone(),
                    schema: self.schema.as_ref().map(|schema| leaven_agent::JsonSchemaRef {
                        name: entry.id.as_str().to_owned(),
                        schema: schema.schema_text.clone(),
                    }),
                };
            }
        }
        leaven_agent::OutputContract::Files {
            paths: self.all_entries().map(|entry| entry.path.clone()).collect(),
        }
    }
}

fn validate_output_path(path: &WorkspacePath) -> Result<(), WorkspacePathError> {
    // Implementation requirement:
    // - path must be under output/
    // - path cannot be output/../x or use any parent traversal segment
    // - path cannot be absolute; WorkspacePath should already reject absolute paths
    // - path cannot be exactly output/ as a file entry
    if !path.starts_with_component("output") {
        return Err(WorkspacePathError::outside_view(path.clone()));
    }
    if path.has_parent_traversal() {
        return Err(WorkspacePathError::outside_view(path.clone()));
    }
    Ok(())
}
```

If `WorkspacePath` does not yet expose `starts_with_component` or `has_parent_traversal`, add segment-level helpers there. Do not validate by raw string prefix.

## 6.3 plan and directive

File:

```text
crates/leaven-stage/src/plan.rs
```

Target:

```rust
use leaven_core::OptimizationProblem;
use leaven_kernel::{BudgetSnapshot, MetadataBag, StageCallId, StageRole};
use serde::{Deserialize, Serialize};

use crate::{StageOutputContract, StageQueryPolicy};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StageDirective {
    pub title: String,
    pub instructions: String,
    pub success_criteria: Vec<String>,
    pub cautions: Vec<String>,
}

impl StageDirective {
    #[must_use]
    pub fn new(title: impl Into<String>, instructions: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            instructions: instructions.into(),
            success_criteria: Vec::new(),
            cautions: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_success_criterion(mut self, criterion: impl Into<String>) -> Self {
        self.success_criteria.push(criterion.into());
        self
    }

    #[must_use]
    pub fn with_caution(mut self, caution: impl Into<String>) -> Self {
        self.cautions.push(caution.into());
        self
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentStagePlan<Req> {
    pub role: StageRole,
    pub request: Req,
    pub directive: StageDirective,
    pub query: StageQueryPolicy,
    pub output: StageOutputContract,
    pub metadata: MetadataBag,
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
            query: StageQueryPolicy::minimal(),
            output,
            metadata: MetadataBag::new(),
        }
    }

    #[must_use]
    pub fn with_query_policy(mut self, query: StageQueryPolicy) -> Self {
        self.query = query;
        self
    }

    #[must_use]
    pub fn with_metadata(mut self, metadata: MetadataBag) -> Self {
        self.metadata = metadata;
        self
    }
}

pub struct AgentStageCallContext<'a, P: OptimizationProblem> {
    engine: &'a crate::StageEngineContext<'a, P>,
}

impl<'a, P: OptimizationProblem> AgentStageCallContext<'a, P> {
    #[must_use]
    pub fn stage_call_id(&self) -> StageCallId { self.engine.stage_call_id() }
    #[must_use]
    pub fn read_scope(&self) -> &ReadScope { self.engine.read_scope() }
    #[must_use]
    pub fn budget_snapshot(&self) -> BudgetSnapshot { self.engine.budget_snapshot() }

    pub fn visible_candidate_ids(
        &self,
        limit: Option<usize>,
    ) -> Result<Vec<CandidateId>, StageReadError> {
        self.engine.graph().visible_candidate_ids(limit)
    }

    pub fn candidate_summary(
        &self,
        candidate: CandidateId,
    ) -> Result<CandidateSummary, StageReadError> {
        self.engine.graph().candidate_summary(candidate)
    }

    pub fn assessment_summary(
        &self,
        assessment: AssessmentId,
    ) -> Result<AssessmentSummary, StageReadError> {
        self.engine.graph().assessment_summary(assessment)
    }
}
```

## 6.4 StageQueryPolicy and query values

File:

```text
crates/leaven-stage/src/query.rs
```

Target:

```rust
use std::collections::BTreeSet;
use leaven_kernel::{AssessmentId, CandidateId, InfoRef};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StageQueryPolicy {
    pub allowed: AllowedQuerySet,
    pub prewarm: Vec<StageQuery>,
    pub max_queries: Option<usize>,
    pub max_materialized_bytes: Option<u64>,
}

impl StageQueryPolicy {
    #[must_use]
    pub fn minimal() -> Self {
        Self {
            allowed: AllowedQuerySet::none(),
            prewarm: Vec::new(),
            max_queries: Some(0),
            max_materialized_bytes: Some(0),
        }
    }

    #[must_use]
    pub fn help_only() -> Self {
        Self {
            allowed: AllowedQuerySet::only([StageQueryKind::Help]),
            prewarm: Vec::new(),
            max_queries: Some(4),
            max_materialized_bytes: Some(0),
        }
    }

    #[must_use]
    pub fn focus_candidate(candidate: CandidateId) -> Self {
        Self {
            allowed: AllowedQuerySet::reflection_default(),
            prewarm: vec![StageQuery::Candidate(CandidateQuery {
                id: candidate,
                projection: CandidateProjection::ArtifactAndAssessments { limit: Some(4) },
            })],
            max_queries: Some(32),
            max_materialized_bytes: Some(32 * 1024 * 1024),
        }
    }

    #[must_use]
    pub fn reflection_default(parent: CandidateId, selected: SelectedFeedbackRefs) -> Self {
        let mut prewarm = vec![StageQuery::Candidate(CandidateQuery {
            id: parent,
            projection: CandidateProjection::ArtifactAndAssessments { limit: Some(8) },
        })];
        for assessment in selected.assessment_refs {
            prewarm.push(StageQuery::Assessment(AssessmentQuery {
                id: assessment,
                projection: AssessmentProjection::FeedbackSummary,
            }));
        }
        for evidence in selected.evidence_refs {
            prewarm.push(StageQuery::Evidence(EvidenceQuery {
                reference: evidence,
                projection: EvidenceProjection::Summary,
            }));
        }
        Self {
            allowed: AllowedQuerySet::reflection_default(),
            prewarm,
            max_queries: Some(64),
            max_materialized_bytes: Some(64 * 1024 * 1024),
        }
    }

    #[must_use]
    pub fn bounded(
        allowed: AllowedQuerySet,
        prewarm: Vec<StageQuery>,
        max_queries: Option<usize>,
        max_materialized_bytes: Option<u64>,
    ) -> Self {
        Self { allowed, prewarm, max_queries, max_materialized_bytes }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AllowedQuerySet {
    allowed: BTreeSet<StageQueryKind>,
}

impl AllowedQuerySet {
    #[must_use]
    pub fn none() -> Self { Self { allowed: BTreeSet::new() } }

    #[must_use]
    pub fn only(kinds: impl IntoIterator<Item = StageQueryKind>) -> Self {
        Self { allowed: kinds.into_iter().collect() }
    }

    #[must_use]
    pub fn all_v0_4() -> Self {
        Self::only([
            StageQueryKind::Help,
            StageQueryKind::ListCandidates,
            StageQueryKind::Candidate,
            StageQueryKind::Assessment,
            StageQueryKind::Evidence,
            StageQueryKind::Lineage,
            StageQueryKind::Diff,
        ])
    }

    #[must_use]
    pub fn reflection_default() -> Self {
        Self::only([
            StageQueryKind::Help,
            StageQueryKind::Candidate,
            StageQueryKind::Assessment,
            StageQueryKind::Evidence,
            StageQueryKind::Lineage,
            StageQueryKind::Diff,
        ])
    }

    #[must_use]
    pub fn contains(&self, kind: StageQueryKind) -> bool {
        self.allowed.contains(&kind)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub enum StageQueryKind {
    Help,
    ListCandidates,
    Candidate,
    Assessment,
    Evidence,
    Lineage,
    Diff,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum StageQuery {
    Help,
    ListCandidates(ListCandidatesQuery),
    Candidate(CandidateQuery),
    Assessment(AssessmentQuery),
    Evidence(EvidenceQuery),
    Lineage(LineageQuery),
    Diff(DiffQuery),
}

impl StageQuery {
    #[must_use]
    pub const fn kind(&self) -> StageQueryKind {
        match self {
            Self::Help => StageQueryKind::Help,
            Self::ListCandidates(_) => StageQueryKind::ListCandidates,
            Self::Candidate(_) => StageQueryKind::Candidate,
            Self::Assessment(_) => StageQueryKind::Assessment,
            Self::Evidence(_) => StageQueryKind::Evidence,
            Self::Lineage(_) => StageQueryKind::Lineage,
            Self::Diff(_) => StageQueryKind::Diff,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ListCandidatesQuery {
    pub frontier_only: bool,
    pub page: PageRequest,
    pub projection: CandidateListProjection,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CandidateQuery {
    pub id: CandidateId,
    pub projection: CandidateProjection,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AssessmentQuery {
    pub id: AssessmentId,
    pub projection: AssessmentProjection,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EvidenceQuery {
    pub reference: InfoRef,
    pub projection: EvidenceProjection,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LineageQuery {
    pub candidate: CandidateId,
    pub depth: usize,
    pub projection: LineageProjection,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DiffQuery {
    pub left: CandidateId,
    pub right: CandidateId,
    pub projection: DiffProjection,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PageRequest {
    pub page: usize,
    pub page_size: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum CandidateListProjection { IdsOnly, Summary, SummaryWithScores }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum CandidateProjection {
    Summary,
    Artifact,
    Assessments { limit: Option<usize> },
    ArtifactAndAssessments { limit: Option<usize> },
    FullWithinPolicy,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AssessmentProjection {
    Summary,
    Scores,
    FeedbackSummary,
    WithEvidence { limit: Option<usize> },
    WithTrace { limit: Option<usize> },
    FullWithinPolicy,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum EvidenceProjection {
    Summary,
    Rendered,
    TraceExcerpt { max_events: usize },
    FullWithinPolicy,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum LineageProjection {
    Summary,
    Tree,
    WithAssessments { assessment_limit_per_candidate: Option<usize> },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum DiffProjection { Summary, ArtifactDiff, AssessmentDelta }

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SelectedFeedbackRefs {
    pub assessment_refs: Vec<AssessmentId>,
    pub evidence_refs: Vec<InfoRef>,
    pub candidate_refs: Vec<CandidateId>,
}
```

There is no `Search` in v0.4.

## 6.5 SlotMarker and AgentBackedPolicy

File:

```text
crates/leaven-stage/src/slots.rs
```

Target:

```rust
use std::marker::PhantomData;
use leaven_core::{OptimizationProblem, ProposalBatch};
use leaven_kernel::StageRole;

pub trait SlotMarker<P>: Send + Sync + 'static
where
    P: OptimizationProblem,
{
    type Request: serde::Serialize + Send + Sync + 'static;
    type Output: Send + Sync + 'static;

    fn role() -> StageRole;
}

pub struct ProposerSlot<Req>(PhantomData<Req>);

impl<P, Req> SlotMarker<P> for ProposerSlot<Req>
where
    P: OptimizationProblem,
    Req: serde::Serialize + Send + Sync + 'static,
{
    type Request = Req;
    type Output = ProposalBatch<P>;

    fn role() -> StageRole {
        StageRole::reflect()
    }
}
```

File:

```text
crates/leaven-stage/src/agent_backed.rs
```

User-facing policy shape:

```rust
#[derive(Clone, Debug)]
pub struct AgentBackedPolicy {
    pub workspace: WorkspaceConfig,
    pub runtime_timeout: Option<Duration>,
    pub on_parse_failure: ParseFailurePolicy,
    pub cleanup: CleanupPolicy,
    pub tool_exposure: LeavenQueryExposure,
    pub receipt_sink: ReceiptSinkPolicy,
}

impl Default for AgentBackedPolicy {
    fn default() -> Self {
        Self {
            workspace: WorkspaceConfig::default(),
            runtime_timeout: None,
            on_parse_failure: ParseFailurePolicy::Strict,
            cleanup: CleanupPolicy::Always,
            tool_exposure: LeavenQueryExposure::ShellTool {
                path: WorkspacePath::new("tools/leaven_query").expect("static path"),
            },
            receipt_sink: ReceiptSinkPolicy::Inline,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParseFailurePolicy {
    Strict,
    RecordAttempt,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CleanupPolicy {
    Always,
    KeepOnFailure,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReceiptSinkPolicy {
    Inline,
    External { sink: ReceiptSinkId },
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct ReceiptSinkId(SmolStr);

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LeavenQueryExposure {
    ShellTool { path: WorkspacePath },
    StructuredTool { name: SmolStr },
    Both { shell_path: WorkspacePath, structured_name: SmolStr },
    Disabled,
}
```

No `record_receipt: bool`. Receipts are mandatory.

## 6.6 central error and summary types

File:

```text
crates/leaven-stage/src/error.rs
```

These types are referenced by the user, adapter, receipt, setup, query, parser, and artifact modules. Define them once here. Later code blocks in this plan use these definitions.

```rust
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Diagnostic {
    pub severity: DiagnosticSeverity,
    pub message: String,
    pub code: Option<String>,
    pub metadata: MetadataBag,
}

impl Diagnostic {
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            severity: DiagnosticSeverity::Error,
            message: message.into(),
            code: None,
            metadata: MetadataBag::new(),
        }
    }

    pub fn warning(message: impl Into<String>) -> Self {
        Self {
            severity: DiagnosticSeverity::Warning,
            message: message.into(),
            code: None,
            metadata: MetadataBag::new(),
        }
    }

    pub fn info(message: impl Into<String>) -> Self {
        Self {
            severity: DiagnosticSeverity::Info,
            message: message.into(),
            code: None,
            metadata: MetadataBag::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum DiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, thiserror::Error)]
pub enum StageOutputContractError {
    #[error("stage output contract must require at least one output")]
    NoRequiredOutputs,

    #[error("invalid output entry id `{0}`")]
    InvalidEntryId(String),

    #[error("invalid output role `{0}`")]
    InvalidOutputRole(String),

    #[error("output `{id:?}` path `{path}` is invalid: {source}")]
    InvalidOutputPath {
        id: OutputEntryId,
        path: WorkspacePath,
        #[source]
        source: WorkspacePathError,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum WorkspaceSetupError {
    #[error(transparent)]
    Workspace(#[from] WorkspaceError),

    #[error(transparent)]
    WorkspacePath(#[from] WorkspacePathError),

    #[error(transparent)]
    OutputContract(#[from] StageOutputContractError),

    #[error(transparent)]
    Query(#[from] StageQueryError),

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[error("budget/accounting failure: {0}")]
    Budget(String),

    #[error("workspace setup failed: {0:?}")]
    Diagnostic(Diagnostic),
}

#[derive(Debug, thiserror::Error)]
pub enum StageReadError {
    #[error("candidate not found or not visible: {0}")]
    CandidateNotVisible(CandidateId),

    #[error("assessment not found or not visible: {0}")]
    AssessmentNotVisible(AssessmentId),

    #[error("evidence not found or not visible: {0:?}")]
    EvidenceNotVisible(InfoRef),

    #[error("stage read failed: {0:?}")]
    Diagnostic(Diagnostic),
}

#[derive(Debug, thiserror::Error)]
pub enum StageQueryError {
    #[error("query policy denied request: {0:?}")]
    PolicyDenied(PolicyDenial),

    #[error("query target not visible: {0:?}")]
    NotVisible(NotVisibleReason),

    #[error("query target not found: {0:?}")]
    NotFound(NotFoundReason),

    #[error(transparent)]
    Workspace(#[from] WorkspaceError),

    #[error(transparent)]
    WorkspacePath(#[from] WorkspacePathError),

    #[error("stage query failed: {0:?}")]
    Diagnostic(Diagnostic),
}

#[derive(Debug, thiserror::Error)]
pub enum StageBootstrapError {
    #[error(transparent)]
    Read(#[from] StageReadError),

    #[error("stage bootstrap produced invalid plan: {0:?}")]
    InvalidPlan(Diagnostic),

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[error("stage bootstrap failed: {0:?}")]
    Diagnostic(Diagnostic),
}

#[derive(Debug, thiserror::Error)]
pub enum StageOutputParseError {
    #[error("required output `{entry:?}` missing at `{path}`")]
    MissingRequiredOutput { entry: OutputEntryId, path: WorkspacePath },

    #[error("output `{entry:?}` at `{path}` exceeded max bytes: {actual} > {max}")]
    TooLarge { entry: OutputEntryId, path: WorkspacePath, actual: u64, max: u64 },

    #[error("malformed output at `{path}`: {diagnostic:?}")]
    Malformed { path: WorkspacePath, diagnostic: Diagnostic },

    #[error(transparent)]
    Workspace(#[from] WorkspaceError),

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[error("stage output parse failed: {0:?}")]
    Diagnostic(Diagnostic),
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct CandidateSummary {
    pub id: CandidateId,
    pub identity: String,
    pub origin: String,
    pub visible_assessment_count: usize,
    pub metadata: MetadataBag,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct AssessmentSummary {
    pub id: AssessmentId,
    pub evaluator: String,
    pub target: String,
    pub evidence_ref: Option<InfoRef>,
    pub score_summary: Option<String>,
    pub feedback_summary: Option<String>,
    pub metadata: MetadataBag,
}
```

If any of these names already exists in the engine or kernel with equivalent behavior, re-export or reuse the existing type rather than duplicating it. The behavior is the contract.

---

# 7. milestone 6 — RECEIPT and ADAPTER data definitions

## Done when, no workarounds

```text
- StageAttemptReceipt stores full ReadScope and read_scope_fingerprint.
- WorkspaceSetupReceipt separates setup files from query-derived entries.
- QueryRecord has QueryTiming and QueryRecordEffect, no eager/lazy materialization split.
- WorkspaceEntryReceipt has source ref, projection, fingerprint, bytes, query id.
- StageAttemptReceipt serde roundtrip is information-preserving.
```

Named tests:

```text
crates/leaven-stage/tests/receipt_roundtrip.rs::stage_attempt_receipt_roundtrips
crates/leaven-stage/tests/receipt_roundtrip.rs::query_record_roundtrips_not_visible
crates/leaven-stage/tests/receipt_roundtrip.rs::workspace_entry_receipt_records_source_and_projection
```

## 7.1 entry types

File:

```text
crates/leaven-stage/src/entry.rs
```

Target:

```rust
use leaven_kernel::{AssessmentId, CandidateId, InfoRef, MetadataBag, ProposalId, RenderedViewId, WorkspaceEntryId};
use leaven_workspace::WorkspacePath;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

use crate::{
    AssessmentProjection, CandidateProjection, DiffProjection, EvidenceProjection,
    LineageProjection, MediaType,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkspaceEntry {
    pub id: WorkspaceEntryId,
    pub role: WorkspaceEntryRole,
    pub source: EntrySource,
    pub projection: EntryProjection,
    pub placement: Placement,
    pub access: EntryAccess,
    pub media_type: Option<MediaType>,
    pub max_bytes: Option<u64>,
    pub metadata: MetadataBag,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WorkspaceEntryRole(SmolStr);

impl WorkspaceEntryRole {
    pub fn new(value: impl Into<SmolStr>) -> Result<Self, WorkspaceEntryRoleError>;
    pub fn new_static(value: &'static str) -> Self;
    pub fn as_str(&self) -> &str;

    pub fn brief() -> Self { Self::new_static("brief") }
    pub fn focus_request() -> Self { Self::new_static("focus_request") }
    pub fn stage_instructions() -> Self { Self::new_static("stage_instructions") }
    pub fn candidate_artifact() -> Self { Self::new_static("candidate_artifact") }
    pub fn selected_part() -> Self { Self::new_static("selected_part") }
    pub fn selected_feedback() -> Self { Self::new_static("selected_feedback") }
    pub fn trace_excerpt() -> Self { Self::new_static("trace_excerpt") }
    pub fn assessment_summary() -> Self { Self::new_static("assessment_summary") }
    pub fn evidence_summary() -> Self { Self::new_static("evidence_summary") }
    pub fn lineage_summary() -> Self { Self::new_static("lineage_summary") }
    pub fn frontier_summary() -> Self { Self::new_static("frontier_summary") }
    pub fn tree_summary() -> Self { Self::new_static("tree_summary") }
    pub fn tool_config() -> Self { Self::new_static("tool_config") }
    pub fn runtime_config() -> Self { Self::new_static("runtime_config") }
    pub fn output_schema() -> Self { Self::new_static("output_schema") }
    pub fn output_directory() -> Self { Self::new_static("output_directory") }
}

#[derive(Debug, thiserror::Error)]
#[error("invalid workspace entry role `{0}`")]
pub struct WorkspaceEntryRoleError(pub String);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum EntrySource {
    InlineText(String),
    InlineBytes(Vec<u8>),
    Generated,
    Candidate(CandidateId),
    Assessment(AssessmentId),
    Evidence(InfoRef),
    Proposal(ProposalId),
    RenderedView(RenderedViewId),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum EntrySourceRef {
    Inline,
    Generated,
    Candidate(CandidateId),
    Assessment(AssessmentId),
    Evidence(InfoRef),
    Proposal(ProposalId),
    RenderedView(RenderedViewId),
    FactoryContext { type_name: String },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum EntryProjection {
    Full,
    Summary,
    Candidate(CandidateProjection),
    Assessment(AssessmentProjection),
    Evidence(EvidenceProjection),
    Lineage(LineageProjection),
    Diff(DiffProjection),
    Inline,
    Generated,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Placement {
    pub path: WorkspacePath,
    pub collision: CollisionPolicy,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum CollisionPolicy {
    Error,
    OverwriteIfSameFingerprint,
    Overwrite,
    CreateSibling,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum EntryAccess {
    InputReadOnly,
    EditableArtifact,
    OutputWritable,
}
```

## 7.2 receipt types

File:

```text
crates/leaven-stage/src/receipt.rs
```

Target:

```rust
use leaven_kernel::{
    AgentSessionId, Cost, Fingerprint, MetadataBag, StageAttemptOutcome,
    StageAttemptReceiptId, StageCallId, StageQueryId, StageRole, WorkspaceEntryId,
    WorkspaceId,
};
use leaven_workspace::{WorkspaceFileFingerprint, WorkspacePath};
use serde::{Deserialize, Serialize};

use crate::{
    EntryProjection, EntrySourceRef, LeavenQueryExposure, OutputEntryId, OutputRole,
    StageQuery, WorkspaceEntryRole,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StageAttemptReceipt {
    pub receipt_id: StageAttemptReceiptId,
    pub workspace_id: WorkspaceId,
    pub stage_call_id: StageCallId,
    pub role: StageRole,
    pub read_scope: ReadScope,
    pub read_scope_fingerprint: Fingerprint,
    pub plan_fingerprint: Fingerprint,
    pub setup: WorkspaceSetupReceipt,
    pub queries: Vec<QueryRecord>,
    pub outputs: Vec<OutputEntryReceipt>,
    pub parse: Option<ParseReceipt>,
    pub session: Option<AgentSessionId>,
    pub cost: Cost,
    pub outcome: StageAttemptOutcome,
    pub metadata: MetadataBag,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkspaceSetupReceipt {
    pub plan_entries: Vec<WorkspaceEntryReceipt>,
    pub query_tool: Option<LeavenQueryExposureReceipt>,
    pub diagnostics: Vec<Diagnostic>,
    pub cost: Cost,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LeavenQueryExposureReceipt {
    pub exposure: LeavenQueryExposure,
    pub path: Option<WorkspacePath>,
    pub fingerprint: Option<Fingerprint>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QueryRecord {
    pub query_id: StageQueryId,
    pub timing: QueryTiming,
    pub query: StageQuery,
    pub effect: QueryRecordEffect,
    pub entries: Vec<WorkspaceEntryReceipt>,
    pub cost: Cost,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum QueryTiming {
    Prewarm,
    AgentRequested,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum QueryRecordEffect {
    WroteEntries(Vec<WorkspaceEntryId>),
    ReturnedSummary(QuerySummary),
    NotVisible(NotVisibleReason),
    NotFound(NotFoundReason),
    PolicyDenied(PolicyDenial),
    Error(Vec<Diagnostic>),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkspaceEntryReceipt {
    pub id: WorkspaceEntryId,
    pub path: WorkspacePath,
    pub role: WorkspaceEntryRole,
    pub source: EntrySourceRef,
    pub projection: EntryProjection,
    pub access: EntryAccess,
    pub fingerprint: Fingerprint,
    pub file: Option<WorkspaceFileFingerprint>,
    pub bytes: Option<u64>,
    pub truncation: Option<TruncationNote>,
    pub produced_by_query: Option<StageQueryId>,
    pub metadata: MetadataBag,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OutputEntryReceipt {
    pub id: OutputEntryId,
    pub path: WorkspacePath,
    pub role: OutputRole,
    pub fingerprint: Option<Fingerprint>,
    pub file: Option<WorkspaceFileFingerprint>,
    pub bytes: Option<u64>,
    pub status: OutputEntryStatus,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum OutputEntryStatus {
    Present,
    Missing,
    TooLarge,
    InvalidMedia,
    NotRead,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ParseReceipt {
    pub status: ParseStatus,
    pub diagnostics: Vec<Diagnostic>,
    pub files_read: Vec<WorkspacePath>,
    pub cost: Cost,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ParseStatus {
    NotAttempted,
    Succeeded,
    Failed,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TruncationNote {
    pub original_bytes: Option<u64>,
    pub written_bytes: u64,
    pub reason: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QuerySummary {
    pub message: String,
    pub paths: Vec<WorkspacePath>,
    pub metadata: MetadataBag,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NotVisibleReason {
    pub message: String,
    pub requested: MetadataBag,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NotFoundReason {
    pub message: String,
    pub requested: MetadataBag,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PolicyDenial {
    pub message: String,
    pub violated: PolicyViolationKind,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum PolicyViolationKind {
    QueryKindDisabled,
    QueryLimitExceeded,
    MaterializedBytesExceeded,
    PathEscape,
    InvalidArguments,
}
```

If `ReadScope` or `Diagnostic` are not serializable yet, make them serializable or create exact serializable mirrors. Do not replace full `ReadScope` with a digest.

## 7.3 receipt store

File:

```text
crates/leaven-stage/src/receipt_store.rs
```

Target:

```rust
#[allow(async_fn_in_trait)]
pub trait StageReceiptStore: Send + Sync {
    async fn write(
        &self,
        receipt: StageAttemptReceipt,
    ) -> Result<StageAttemptReceiptRef, ReceiptStoreError>;

    async fn read(
        &self,
        id: StageAttemptReceiptId,
    ) -> Result<Option<StageAttemptReceipt>, ReceiptStoreError>;
}

#[derive(Debug, thiserror::Error)]
pub enum ReceiptStoreError {
    #[error("receipt serialization failed: {0}")]
    Serialize(String),

    #[error("receipt store failed: {0}")]
    Store(String),
}

#[derive(Clone, Default)]
pub struct InlineReceiptStore {
    receipts: Arc<Mutex<BTreeMap<StageAttemptReceiptId, StageAttemptReceipt>>>,
}
```

For the first proof, inline is acceptable. External sinks can come later.

---

# 8. milestone 7 — StageReadAuthority and query executor

## Done when, no workarounds

```text
- StageReadAuthority is the only code path for query-derived workspace entries.
- StageReadAuthority::query enforces AllowedQuerySet before graph/evidence lookup.
- Prewarm and agent-requested queries use the same method.
- Query caps are checked before expensive materialization.
- NotVisible is distinct from NotFound.
```

Named tests:

```text
crates/leaven-stage/tests/stage_query.rs::disabled_query_kind_is_policy_denied
crates/leaven-stage/tests/stage_query.rs::visible_candidate_query_writes_entry
crates/leaven-stage/tests/stage_query.rs::hidden_assessment_is_not_visible_not_not_found
crates/leaven-stage/tests/stage_query.rs::query_limit_enforced_before_lookup
crates/leaven-stage/tests/stage_query.rs::prewarm_and_agent_requested_share_authority
```

## Forbidden proxy proofs

```text
- Candidate query uses graph.artifact directly and bypasses ReadScope for assessments/evidence.
- Hidden-assessment test uses missing id rather than existing-but-hidden id.
- Byte cap is checked after reading artifact bytes.
- Query help text is hardcoded and drifts from StageQuery variants.
```

## 8.1 StageEngineContext

File:

```text
crates/leaven-stage/src/read_authority.rs
```

Read-authority support types used in this section:

```rust
#[allow(async_fn_in_trait)]
pub trait EvidenceStore<E>: Send + Sync {
    async fn get(&self, reference: &InfoRef) -> Result<Option<EvidenceEnvelope<E>>, EvidenceReadError>;
}

#[derive(Clone, Debug)]
pub struct EvidenceEnvelope<E> {
    pub reference: InfoRef,
    pub evidence: E,
    pub fingerprint: Fingerprint,
    pub metadata: MetadataBag,
}

#[derive(Debug, thiserror::Error)]
pub enum EvidenceReadError {
    #[error("evidence store failed: {0}")]
    Store(String),
}
```

Target shape:

```rust
pub struct StageEngineContext<'a, P: OptimizationProblem> {
    graph: ScopedRunGraphView<'a, P>,
    read_scope: ReadScope,
    evidence_store: Option<&'a dyn EvidenceStore<P::Evidence>>,
    budget: BudgetHandle<'a>,
    budget_snapshot: BudgetSnapshot,
    stage_call_id: StageCallId,
}

impl<'a, P: OptimizationProblem> StageEngineContext<'a, P> {
    pub fn new(
        graph: ScopedRunGraphView<'a, P>,
        read_scope: ReadScope,
        evidence_store: Option<&'a dyn EvidenceStore<P::Evidence>>,
        budget: BudgetHandle<'a>,
        budget_snapshot: BudgetSnapshot,
        stage_call_id: StageCallId,
    ) -> Self { ... }

    #[must_use]
    pub const fn graph(&self) -> &ScopedRunGraphView<'a, P> { &self.graph }
    #[must_use]
    pub const fn read_scope(&self) -> &ReadScope { &self.read_scope }
    #[must_use]
    pub const fn budget_snapshot(&self) -> BudgetSnapshot { self.budget_snapshot }
    #[must_use]
    pub const fn stage_call_id(&self) -> StageCallId { self.stage_call_id }
}
```

## 8.2 StageReadAuthority

Same file:

```rust
pub struct StageReadAuthority<'a, P: OptimizationProblem> {
    graph: ScopedRunGraphView<'a, P>,
    read_scope: ReadScope,
    evidence_store: Option<&'a dyn EvidenceStore<P::Evidence>>,
    budget: BudgetHandle<'a>,
    stage_call_id: StageCallId,
    counters: QueryLimitState,
}

#[derive(Clone, Debug)]
pub struct QueryLimitState {
    pub max_queries: Option<usize>,
    pub queries_used: usize,
    pub max_materialized_bytes: Option<u64>,
    pub materialized_bytes: u64,
}

impl<'a, P: OptimizationProblem> StageReadAuthority<'a, P> {
    pub fn from_engine_context(
        ctx: StageEngineContext<'a, P>,
        policy: &StageQueryPolicy,
    ) -> Self {
        Self {
            graph: ctx.graph,
            read_scope: ctx.read_scope,
            evidence_store: ctx.evidence_store,
            budget: ctx.budget,
            stage_call_id: ctx.stage_call_id,
            counters: QueryLimitState {
                max_queries: policy.max_queries,
                queries_used: 0,
                max_materialized_bytes: policy.max_materialized_bytes,
                materialized_bytes: 0,
            },
        }
    }

    #[must_use]
    pub const fn read_scope(&self) -> &ReadScope { &self.read_scope }

    pub async fn query(
        &mut self,
        query: StageQuery,
        workspace: &mut WorkspaceView<'_>,
        timing: QueryTiming,
        policy: &StageQueryPolicy,
    ) -> Result<QueryResult, StageQueryError> {
        if !policy.allowed.contains(query.kind()) {
            return Ok(QueryResult::policy_denied(
                query,
                timing,
                PolicyViolationKind::QueryKindDisabled,
            ));
        }
        self.reserve_query(&query)?;
        match query {
            StageQuery::Help => self.query_help(timing).await,
            StageQuery::ListCandidates(q) => self.query_list_candidates(q, workspace, timing).await,
            StageQuery::Candidate(q) => self.query_candidate(q, workspace, timing).await,
            StageQuery::Assessment(q) => self.query_assessment(q, workspace, timing).await,
            StageQuery::Evidence(q) => self.query_evidence(q, workspace, timing).await,
            StageQuery::Lineage(q) => self.query_lineage(q, workspace, timing).await,
            StageQuery::Diff(q) => self.query_diff(q, workspace, timing).await,
        }
    }

    fn reserve_query(&mut self, _query: &StageQuery) -> Result<(), StageQueryError> {
        if let Some(max) = self.counters.max_queries {
            if self.counters.queries_used >= max {
                return Err(StageQueryError::PolicyDenied(PolicyDenial {
                    message: "stage query limit exceeded".to_owned(),
                    violated: PolicyViolationKind::QueryLimitExceeded,
                }));
            }
        }
        self.counters.queries_used += 1;
        Ok(())
    }

    fn reserve_bytes(&mut self, bytes: u64) -> Result<(), StageQueryError> {
        let next = self.counters.materialized_bytes.saturating_add(bytes);
        if let Some(max) = self.counters.max_materialized_bytes {
            if next > max {
                return Err(StageQueryError::PolicyDenied(PolicyDenial {
                    message: format!("stage query byte limit exceeded: {next} > {max}"),
                    violated: PolicyViolationKind::MaterializedBytesExceeded,
                }));
            }
        }
        self.counters.materialized_bytes = next;
        Ok(())
    }
}
```

Query helpers must return `QueryResult` with a `QueryRecord`-ready effect and workspace entry receipts. The first implementation may only support help, candidate summary/artifact, assessment summary, and lineage summary; but unsupported projections must be policy denied or explicit error, not silently approximated.

## 8.3 QueryResult helpers

```rust
#[derive(Clone, Debug)]
pub struct QueryResult {
    pub query_id: StageQueryId,
    pub timing: QueryTiming,
    pub query: StageQuery,
    pub effect: QueryEffect,
    pub entries: Vec<WorkspaceEntryReceipt>,
    pub cost: Cost,
}

#[derive(Clone, Debug)]
pub enum QueryEffect {
    WroteEntries { summary: QuerySummary },
    ReturnedSummary(QuerySummary),
    NotVisible(NotVisibleReason),
    NotFound(NotFoundReason),
    PolicyDenied(PolicyDenial),
    Error(Diagnostic),
}

impl QueryResult {
    pub fn into_record(self) -> QueryRecord {
        QueryRecord {
            query_id: self.query_id,
            timing: self.timing,
            query: self.query,
            effect: self.effect.into_record_effect(&self.entries),
            entries: self.entries,
            cost: self.cost,
        }
    }
}
```

---

# 9. milestone 8 — setup_stage_workspace

## Done when, no workarounds

```text
- setup_stage_workspace writes exactly plan-derived files plus prewarm query entries.
- Plan-derived files go into WorkspaceSetupReceipt.
- Prewarm query entries go into QueryRecord with QueryTiming::Prewarm.
- Output contract validates before any file write.
- Fingerprints are computed by re-reading persisted files.
- Receipt builder can finish success or failure outcome.
```

Named tests:

```text
crates/leaven-stage/tests/setup_workspace.rs::setup_writes_brief_focus_output_and_leaven_files
crates/leaven-stage/tests/setup_workspace.rs::setup_rejects_invalid_output_contract_before_writes
crates/leaven-stage/tests/setup_workspace.rs::prewarm_query_records_query_timing
crates/leaven-stage/tests/setup_workspace.rs::setup_fingerprints_re_read_bytes
crates/leaven-stage/tests/setup_workspace.rs::setup_writes_no_extra_files
```

## 9.1 setup input/output

File:

```text
crates/leaven-stage/src/setup.rs
```

Target:

```rust
pub async fn setup_stage_workspace<P, Req>(
    workspace: &mut Workspace,
    plan: &AgentStagePlan<Req>,
    authority: &mut StageReadAuthority<'_, P>,
    policy: &AgentBackedPolicy,
) -> Result<StageAttemptReceiptBuilder, WorkspaceSetupError>
where
    P: OptimizationProblem,
    Req: serde::Serialize,
{
    plan.output.validate()?;

    let workspace_id = workspace.id();
    let mut view = workspace.view();
    let mut builder = StageAttemptReceiptBuilder::new(
        workspace_id,
        authority.stage_call_id(),
        plan.role.clone(),
        authority.read_scope().clone(),
        fingerprint_plan(plan)?,
    );

    write_plan_derived_setup_files(&mut view, plan, policy, &mut builder).await?;

    for query in plan.query.prewarm.clone() {
        let result = authority
            .query(query, &mut view, QueryTiming::Prewarm, &plan.query)
            .await?;
        builder.push_query(result.into_record());
    }

    Ok(builder)
}
```

## 9.2 StageAttemptReceiptBuilder

Same file:

```rust
pub struct StageAttemptReceiptBuilder {
    receipt_id: StageAttemptReceiptId,
    workspace_id: WorkspaceId,
    stage_call_id: StageCallId,
    role: StageRole,
    read_scope: ReadScope,
    read_scope_fingerprint: Fingerprint,
    plan_fingerprint: Fingerprint,
    setup: WorkspaceSetupReceipt,
    queries: Vec<QueryRecord>,
    outputs: Vec<OutputEntryReceipt>,
    parse: Option<ParseReceipt>,
    session: Option<AgentSessionId>,
    cost: Cost,
    metadata: MetadataBag,
}

impl StageAttemptReceiptBuilder {
    pub fn new(
        workspace_id: WorkspaceId,
        stage_call_id: StageCallId,
        role: StageRole,
        read_scope: ReadScope,
        plan_fingerprint: Fingerprint,
    ) -> Self { ... }

    pub fn push_setup_entry(&mut self, entry: WorkspaceEntryReceipt) { ... }
    pub fn set_query_tool(&mut self, tool: LeavenQueryExposureReceipt) { ... }
    pub fn push_query(&mut self, query: QueryRecord) { ... }
    pub fn push_output(&mut self, output: OutputEntryReceipt) { ... }
    pub fn set_session(&mut self, session: AgentSessionId) { ... }
    pub fn set_parse(&mut self, parse: ParseReceipt) { ... }
    pub fn add_cost(&mut self, cost: Cost) { ... }

    pub fn finish(self, outcome: StageAttemptOutcome) -> StageAttemptReceipt {
        StageAttemptReceipt {
            receipt_id: self.receipt_id,
            workspace_id: self.workspace_id,
            stage_call_id: self.stage_call_id,
            role: self.role,
            read_scope: self.read_scope,
            read_scope_fingerprint: self.read_scope_fingerprint,
            plan_fingerprint: self.plan_fingerprint,
            setup: self.setup,
            queries: self.queries,
            outputs: self.outputs,
            parse: self.parse,
            session: self.session,
            cost: self.cost,
            outcome,
            metadata: self.metadata,
        }
    }
}
```

## 9.3 write plan-derived files

Plan-derived files:

```text
BRIEF.md
focus/stage_role.txt
focus/request.json
focus/instructions.md
.leaven/plan.json
.leaven/output_schema.json when schema exists
.leaven/receipt.partial.json
output/ parent directories / markers
tools/leaven_query when policy exposes shell tool
```

Target helper:

```rust
async fn write_plan_derived_setup_files<Req>(
    view: &mut WorkspaceView<'_>,
    plan: &AgentStagePlan<Req>,
    policy: &AgentBackedPolicy,
    builder: &mut StageAttemptReceiptBuilder,
) -> Result<(), WorkspaceSetupError>
where
    Req: serde::Serialize,
{
    write_setup_file(
        view,
        builder,
        WorkspaceEntryRole::brief(),
        WorkspacePath::new("BRIEF.md")?,
        render_brief(plan, policy)?.into_bytes(),
        MediaType::Markdown,
    )?;

    write_setup_file(
        view,
        builder,
        WorkspaceEntryRole::focus_request(),
        WorkspacePath::new("focus/stage_role.txt")?,
        plan.role.as_str().as_bytes().to_vec(),
        MediaType::Text,
    )?;

    write_setup_file(
        view,
        builder,
        WorkspaceEntryRole::focus_request(),
        WorkspacePath::new("focus/request.json")?,
        serde_json::to_vec_pretty(&plan.request)?,
        MediaType::Json,
    )?;

    write_setup_file(
        view,
        builder,
        WorkspaceEntryRole::stage_instructions(),
        WorkspacePath::new("focus/instructions.md")?,
        render_instructions(&plan.directive).into_bytes(),
        MediaType::Markdown,
    )?;

    write_setup_file(
        view,
        builder,
        WorkspaceEntryRole::runtime_config(),
        WorkspacePath::new(".leaven/plan.json")?,
        serde_json::to_vec_pretty(&erase_plan(plan)?)?,
        MediaType::Json,
    )?;

    if let Some(schema) = &plan.output.schema {
        write_setup_file(
            view,
            builder,
            WorkspaceEntryRole::output_schema(),
            WorkspacePath::new(".leaven/output_schema.json")?,
            serde_json::to_vec_pretty(schema)?,
            MediaType::Json,
        )?;
    }

    for output in plan.output.all_entries() {
        builder.push_output(OutputEntryReceipt {
            id: output.id.clone(),
            path: output.path.clone(),
            role: output.role.clone(),
            fingerprint: None,
            file: None,
            bytes: None,
            status: OutputEntryStatus::NotRead,
        });
    }

    maybe_write_leaven_query_tool(view, policy, builder)?;
    Ok(())
}
```

`write_setup_file` must write bytes then fingerprint by re-reading:

```rust
fn write_setup_file(
    view: &mut WorkspaceView<'_>,
    builder: &mut StageAttemptReceiptBuilder,
    role: WorkspaceEntryRole,
    path: WorkspacePath,
    bytes: Vec<u8>,
    media_type: MediaType,
) -> Result<(), WorkspaceSetupError> {
    view.write_file(&path, &bytes)?;
    let file = fingerprint_file(view, &path)?;
    builder.push_setup_entry(WorkspaceEntryReceipt {
        id: WorkspaceEntryId::new(),
        path,
        role,
        source: EntrySourceRef::Generated,
        projection: EntryProjection::Generated,
        access: EntryAccess::InputReadOnly,
        fingerprint: file.fingerprint,
        bytes: Some(file.bytes),
        file: Some(file),
        truncation: None,
        produced_by_query: None,
        metadata: MetadataBag::new(),
    });
    Ok(())
}
```

---

# 10. milestone 9 — MaterializableArtifact tier

## Done when, no workarounds

```text
- MaterializableArtifact lives in leaven-stage, not leaven-core.
- TextArtifact unchanged slot returns Ok(None).
- TextArtifact modified slot returns Some(Change).
- Missing/invalid file returns ArtifactReadbackError, never Ok(None).
- write_to cannot escape the slot.
- factory_context works for artifacts that need it.
```

Named tests:

```text
crates/leaven-stage/tests/artifact_text.rs::text_artifact_unchanged_slot_reads_back_none
crates/leaven-stage/tests/artifact_text.rs::text_artifact_changed_slot_reads_back_replace_text
crates/leaven-stage/tests/artifact_text.rs::text_artifact_missing_file_fails_readback
crates/leaven-stage/tests/artifact_text.rs::text_artifact_invalid_utf8_fails_readback
crates/leaven-stage/tests/artifact_text.rs::write_to_cannot_escape_slot
```

## 10.1 trait definitions

File:

```text
crates/leaven-stage/src/artifact.rs
```

Target:

```rust
use leaven_core::Artifact;
use leaven_workspace::WorkspaceSlot;

#[allow(async_fn_in_trait)]
pub trait MaterializableArtifact: Artifact {
    async fn write_to(
        &self,
        slot: &mut WorkspaceSlot<'_>,
    ) -> Result<MaterializationReport, WorkspaceSetupError>;

    async fn read_back_change(
        &self,
        slot: &WorkspaceSlot<'_>,
    ) -> Result<Option<Self::Change>, ArtifactReadbackError>;
}

#[allow(async_fn_in_trait)]
pub trait ReconstructibleArtifact: MaterializableArtifact {
    async fn parse_from(slot: &WorkspaceSlot<'_>) -> Result<Self, ArtifactReadbackError>
    where
        Self: Sized;
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct MaterializationReport {
    pub entries: Vec<WorkspaceEntryReceipt>,
    pub diagnostics: Vec<Diagnostic>,
    pub cost: Cost,
}

#[derive(Debug, thiserror::Error)]
pub enum ArtifactReadbackError {
    #[error(transparent)]
    Workspace(#[from] WorkspaceError),

    #[error(transparent)]
    FactoryContext(#[from] WorkspaceFactoryContextError),

    #[error("artifact readback failed: {0}")]
    InvalidArtifact(String),

    #[error("artifact readback failed: {message}")]
    WithSource {
        message: String,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },
}
```

## 10.2 TextArtifact proof fixture

Test fixture:

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
struct TextArtifact { text: String }

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
struct ReplaceText { text: String }

impl Artifact for TextArtifact {
    type Change = ReplaceText;
    type ApplyError = TextArtifactError;

    fn identity(&self) -> ArtifactIdentity {
        ArtifactIdentity::from_fingerprint(fingerprint_bytes(self.text.as_bytes()))
    }

    fn apply_change(&self, change: &ReplaceText) -> Result<Self, TextArtifactError> {
        Ok(Self { text: change.text.clone() })
    }
}

impl MaterializableArtifact for TextArtifact {
    async fn write_to(
        &self,
        slot: &mut WorkspaceSlot<'_>,
    ) -> Result<MaterializationReport, WorkspaceSetupError> {
        slot.write_file(&WorkspacePath::new("artifact.txt")?, self.text.as_bytes())?;
        Ok(MaterializationReport::default())
    }

    async fn read_back_change(
        &self,
        slot: &WorkspaceSlot<'_>,
    ) -> Result<Option<ReplaceText>, ArtifactReadbackError> {
        let bytes = slot.read_file(&WorkspacePath::new("artifact.txt")?)?;
        let text = String::from_utf8(bytes)
            .map_err(|error| ArtifactReadbackError::WithSource {
                message: "artifact.txt was not valid utf8".to_owned(),
                source: Box::new(error),
            })?;
        if text == self.text {
            Ok(None)
        } else {
            Ok(Some(ReplaceText { text }))
        }
    }
}
```

---

# 11. milestone 10 — bootstrap and parser contracts

## Done when, no workarounds

```text
- AgentStageBootstrap is slot-typed through SlotMarker.
- StageOutputParser parses Slot::Output and consults plan.output paths.
- Missing required output is distinct from malformed output.
- Parser does not receive StageReadAuthority and cannot query more graph/evidence.
- Same parser can parse two bootstraps with different output paths.
```

Named tests:

```text
crates/leaven-stage/tests/bootstrap_parser.rs::bootstrap_output_contract_is_validated
crates/leaven-stage/tests/bootstrap_parser.rs::parser_uses_declared_output_path
crates/leaven-stage/tests/bootstrap_parser.rs::missing_required_output_is_distinct_from_malformed
crates/leaven-stage/tests/bootstrap_parser.rs::parser_does_not_receive_read_authority
```

## 11.1 bootstrap trait

File:

```text
crates/leaven-stage/src/bootstrap.rs
```

Target:

```rust
#[allow(async_fn_in_trait)]
pub trait AgentStageBootstrap<P, Slot>: Send + Sync
where
    P: OptimizationProblem,
    Slot: SlotMarker<P>,
{
    async fn plan(
        &self,
        request: Slot::Request,
        ctx: AgentStageCallContext<'_, P>,
    ) -> Result<AgentStagePlan<Slot::Request>, StageBootstrapError>;
}

#[derive(Debug, thiserror::Error)]
pub enum StageBootstrapError {
    #[error("stage bootstrap read failed: {0}")]
    Read(String),

    #[error("stage bootstrap produced invalid plan: {0}")]
    InvalidPlan(String),

    #[error(transparent)]
    Serialize(#[from] serde_json::Error),
}
```

## 11.2 parser trait

File:

```text
crates/leaven-stage/src/parser.rs
```

Target:

```rust
#[allow(async_fn_in_trait)]
pub trait StageOutputParser<P, Slot>: Send + Sync
where
    P: OptimizationProblem,
    Slot: SlotMarker<P>,
{
    async fn parse(
        &self,
        workspace: &mut WorkspaceView<'_>,
        session: &AgentSession,
        plan: &ErasedStagePlan,
        ctx: AgentStageCallContext<'_, P>,
    ) -> Result<Metered<Slot::Output>, StageOutputParseError>;
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ErasedStagePlan {
    pub role: StageRole,
    pub request_json: serde_json::Value,
    pub directive: StageDirective,
    pub query: StageQueryPolicy,
    pub output: StageOutputContract,
    pub metadata: MetadataBag,
    pub fingerprint: Fingerprint,
}

impl<Req: serde::Serialize> TryFrom<&AgentStagePlan<Req>> for ErasedStagePlan {
    type Error = serde_json::Error;

    fn try_from(plan: &AgentStagePlan<Req>) -> Result<Self, Self::Error> {
        let request_json = serde_json::to_value(&plan.request)?;
        let mut builder = FingerprintBuilder::new();
        builder.update(b"leaven.stage.plan.v1").update(serde_json::to_vec(plan)?);
        Ok(Self {
            role: plan.role.clone(),
            request_json,
            directive: plan.directive.clone(),
            query: plan.query.clone(),
            output: plan.output.clone(),
            metadata: plan.metadata.clone(),
            fingerprint: builder.finish(),
        })
    }
}
```

Parser error:

```rust
#[derive(Debug, thiserror::Error)]
pub enum StageOutputParseError {
    #[error("required output `{entry:?}` missing at `{path}`")]
    MissingRequiredOutput { entry: OutputEntryId, path: WorkspacePath },

    #[error("output `{entry:?}` at `{path}` exceeded max bytes: {actual} > {max}")]
    TooLarge { entry: OutputEntryId, path: WorkspacePath, actual: u64, max: u64 },

    #[error("malformed output at `{path}`: {diagnostic:?}")]
    Malformed { path: WorkspacePath, diagnostic: Diagnostic },

    #[error(transparent)]
    Workspace(#[from] WorkspaceError),

    #[error(transparent)]
    Json(#[from] serde_json::Error),
}
```

---

# 12. milestone 11 — AgentBacked<ProposerSlot>

## Done when, no workarounds

```text
- AgentBacked<ProposerSlot<Req>, Runtime, Bootstrap, Parser> implements Proposer<P>.
- Lifecycle runs bootstrap -> setup -> prewarm -> runtime -> output validation -> parser -> receipt -> cleanup.
- StageAttemptRecorded event emitted exactly once per stage call after receipt is written.
- Cleanup runs on success and all failure paths.
- Parse failure records OutputParse and no ApplyFailed.
```

Named tests:

```text
crates/leaven-stage/tests/agent_backed_proposer.rs::fake_runtime_writes_proposal_and_parser_returns_batch
crates/leaven-stage/tests/agent_backed_proposer.rs::cleanup_runs_on_parser_error
crates/leaven-stage/tests/agent_backed_proposer.rs::parse_failure_records_stage_attempt_not_apply_failed
crates/leaven-stage/tests/agent_backed_proposer.rs::receipt_contains_setup_query_output_parse
```

## 12.1 AgentBacked struct

File:

```text
crates/leaven-stage/src/agent_backed.rs
```

Target:

```rust
pub struct AgentBacked<Slot, Runtime, Bootstrap, Parser> {
    pub workspace_factory: Arc<dyn WorkspaceFactory>,
    pub runtime: Runtime,
    pub bootstrap: Bootstrap,
    pub parser: Parser,
    pub policy: AgentBackedPolicy,
    _marker: PhantomData<Slot>,
}

impl<Slot, Runtime, Bootstrap, Parser> AgentBacked<Slot, Runtime, Bootstrap, Parser> {
    #[must_use]
    pub fn new(
        workspace_factory: Arc<dyn WorkspaceFactory>,
        runtime: Runtime,
        bootstrap: Bootstrap,
        parser: Parser,
        policy: AgentBackedPolicy,
    ) -> Self {
        Self { workspace_factory, runtime, bootstrap, parser, policy, _marker: PhantomData }
    }
}
```

## 12.2 Proposer impl

```rust
impl<P, Req, Runtime, Bootstrap, Parser> Proposer<P>
    for AgentBacked<ProposerSlot<Req>, Runtime, Bootstrap, Parser>
where
    P: OptimizationProblem,
    P::Artifact: MaterializableArtifact,
    Req: serde::Serialize + Send + Sync + 'static,
    Runtime: AgentRuntime,
    Bootstrap: AgentStageBootstrap<P, ProposerSlot<Req>>,
    Parser: StageOutputParser<P, ProposerSlot<Req>>,
{
    type Request = Req;

    fn id(&self) -> ProposerId { /* from policy or generated config */ }
    fn arity(&self) -> Arity { Arity::Single }

    async fn propose(
        &self,
        request: Req,
        ctx: ProposalContext<'_, P>,
    ) -> Result<Metered<ProposalBatch<P>>, ProposalError> {
        let stage_call_id = ctx.stage_call_id();
        let stage_ctx = ctx.to_stage_engine_context()?;
        let call_ctx = AgentStageCallContext::new(&stage_ctx);

        let plan = self.bootstrap.plan(request, call_ctx).await?;
        plan.output.validate()?;

        let mut workspace = self.workspace_factory.allocate(self.policy.workspace.clone()).await?;
        let workspace_id = workspace.id();

        let result = async {
            let mut authority = StageReadAuthority::from_engine_context(stage_ctx, &plan.query);
            let mut builder = setup_stage_workspace(
                &mut workspace,
                &plan,
                &mut authority,
                &self.policy,
            ).await?;

            let erased = ErasedStagePlan::try_from(&plan)?;
            let agent_request = build_agent_run_request(&plan, &erased, &self.policy)?;
            let session = self.runtime
                .run_session(&mut workspace.view(), agent_request, AgentRunContext::new(...))
                .await?;
            builder.set_session(session.value.id());
            builder.add_cost(session.cost.clone());

            validate_outputs(&mut workspace.view(), &plan.output, &mut builder)?;

            let parsed = self.parser
                .parse(&mut workspace.view(), &session.value, &erased, AgentStageCallContext::new(&stage_ctx))
                .await;

            match parsed {
                Ok(parsed) => {
                    builder.set_parse(ParseReceipt::succeeded(parsed.cost.clone()));
                    builder.add_cost(parsed.cost.clone());
                    let receipt = builder.finish(StageAttemptOutcome::Completed);
                    let receipt_ref = self.write_receipt(receipt).await?;
                    ctx.record_stage_attempt(plan.role.clone(), receipt_ref, StageAttemptOutcome::Completed);
                    Ok(parsed)
                }
                Err(error) => {
                    builder.set_parse(ParseReceipt::failed(error.diagnostic(), Cost::zero()));
                    let outcome = StageAttemptOutcome::Failed(StageAttemptFailure::OutputParse);
                    let receipt = builder.finish(outcome.clone());
                    let receipt_ref = self.write_receipt(receipt).await?;
                    ctx.record_stage_attempt(plan.role.clone(), receipt_ref, outcome);
                    Err(ProposalError::with_source("agent-backed output parse failed", error))
                }
            }
        }.await;

        let cleanup = workspace.cleanup().await;
        match (result, cleanup) {
            (Ok(value), Ok(())) => Ok(value),
            (Err(stage), Ok(())) => Err(stage),
            (Ok(_), Err(cleanup)) => Err(ProposalError::with_source("agent-backed cleanup failed", cleanup)),
            (Err(stage), Err(cleanup)) => Err(ProposalError::with_source(
                "agent-backed stage failed and cleanup also failed",
                StageAndCleanupError { stage: Box::new(stage), cleanup },
            )),
        }
    }
}
```

The code above is shape-level. The implementation must avoid borrow conflicts by not holding `workspace.view()` across awaits longer than needed. If necessary, create scoped blocks around runtime and parser calls.

---

# 13. milestone 12 — fake-runtime integrated proof

## Done when, no workarounds

```text
- FakeAgentRuntime writes randomized output/proposal.json bytes.
- Parser reads those exact bytes and constructs ProposalBatch<P>.
- RunContext::propose records the batch.
- RunContext::apply_batch creates a new candidate whose artifact equals the agent-written content.
- Proposal informed_by refs equal expected selected feedback refs by id set.
- Malformed output leaves RunContext usable for a later successful propose.
```

Named tests:

```text
crates/leaven-stage/tests/agent_backed_gepa_like.rs::fake_runtime_output_controls_applied_candidate
crates/leaven-stage/tests/agent_backed_gepa_like.rs::malformed_output_records_output_parse_and_run_context_recovers
```

## 13.1 test problem

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
struct TestEvidence { feedback: String }
impl Evidence for TestEvidence {}
```

## 13.2 raw proposal parser

```rust
#[derive(serde::Deserialize)]
struct RawTextProposal {
    target: CandidateId,
    replacement: String,
    informed_by: Vec<InfoRef>,
}

struct RawTextProposalParser;

impl StageOutputParser<TextProblem, ProposerSlot<TextReflectRequest>> for RawTextProposalParser {
    async fn parse(
        &self,
        workspace: &mut WorkspaceView<'_>,
        _session: &AgentSession,
        plan: &ErasedStagePlan,
        _ctx: AgentStageCallContext<'_, TextProblem>,
    ) -> Result<Metered<ProposalBatch<TextProblem>>, StageOutputParseError> {
        let output = plan
            .output
            .required
            .iter()
            .find(|entry| entry.role == OutputRole::proposal_json())
            .ok_or_else(|| StageOutputParseError::MissingRequiredOutput {
                entry: OutputEntryId::new_static("proposal"),
                path: WorkspacePath::new("output/proposal.json")?,
            })?;
        let bytes = workspace.read_file(&output.path)?;
        let raw: RawTextProposal = serde_json::from_slice(&bytes)?;
        let proposal = Proposal::mutate(raw.target, ReplaceText { text: raw.replacement })
            .informed_by(raw.informed_by)
            .build();
        Ok(Metered::new(ProposalBatch::alternatives(vec![proposal]), Cost::zero()))
    }
}
```

The parser must consult `plan.output`; no hardcoded output path.

---

# 14. milestone 13 — GEPA request, feedback selection, and routing

## Done when, no workarounds

```text
- FixedSurfaceEdit is canonical; ReflectiveMutation is deprecated alias.
- ReflectRequest<PartId>, SelectedFeedback, FeedbackSelector exist and serde roundtrip.
- ParentAssessmentFeedback derives refs by reading graph, not echoing request.
- Hidden existing assessment yields AssessmentNotVisible, distinct from missing.
- GEPA reflection path can call RunContext::propose with Proposer<P> reflector.
```

Named tests:

```text
crates/leaven-gepa/tests/reflection_types.rs::reflect_request_roundtrips_enum_part_id
crates/leaven-gepa/tests/feedback_selector.rs::parent_assessment_feedback_reads_graph_refs
crates/leaven-gepa/tests/feedback_selector.rs::hidden_assessment_is_not_visible
crates/leaven-gepa/tests/fixed_surface_edit.rs::reflective_mutation_deprecated_alias_unused_in_crate
crates/leaven-gepa/tests/gepa_reflection_path.rs::gepa_reflection_uses_run_context_propose
```

## 14.1 FixedSurfaceEdit

```rust
#[derive(Clone, Debug)]
pub struct FixedSurfaceEdit<E> {
    edit: E,
}

impl<E> FixedSurfaceEdit<E> {
    #[must_use]
    pub const fn new(edit: E) -> Self { Self { edit } }
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

#[deprecated(note = "use FixedSurfaceEdit; ReflectiveMutation was a fixed fixture, not reflection")]
pub type ReflectiveMutation<E> = FixedSurfaceEdit<E>;
```

## 14.2 GEPA reflection types

File:

```text
crates/leaven-gepa/src/reflection.rs
```

Target:

```rust
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ReflectRequest<PartId> {
    pub parent: CandidateId,
    pub parent_assessment: Option<AssessmentId>,
    pub selected_part: PartId,
    pub feedback: SelectedFeedback,
    pub objective: ReflectionObjective,
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct SelectedFeedback {
    pub assessment_refs: Vec<AssessmentId>,
    pub evidence_refs: Vec<InfoRef>,
    pub candidate_refs: Vec<CandidateId>,
    pub case_summaries: Vec<CaseFeedbackSummary>,
    pub provenance_refs: Vec<InfoRef>,
}

impl SelectedFeedback {
    #[must_use]
    pub fn informed_by_refs(&self) -> Vec<InfoRef> {
        let mut refs = BTreeSet::new();
        refs.extend(self.provenance_refs.iter().cloned());
        refs.extend(self.candidate_refs.iter().copied().map(InfoRef::Candidate));
        refs.extend(self.assessment_refs.iter().copied().map(InfoRef::Assessment));
        refs.extend(self.evidence_refs.iter().cloned());
        refs.into_iter().collect()
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct CaseFeedbackSummary {
    pub case_id: Option<CaseId>,
    pub assessment: AssessmentId,
    pub score: Option<f64>,
    pub summary: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum ReflectionObjective {
    ImproveSelectedPart,
    FixValidationError,
    ImproveFailingCases,
    ExploreAlternative,
    Other(String),
}
```

## 14.3 FeedbackSelector

```rust
#[derive(Clone, Debug)]
pub struct FeedbackSelectionRequest<PartId> {
    pub parent: CandidateId,
    pub selected_part: PartId,
    pub parent_assessment: AssessmentId,
}

pub struct FeedbackSelectionContext<'a, P: OptimizationProblem> {
    pub graph: ScopedRunGraphView<'a, P>,
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
    #[error("selected parent assessment exists but is not visible: {0}")]
    AssessmentNotVisible(AssessmentId),

    #[error("selected parent assessment does not exist: {0}")]
    AssessmentMissing(AssessmentId),

    #[error("feedback selection failed: {0}")]
    Message(String),
}

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
            .ok_or(FeedbackSelectionError::AssessmentNotVisible(request.parent_assessment))?;
        Ok(SelectedFeedback {
            assessment_refs: vec![assessment.id()],
            evidence_refs: vec![assessment.evidence_ref().clone().into_info_ref()],
            candidate_refs: vec![request.parent],
            case_summaries: Vec::new(),
            provenance_refs: vec![InfoRef::Candidate(request.parent)],
        })
    }
}
```

If the current `EvidenceRef` cannot become `InfoRef`, either add the conversion or keep separate `evidence_refs: Vec<EvidenceRef>` and convert in `informed_by_refs`.

---

# 15. milestone 14 — agent-facing leaven_query exposure

## Done when, no workarounds

```text
- Shell and/or structured leaven_query exposure lowers to StageQuery values.
- help is derived from StageQueryKind variants.
- Unknown commands/flags are policy denied or error, never silently help.
- Hidden existing assessment returns NotVisible.
- Query caps enforced before materialization.
```

Named tests:

```text
crates/leaven-stage/tests/leaven_query_cli.rs::help_lists_all_v0_4_variants
crates/leaven-stage/tests/leaven_query_cli.rs::unknown_command_is_error
crates/leaven-stage/tests/leaven_query_cli.rs::unknown_flag_is_error
crates/leaven-stage/tests/leaven_query_cli.rs::cli_candidate_maps_to_stage_query
crates/leaven-stage/tests/leaven_query_cli.rs::cli_path_args_reject_parent_traversal
```

## 15.1 shell tool parser

File:

```text
crates/leaven-stage/src/tool.rs
```

Target:

```rust
pub fn parse_leaven_query_args(args: &[String]) -> Result<StageQuery, LeavenQueryCliError> {
    match args.first().map(String::as_str) {
        Some("help") | None => Ok(StageQuery::Help),
        Some("list") => parse_list(&args[1..]),
        Some("candidate") => parse_candidate(&args[1..]),
        Some("assessment") => parse_assessment(&args[1..]),
        Some("evidence") => parse_evidence(&args[1..]),
        Some("lineage") => parse_lineage(&args[1..]),
        Some("diff") => parse_diff(&args[1..]),
        Some(other) => Err(LeavenQueryCliError::UnknownCommand(other.to_owned())),
    }
}

#[derive(Debug, thiserror::Error)]
pub enum LeavenQueryCliError {
    #[error("unknown leaven_query command `{0}`")]
    UnknownCommand(String),

    #[error("unknown leaven_query flag `{0}`")]
    UnknownFlag(String),

    #[error("missing argument `{0}`")]
    MissingArgument(&'static str),

    #[error("invalid argument `{name}`: {value}")]
    InvalidArgument { name: &'static str, value: String },
}
```

## 15.2 help text

```rust
pub fn leaven_query_help() -> String {
    let mut out = String::new();
    out.push_str("leaven_query help\n");
    out.push_str("leaven_query list candidates [--frontier] [--page N] [--page-size N]\n");
    out.push_str("leaven_query candidate <candidate_id> [--summary] [--artifact] [--assessments N]\n");
    out.push_str("leaven_query assessment <assessment_id> [--summary] [--with-evidence N] [--with-trace N]\n");
    out.push_str("leaven_query evidence <evidence_ref> [--summary] [--rendered] [--trace-excerpt N]\n");
    out.push_str("leaven_query lineage <candidate_id> [--depth N]\n");
    out.push_str("leaven_query diff <left_candidate_id> <right_candidate_id> [--artifact] [--assessment-delta]\n");
    out
}
```

Test should assert each `StageQueryKind::all_v0_4()` label appears.

---

# 16. milestone 15 — jj spike prerequisites

## Done when, no workarounds

```text
- JjWorkspaceFactory attaches Arc<JjRepoHandle> via WorkspaceFactoryContext.
- JjCodebase::write_to uses only slot-rooted commands and declared factory context.
- read_back_change reads actual jj change id from slot, not a synthetic id.
- apply_change verifies new change exists.
- cache_identity keys on commit/content id, not change handle.
```

Named tests:

```text
crates/leaven-artifact-jj/tests/materializable.rs::jj_factory_context_roundtrips_repo_handle
crates/leaven-artifact-jj/tests/materializable.rs::jj_write_to_does_not_depend_on_process_cwd
crates/leaven-artifact-jj/tests/materializable.rs::jj_unchanged_slot_reads_back_none
crates/leaven-artifact-jj/tests/materializable.rs::jj_changed_slot_reads_back_actual_change_id
crates/leaven-artifact-jj/tests/materializable.rs::jj_cache_identity_uses_commit_id
```

Target types:

```rust
#[derive(Clone, Debug)]
pub struct JjRepoHandle {
    root: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, serde::Serialize, serde::Deserialize)]
pub struct JjChangeId(String);

#[derive(Clone, Debug, Eq, PartialEq, Hash, serde::Serialize, serde::Deserialize)]
pub struct JjCommitId(String);

#[derive(Clone, Debug)]
pub struct JjCodebase {
    pub change_id: JjChangeId,
    pub repo: Arc<JjRepoHandle>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct JjAdvance {
    pub new_change_id: JjChangeId,
}
```

`MaterializableArtifact` implementation:

```rust
impl MaterializableArtifact for JjCodebase {
    async fn write_to(
        &self,
        slot: &mut WorkspaceSlot<'_>,
    ) -> Result<MaterializationReport, WorkspaceSetupError> {
        let repo = slot.factory_context::<JjRepoHandle>()?;
        // run jj with explicit repo/working dir; never rely on process cwd.
        slot.run_command(Command::new("jj")
            .arg("workspace")
            .arg("add")
            .arg(slot.root().as_str())
            .arg("--revision")
            .arg(self.change_id.as_str())
            .env("JJ_REPO", repo.root().display().to_string()))?;
        Ok(MaterializationReport::default())
    }

    async fn read_back_change(
        &self,
        slot: &WorkspaceSlot<'_>,
    ) -> Result<Option<JjAdvance>, ArtifactReadbackError> {
        let out = slot.run_command(Command::new("jj")
            .arg("log")
            .arg("-r")
            .arg("@")
            .arg("--no-graph")
            .arg("-T")
            .arg("change_id"))?;
        let next = JjChangeId::parse(out.stdout_trim())?;
        if next == self.change_id {
            Ok(None)
        } else {
            Ok(Some(JjAdvance { new_change_id: next }))
        }
    }
}
```

The exact jj command flags can change; the laws cannot.

---

# 17. dependency order with behavioral gates

```text
1. Milestone 1 — docs boundary / AgentCase hardening
   gate: hidden target cannot leak through stock presenters.

2. Milestone 2 — ids / WorkspaceFactoryContext / WorkspaceSlot / fingerprints
   gate: scoped slots cannot escape, context downcasts correctly, tree fingerprint is deterministic.

3. Milestone 3 — engine scoped boundary / StageAttemptRecorded
   gate: stage attempt event is emitted once with receipt ref/outcome, including error path.

4. Milestone 4 — leaven-stage skeleton
   gate: no leaven-gepa or leaven-agentic dependency; public surface tiered.

5. Milestone 5 — USER definitions
   gate: StageQueryPolicy replaces eager/lazy; StageOutputContract validates.

6. Milestone 6 — RECEIPT / ADAPTER definitions
   gate: StageAttemptReceipt roundtrips and records setup/query/output/parse.

7. Milestone 7 — StageReadAuthority query executor
   gate: prewarm and agent-requested queries share one authority.

8. Milestone 8 — setup_stage_workspace
   gate: setup writes exact files and runs prewarm through StageReadAuthority.

9. Milestone 9 — MaterializableArtifact / TextArtifact
   gate: unchanged -> None, changed -> Some(Change), invalid -> Err.

10. Milestone 10 — bootstrap/parser
    gate: parser uses declared output path and returns Slot::Output.

11. Milestone 11 — AgentBacked<ProposerSlot>
    gate: fake runtime output becomes ProposalBatch through RunContext::propose.

12. Milestone 12 — fake-runtime integrated proof
    gate: applied candidate equals agent-written randomized bytes; parse failure recovers.

13. Milestone 13 — GEPA request/feedback/routing
    gate: real GEPA reflection path can use RunContext::propose.

14. Milestone 14 — leaven_query exposure
    gate: shell/structured tool maps to StageQuery and respects caps/scope.

15. Milestone 15 — jj spike
    gate: jj uses factory context and read_back_change reads actual change id.
```

---

# 18. public deprecation surface

Do not delete these immediately:

```text
AgenticProposer
RepairingAgenticProposer
AgenticRunInput
ProposalParser
ProposalRepairPromptBuilder
```

Add docs:

```rust
/// Transitional pre-stage-workspace adapter.
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

---

# 19. do not implement in these milestones

```text
- delete AgentCase or AgentCaseEvaluator
- make AgentCase a dependency of AgentStagePlan
- implement all AgentBacked slots at once
- build a Harbor/Inspect task compiler
- dispatch parser behavior from StageRole strings
- put parser refs on OutputEntry
- rely on prompt-only hiding for hidden data
- make jj the first proof
- run live AIME before fake reflection proves real proposal production
- reintroduce EagerMaterializationPolicy / QueryPolicy split
- reintroduce StageReceipt eager/lazy fields
- expose unscoped RunGraphView to leaven-stage
```

---

# 20. promotion checklist

```text
[x] docs distinguish AgentCase workload from AgentStage workspace across relevant specs and crate AGENTS files
    evidence: docs/specs/agentic_stage_materialization.md, crates/leaven-stage/AGENTS.md, crates/leaven-agentic/src/case.rs
[x] hidden-target presenter law passes against stock workload surfaces
    evidence: crates/leaven-agentic/tests/agentic_workload.rs
[x] leaven-stage has no leaven-gepa or leaven-agentic transitive dependency
    evidence: crates/leaven-stage/tests/dependency_shape.rs and topology_contract
[x] WorkspaceSlot containment tested adversarially
    evidence: crates/leaven-workspace/tests/workspace_view.rs
[x] WorkspaceFactoryContext typed registry works and rejects wrong types
    evidence: crates/leaven-workspace/tests/workspace_view.rs
[x] StageAttemptRecorded emitted on success and scaffolded failure surfaces are explicit
    evidence: crates/leaven-engine/tests/context_services.rs, crates/leaven-stage/tests/agent_backed.rs
[x] StageQueryPolicy has prewarm and allowed; no eager/lazy split remains
    evidence: crates/leaven-stage/tests/query_policy.rs
[x] setup_stage_workspace writes plan-derived files plus output skeleton
    evidence: crates/leaven-stage/tests/setup_workspace.rs
[x] StageReadAuthority is the single query path for prewarm query execution
    evidence: crates/leaven-stage/tests/read_authority.rs
[x] StageAttemptReceipt roundtrips and records setup/query/output/parse vocabulary
    evidence: crates/leaven-stage/tests/receipt_store.rs, crates/leaven-stage/tests/serde_roundtrip.rs
[x] MaterializableArtifact proof exercises None / Some(Change) / Err
    evidence: crates/leaven-artifact-jj/tests/materializable.rs
[x] parser reads declared plan output path in the fake-runtime proof
    evidence: crates/leaven-stage/tests/agent_backed.rs
[x] AgentBacked<ProposerSlot> runs through RunContext::propose end to end
    evidence: crates/leaven-stage/tests/agent_backed.rs::agent_backed_fake_runtime_records_receipt_and_applies_candidate
[x] fake runtime bytes become applied candidate bytes
    evidence: crates/leaven-stage/tests/agent_backed.rs::agent_backed_fake_runtime_records_receipt_and_applies_candidate
[x] malformed output surfaces parse failure, leaves the graph usable, and records a failed attempt receipt
    evidence: crates/leaven-stage/tests/agent_backed.rs
[x] setup installs an executable leaven_query help shim and parser rejects unknown commands/flags
    evidence: crates/leaven-stage/tests/setup_workspace.rs, crates/leaven-stage/tests/leaven_query.rs
[~] GEPA stage request/bootstrap derives selected refs, but full optimizer switch remains follow-on
    evidence: crates/leaven-gepa/src/agent_stage.rs, crates/leaven-gepa/tests/agent_stage_routing.rs
[~] fixed-edit GEPA reflection remains explicit scaffold, not production reflection proof
    evidence: crates/leaven-gepa/AGENTS.md and crates/leaven-gepa/src/proposer.rs
[~] example migration/product proof remains follow-on for the full implementation goal
    evidence: milestone examples are proxy proof unless they execute the production stage path
```

---

# 21. final architectural note

The plan is not trying to make the stage layer small internally. It is trying to make its jurisdictions crisp:

```text
AgentCase owns candidate-evaluation workload shape.
AgentStagePlan owns optimizer-stage deliberation shape.
StageReadAuthority owns trusted query reads.
Workspace owns file/command substrate.
RunContext owns graph truth.
StageAttemptReceipt owns auditability.
```

If an implementation change blurs one of those lines, it is probably a workaround, not progress.

---

# 22. changelog

## v0.4 rewrite

```text
- Replaced materialize_stage_workspace with setup_stage_workspace.
- Replaced StageReceipt with StageAttemptReceipt.
- Replaced ScopedStageSource with StageReadAuthority.
- Replaced MaterializationEntry/Materialized* family with WorkspaceEntry/WorkspaceEntryReceipt family.
- Replaced EagerMaterializationPolicy + QueryPolicy split with StageQueryPolicy { allowed, prewarm, caps }.
- Moved query executor earlier in dependency order because prewarm uses it.
- Replaced multiple stage lifecycle events with one StageAttemptRecorded event.
- Added StageEngineContext and ScopedRunGraphView boundary so leaven-stage does not receive unscoped RunGraphView.
- Collapsed AgentBacked to four type parameters through SlotMarker<P>::Output.
- Removed record_receipt toggle; receipts are mandatory.
- Removed RetryWithFeedback from v0.4 policy.
- Replaced ReadScopeDigest-only receipt claim with full ReadScope plus read_scope_fingerprint.
- Replaced broad AccessMode with EntryAccess.
- Added explicit WorkspaceFactoryContext typed registry.
- Added pinned v0.4 StageQuery variants and projections.
- Added exact setup/query/output/parse receipt separation.
```
