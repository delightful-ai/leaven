# leaven agentic stage workspace — goal-state spec v0.4

Date: 2026-05-13  
Status: governing implementation spec; appendix reconciled with live code on 2026-05-14

## 0. decision summary

Leaven should distinguish three layers and keep the boundary mechanically visible in names, crate ownership, and trait bounds.

```text
A. candidate evaluation workload
   AgentCase / CaseSuite / AgentWorkload / AgentCaseEvaluator
   candidate artifact + user case world -> Assessment<P>

B. optimizer agentic stage workspace
   AgentStagePlan / AgentBacked / StageAttemptReceipt / leaven_query
   stage call request + scoped graph/evidence -> typed optimizer decision

C. raw workspace substrate
   Workspace / WorkspaceView / WorkspaceFactory / WorkspaceBackend / WorkspacePath / Command
   files, commands, allocation, cleanup
```

This spec defines **B** only. It does not delete or replace the task/workload layer in **A**. It also does not make the workspace substrate in **C** know optimizer vocabulary.

The central contract is:

```text
typed stage request
  -> AgentStageBootstrap builds AgentStagePlan<Req>
  -> adapter sets up a bounded workspace
  -> StageQueryPolicy prewarms selected graph/evidence queries
  -> AgentRuntime runs one bounded session
  -> StageOutputParser reads typed output
  -> adapter records StageAttemptReceipt
  -> owning optimizer / RunContext finalizes graph truth
```

The surface is intentionally split:

```text
USER     types users construct in bootstrap/parser code
ADAPTER  internal plumbing used by AgentBacked and leaven-stage
RECEIPT  durable audit records written by the adapter
```

The user-facing common case is small:

```text
StageRole
StageDirective
AgentStagePlan
AgentStageCallContext
StageQueryPolicy
StageOutputContract
StageOutputParser
MaterializableArtifact
AgentBacked
```

Everything else is internal machinery or debug/audit output.

## 1. canonical vocabulary

This section fixes the words the rest of the spec uses. Do not use these words with other meanings.

### 1.1 stage

A **stage** is an optimizer slot or role: reflect, merge, accept, select parent, select part, repair proposal, summarize frontier.

A **stage call** or **stage attempt** is one invocation of a stage.

Use:

```text
StageRole              metadata tag for the kind of stage
AgentStagePlan         declarative plan for one stage call
AgentStageCallContext  context for one stage call
StageAttemptReceipt    durable receipt for one stage attempt
StageCallId            id for one call/attempt
```

Do not use `StageReceipt`; it is ambiguous because it sounds like it might describe the abstract stage rather than one call.

### 1.2 workspace setup

**Workspace setup** means writing plan-derived files and adapter-owned scaffolding into the workspace.

Examples:

```text
BRIEF.md
focus/request.json
focus/stage_role.txt
focus/instructions.md
output/ skeleton
.leaven/plan.json
.leaven/output_schema.json
tools/leaven_query
```

Workspace setup is generated from the stage plan and adapter policy. It is not a graph/evidence query.

### 1.3 query

A **stage query** is a read from scoped graph/evidence/artifact state that may also write files into the workspace.

Queries can happen at two timings:

```text
Prewarm         adapter runs query before the agent starts
AgentRequested agent invokes leaven_query during the session
```

Prewarm and agent-requested queries use the same `StageReadAuthority`, same `ReadScope`, same visibility enforcement, same receipt record shape, and same budget path.

There is no separate eager-vs-lazy materialization ontology.

### 1.4 entry

A **workspace entry** is a file, directory, or tree created inside the workspace. It may come from setup, from a query, or from agent output.

Use:

```text
WorkspaceEntry         requested graph/evidence-derived entry
WorkspaceEntryRole     open tag describing the entry's purpose
EntrySource            where the entry came from
WorkspaceEntryReceipt  durable record of what was written
```

Avoid `MaterializationEntry`, `MaterializedRole`, and `MaterializedSource` for stage workspace state. Those names overuse “materialization” and blur setup/query/output.

### 1.5 read authority

A **stage read authority** is the only object that can turn scoped run state into agent-visible workspace entries.

Use:

```text
StageReadAuthority<'a, P>
```

The authority is not just a view. Its load-bearing job is to enforce read scope, charge query budget, and record what was exposed.

### 1.6 role tag axes

The spec uses three independent role-tag axes:

```text
StageRole           what kind of optimizer work is this call doing?
WorkspaceEntryRole  what is this workspace entry for?
OutputRole          what is this output file for?
```

All three are open tags with named constants. Do not dispatch graph semantics from any of them. Typed slot traits and typed parsers own authority.

## 2. existing names and local definitions

This spec uses Rust-like definitions. Some names already exist in Leaven. To keep the spec self-contained, this section states the minimum semantics assumed for each external name.

Exact module paths may change during implementation. The contracts should not.

### 2.1 core problem and artifact traits

```rust
pub trait OptimizationProblem: Send + Sync + 'static {
    type Artifact: Artifact;
    type Evidence: Send + Sync + 'static;
}

pub trait Artifact: Clone + Send + Sync + 'static {
    type Change: Send + Sync + 'static;
    type ApplyError: std::error::Error + Send + Sync + 'static;

    fn identity(&self) -> ArtifactIdentity;

    fn apply_change(&self, change: &Self::Change) -> Result<Self, Self::ApplyError>
    where
        Self: Sized;
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum ArtifactIdentity {
    Inline(Fingerprint),
    External(ExternalRef),
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct ExternalRef {
    pub scheme: String,
    pub value: String,
}
```

`Artifact::apply_change` is the graph-finalized mutation path. Agent stages may edit a workspace, but graph truth is still created by parsing a typed change/proposal and applying it through engine APIs.

### 2.2 ids and references

```rust
#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct CandidateId(pub String);

#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct AssessmentId(pub String);

#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct ProposalId(pub String);

#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct ProposalBatchId(pub String);

#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct InfoRef(pub String);

#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct EvidenceRef(pub String);

#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct TraceRef(pub String);

#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct WorkspaceId(pub String);

#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct WorkspaceEntryId(pub String);

#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct OutputEntryId(pub String);

#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct StageCallId(pub String);

#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct StageAttemptReceiptId(pub String);

#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct StageQueryId(pub String);

#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct AgentSessionId(pub String);
```

### 2.3 fingerprints, cost, metadata, diagnostics

```rust
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Fingerprint {
    pub algorithm: String,
    pub value: String,
}

#[derive(Clone, Debug, Default)]
pub struct Cost {
    pub wall_time_ms: u64,
    pub cpu_time_ms: Option<u64>,
    pub tokens_in: Option<u64>,
    pub tokens_out: Option<u64>,
    pub tool_calls: u64,
    pub bytes_read: u64,
    pub bytes_written: u64,
    pub provider_units: Option<u64>,
}

#[derive(Clone, Debug)]
pub struct Metered<T> {
    pub value: T,
    pub cost: Cost,
}

#[derive(Clone, Debug, Default)]
pub struct MetadataBag {
    pub entries: std::collections::BTreeMap<String, serde_json::Value>,
}

#[derive(Clone, Debug)]
pub struct Diagnostic {
    pub severity: DiagnosticSeverity,
    pub message: String,
    pub location: Option<String>,
    pub metadata: MetadataBag,
}

#[derive(Clone, Debug)]
pub enum DiagnosticSeverity {
    Note,
    Warning,
    Error,
}
```

### 2.4 read scope and trust

```rust
#[derive(Clone, Debug)]
pub struct ReadScope {
    pub visible_splits: std::collections::BTreeSet<SplitName>,
    pub visible_candidates: CandidateVisibility,
    pub visible_evidence: EvidenceVisibility,
    pub trust_policy: TrustPolicy,
    pub metadata: MetadataBag,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct SplitName(pub String);

#[derive(Clone, Debug)]
pub enum CandidateVisibility {
    FrontierOnly,
    AncestorsOf(CandidateId),
    Explicit(std::collections::BTreeSet<CandidateId>),
    AllTrainingVisible,
}

#[derive(Clone, Debug)]
pub enum EvidenceVisibility {
    AssessmentSummariesOnly,
    SelectedEvidenceOnly(std::collections::BTreeSet<InfoRef>),
    TrainingEvidence,
}

#[derive(Clone, Debug)]
pub enum TrustPolicy {
    TrainingOnly,
    TrainingAndValidationSummaries,
    Custom(String),
}
```

A `ReadScope` is the structured value the engine uses to decide what an optimizer stage may see. A receipt stores the full `ReadScope` and a derived fingerprint.

### 2.5 graph, evidence, proposals

```rust
pub struct Proposal<P: OptimizationProblem> {
    pub parent: CandidateId,
    pub change: <P::Artifact as Artifact>::Change,
    pub rationale: Option<String>,
    pub informed_by: Vec<InfoRef>,
    pub metadata: MetadataBag,
}

pub struct ProposalBatch<P: OptimizationProblem> {
    pub proposals: Vec<Proposal<P>>,
    pub informed_by: Vec<InfoRef>,
    pub metadata: MetadataBag,
}

pub struct Assessment<P: OptimizationProblem> {
    pub candidate: CandidateId,
    pub score: AssessmentScore,
    pub evidence: Vec<InfoRef>,
    pub metadata: MetadataBag,
    pub _marker: std::marker::PhantomData<P>,
}

#[derive(Clone, Debug)]
pub struct AssessmentScore {
    pub primary: Option<f64>,
    pub axes: std::collections::BTreeMap<String, f64>,
    pub summary: Option<String>,
}

pub trait EvidenceStore<E>: Send + Sync {
    async fn load(&self, reference: &InfoRef) -> Result<E, EvidenceLoadError>;
}

#[derive(Debug)]
pub struct EvidenceLoadError {
    pub message: String,
}
```

### 2.6 workspace substrate

```rust
#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct WorkspacePath(String);

impl WorkspacePath {
    pub fn new(value: impl Into<String>) -> Result<Self, WorkspacePathError>;
    pub fn as_str(&self) -> &str;
    pub fn join(&self, child: impl AsRef<str>) -> Result<Self, WorkspacePathError>;
}

#[derive(Debug)]
pub struct WorkspacePathError {
    pub message: String,
}

pub struct Workspace {
    pub id: WorkspaceId,
}

pub struct WorkspaceView<'a> {
    workspace: &'a mut Workspace,
    root: WorkspacePath,
}

pub trait WorkspaceFactory: Send + Sync {
    async fn allocate(&self, config: &WorkspaceConfig) -> Result<Workspace, WorkspaceAllocateError>;
    fn context(&self) -> &WorkspaceFactoryContext;
}

#[derive(Clone, Debug)]
pub struct WorkspaceConfig {
    pub backend: WorkspaceBackendKind,
    pub limits: WorkspaceLimits,
    pub cleanup: CleanupPolicy,
    pub metadata: MetadataBag,
}

#[derive(Clone, Debug)]
pub enum WorkspaceBackendKind {
    Local,
    Docker,
    E2B,
    Firecracker,
    Kubernetes,
    Custom(String),
}

#[derive(Clone, Debug, Default)]
pub struct WorkspaceLimits {
    pub max_bytes: Option<u64>,
    pub max_files: Option<u64>,
    pub max_processes: Option<u64>,
    pub max_wall_time_ms: Option<u64>,
}

#[derive(Clone, Debug)]
pub enum CleanupPolicy {
    Always,
    OnSuccess,
    PreserveOnFailure,
}

#[derive(Debug)]
pub struct WorkspaceAllocateError {
    pub message: String,
}
```

`WorkspacePath::new` must reject absolute paths, parent-directory escapes, empty components, and backend-specific invalid components. All stage workspace paths are workspace-relative.

### 2.7 agent runtime substrate

```rust
pub trait AgentRuntime: Send + Sync {
    async fn run(&self, request: AgentRunRequest) -> Result<Metered<AgentSession>, AgentRuntimeError>;
}

pub struct AgentRunRequest {
    pub workspace_id: WorkspaceId,
    pub instructions: AgentInstructions,
    pub output_contract: AgentOutputContract,
    pub tool_policy: AgentToolPolicy,
    pub limits: AgentLimits,
    pub metadata: MetadataBag,
}

pub struct AgentInstructions {
    pub brief_path: WorkspacePath,
    pub inline_summary: Option<String>,
}

pub struct AgentSession {
    pub id: AgentSessionId,
    pub transcript: Vec<AgentMessage>,
    pub observed_outputs: Vec<WorkspacePath>,
    pub cost: Cost,
    pub metadata: MetadataBag,
}

pub struct AgentMessage {
    pub role: String,
    pub content: String,
    pub metadata: MetadataBag,
}

pub struct AgentOutputContract {
    pub required_paths: Vec<WorkspacePath>,
    pub optional_paths: Vec<WorkspacePath>,
    pub schema_hint: Option<serde_json::Value>,
}

#[derive(Clone, Debug)]
pub struct AgentToolPolicy {
    pub expose_shell_leaven_query: bool,
    pub expose_structured_leaven_query: bool,
    pub allow_shell: bool,
    pub metadata: MetadataBag,
}

#[derive(Clone, Debug, Default)]
pub struct AgentLimits {
    pub timeout_ms: Option<u64>,
    pub max_tool_calls: Option<u64>,
    pub max_tokens: Option<u64>,
}

#[derive(Debug)]
pub struct AgentRuntimeError {
    pub kind: AgentRuntimeErrorKind,
    pub message: String,
}

#[derive(Debug)]
pub enum AgentRuntimeErrorKind {
    Timeout,
    Provider,
    WorkspaceUnavailable,
    OutputContract,
}
```

The stage layer maps `StageOutputContract` to `AgentOutputContract`. Provider-specific runtimes choose how to expose `leaven_query`: structured tool, shell command, or both.


### 2.8 engine context and error placeholders

The exact engine structs already exist or will be tightened during implementation. This spec assumes at least the following public meanings.

```rust
pub struct RunContext<P: OptimizationProblem> {
    // private; owns graph mutation, proposal finalization, evaluation, budget, events
    _marker: std::marker::PhantomData<P>,
}

pub struct ProposalContext<'a, P: OptimizationProblem> {
    // private; passed to Proposer<P>::propose; can be lowered into StageEngineContext
    _marker: std::marker::PhantomData<&'a P>,
}

pub struct ApplyBatchReport {
    pub created_candidates: Vec<CandidateId>,
    pub failed_proposals: Vec<ProposalId>,
    pub cost: Cost,
}

#[derive(Debug)]
pub struct ProposalError {
    pub message: String,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug)]
pub struct ApplyBatchError {
    pub message: String,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug)]
pub struct EvaluationError {
    pub message: String,
    pub diagnostics: Vec<Diagnostic>,
}
```

GEPA-specific contexts and request/error types are also named locally when used:

```rust
pub struct GepaStageContext<'a, P: OptimizationProblem> {
    pub engine: StageEngineContext<'a, P>,
    pub gepa_state: GepaStateView<'a, P>,
}

pub struct GepaStateView<'a, P: OptimizationProblem> {
    // private; read-only GEPA optimizer state visible to this slot
    _marker: std::marker::PhantomData<&'a P>,
}

pub struct ParentSelectionRequest {
    pub frontier: Vec<CandidateId>,
    pub objective: Option<String>,
    pub metadata: MetadataBag,
}

#[derive(Debug)]
pub struct ParentSelectionError {
    pub message: String,
    pub diagnostics: Vec<Diagnostic>,
}
```

### 2.9 crate aliases used in examples

```rust
pub type SmolStr = smol_str::SmolStr;
```

## 3. scope and non-goals

### 3.1 in scope

This spec covers optimizer-owned agentic stages:

```text
GEPA reflection
GEPA parent selection
GEPA part selection
GEPA feedback/context selection when agent-backed
batch sampling
acceptance/gating
merge
conflict resolution
proposal repair
frontier summarization
future optimizer-owned slots
```

It covers:

```text
bounded workspace setup
stage queries against scoped graph/evidence/artifact state
output contracts
typed output parsing
stage attempt receipts
checkpoint/reconstruction metadata
artifact workspace representation for optimizer-stage agents
```

### 3.2 out of scope

This spec does not define:

```text
user-built candidate execution harnesses
SWE-bench-style checkout/test/grader plans
Harbor/Terminal-Bench task package layouts
Inspect sample-file conventions
hidden verifier packaging
provider-specific prompt formatting
long-lived multi-stage agent sessions
```

Those concerns remain possible through lower-level `Workspace`, `AgentRuntime`, and `AgentCaseEvaluator` primitives. They are not the semantic center of optimizer-stage workspaces.

## 4. core invariants

1. **typed slot authority beats string role authority.** `StageRole` is metadata for prompts, layout, receipts, and reports. It never determines how an output is parsed or finalized.

2. **workspace setup, query, output readback, and graph finalization are distinct.**

   ```text
   setup      writes plan-derived workspace files
   query      exposes scoped graph/evidence/artifact data
   readback   parses agent output from workspace/session
   finalizing records/applies typed result into graph truth
   ```

3. **the workspace is not truth.** The workspace is an ephemeral deliberation surface. The graph, evidence store, cache, and receipts are truth.

4. **a directory layout is not the ontology.** Layout is for model ergonomics. The semantic contract is the typed plan, read scope, query policy, output contract, parser, and receipt.

5. **one read authority.** All prewarm and agent-requested graph/evidence/artifact queries go through `StageReadAuthority`. Hidden-data filtering must not be copied into ad hoc accessors.

6. **agent-visible means physically reachable.** If data is not visible under the stage `ReadScope`, it is not written to shell-reachable workspace paths and is not retrievable by `leaven_query`.

7. **output contract and parser authority are different.** The contract declares what files the agent must produce. The parser interprets those files into a typed result. The parser may use the contract as a schema/hint, but the contract itself does not own typed parsing authority.

8. **RunContext finalizes graph mutation.** Agentic stages may produce proposal data, candidate selections, merge decisions, or acceptance decisions. They do not mutate the graph directly except through finalizing engine APIs.

9. **artifact materialization is opt-in.** Classical optimizers need only `Artifact`. Agentic stages that expose artifacts in a workspace require `MaterializableArtifact`. Deterministic cache identity remains a separate promise.

10. **receipts are always recorded.** A stage attempt that allocates a workspace must produce or attempt to produce a `StageAttemptReceipt`. There is no `record_receipt: bool` escape hatch.

11. **prewarm is just query timing.** Prewarm queries and `leaven_query` calls differ only by who initiates them and when. The API and receipt structure should not expose them as different concepts.

12. **parse failure is not apply failure.** `OutputParse` means no typed proposal/change was successfully parsed. `ApplyFailed` means a parsed proposal existed and artifact application failed.

## 5. crate responsibilities

### 5.1 `leaven-workspace`

Owns raw workspace substrate:

```text
Workspace
WorkspaceView
WorkspaceFactory
WorkspaceFactoryContext
WorkspaceBackend
WorkspacePath
WorkspaceSlot
Command
fingerprint helpers
cleanup semantics
```

Must not know:

```text
candidate ids
assessment ids
GEPA
evidence visibility
stage roles
proposal batches
```

### 5.2 `leaven-agent`

Owns provider-neutral agent runtime vocabulary:

```text
AgentRuntime
AgentRunRequest
AgentSession
AgentInstructions
AgentOutputContract
AgentToolPolicy
AgentLimits
```

Must not know:

```text
candidate graph
stage read scope
GEPA
proposal application
```

### 5.3 `leaven-agentic`

Owns A-shaped candidate evaluation workload:

```text
AgentCase
CaseSuite
CasePartitions
AgentWorkload
AgentCaseEvaluator
AgentCasePresenter
AgentCaseScorer
AgentCaseRunRecord
CaseFiles
CaseInput
CaseTarget
SetupScript
WorkspaceRequirement
```

`AgenticProposer` and `RepairingAgenticProposer`, if retained, are transitional compatibility adapters. The general optimizer-stage adapter lives in `leaven-stage`.

### 5.4 `leaven-stage`

Owns B-shaped optimizer-stage workspace layer:

```text
USER:
  StageRole
  StageDirective
  AgentStagePlan
  AgentStageCallContext
  StageQueryPolicy
  StageOutputContract
  StageOutputParser
  MaterializableArtifact
  ReconstructibleArtifact
  AgentBacked

ADAPTER:
  StageEngineContext
  StageReadAuthority
  WorkspaceEntry
  WorkspaceEntryRole
  EntrySource
  StageQuery
  setup_stage_workspace
  leaven_query implementation

RECEIPT:
  StageAttemptReceipt
  WorkspaceSetupReceipt
  QueryRecord
  WorkspaceEntryReceipt
  OutputEntryReceipt
  ParseReceipt
```

`leaven-stage` may depend on `leaven-engine`, `leaven-agent`, and `leaven-workspace`. It should not depend on `leaven-gepa`.

### 5.5 `leaven-engine`

Owns graph finalization and read-scope construction.

Public finalizing surfaces remain:

```rust
impl<P: OptimizationProblem> RunContext<P> {
    pub async fn propose<R>(
        &mut self,
        proposer: &R,
        request: R::Request,
    ) -> Result<Metered<ProposalBatch<P>>, ProposalError>
    where
        R: Proposer<P>;

    pub async fn apply_batch(
        &mut self,
        batch: ProposalBatch<P>,
    ) -> Result<ApplyBatchReport, ApplyBatchError>;

    pub async fn evaluate(
        &mut self,
        candidate: CandidateId,
    ) -> Result<Metered<Assessment<P>>, EvaluationError>;
}
```

Low-level batch recording should not be a normal external path:

```rust
// crate-private or otherwise not part of ordinary GEPA/stage API
pub(crate) async fn record_proposal_batch_internal(...);
```

Engine-to-stage handoff must pass a scoped graph view, not the full graph.

### 5.6 `leaven-gepa`

Owns GEPA rhythm and GEPA-specific strategy slots:

```text
parent selection
part selection
feedback/context selection
reflection request construction
population observation
acceptance/gating
validation rhythm
merge/conflict rhythm
checkpoint state
```

GEPA may use `AgentBacked` for GEPA slots, but `leaven-stage` should not depend on GEPA. GEPA-specific `AgentBacked<Slot, ...>` impls live in `leaven-gepa` or a feature-gated bridge crate.

### 5.7 slot impl ownership rule

The crate that defines a slot trait owns the `AgentBacked` impl for that slot.

```text
engine-defined slot trait -> impl in leaven-stage or leaven-engine-stage bridge
GEPA-defined slot trait   -> impl in leaven-gepa or leaven-gepa-stage bridge
user-defined slot trait   -> impl in user's crate
```

This avoids forcing `leaven-stage` to depend on every future optimizer crate.

## 6. USER surface

### 6.1 USER: `StageRole`

`StageRole` is an open tag. It is descriptive metadata only.

```rust
#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct StageRole(SmolStr);

impl StageRole {
    pub const REFLECT: Self = Self(SmolStr::new_static("reflect"));
    pub const SELECT_PARENT: Self = Self(SmolStr::new_static("select_parent"));
    pub const SELECT_PART: Self = Self(SmolStr::new_static("select_part"));
    pub const SAMPLE_BATCH: Self = Self(SmolStr::new_static("sample_batch"));
    pub const ACCEPT: Self = Self(SmolStr::new_static("accept"));
    pub const MERGE: Self = Self(SmolStr::new_static("merge"));
    pub const RESOLVE_CONFLICTS: Self = Self(SmolStr::new_static("resolve_conflicts"));
    pub const REPAIR_PROPOSAL: Self = Self(SmolStr::new_static("repair_proposal"));
    pub const SUMMARIZE_FRONTIER: Self = Self(SmolStr::new_static("summarize_frontier"));

    pub fn custom(value: impl Into<SmolStr>) -> Result<Self, RoleTagError>;
    pub fn as_str(&self) -> &str;
}

#[derive(Debug)]
pub struct RoleTagError {
    pub value: String,
    pub message: String,
}
```

Validation for custom roles:

```text
non-empty
lowercase ascii preferred
allowed chars: [a-z0-9_.:-]
no path separators
not longer than 128 bytes
```

Law:

```text
StageRole never dispatches parser authority.
StageRole never determines graph mutation semantics.
StageRole is safe to render into BRIEF.md and receipts.
```

### 6.2 USER: `StageDirective`

A directive is the human/model-facing instruction for one stage call.

```rust
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct StageDirective {
    pub title: String,
    pub instructions: String,
    pub success_criteria: Vec<String>,
    pub cautions: Vec<String>,
}

impl StageDirective {
    pub fn new(title: impl Into<String>, instructions: impl Into<String>) -> Self;
    pub fn with_success_criterion(mut self, criterion: impl Into<String>) -> Self;
    pub fn with_caution(mut self, caution: impl Into<String>) -> Self;
}
```

Rendering rules:

```text
StageDirective.title appears near the top of BRIEF.md.
StageDirective.instructions appears as the main imperative body.
StageDirective.success_criteria are rendered as checklist items.
StageDirective.cautions are rendered as constraints, not as parser truth.
```

The directive is not a source of truth for output parsing.

### 6.3 USER: `AgentStagePlan<Req>`

Bootstrap returns a plan, not a live workspace.

```rust
#[derive(Clone, Debug, serde::Serialize)]
pub struct AgentStagePlan<Req> {
    pub role: StageRole,
    pub request: Req,
    pub directive: StageDirective,
    pub query: StageQueryPolicy,
    pub output: StageOutputContract,
    pub metadata: MetadataBag,
}
```

`Req` remains typed inside Leaven. It is rendered to `focus/request.json` during workspace setup.

No `StageLayout` field exists. The default layout is adapter-owned. Richness of graph-derived data is controlled by `StageQueryPolicy`, especially `prewarm`.

### 6.4 USER: `AgentStageCallContext<'a, P>`

The adapter passes this to bootstrap. Users can inspect bounded run context to choose a plan.

```rust
pub struct AgentStageCallContext<'a, P: OptimizationProblem> {
    engine: &'a StageEngineContext<'a, P>,
}

impl<'a, P: OptimizationProblem> AgentStageCallContext<'a, P> {
    pub fn stage_call_id(&self) -> &StageCallId;
    pub fn read_scope(&self) -> &ReadScope;
    pub fn budget(&self) -> &BudgetSnapshot;
    pub fn metadata(&self) -> &MetadataBag;

    pub fn visible_frontier_summary(&self) -> FrontierSummary;
    pub fn candidate_summary(&self, id: &CandidateId) -> Option<CandidateSummary>;
}

#[derive(Clone, Debug)]
pub struct BudgetSnapshot {
    pub remaining_wall_time_ms: Option<u64>,
    pub remaining_tool_calls: Option<u64>,
    pub remaining_provider_units: Option<u64>,
    pub metadata: MetadataBag,
}

#[derive(Clone, Debug)]
pub struct FrontierSummary {
    pub candidates: Vec<CandidateSummary>,
    pub truncated: bool,
}

#[derive(Clone, Debug)]
pub struct CandidateSummary {
    pub id: CandidateId,
    pub score: Option<AssessmentScore>,
    pub parent: Option<CandidateId>,
    pub has_artifact: bool,
    pub assessment_count: usize,
    pub metadata: MetadataBag,
}
```

`AgentStageCallContext` is read-only and intentionally narrow. It does not expose unscoped `RunGraphView`. It does not allocate workspaces or write files.

### 6.5 USER: `StageQueryPolicy`

`StageQueryPolicy` controls graph/evidence/artifact queries. `prewarm` is simply the list of queries the adapter runs before the agent starts.

```rust
#[derive(Clone, Debug)]
pub struct StageQueryPolicy {
    pub allowed: AllowedQuerySet,
    pub prewarm: Vec<StageQuery>,
    pub max_queries: Option<usize>,
    pub max_materialized_bytes: Option<u64>,
}

impl StageQueryPolicy {
    pub fn none() -> Self;
    pub fn minimal() -> Self;
    pub fn focus_candidate(id: CandidateId) -> Self;
    pub fn focus_candidate_with_recent_evidence(id: CandidateId, k: usize) -> Self;
    pub fn bounded(allowed: AllowedQuerySet, limits: QueryLimits) -> Self;
    pub fn with_prewarm(mut self, query: StageQuery) -> Self;
}

#[derive(Clone, Debug, Default)]
pub struct QueryLimits {
    pub max_queries: Option<usize>,
    pub max_materialized_bytes: Option<u64>,
}
```

Semantics:

```text
max_queries counts prewarm and agent-requested queries.
max_materialized_bytes counts all query-written entries, not setup files.
allowed gates both prewarm and agent-requested queries.
prewarm queries that are not allowed are setup errors.
```

Recommended defaults:

```text
none      no graph/evidence queries; setup files and output skeleton only
minimal   Help + selected request refs only, no broad graph index
focus_candidate(id)
          prewarm Candidate(id, SummaryAndArtifact) plus shallow lineage
focus_candidate_with_recent_evidence(id, k)
          focus_candidate plus recent assessments/evidence summaries
```

### 6.6 USER: `AllowedQuerySet`

```rust
#[derive(Clone, Debug)]
pub struct AllowedQuerySet {
    pub help: bool,
    pub list_candidates: bool,
    pub candidate: bool,
    pub assessment: bool,
    pub evidence: bool,
    pub lineage: bool,
    pub diff: bool,
}

impl AllowedQuerySet {
    pub fn none() -> Self;
    pub fn help_only() -> Self;
    pub fn standard_reflection() -> Self;
    pub fn all_v0_4() -> Self;
    pub fn allows(&self, query: &StageQuery) -> bool;
}
```

`Search` is deliberately absent in v0.4. Add it only after real traces show the need and after its visibility semantics are explicit.

### 6.7 USER/ADAPTER: `StageQuery`

Users mostly choose prewarm queries through constructors. The same query type is used by `leaven_query`.

```rust
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum StageQuery {
    Help,
    ListCandidates(ListCandidatesQuery),
    Candidate(CandidateQuery),
    Assessment(AssessmentQuery),
    Evidence(EvidenceQuery),
    Lineage(LineageQuery),
    Diff(DiffQuery),
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ListCandidatesQuery {
    pub frontier_only: bool,
    pub include_archived: bool,
    pub page: Option<PageRequest>,
    pub projection: CandidateListProjection,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct CandidateQuery {
    pub id: CandidateId,
    pub projection: CandidateProjection,
    pub placement: Option<WorkspacePath>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct AssessmentQuery {
    pub id: AssessmentId,
    pub projection: AssessmentProjection,
    pub placement: Option<WorkspacePath>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct EvidenceQuery {
    pub reference: InfoRef,
    pub projection: EvidenceProjection,
    pub placement: Option<WorkspacePath>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct LineageQuery {
    pub id: CandidateId,
    pub depth: Option<usize>,
    pub include_assessments: bool,
    pub placement: Option<WorkspacePath>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct DiffQuery {
    pub left: CandidateId,
    pub right: CandidateId,
    pub projection: DiffProjection,
    pub placement: Option<WorkspacePath>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct PageRequest {
    pub page: usize,
    pub page_size: usize,
}
```

Projections:

```rust
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum CandidateListProjection {
    IdsOnly,
    Summary,
    SummaryWithScores,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum CandidateProjection {
    Summary,
    Artifact,
    Assessments { limit: Option<usize> },
    SummaryArtifactAndAssessments { limit: Option<usize> },
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum AssessmentProjection {
    Summary,
    WithEvidenceRefs,
    WithTraceRefs,
    WithEvidenceSummaries { limit: Option<usize> },
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum EvidenceProjection {
    Summary,
    RenderedText { max_bytes: Option<u64> },
    RawIfPolicyAllows { max_bytes: Option<u64> },
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum DiffProjection {
    Summary,
    ArtifactDiff,
    ScoreAndAssessmentDiff,
}
```

Placement rules:

```text
If placement is None, StageReadAuthority chooses a default path.
If placement is Some(path), the path must be workspace-relative and under an allowed graph/evidence/diffs subtree.
Queries cannot write into output/ except through explicit parser-owned workflows, which v0.4 does not define.
```

### 6.8 USER: `StageOutputContract`

The output contract declares expected files. It does not parse them.

```rust
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct StageOutputContract {
    pub required: Vec<OutputEntry>,
    pub optional: Vec<OutputEntry>,
    pub schema: Option<OutputSchema>,
}

impl StageOutputContract {
    pub fn new(required: Vec<OutputEntry>) -> Self;
    pub fn proposal_json(path: WorkspacePath) -> Self;
    pub fn candidate_selection_json(path: WorkspacePath) -> Self;
    pub fn acceptance_decision_json(path: WorkspacePath) -> Self;
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct OutputEntry {
    pub id: OutputEntryId,
    pub path: WorkspacePath,
    pub role: OutputRole,
    pub media_type: MediaType,
    pub max_bytes: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, serde::Serialize, serde::Deserialize)]
pub struct OutputRole(SmolStr);

impl OutputRole {
    pub const PROPOSAL_JSON: Self = Self(SmolStr::new_static("proposal_json"));
    pub const CANDIDATE_SELECTION: Self = Self(SmolStr::new_static("candidate_selection"));
    pub const PART_SELECTION: Self = Self(SmolStr::new_static("part_selection"));
    pub const MERGE_PLAN: Self = Self(SmolStr::new_static("merge_plan"));
    pub const ACCEPTANCE_DECISION: Self = Self(SmolStr::new_static("acceptance_decision"));
    pub const NOTES: Self = Self(SmolStr::new_static("notes"));
    pub const PATCH: Self = Self(SmolStr::new_static("patch"));
    pub const WORKSPACE_DIFF: Self = Self(SmolStr::new_static("workspace_diff"));

    pub fn custom(value: impl Into<SmolStr>) -> Result<Self, RoleTagError>;
    pub fn as_str(&self) -> &str;
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum MediaType {
    Json,
    Markdown,
    PlainText,
    Diff,
    Binary,
    Custom(String),
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct OutputSchema {
    pub schema_id: String,
    pub media_type: MediaType,
    pub json_schema: Option<serde_json::Value>,
    pub prose: Option<String>,
}
```

Output path rules:

```text
required and optional paths must be under output/ unless an adapter explicitly permits a different output subtree.
required output missing -> OutputContract failure.
output exceeds max_bytes -> OutputContract failure.
malformed output that exists -> OutputParse failure.
```

### 6.9 USER: `SlotMarker`

A slot marker binds a slot to its typed output. This removes the extra `Out` type parameter from `AgentBacked`.

```rust
pub trait SlotMarker: Send + Sync + 'static {
    fn role() -> StageRole;

    type Output<P>: Send + Sync + 'static
    where
        P: OptimizationProblem;
}

pub struct ProposerSlot;

impl SlotMarker for ProposerSlot {
    fn role() -> StageRole {
        StageRole::REFLECT
    }

    type Output<P>
        = ProposalBatch<P>
    where
        P: OptimizationProblem;
}
```

GEPA-specific slots define their own markers in `leaven-gepa`:

```rust
pub struct GepaParentSelectorSlot;
pub struct GepaPartSelectorSlot;
pub struct GepaAcceptanceSlot;
pub struct GepaMergeSlot;
```

### 6.10 USER: `AgentStageBootstrap`

Bootstrap converts a typed slot request plus bounded context into a declarative plan.

```rust
pub trait AgentStageBootstrap<P, Slot>: Send + Sync
where
    P: OptimizationProblem,
    Slot: SlotMarker,
{
    type Request: serde::Serialize + Send + Sync + 'static;

    async fn plan(
        &self,
        request: Self::Request,
        ctx: AgentStageCallContext<'_, P>,
    ) -> Result<AgentStagePlan<Self::Request>, StageBootstrapError>;
}

#[derive(Debug)]
pub struct StageBootstrapError {
    pub message: String,
    pub diagnostics: Vec<Diagnostic>,
}
```

Bootstrap must not:

```text
allocate workspace
write workspace files
bypass read scope
mutate graph
parse agent outputs
```

Bootstrap may:

```text
choose role/directive
serialize typed request
choose output contract
choose allowed/prewarm queries
set metadata for receipt/debugging
```

### 6.11 USER: `StageOutputParser`

Parser converts the session/workspace into the typed slot output.

```rust
pub trait StageOutputParser<P, Slot>: Send + Sync
where
    P: OptimizationProblem,
    Slot: SlotMarker,
{
    async fn parse(
        &self,
        workspace: &mut WorkspaceView<'_>,
        session: &AgentSession,
        plan: &ErasedStagePlan,
        ctx: AgentStageCallContext<'_, P>,
    ) -> Result<Metered<Slot::Output<P>>, StageOutputParseError>;
}

#[derive(Clone, Debug)]
pub struct ErasedStagePlan {
    pub role: StageRole,
    pub request_json: serde_json::Value,
    pub directive: StageDirective,
    pub query: StageQueryPolicy,
    pub output: StageOutputContract,
    pub metadata: MetadataBag,
    pub fingerprint: Fingerprint,
}

#[derive(Debug)]
pub struct StageOutputParseError {
    pub message: String,
    pub diagnostics: Vec<Diagnostic>,
    pub files_read: Vec<WorkspacePath>,
}
```

Parser laws:

```text
parse the whole stage output, not one file in isolation.
parser may read required and optional output files.
parser may read agent session metadata/transcript.
parser may read artifact-edit workspace slots if that is the slot's declared output path.
parser must not call StageReadAuthority to reveal more graph data.
parser must return OutputParse error rather than inventing typed output.
```

### 6.12 USER: `MaterializableArtifact`

An artifact opts into workspace representation and change readback.

```rust
pub trait MaterializableArtifact: Artifact {
    async fn write_to(
        &self,
        slot: &mut WorkspaceSlot<'_>,
    ) -> Result<MaterializationReport, WorkspaceSetupError>;

    async fn read_back_change(
        &self,
        slot: &WorkspaceSlot<'_>,
    ) -> Result<Option<Self::Change>, ParseError>;
}

pub trait ReconstructibleArtifact: MaterializableArtifact {
    async fn parse_from(slot: &WorkspaceSlot<'_>) -> Result<Self, ParseError>
    where
        Self: Sized;
}

#[derive(Clone, Debug, Default)]
pub struct MaterializationReport {
    pub entries: Vec<WorkspaceEntryReceipt>,
    pub external_refs: Vec<ExternalRef>,
    pub cost: Cost,
    pub metadata: MetadataBag,
}

#[derive(Debug)]
pub struct WorkspaceSetupError {
    pub message: String,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug)]
pub struct ParseError {
    pub message: String,
    pub diagnostics: Vec<Diagnostic>,
}
```

`None` from `read_back_change` means no meaningful change was produced or the workspace still represents the original artifact.

### 6.13 USER: `AgentBacked`

`AgentBacked` adapts any typed optimizer slot to an agent runtime.

```rust
pub struct AgentBacked<Slot, Runtime, Bootstrap, Parser> {
    pub workspace_factory: std::sync::Arc<dyn WorkspaceFactory>,
    pub runtime: Runtime,
    pub bootstrap: Bootstrap,
    pub parser: Parser,
    pub policy: AgentBackedPolicy,
    _marker: std::marker::PhantomData<Slot>,
}

pub struct AgentBackedPolicy {
    pub workspace: WorkspaceConfig,
    pub runtime_timeout_ms: Option<u64>,
    pub on_parse_failure: ParseFailurePolicy,
    pub cleanup: CleanupPolicy,
    pub tool_exposure: LeavenQueryToolExposure,
    pub receipt_store: ReceiptStorePolicy,
}

pub enum ParseFailurePolicy {
    Strict,
    RecordAttempt,
}

pub enum LeavenQueryToolExposure {
    ShellTool,
    StructuredTool,
    ShellAndStructured,
    Disabled,
}

pub enum ReceiptStorePolicy {
    InlineIfSmall { max_inline_bytes: u64 },
    External,
}
```

No `record_receipt` flag exists. Receipts are always recorded; the policy only chooses storage shape.

`RetryWithFeedback` is intentionally absent. It requires a separate feedback contract and rerun lifecycle.

## 7. ADAPTER surface

### 7.1 ADAPTER: `StageEngineContext<'a, P>`

The engine constructs this and hands it to `leaven-stage`. It is the only engine-to-stage context.

```rust
pub struct StageEngineContext<'a, P: OptimizationProblem> {
    graph: ScopedRunGraphView<'a, P>,
    read_scope: ReadScope,
    evidence_store: Option<&'a dyn EvidenceStore<P::Evidence>>,
    budget: BudgetHandle,
    budget_snapshot: BudgetSnapshot,
    stage_call_id: StageCallId,
    metadata: MetadataBag,
}

impl<'a, P: OptimizationProblem> StageEngineContext<'a, P> {
    pub fn call_context(&'a self) -> AgentStageCallContext<'a, P>;
    pub fn read_authority(&'a self) -> StageReadAuthority<'a, P>;
}
```

`ScopedRunGraphView`, not `RunGraphView`, crosses the boundary.

```rust
pub struct ScopedRunGraphView<'a, P: OptimizationProblem> {
    // private; already read-scope-filtered by the engine
    _marker: std::marker::PhantomData<&'a P>,
}
```

The full graph is not reachable from `leaven-stage`.

### 7.2 ADAPTER: `BudgetHandle`

Budget handle is used by setup, query, runtime, and parse paths to charge cost.

```rust
pub struct BudgetHandle {
    // private
}

impl BudgetHandle {
    pub async fn charge(&mut self, cost: Cost) -> Result<(), BudgetError>;
    pub fn snapshot(&self) -> BudgetSnapshot;
}

#[derive(Debug)]
pub struct BudgetError {
    pub message: String,
}
```

`StageQueryPolicy::max_queries` is a stage-local cap. `BudgetHandle` is the run-level budget path.

### 7.3 ADAPTER: `StageReadAuthority<'a, P>`

The only trusted read/query interface for stage workspaces.

```rust
pub struct StageReadAuthority<'a, P: OptimizationProblem> {
    graph: ScopedRunGraphView<'a, P>,
    read_scope: ReadScope,
    evidence_store: Option<&'a dyn EvidenceStore<P::Evidence>>,
    budget: BudgetHandle,
    stage_call_id: StageCallId,
    counters: QueryCounters,
}

impl<'a, P: OptimizationProblem> StageReadAuthority<'a, P> {
    pub fn read_scope(&self) -> &ReadScope;
    pub fn read_scope_fingerprint(&self) -> Fingerprint;

    pub async fn query(
        &mut self,
        query: StageQuery,
        workspace: &mut WorkspaceView<'_>,
        timing: QueryTiming,
        policy: &StageQueryPolicy,
    ) -> Result<QueryResult, StageQueryError>;
}

#[derive(Clone, Debug, Default)]
pub struct QueryCounters {
    pub total_queries: usize,
    pub total_materialized_bytes: u64,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum QueryTiming {
    Prewarm,
    AgentRequested,
}
```

`StageReadAuthority::query` must:

```text
check AllowedQuerySet
check max_queries
check max_materialized_bytes
check ReadScope
choose safe placement
write entries into workspace if query result has files
charge budget
return QueryResult
produce QueryRecord through receipt builder
```

### 7.4 ADAPTER: query results

```rust
pub struct QueryResult {
    pub query_id: StageQueryId,
    pub timing: QueryTiming,
    pub query: StageQuery,
    pub effect: QueryEffect,
    pub cost: Cost,
}

pub enum QueryEffect {
    WroteEntries {
        entries: Vec<WorkspaceEntryReceipt>,
        summary: QuerySummary,
    },
    ReturnedSummary(QuerySummary),
    NotVisible(NotVisibleReason),
    NotFound(NotFoundReason),
    PolicyDenied(PolicyDenial),
}

pub struct QuerySummary {
    pub text: Option<String>,
    pub json: Option<serde_json::Value>,
    pub truncated: bool,
}

pub struct NotVisibleReason {
    pub message: String,
    pub requested: serde_json::Value,
}

pub struct NotFoundReason {
    pub message: String,
    pub requested: serde_json::Value,
}

pub struct PolicyDenial {
    pub message: String,
    pub policy: String,
}

#[derive(Debug)]
pub struct StageQueryError {
    pub message: String,
    pub diagnostics: Vec<Diagnostic>,
}
```

`NotVisible` is a normal query result. Path escape, malformed query syntax, and disabled query kinds are errors or policy denials.

### 7.5 ADAPTER: workspace entries

A `WorkspaceEntry` is graph/evidence-derived state to be written into a workspace by a query. Users should rarely construct one manually.

```rust
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

#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, serde::Serialize, serde::Deserialize)]
pub struct WorkspaceEntryRole(SmolStr);

impl WorkspaceEntryRole {
    pub const BRIEF: Self = Self(SmolStr::new_static("brief"));
    pub const FOCUS_REQUEST: Self = Self(SmolStr::new_static("focus_request"));
    pub const STAGE_INSTRUCTIONS: Self = Self(SmolStr::new_static("stage_instructions"));
    pub const CANDIDATE_ARTIFACT: Self = Self(SmolStr::new_static("candidate_artifact"));
    pub const SELECTED_PART: Self = Self(SmolStr::new_static("selected_part"));
    pub const SELECTED_FEEDBACK: Self = Self(SmolStr::new_static("selected_feedback"));
    pub const TRACE_EXCERPT: Self = Self(SmolStr::new_static("trace_excerpt"));
    pub const ASSESSMENT_SUMMARY: Self = Self(SmolStr::new_static("assessment_summary"));
    pub const LINEAGE_SUMMARY: Self = Self(SmolStr::new_static("lineage_summary"));
    pub const FRONTIER_SUMMARY: Self = Self(SmolStr::new_static("frontier_summary"));
    pub const TREE_SUMMARY: Self = Self(SmolStr::new_static("tree_summary"));
    pub const TOOL_CONFIG: Self = Self(SmolStr::new_static("tool_config"));
    pub const RUNTIME_CONFIG: Self = Self(SmolStr::new_static("runtime_config"));
    pub const OUTPUT_SCHEMA: Self = Self(SmolStr::new_static("output_schema"));
    pub const OUTPUT_DIRECTORY: Self = Self(SmolStr::new_static("output_directory"));

    pub fn custom(value: impl Into<SmolStr>) -> Result<Self, RoleTagError>;
    pub fn as_str(&self) -> &str;
}
```

Entry source:

```rust
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum EntrySource {
    InlineText,
    InlineBytes,
    Generated,
    Candidate(CandidateId),
    Assessment(AssessmentId),
    Evidence(InfoRef),
    Proposal(ProposalId),
    Trace(TraceRef),
    RenderedView(RenderedViewId),
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, serde::Serialize, serde::Deserialize)]
pub struct RenderedViewId(pub String);

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum EntryProjection {
    Full,
    Summary,
    Artifact,
    Assessments,
    EvidenceSummary,
    TraceExcerpt,
    Diff,
    Custom(String),
}
```

There is one candidate source variant. “Candidate artifact” is a projection/role, not a separate source.

Entry access:

```rust
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum EntryAccess {
    InputReadOnly,
    EditableArtifact,
    OutputWritable,
}
```

No `Execute` variant exists. Executability belongs to Leaven-owned tools installed during workspace setup, not arbitrary materialized entries.

Placement:

```rust
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Placement {
    pub path: WorkspacePath,
    pub collision: CollisionPolicy,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum CollisionPolicy {
    Error,
    OverwriteIfSameFingerprint,
    Replace,
    CreateSibling,
}
```

No `MaterializationTarget` exists. If a query returns summary only and writes no file, represent that with `QueryEffect::ReturnedSummary`, not with a receipt-only workspace entry.

### 7.6 ADAPTER: workspace setup

```rust
pub async fn setup_stage_workspace<P, Req>(
    workspace: &mut Workspace,
    plan: &AgentStagePlan<Req>,
    authority: &mut StageReadAuthority<'_, P>,
    policy: &AgentBackedPolicy,
) -> Result<StageAttemptReceiptBuilder, WorkspaceSetupError>
where
    P: OptimizationProblem,
    Req: serde::Serialize;
```

`setup_stage_workspace` performs:

```text
1. create default directory skeleton
2. write BRIEF.md
3. write focus/stage_role.txt
4. serialize request to focus/request.json
5. write focus/instructions.md
6. write .leaven/plan.json
7. write .leaven/output_schema.json if present
8. create required/optional output placeholder dirs
9. install tools/leaven_query if enabled
10. run plan.query.prewarm through StageReadAuthority::query(..., Prewarm, ...)
11. create receipt builder with setup and query records
```

It must not:

```text
parse output
run the agent
mutate graph
access unscoped graph view
materialize hidden verifier data
```

### 7.7 ADAPTER: `StageAttemptReceiptBuilder`

The setup function returns a builder because the receipt is assembled across setup, runtime, output validation, parsing, and cleanup.

```rust
pub struct StageAttemptReceiptBuilder {
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
    pub metadata: MetadataBag,
}

impl StageAttemptReceiptBuilder {
    pub fn record_query(&mut self, record: QueryRecord);
    pub fn record_output(&mut self, output: OutputEntryReceipt);
    pub fn record_parse(&mut self, parse: ParseReceipt);
    pub fn record_session(&mut self, session: AgentSessionId);
    pub fn charge(&mut self, cost: Cost);

    pub fn finish(self, outcome: StageAttemptOutcome) -> StageAttemptReceipt;
}
```

### 7.8 ADAPTER: default setup files

```rust
pub struct WorkspaceSetupReceipt {
    pub entries: Vec<WorkspaceEntryReceipt>,
    pub tool_paths: Vec<WorkspacePath>,
    pub output_paths: Vec<WorkspacePath>,
    pub plan_path: WorkspacePath,
    pub brief_path: WorkspacePath,
    pub cost: Cost,
}
```

Setup entries have `EntrySource::Generated` or `EntrySource::InlineText`. They are not `StageQuery` results.

## 8. RECEIPT surface

### 8.1 RECEIPT: `StageAttemptReceipt`

```rust
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
```

Receipt minimum requirements:

```text
full ReadScope plus fingerprint
plan fingerprint
source refs for query-written entries
path and fingerprint for setup/query/output files
query records for prewarm and agent-requested queries
output file fingerprints
parse status and diagnostics
session id when an agent session exists
outcome
```

### 8.2 RECEIPT: `QueryRecord`

```rust
pub struct QueryRecord {
    pub query_id: StageQueryId,
    pub timing: QueryTiming,
    pub query: StageQuery,
    pub effect: QueryRecordEffect,
    pub cost: Cost,
}

pub enum QueryRecordEffect {
    WroteEntries(Vec<WorkspaceEntryReceipt>),
    ReturnedSummary(QuerySummary),
    NotVisible(NotVisibleReason),
    NotFound(NotFoundReason),
    PolicyDenied(PolicyDenial),
    Error(Vec<Diagnostic>),
}
```

There is no `eager` or `lazy` field. Timing is `Prewarm` or `AgentRequested`.

### 8.3 RECEIPT: `WorkspaceEntryReceipt`

```rust
pub struct WorkspaceEntryReceipt {
    pub id: WorkspaceEntryId,
    pub path: WorkspacePath,
    pub role: WorkspaceEntryRole,
    pub source: EntrySourceRef,
    pub projection: EntryProjection,
    pub access: EntryAccess,
    pub fingerprint: Fingerprint,
    pub bytes: Option<u64>,
    pub truncation: Option<TruncationNote>,
    pub produced_by_query: Option<StageQueryId>,
    pub metadata: MetadataBag,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum EntrySourceRef {
    InlineText { fingerprint: Fingerprint },
    InlineBytes { fingerprint: Fingerprint },
    Generated { generator: String },
    Candidate(CandidateId),
    Assessment(AssessmentId),
    Evidence(InfoRef),
    Proposal(ProposalId),
    Trace(TraceRef),
    RenderedView(RenderedViewId),
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct TruncationNote {
    pub original_bytes: Option<u64>,
    pub retained_bytes: u64,
    pub reason: String,
}
```

Without `EntrySourceRef`, Leaven cannot audit no-hidden-data claims.

### 8.4 RECEIPT: output and parse

```rust
pub struct OutputEntryReceipt {
    pub id: OutputEntryId,
    pub path: WorkspacePath,
    pub role: OutputRole,
    pub fingerprint: Fingerprint,
    pub bytes: Option<u64>,
    pub status: OutputFileStatus,
}

pub enum OutputFileStatus {
    Present,
    MissingRequired,
    MissingOptional,
    ExceededMaxBytes,
    Unreadable,
}

pub struct ParseReceipt {
    pub status: ParseStatus,
    pub diagnostics: Vec<Diagnostic>,
    pub files_read: Vec<WorkspacePath>,
    pub cost: Cost,
}

pub enum ParseStatus {
    NotAttempted,
    Succeeded,
    Failed,
}
```

### 8.5 RECEIPT: attempt outcome

```rust
pub enum StageAttemptOutcome {
    Completed,
    Failed(StageAttemptFailure),
}

pub enum StageAttemptFailure {
    WorkspaceAllocate,
    WorkspaceSetup,
    Query(StageQueryId),
    RuntimeTimeout,
    Runtime,
    OutputContract,
    OutputParse,
    Cleanup,
    StageAndCleanup {
        stage: Box<StageAttemptFailure>,
        cleanup: Box<StageAttemptFailure>,
    },
}
```

### 8.6 RECEIPT: storage

Receipts are durable. They may be inline or external depending on size.

```rust
pub trait StageReceiptStore: Send + Sync {
    async fn write_receipt(
        &self,
        receipt: StageAttemptReceipt,
    ) -> Result<StageAttemptReceiptRef, StageReceiptStoreError>;

    async fn read_receipt(
        &self,
        id: &StageAttemptReceiptId,
    ) -> Result<StageAttemptReceipt, StageReceiptStoreError>;
}

#[derive(Clone, Debug)]
pub enum StageAttemptReceiptRef {
    Inline(StageAttemptReceiptId),
    External {
        id: StageAttemptReceiptId,
        uri: String,
        fingerprint: Fingerprint,
    },
}

#[derive(Debug)]
pub struct StageReceiptStoreError {
    pub message: String,
}
```

Large receipts should use a store, not bloat the graph directly. The graph can carry the receipt ref.

## 9. stage lifecycle

Every agent-backed optimizer slot follows this lifecycle.

```text
1. owning optimizer creates typed slot request
2. engine creates StageEngineContext with scoped graph view and ReadScope
3. adapter calls bootstrap.plan(request, AgentStageCallContext)
4. adapter allocates workspace through WorkspaceFactory
5. setup_stage_workspace writes setup files and runs prewarm queries
6. adapter builds AgentRunRequest from plan, output contract, workspace, and tool policy
7. AgentRuntime runs one session
8. runtime/agent may call leaven_query; each call goes through StageReadAuthority
9. adapter checks required output files
10. StageOutputParser parses workspace/session into Slot::Output<P>
11. adapter records StageAttemptReceipt and emits StageAttemptRecorded
12. workspace cleanup runs
13. adapter returns typed output to slot trait implementation
14. owning optimizer / RunContext finalizes the typed result
```

Failure mapping:

```text
workspace allocation failure -> StageAttemptFailure::WorkspaceAllocate
setup failure                -> StageAttemptFailure::WorkspaceSetup
prewarm query failure        -> StageAttemptFailure::Query(query_id) or WorkspaceSetup
runtime timeout              -> StageAttemptFailure::RuntimeTimeout
runtime provider failure     -> StageAttemptFailure::Runtime
missing required output      -> StageAttemptFailure::OutputContract
malformed output             -> StageAttemptFailure::OutputParse
cleanup failure              -> StageAttemptFailure::Cleanup or StageAndCleanup
parsed proposal apply fail   -> existing ApplyFailed, outside stage attempt failure
```

`ParseFailurePolicy` behavior:

```text
Strict
  record receipt with OutputParse failure, return error to caller.

RecordAttempt
  record receipt with OutputParse failure, return a slot-specific error or empty/no-op result only if the slot trait explicitly permits no-op results.
  Do not pretend parse failure is ApplyFailed.
```

## 10. default workspace layout

Layout is stable for model ergonomics. It is not the semantic contract.

Always present after workspace setup:

```text
/workspace/
├── BRIEF.md
├── focus/
│   ├── stage_role.txt
│   ├── request.json
│   └── instructions.md
├── output/
│   └── ... required/optional paths from StageOutputContract
└── .leaven/
    ├── plan.json
    ├── output_schema.json
    ├── receipt.partial.json
    └── query_policy.json
```

Present when enabled by `StageQueryPolicy`, either prewarm or agent-requested:

```text
/workspace/
├── graph/
│   ├── index.json
│   ├── frontier.md
│   ├── tree.md
│   ├── candidates/<candidate_id>/...
│   ├── assessments/<assessment_id>/...
│   └── lineage/<candidate_id>.md
├── artifacts/
│   └── <candidate_id>/...
├── evidence/
│   └── <evidence_ref>/...
├── traces/
│   └── <trace_ref>/...
├── diffs/
│   └── <left>__<right>.md
├── tools/
│   └── leaven_query
└── logs/
```

`BRIEF.md` is generated from:

```text
role
directive
output contract
available directories/tools
query policy summary
no-leak invariant
expected completion format
```

Do not eagerly write a giant graph by default. The plan controls prewarm richness.

## 11. `leaven_query`

### 11.1 shell CLI

Initial shell surface:

```text
leaven_query help
leaven_query list candidates [--frontier] [--page N] [--page-size K]
leaven_query candidate <candidate_id> [--summary] [--artifact] [--assessments K]
leaven_query assessment <assessment_id> [--summary] [--with-evidence-refs] [--with-trace-refs]
leaven_query evidence <info_ref> [--summary] [--text] [--max-bytes N]
leaven_query lineage <candidate_id> [--depth N] [--include-assessments]
leaven_query diff <left_candidate_id> <right_candidate_id> [--artifact] [--scores]
```

The CLI prints a concise result and, when files are written, paths.

Example:

```text
$ leaven_query candidate cand_047 --artifact --assessments 3
wrote:
  graph/candidates/cand_047/summary.json
  artifacts/cand_047/
  graph/candidates/cand_047/assessments/asmt_101.json
  graph/candidates/cand_047/assessments/asmt_102.json
  graph/candidates/cand_047/assessments/asmt_103.json
```

### 11.2 structured tool

Provider adapters may expose the same query as a structured tool:

```rust
pub struct StructuredLeavenQueryCall {
    pub query: StageQuery,
}

pub struct StructuredLeavenQueryResult {
    pub result: QueryEffect,
    pub paths: Vec<WorkspacePath>,
    pub summary: Option<QuerySummary>,
}
```

The runtime adapter decides how to expose the tool:

```text
Codex app server: structured tool if supported
Claude Code CLI: shell command
command runtime: shell command
fake runtime: direct call in tests
```

The policy is semantic, not provider-specific.

### 11.3 cost accounting

Every query charges budget through the stage's `BudgetHandle`.

Cost includes:

```text
bytes read from graph/evidence store
bytes written into workspace
rendering/summarization cost
provider/tool-call overhead if any
wall time
```

Prewarm query cost is charged before agent runtime starts. Agent-requested query cost is charged during the session and appears in both `AgentSession` metadata and `StageAttemptReceipt.queries`.

### 11.4 no-leak behavior

A query for hidden data returns:

```rust
QueryEffect::NotVisible(NotVisibleReason { ... })
```

A query that tries to escape workspace paths, exceed policy, or use disabled query kind returns `PolicyDenied` or `StageQueryError` depending on severity.

## 12. workspace factory context and slots

### 12.1 `WorkspaceFactoryContext`

Some artifact implementations need declared factory state, such as a jj repo handle. The factory context is a typed registry.

```rust
pub struct WorkspaceFactoryContext {
    entries: std::collections::HashMap<std::any::TypeId, std::sync::Arc<dyn std::any::Any + Send + Sync>>,
}

impl WorkspaceFactoryContext {
    pub fn new() -> Self;

    pub fn insert<T>(&mut self, value: T)
    where
        T: Send + Sync + 'static;

    pub fn get<T>(&self) -> Result<std::sync::Arc<T>, FactoryContextError>
    where
        T: Send + Sync + 'static;
}

#[derive(Debug)]
pub struct FactoryContextError {
    pub type_name: &'static str,
    pub message: String,
}
```

Rules:

```text
factory context is declared at WorkspaceFactory construction time
retrieval is by TypeId
missing context is an explicit error
context values must be Send + Sync + 'static
context access is recorded in MaterializationReport metadata when relevant
```

### 12.2 `WorkspaceSlot<'a>`

A slot is a scoped subdirectory view plus access to declared factory context.

```rust
pub struct WorkspaceSlot<'a> {
    view: WorkspaceView<'a>,
    root: WorkspacePath,
    factory_context: &'a WorkspaceFactoryContext,
}

impl<'a> WorkspaceSlot<'a> {
    pub fn root(&self) -> &WorkspacePath;
    pub fn view(&mut self) -> &mut WorkspaceView<'a>;

    pub fn factory_context<T>(&self) -> Result<std::sync::Arc<T>, FactoryContextError>
    where
        T: Send + Sync + 'static;

    pub async fn write_file(
        &mut self,
        path: WorkspacePath,
        bytes: &[u8],
    ) -> Result<(), WorkspaceIoError>;

    pub async fn read_file(
        &self,
        path: WorkspacePath,
    ) -> Result<Vec<u8>, WorkspaceIoError>;

    pub async fn run_command(
        &mut self,
        command: Command,
    ) -> Result<CommandOutput, CommandError>;
}

#[derive(Debug)]
pub struct WorkspaceIoError {
    pub message: String,
}

pub struct Command {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: Option<WorkspacePath>,
    pub timeout_ms: Option<u64>,
}

pub struct CommandOutput {
    pub status: i32,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub cost: Cost,
}

#[derive(Debug)]
pub struct CommandError {
    pub message: String,
    pub output: Option<CommandOutput>,
}
```

Slot laws:

```text
all paths are relative to the slot root
path escapes fail
factory context access is explicit
run_command cwd defaults to slot root
slot commands cannot access hidden workspace paths by convention alone; backend policy must enforce path containment where security matters
```

## 13. artifact capability tiers

### 13.1 tier model

```text
Tier 0: P::Artifact: Artifact
  classical optimizers and LM-only proposers can work.

Tier 1: P::Artifact: MaterializableArtifact
  agent-backed stages that need workspace access to artifacts can work.

Tier 2: P::Artifact: MaterializableArtifact + CacheIdentified
  deterministic cache-safe evaluation can work.
```

```rust
pub trait CacheIdentified {
    fn cache_identity(&self) -> Option<CacheIdentity>;
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum CacheIdentity {
    Inline(Fingerprint),
    ExternalContent(ExternalRef),
}
```

### 13.2 laws for `MaterializableArtifact`

```text
write_to may only create agent-visible workspace state inside the provided slot.

write_to may use declared WorkspaceFactory capabilities, such as a jj repo handle,
but those capabilities must be explicit and receipt-visible.

read_back_change may depend on the slot and declared factory context.
It must not depend on ambient host filesystem state outside the slot.

write_to followed by read_back_change on an unchanged slot must return None or
a no-op-equivalent change.

read_back_change must fail rather than invent a change when the workspace is invalid.

For codebase-like artifacts, read_back_change should usually return a compact typed change
such as JjAdvance, GitPatch, or SkillFilePatch, not a reconstructed giant artifact.
```

### 13.3 worked example: small text artifact with both paths

```rust
#[derive(Debug)]
pub struct TextApplyError {
    pub message: String,
}

impl std::fmt::Display for TextApplyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for TextApplyError {}

fn fingerprint_bytes(bytes: &[u8]) -> Fingerprint {
    Fingerprint {
        algorithm: "sha256".into(),
        value: hex_sha256(bytes),
    }
}

fn hex_sha256(_bytes: &[u8]) -> String {
    // placeholder for spec; implementation uses the workspace/kernel fingerprint helper
    "<sha256>".into()
}

#[derive(Clone)]
pub struct TextArtifact {
    pub text: String,
}

pub struct ReplaceText {
    pub new_text: String,
}

impl Artifact for TextArtifact {
    type Change = ReplaceText;
    type ApplyError = TextApplyError;

    fn identity(&self) -> ArtifactIdentity {
        ArtifactIdentity::Inline(fingerprint_bytes(self.text.as_bytes()))
    }

    fn apply_change(&self, change: &ReplaceText) -> Result<Self, TextApplyError> {
        Ok(TextArtifact {
            text: change.new_text.clone(),
        })
    }
}

impl MaterializableArtifact for TextArtifact {
    async fn write_to(
        &self,
        slot: &mut WorkspaceSlot<'_>,
    ) -> Result<MaterializationReport, WorkspaceSetupError> {
        slot.write_file(WorkspacePath::new("artifact.txt")?, self.text.as_bytes()).await?;
        Ok(MaterializationReport::default())
    }

    async fn read_back_change(
        &self,
        slot: &WorkspaceSlot<'_>,
    ) -> Result<Option<ReplaceText>, ParseError> {
        let bytes = slot.read_file(WorkspacePath::new("artifact.txt")?).await?;
        let new_text = String::from_utf8(bytes).map_err(|e| ParseError {
            message: e.to_string(),
            diagnostics: vec![],
        })?;

        if new_text == self.text {
            Ok(None)
        } else {
            Ok(Some(ReplaceText { new_text }))
        }
    }
}

impl ReconstructibleArtifact for TextArtifact {
    async fn parse_from(slot: &WorkspaceSlot<'_>) -> Result<Self, ParseError> {
        let bytes = slot.read_file(WorkspacePath::new("artifact.txt")?).await?;
        let text = String::from_utf8(bytes).map_err(|e| ParseError {
            message: e.to_string(),
            diagnostics: vec![],
        })?;
        Ok(TextArtifact { text })
    }
}
```

This example exercises both paths:

```text
edit-in-place: write_to -> agent edits artifact.txt -> read_back_change -> ReplaceText
fresh-create/full reconstruction: parse_from -> TextArtifact
```

### 13.4 worked example: jj codebase artifact

```rust
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct JjChangeId(String);

impl JjChangeId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct JjCommitId(String);

impl JjCommitId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone)]
pub struct JjRepoHandle {
    // private durable repo handle
}

impl JjRepoHandle {
    pub fn contains(&self, _change: &JjChangeId) -> Result<(), JjError> {
        todo!()
    }

    pub fn commit_for(&self, _change: &JjChangeId) -> Result<JjCommitId, JjError> {
        todo!()
    }

    pub async fn add_workspace(
        &self,
        _path: &WorkspacePath,
        _change: &JjChangeId,
    ) -> Result<(), JjError> {
        todo!()
    }

    pub async fn current_change_id(
        &self,
        _path: &WorkspacePath,
    ) -> Result<JjChangeId, JjError> {
        todo!()
    }
}

#[derive(Debug)]
pub struct JjError {
    pub message: String,
}

impl std::fmt::Display for JjError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for JjError {}

#[derive(Clone)]
pub struct JjCodebase {
    pub change_id: JjChangeId,
    pub repo: std::sync::Arc<JjRepoHandle>,
}

#[derive(Clone)]
pub struct JjAdvance {
    pub new_change_id: JjChangeId,
}

impl Artifact for JjCodebase {
    type Change = JjAdvance;
    type ApplyError = JjError;

    fn identity(&self) -> ArtifactIdentity {
        ArtifactIdentity::External(ExternalRef {
            scheme: "jj-change".into(),
            value: self.change_id.as_str().to_string(),
        })
    }

    fn apply_change(&self, change: &JjAdvance) -> Result<Self, JjError> {
        self.repo.contains(&change.new_change_id)?;
        Ok(JjCodebase {
            change_id: change.new_change_id.clone(),
            repo: self.repo.clone(),
        })
    }
}

impl CacheIdentified for JjCodebase {
    fn cache_identity(&self) -> Option<CacheIdentity> {
        let commit = self.repo.commit_for(&self.change_id).ok()?;
        Some(CacheIdentity::ExternalContent(ExternalRef {
            scheme: "jj-commit".into(),
            value: commit.as_str().to_string(),
        }))
    }
}

impl MaterializableArtifact for JjCodebase {
    async fn write_to(
        &self,
        slot: &mut WorkspaceSlot<'_>,
    ) -> Result<MaterializationReport, WorkspaceSetupError> {
        let repo = slot.factory_context::<JjRepoHandle>()?;
        repo.add_workspace(slot.root(), &self.change_id).await?;
        Ok(MaterializationReport {
            external_refs: vec![ExternalRef {
                scheme: "jj-change".into(),
                value: self.change_id.as_str().to_string(),
            }],
            ..Default::default()
        })
    }

    async fn read_back_change(
        &self,
        slot: &WorkspaceSlot<'_>,
    ) -> Result<Option<JjAdvance>, ParseError> {
        let repo = slot.factory_context::<JjRepoHandle>()?;
        let new_change_id = repo.current_change_id(slot.root()).await?;

        if new_change_id == self.change_id {
            Ok(None)
        } else {
            Ok(Some(JjAdvance { new_change_id }))
        }
    }
}
```

For codebases, `read_back_change` is the natural path. Full reconstruction is usually unnecessary.

## 14. `AgentBacked` slot implementations

### 14.1 engine proposer slot

```rust
pub trait Proposer<P: OptimizationProblem>: Send + Sync {
    type Request: Send + Sync + 'static;

    async fn propose(
        &self,
        request: Self::Request,
        ctx: ProposalContext<'_, P>,
    ) -> Result<Metered<ProposalBatch<P>>, ProposalError>;
}

impl<P, Runtime, Bootstrap, Parser> Proposer<P>
    for AgentBacked<ProposerSlot, Runtime, Bootstrap, Parser>
where
    P: OptimizationProblem,
    Runtime: AgentRuntime,
    Bootstrap: AgentStageBootstrap<P, ProposerSlot>,
    Parser: StageOutputParser<P, ProposerSlot>,
{
    type Request = Bootstrap::Request;

    async fn propose(
        &self,
        request: Self::Request,
        ctx: ProposalContext<'_, P>,
    ) -> Result<Metered<ProposalBatch<P>>, ProposalError> {
        // 1. engine builds StageEngineContext from ProposalContext
        // 2. bootstrap.plan(...)
        // 3. allocate workspace
        // 4. setup_stage_workspace(...)
        // 5. runtime.run(...)
        // 6. parser.parse(...)
        // 7. record receipt/event
        // 8. cleanup
        // 9. return ProposalBatch<P>
        todo!()
    }
}
```

### 14.2 GEPA slots

GEPA-defined slot traits get GEPA-owned impls:

```rust
pub trait ParentSelector<P: OptimizationProblem>: Send + Sync {
    async fn select_parent(
        &self,
        request: ParentSelectionRequest,
        ctx: GepaStageContext<'_, P>,
    ) -> Result<Metered<CandidateId>, ParentSelectionError>;
}

impl<P, Runtime, Bootstrap, Parser> ParentSelector<P>
    for AgentBacked<GepaParentSelectorSlot, Runtime, Bootstrap, Parser>
where
    P: OptimizationProblem,
    Runtime: AgentRuntime,
    Bootstrap: AgentStageBootstrap<P, GepaParentSelectorSlot>,
    Parser: StageOutputParser<P, GepaParentSelectorSlot>,
{
    async fn select_parent(
        &self,
        request: ParentSelectionRequest,
        ctx: GepaStageContext<'_, P>,
    ) -> Result<Metered<CandidateId>, ParentSelectionError> {
        todo!()
    }
}
```

The pattern is identical. Only the typed request and typed parser output differ.

### 14.3 adding a new agentic slot

To add a new agent-backed slot:

```text
1. define the slot trait, if it does not already exist
2. define a SlotMarker with role() and Output<P>
3. define typed request struct
4. implement AgentStageBootstrap<P, Slot>
5. implement StageOutputParser<P, Slot>
6. add impl SlotTrait for AgentBacked<Slot, Runtime, Bootstrap, Parser>
7. add contract tests for setup/query/output/receipt behavior
```

Do not add a new workspace type per slot.

## 15. GEPA goal state

### 15.1 GEPA owns rhythm, not substrate

GEPA owns:

```text
parent selection
part selection
feedback/context selection
reflection request construction
population observation
acceptance/gating
validation rhythm
merge/conflict rhythm
checkpoint state
```

GEPA does not own:

```text
workspace backend semantics
agent runtime provider details
graph mutation finalization
cache truth
workspace cleanup guarantees
```

### 15.2 selected-pull, owned-push reflection

GEPA reflection should be neither pure push nor pure pull.

Goal flow:

```text
FeedbackSelector pulls eligible context under ReadScope.
ReflectRequest owns selected context refs/summaries.
AgentBacked sets up workspace and prewarms selected context queries.
Agent may request more via leaven_query, still under same ReadScope.
Parser returns ProposalBatch<P>.
RunContext records/applies the batch.
```

### 15.3 reflect request

```rust
pub struct ReflectRequest<P, S>
where
    P: OptimizationProblem,
    S: EditSurface<P::Artifact>,
{
    pub parent: CandidateId,
    pub parent_assessment: Option<AssessmentId>,
    pub selected_part: Option<S::PartId>,
    pub selected_feedback: SelectedFeedback,
    pub objective: ReflectionObjective,
    pub proposal_count: std::num::NonZeroUsize,
    pub surface_fingerprint: Option<Fingerprint>,
    pub metadata: MetadataBag,
    pub _marker: std::marker::PhantomData<P>,
}

pub trait EditSurface<A: Artifact>: Send + Sync {
    type PartId: Clone + Send + Sync + serde::Serialize + 'static;
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct SelectedFeedback {
    pub assessment_refs: Vec<AssessmentId>,
    pub evidence_refs: Vec<InfoRef>,
    pub candidate_refs: Vec<CandidateId>,
    pub case_summaries: Vec<CaseFeedbackSummary>,
    pub trace_refs: Vec<InfoRef>,
    pub attribution: Option<AttributionSummary>,
    pub provenance_refs: Vec<InfoRef>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct CaseFeedbackSummary {
    pub case_id: String,
    pub score: Option<f64>,
    pub feedback: Option<String>,
    pub evidence_refs: Vec<InfoRef>,
    pub metadata: MetadataBag,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct AttributionSummary {
    pub selected_part: Option<String>,
    pub supporting_traces: Vec<InfoRef>,
    pub explanation: Option<String>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ReflectionObjective {
    pub goal: String,
    pub constraints: Vec<String>,
    pub optimize_for: Vec<String>,
}
```

For a non-surface proposer, `selected_part` and `surface_fingerprint` are absent.

### 15.4 feedback selector

```rust
pub trait FeedbackSelector<P: OptimizationProblem>: Send + Sync {
    async fn select(
        &mut self,
        request: FeedbackSelectionRequest,
        authority: &mut StageReadAuthority<'_, P>,
    ) -> Result<Metered<SelectedFeedback>, FeedbackSelectionError>;
}

#[derive(Clone, Debug)]
pub struct FeedbackSelectionRequest {
    pub parent: CandidateId,
    pub selected_part_hint: Option<String>,
    pub max_cases: Option<usize>,
    pub objective: ReflectionObjective,
    pub metadata: MetadataBag,
}

#[derive(Debug)]
pub struct FeedbackSelectionError {
    pub message: String,
    pub diagnostics: Vec<Diagnostic>,
}
```

Default selector:

```text
current parent assessment
current minibatch failures
casewise scalar/feedback summaries
selected part attribution if available
```

Stronger selectors may include:

```text
historical failures for parent
top-k failures attributed to selected part
neighboring frontier candidates
prior rejected proposal diagnostics
merge/conflict diagnostics
```

### 15.5 finalizing path

GEPA reflection should use the engine proposer path when the reflector is a `Proposer<P>`.

```rust
let request = ReflectRequest { /* selected context */ };
let batch = ctx.propose(&self.reflector, request).await?;
let applied = ctx.apply_batch(batch.batch_id)?;
```

Surface-native reflection may use a GEPA-owned adapter that lowers surface edits into `ProposalBatch<P>` before returning from `Proposer::propose`.

`LmBackedReflector` and agent-backed GEPA reflectors should not name a fixed-edit fixture in the public ordinary surface.

## 16. retained task/workload boundary

The existing task-ish format should be retained under a narrower responsibility.

### 16.1 A-shaped candidate evaluation workload

Keep:

```text
AgentCase
CaseSuite
CasePartitions
AgentWorkload
AgentCaseEvaluator
AgentCasePresenter
AgentCaseScorer
AgentCaseRunRecord
CaseFiles
CaseInput
CaseTarget
SetupScript
WorkspaceRequirement
```

This layer answers:

```text
what world does the candidate artifact run in, and how is that run scored?
```

### 16.2 B-shaped optimizer stage workspace

Stage workspace answers:

```text
what world does the optimizer's own agent deliberate in, and what typed decision does it return?
```

Do not put `AgentCase` in signatures for:

```text
AgentStagePlan
AgentBacked
StageAttemptReceipt
GEPA slot traits
StageOutputParser
StageReadAuthority
```

`AgentCase` may appear upstream as evidence. The stage layer sees selected assessment/evidence refs, not the task format itself.

### 16.3 flow over an agentic workload

```text
AgentCaseEvaluator runs candidate on AgentCase
  -> Assessment<P> + evidence/metadata
  -> GEPA FeedbackSelector selects eligible evidence
  -> AgentStagePlan prewarms selected context into optimizer-stage workspace
  -> reflector writes output/proposal.json
  -> StageOutputParser converts to ProposalBatch<P>
  -> RunContext records/applies proposal batch
```

Same workspace substrate, different sovereignty.

## 17. events and graph effects

### 17.1 stage attempt event

Use one generic event.

```rust
pub enum RunEvent {
    StageAttemptRecorded {
        stage_call_id: StageCallId,
        role: StageRole,
        receipt: StageAttemptReceiptRef,
        outcome: StageAttemptOutcome,
    },

    ProposalBatchProduced {
        batch_id: ProposalBatchId,
    },

    ProposalRecorded {
        proposal_id: ProposalId,
    },

    ApplySucceeded {
        proposal_id: ProposalId,
        candidate_id: CandidateId,
    },

    ApplyFailed {
        proposal_id: ProposalId,
        diagnostic: Diagnostic,
    },
}
```

Do not add many `AgentStageStarted`, `AgentStageMaterialized`, `AgentStageCompleted`, etc. variants until observers actually need that distinction. The receipt has the detail.

### 17.2 graph mutation boundary

Stage attempts do not create candidates by themselves. They create typed outputs.

```text
ProposerSlot output         -> ProposalBatch<P> -> RunContext records/applies
ParentSelectorSlot output   -> CandidateId      -> GEPA uses for next rhythm step
AcceptanceSlot output       -> decision         -> GEPA controls population update
MergeSlot output            -> ProposalBatch<P> or merge plan -> finalizing path
```

## 18. checkpoint and resume

Workspaces are ephemeral. Receipts are durable.

A checkpoint stores:

```text
graph truth
budget/cache state
optimizer private state
stage attempt receipt refs
selected source refs
output fingerprints
read scopes
plan fingerprints
```

A checkpoint does not normally store live workspace contents.

Reconstruction requires:

```text
graph state at checkpoint
stage plan fingerprint
read scope
query records and source refs
output contract
workspace backend capability
```

If graph state no longer matches receipt source fingerprints, reconstruction fails loudly.

## 19. contract tests

### 19.1 A/B boundary

```text
AgentCaseEvaluator runs a candidate case without depending on leaven-stage.
AgentBacked<ProposerSlot> runs a reflector without depending on AgentCase.
A GEPA run over AgentCaseEvaluator evidence can prewarm selected assessment summaries into a stage workspace.
```

### 19.2 read-scope laws

```text
hidden partitions are absent from prewarm queries.
hidden partitions return NotVisible through leaven_query.
receipt records source refs for every query-written entry.
StageReadAuthority is used by both prewarm and agent-requested queries.
```

### 19.3 output parsing

```text
missing required output produces OutputContract failure.
malformed output produces OutputParse failure.
parse failure does not emit ApplyFailed.
parsed proposal that fails artifact application emits ApplyFailed.
```

### 19.4 artifact materialization

```text
unchanged workspace returns None or no-op change.
invalid workspace returns parse error.
declared factory context is available in WorkspaceSlot.
workspace path escape attempts fail.
write_to cannot write outside the slot.
```

### 19.5 query accounting

```text
prewarm queries count against max_queries.
agent-requested queries count against max_queries.
prewarm and agent-requested query records have same structure.
max_materialized_bytes counts query-written bytes.
query cost charges BudgetHandle.
```

### 19.6 receipt durability

```text
receipt contains full ReadScope.
receipt contains read_scope_fingerprint.
receipt contains plan_fingerprint.
receipt contains query records.
receipt contains output fingerprints.
receipt can be stored inline if small and external if large.
```

### 19.7 GEPA reflection proof

```text
fake agent reads selected feedback.
fake agent writes output/proposal.json.
parser returns a non-hardcoded ProposalBatch<P>.
RunContext::propose records the proposal batch.
RunContext::apply_batch applies it.
receipt includes selected feedback source refs.
```

## 20. implementation ledger

This ledger reconciles the goal-state order with the live repository as of the
v0.4 cutover. It is not a second plan; it prevents the appendix from claiming
that already-landed substrate is still missing.

Landed:

```text
WorkspaceFactoryContext, WorkspaceSlot, path containment, command cwd scoping, and fingerprints in leaven-workspace.
StageEngineContext, ScopedRunGraphView, and StageAttemptRecorded in leaven-engine.
leaven-stage USER/ADAPTER/RECEIPT types, including AgentStagePlan, AgentBacked, StageReadAuthority, StageQueryPolicy, StageAttemptReceipt, setup_stage_workspace, output contracts, parser contracts, and receipt storage.
Prewarm and agent-requested query policy share one StageReadAuthority and receipt record shape.
AgentBacked<ProposerSlot<Req>, Runtime, Bootstrap, Parser> implements Proposer<P> and records mandatory receipts on success and post-allocation failures.
GEPA owns ReflectRequest, SelectedFeedback, LmBackedReflector, GepaReflector, and the agent-backed stage proposer bridge.
GEPA LM-backed and agent-backed reflection route through RunContext::propose and then RunContext::apply_batch.
The public AIME GEPA example exercises the LM-backed reflection path; FixedSurfaceEdit remains scaffold/proxy evidence only.
JJ artifact file-snapshot materialization has deterministic proof, but not full live jj apply semantics.
```

Still deferred:

```text
provider-native structured leaven_query beyond adapter-specific support
long-lived cross-stage agent sessions
generic type-erased parser registry
automatic hidden verifier packaging
universal Harbor/Inspect/SWE-bench task layout compilation
RetryWithFeedback parse reruns
full live jj command/apply implementation beyond file-snapshot materialization
file-read tracking beyond leaven_query calls and parser-declared files
```

## 21. deferred choices

Deferred:

```text
rich search query syntax
provider-native structured leaven_query beyond adapter-specific support
long-lived cross-stage agent sessions
generic type-erased parser registry
automatic hidden verifier packaging
universal Harbor/Inspect/SWE-bench task layout compilation
RetryWithFeedback parse reruns
complete jj implementation before first fake-runtime proof
file-read tracking beyond leaven_query calls
```

File-read tracking may become useful later, but v0.4 receipts record setup entries, query entries, outputs, parse files read, and agent session id. That is enough for the initial no-leak and reproducibility claims.

## 22. current state appendix

This appendix describes live repository state, not the pre-implementation
starting point. If this section disagrees with `Cargo.toml`, live code, tests,
or crate `AGENTS.md`, update it in the same change.

### 22.1 workspace substrate exists

`crates/leaven-workspace` already has the core substrate:

```text
Workspace
WorkspaceView
WorkspaceBackend
WorkspaceFactory
WorkspaceConfig
WorkspacePath
Command
with_workspace
```

`WorkspaceView` supports scoped subdirectories, file read/write/list,
executable bits, command execution, and optional local mounts.
`Workspace::cleanup(self)` is explicit.

Also landed for v0.4:

```text
WorkspaceSlot
WorkspaceFactoryContext
fingerprint helpers
slot containment tests
command cwd scoping tests
```

### 22.2 provider-neutral agent runtime exists

`crates/leaven-agent` already has:

```text
AgentRuntime
AgentRunRequest
AgentRunContext
AgentSession
AgentInstructions
AgentContextRef
OutputContract
AgentToolPolicy
AgentLimits
FakeAgentRuntime
```

`FakeAgentRuntime` can write files, read files, run commands, emit messages, and validate output contracts. This is enough for a deterministic first stage proof.

Landed for v0.4:

```text
translation from StageOutputContract to AgentOutputContract
runtime sessions tied to stage receipts
fake-runtime contract tests for agent-backed stage proof
shell help shim installation for leaven_query
```

Structured provider-native `leaven_query` exposure remains deferred.

### 22.3 task/workload format exists and should be kept

`crates/leaven-agentic` already exports:

```text
AgentCase
CaseSuite
CasePartitions
AgentWorkload
AgentCaseEvaluator
AgentCasePresenter
AgentCaseScorer
AgentCaseRunRecord
```

`AgentCase` includes input, target, metadata, files, setup, and workspace requirement. `CaseTarget::Hidden` is scorer-visible/candidate-hidden in intent. `CaseSuite` fingerprints cases and partitions. `AgentCaseEvaluator` allocates a workspace, calls a presenter, runs an `AgentRuntime`, scores the session, records `AgentCaseRunRecord` metadata, and cleans up.

This is the A-shaped layer. Preserve it.

Current status:

```text
crate AGENTS and this spec classify AgentCase as candidate-evaluation workload, not optimizer-stage workspace
hidden-target/presentation contract tests exist for the current presenter surface
AgenticProposer remains a legacy adapter pattern, not the optimizer-stage route
builder ergonomics for AgentWorkload are outside this v0.4 stage cutover
```

### 22.4 legacy agentic proposer adapters exist

`leaven-agentic::AgenticProposer` and `RepairingAgenticProposer` allocate a workspace, materialize input via `Materializer<P, Input>`, render instructions via `Renderer`, run an `AgentRuntime`, parse proposals, and clean up.

They are useful implementation precedent, but not the goal-state abstraction.
Do not extend them to satisfy optimizer-stage work.

Limitations relative to goal state:

```text
materializer and renderer are per-input composition, not stage-plan setup
legacy adapters are not the default stage workspace layout
legacy adapters do not own StageAttemptReceipt
legacy adapters do not own leaven_query
legacy adapters do not own typed StageRole / StageOutputContract distinction
legacy adapters do not own StageReadAuthority
parser belongs to proposal parser over AgenticRunInput, not generic stage output parsing
```

### 22.5 engine context owns finalization

`leaven-engine` already has:

```text
Renderer<P, T, Target>
Materializer<P, T>
MaterializeContext
RenderContext
ProposalContext
RunContext::propose
RunContext::record_proposal_batch
RunContext::apply_batch
ReadScope
TrustPolicy
StageEngineContext
ScopedRunGraphView
StageAttemptRecorded
```

`RunContext::propose` wraps a `Proposer<P>` call, drains receipt-backed
`StageAttemptRecorded` events, and records the returned `ProposalBatch<P>`.
`RunContext::apply_batch` remains the candidate-creation finalizer.

Current hazards:

```text
RunContext::record_proposal_batch remains public for existing low-level tests and scaffold paths; ordinary GEPA agent reflection must use RunContext::propose.
ProposalContext::graph() is still a raw proposer context hole; optimizer-stage agent work must cross through StageEngineContext and StageReadAuthority.
```

### 22.6 GEPA reflection has a production stage path

`crates/leaven-gepa` currently has:

```text
SurfaceProposer<A, S>::propose_edit(&artifact, &surface, &part)
FixedSurfaceEdit<E> fixed-edit scaffold
ReflectRequest
SelectedFeedback
LmBackedReflector
GepaReflector
GepaStageProposer
Gepa::propose_candidate(...)
```

`Gepa::propose_candidate` selects a parent and part, then calls
`GepaReflector`. `FixedSurfaceEdit` still records/applies a fixed edit as
scaffold. LM-backed and agent-backed `GepaReflector` implementations build a
`ReflectRequest` with `SelectedFeedback`, call `RunContext::propose`, then call
`RunContext::apply_batch`.

Current proof anchors:

```text
cargo nextest run -p leaven-gepa --test lm_reflection
cargo nextest run -p leaven-gepa --test gepa_contract
cargo test -p p8_aime_gepa
```

Proxy-only proof:

```text
FixedSurfaceEdit
topology-only checks
fake runtime without RunContext::propose/apply_batch
tests that parse hardcoded proposal bytes but do not apply a candidate
```

### 22.7 jj artifact crate is a file-snapshot vocabulary

`crates/leaven-artifact-jj` currently exports `JjArtifact` and `JjChange`.
`JjArtifact` materializes a file map into a workspace slot, derives
content/cache identity from that map, and reads back
`.leaven/jj/change.patch` as `JjChange::Patch`.

The old placeholder names (`JjArtifactIdentityMode`, `JjOp`, conflict-region
types, and JJ surface markers) remain absent. This crate is still not full live
JJ command execution, operation-log handling, conflict parsing, surface
projection, or production apply semantics.

## 23. changelog

### v0.4

- Split the spec surface into `USER`, `ADAPTER`, and `RECEIPT` tiers.
- Replaced eager/lazy materialization policy with one `StageQueryPolicy` using `prewarm: Vec<StageQuery>`.
- Renamed stage-call records from `StageReceipt` to `StageAttemptReceipt`.
- Renamed materialization-family workspace types to workspace/query vocabulary: `WorkspaceEntry`, `WorkspaceEntryRole`, `EntrySource`, `StageReadAuthority`, `setup_stage_workspace`.
- Removed `StageLayout` from `AgentStagePlan`.
- Removed `MaterializationTarget`; summary-only query results now use `QueryEffect::ReturnedSummary`.
- Removed `ReadScopeDigest` as a separate semantic type; receipts store full `ReadScope` plus `read_scope_fingerprint`.
- Removed `AgentBackedPolicy::record_receipt`; receipts are always recorded.
- Removed `ParseFailurePolicy::RetryWithFeedback`; retry-with-feedback is deferred.
- Collapsed `AgentBacked<Slot, Runtime, Bootstrap, Parser, Out>` to `AgentBacked<Slot, Runtime, Bootstrap, Parser>` using `SlotMarker::Output<P>`.
- Collapsed candidate source variants; artifact-vs-summary is now a projection.
- Replaced broad `AccessMode` with `EntryAccess` and no `Execute` variant.
- Made `WorkspaceEntryRole` an open tag consistent with `StageRole` and `OutputRole`.
- Defined `WorkspaceFactoryContext` and `WorkspaceSlot::factory_context::<T>()`.
- Defined the engine-to-stage handoff as `StageEngineContext` carrying `ScopedRunGraphView`, not full `RunGraphView`.
- Pinned the v0.4 `StageQuery` set: help, list candidates, candidate, assessment, evidence, lineage, diff. Search remains deferred.
- Added explicit bootstrap, parser, slot marker, receipt store, query result, and event definitions.
- Added worked artifact examples for both change-style and reconstructible artifacts.
- Preserved the A/B/C boundary and the current-state appendix pattern.
