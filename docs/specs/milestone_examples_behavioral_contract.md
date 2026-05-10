# Milestone Examples Behavioral Contract

Status: executable requirements spec for P0 through P4.
Date: 2026-05-07.

This spec defines the code-level behavior required before the milestone
examples under `examples/` count as real. It is subordinate to:

- `docs/specs/initial_library.md`
- `docs/specs/leaven_v0_2_1b_corrected_crate_topology_lib_rs.md`
- `docs/specs/guiding_principles.md`
- `docs/specs/p1_scalar_keep_best_verification_contract.md`

When this document disagrees with those specs, preserve the v0.2.2 topology:
`leaven-core` stays cold, `leaven-engine` owns graph/context/runtime behavior,
surfaces are explicit, and workspace population is `Materializer`, not
`WorkspaceRenderer`. Do not add compatibility paths.

## Goal

Make the milestone examples fully executable and falsifiable:

```text
P0: graph skeleton
P1: scalar keep-best
P2: pairwise tournament
P3: GEPA parity over an edit surface
P4: meta-harness lite over materialized workspace history
```

Each milestone must be both:

- a runnable example package (`cargo run -p pN_name`), and
- a library-contract proof backed by focused tests in the owning crates.

The examples are not demos that merely print something plausible. They are
acceptance tests for the public surface. If an example needs a primitive that
does not exist yet, implement the primitive in the crate that owns that
knowledge boundary.

## Global Non-Negotiables

- `RunContext` is the only public mutation path into `RunGraph`.
- `RunGraph` storage mutators remain crate-private.
- Graph query behavior is exposed through `RunGraphView` and view structs, not
  public storage maps.
- Every graph record gets its own typed ID; no raw UUID/string IDs in public
  APIs where a typed ID exists.
- `CandidateId` is graph-local. Equal artifact identity may appear under
  multiple candidates when reached by different proposals.
- Proposal causal lineage and informational provenance remain distinct:
  `CausalInputs` is parentage; `InfoRef` is influence.
- Assessment evidence is stored by reference through `EvidenceStore`; the graph
  stores `EvidenceRef`, metadata, target, evaluator, and request IDs.
- Every costful public context method charges budget or fails before mutation.
- Every fallible public boundary returns a typed error or records a typed
  `ErrorRecord` event; no stringly silent failure paths.
- `leaven-core` must not know graph, engine, store, surface, renderer,
  workspace, GEPA, LLM, agent, or backend crates.
- `lib.rs` and `prelude.rs` files stay import maps only.
- New behavior-bearing files get focused tests and enter the coverage gate.
- `just check` remains the completion gate.

## Ownership Map

| Fact / behavior | Owning crate | Must not know |
| --- | --- | --- |
| Universal IDs, finite floats, cost, metadata, fingerprints, time, error records | `leaven-kernel` | Artifacts, graph, stores, workspaces, optimizers |
| Artifact/evidence/proposal/evaluation/preference algebra | `leaven-core` | Graph storage, context, stage runtime, surfaces, stores |
| Graph storage, graph views, run context, budget ledger, events, stage traits, engine loop, evaluator registry | `leaven-engine` | GEPA-specific policy, concrete stores, concrete workspaces |
| Evidence storage capabilities | `leaven-store` | `RunGraph`, `Engine`, concrete backend state |
| Inline evidence store | `leaven-store-inline` | Graph internals |
| Scalar, pairwise, casewise, attribution evidence shapes | `leaven-evidence` | Graph/context/stage runtime |
| Stateless preferences | `leaven-preference` | Population state, graph mutation |
| Keep-best, tournament, Pareto population/frontier state | `leaven-population` | Engine mutation internals, evidence stores |
| Edit surfaces and part/address vocabulary | `leaven-surface` | GEPA, graph, stores, workspace backends |
| GEPA optimizer and GEPA policy values | `leaven-gepa` | Concrete LLM providers, concrete workspace backends |
| Value renderers and workspace materializers | `leaven-render` | Engine graph mutation |
| Workspace substrate, `WorkspacePath`, cleanup contract | `leaven-workspace` | Artifacts, graph, stores, optimizers |
| Example-only tiny artifacts/proposers/evaluators | `examples/p*/src/main.rs` | Reusable library behavior |

## Universal ID And Record Requirements

These IDs are load-bearing in all milestones:

- `RunId`: stamped on optimization start/end.
- `CandidateId`: one graph-local artifact state.
- `ProposalBatchId`: one proposer output batch.
- `ProposalId`: one proposal inside a batch.
- `ApplyAttemptId`: one attempt to apply a proposal.
- `EvaluationRequestId`: one request issued through `RunContext`.
- `ResolvedEvaluationSetId`: one frozen evaluation set resolution.
- `AssessmentId`: one graph assessment record.
- `PopulationId`: one population/frontier instance.
- `StageId`: cost/error/event attribution for proposer/evaluator/renderer/
  materializer/stopper/custom optimizer work.
- `ProposerId`, `EvaluatorId`, `RendererId`: stable stage names.

Run graph records must be append-only:

- `CandidateRecord<P>` stores `CandidateId`, artifact, origin, created time.
- `ProposalBatchRecord` stores `ProposalBatchId`, stage, proposal IDs,
  semantics, metadata, iteration, created time.
- `ProposalRecord<P>` stores `ProposalId`, batch ID, proposal payload,
  created time.
- `ApplyAttemptRecord` stores `ApplyAttemptId`, proposal ID, success/failure,
  created time.
- `EvaluationRequestRecord` stores original `EvaluationRequest`, resolved set,
  evaluator ID, created time.
- `AssessmentRecord` stores `AssessmentId`, request ID, evaluator ID, target,
  metadata, `EvidenceRef`, created time.
- `RunEvent` records observable state transitions in order.

Property test:

```text
for any generated valid sequence of seed insertions, proposal batch recordings,
proposal applications, and evaluation recordings:
  record counts never decrease
  existing record IDs keep pointing to the same facts
  existing lineage edges never change
  event order is append-only
```

## Universal Proposal Laws

Proposal validation is performed before candidate insertion. Invalid proposal
lineage records a failed apply attempt and no candidate.

| Effect | Causal input | Valid? | Requirement |
| --- | --- | --- | --- |
| `Create` | `None` | yes | Fresh authored artifact |
| `Create` | `NAry(xs)` | yes | Aggregate or synthesis with influence lineage |
| `Create` | `Single(_)` | no | A create cannot have a single causal parent |
| `Create` | `Pair(_, _)` | no | A create cannot be pair lineage |
| `Change { target }` | `Single(p)` | yes iff `target == p` | Ordinary mutation |
| `Change { target }` | `Pair(a, b)` | yes iff `target == a || target == b` | Merge canonicalizes onto one target |
| `Change { target }` | `NAry(xs)` | yes iff `xs.contains(target)` | Multi-parent canonicalized change |
| `Change { .. }` | `None` | no | Cannot change no artifact |

Properties:

- Applying a successful proposal twice creates only one candidate; the second
  attempt fails and records no duplicate candidate.
- Applying a failed proposal twice records failures but never inserts a
  candidate.
- `InfoRef::Candidate(x)` never creates a causal edge.
- Sibling proposals in one batch may have different causal inputs.

## Universal Event Order Requirements

Event subsequences must be stable enough for tests and callbacks to rely on.
Extra events may be inserted only if this spec and the tests are updated in the
same change.

Proposal/apply success:

```text
BudgetCharged
ProposalBatchProduced
ProposalRecorded...
ApplySucceeded...
```

Proposal/apply failure:

```text
BudgetCharged
ProposalBatchProduced
ProposalRecorded...
ApplyFailed
Error
```

Evaluation miss:

```text
EvaluationRequested
BudgetCharged
EvaluationCompleted(cache = Miss | Bypassed)
```

Evaluation cache hit:

```text
EvaluationRequested
EvaluationCompleted(cache = Hit, cost = 0)
```

Engine run:

```text
OptimizationStarted
IterationStarted
...
IterationEnded
OptimizationStopping(reason = OptimizerDone | BudgetExceeded | Error | ...)
OptimizationEnded
```

Population update:

```text
PopulationUpdated { population_id, events }
```

Population events are optimizer-driven: the engine records assessments; the
optimizer chooses which population observes them.

## Universal Evaluation Requirements

`EvaluationRequest` shape is semantically significant:

- `Independent` scoring of `[A, B]` is two independent assessments.
- `Pairwise(A, B)` is one comparison assessment.
- `Listwise([A, B, C])` is one ranking assessment.

`RunContext` must:

1. Resolve `EvaluationSet` before calling an evaluator.
2. Pass only `ResolvedEvaluationRequest` to the evaluator.
3. Record the original request and resolved set in the graph.
4. Apply the evaluator's `CachePolicy`.
5. Store returned evidence in `EvidenceStore`.
6. Record assessment metadata and `EvidenceRef` in the graph.
7. Emit evaluation events.

Cache keys must include:

- evaluator fingerprint,
- cache policy,
- resolved case-set version,
- resolved case IDs,
- candidate IDs in request order unless the evaluator/request explicitly
  declares unordered symmetry.

Default cache behavior is no-cache. Deterministic cache behavior must be
opt-in by evaluator policy.

## Universal Budget Requirements

- Stages return `Metered<T>` when they produce cost-bearing work.
- `RunContext` charges returned metered cost before recording the cost-bearing
  mutation that depends on that work.
- If budget charging fails, no proposal/evaluation graph mutation from that
  call is allowed after the failed charge.
- `BudgetCharged` events carry `StageId`, charged `Cost`, and remaining
  `BudgetSnapshot`.
- Budget overflow uses typed cost/budget errors. Saturation is allowed only for
  explicitly non-authoritative reporting.

## Universal Trust Requirements

Trust policy must be enforced by views, not only carried as metadata:

- Proposer-facing views cannot read hidden held-out/test assessments.
- Evaluator-facing views follow evaluator scope.
- Callback-facing views follow callback scope.
- Materialized workspaces must obey the same read rules as the actor causing
  materialization.
- A denied read or forbidden evaluation request returns `TrustViolation` and
  records an error event.

## P0: Graph Skeleton

### Purpose

P0 proves proposal recording, proposal application, graph-local candidate
identity, lineage, and event order without engine loop, evaluator, population,
or evidence store behavior.

### Required Example

Package:

```text
examples/p0_graph_skeleton
```

The example defines local test-domain types:

```rust
struct TextArtifact(String);

enum TextChange {
    Append(&'static str),
    Replace(String),
}

struct TextError;
```

`TextArtifact` implements:

```rust
impl leaven::Artifact for TextArtifact {
    type Change = TextChange;
    type ApplyError = TextError;

    fn identity(&self) -> leaven::ArtifactIdentity;
    fn apply_change(&self, change: &TextChange) -> Result<Self, TextError>;
}
```

The example flow is:

```text
create RunGraph<TestProblem>
create BudgetLedger
create RunContext
insert seed "a"
record Proposal::create("fresh")
apply create proposal
record Proposal::mutate(seed, Append("b"))
apply mutation proposal
query graph view
assert seed has mutation child
assert created candidate has no parents
assert event sequence is stable
print summary
```

### Behavioral Requirements

- Seed candidates have `CandidateOrigin::Seed`.
- Create proposals create candidates with no causal parents.
- Change proposals create candidates with one or more causal parents.
- Same content inserted through two proposals produces two `CandidateId`s.
- Graph view exposes candidate, artifact, parents, children, proposal batch,
  proposal that created candidate, and events.
- No example code accesses graph storage maps directly.

### Required Tests

Existing or new tests in `crates/leaven-engine/tests/graph_surface.rs` must
cover:

- create proposal has no causal parent,
- change proposal creates parent-child edge,
- invalid proposal provenance fails without candidate insertion,
- `informed_by` does not affect lineage,
- merge records pair lineage while applying to one target,
- duplicate apply is rejected,
- graph append-only property.

### Verification

```bash
cargo run -p p0_graph_skeleton
cargo nextest run -p leaven-engine --test graph_surface
```

## P1: Scalar Keep-Best

### Purpose

P1 proves `Optimizer + Engine + RunContext + RunGraph + EvidenceStore +
KeepBest` for a single-objective scalar problem.

### Required Library Types

`leaven-evidence`:

```rust
pub struct ScalarEvidence { /* finite score */ }

impl ScalarEvidence {
    pub fn new(score: f64) -> Result<Self, ScalarEvidenceError>;
    pub fn score(&self) -> f64;
}

impl leaven_core::Evidence for ScalarEvidence {}
```

Requirements:

- `new(NaN)` fails.
- `new(+infinity)` fails.
- `new(-infinity)` fails.
- Stored score comparisons cannot be poisoned by non-finite values.

`leaven-preference`:

```rust
pub struct HigherScoreIsBetter;
pub struct LowerScoreIsBetter;
```

Requirements:

- Higher score returns `Preference::LeftBetter`.
- Lower score returns `Preference::RightBetter`.
- Equal score returns `Preference::Equivalent`.

`leaven-population`:

```rust
pub struct KeepBest;

impl KeepBest {
    pub fn new() -> Self;
    pub fn id(&self) -> PopulationId;
    pub fn best(&self) -> Option<CandidateId>;
    pub fn best_score(&self) -> Option<f64>;
    pub fn best_assessment(&self) -> Option<AssessmentId>;
    pub fn observe(
        &mut self,
        candidate: CandidateId,
        assessment: AssessmentId,
        score: ScalarEvidence,
    ) -> Vec<PopulationEvent>;
}
```

Tie policy:

- First observation wins when population is empty.
- Higher score replaces current best.
- Lower score is ignored.
- Equal score is ignored; the earlier best remains best.

### Required Example

Package:

```text
examples/p1_keep_best
```

The example may keep these local:

- `TextArtifact`
- `TextChange`
- `TwoMutations`
- `TextLengthEvaluator`
- `ScalarKeepBestOptimizer`

The example flow is:

```text
seed "a"
optimizer step starts
proposer emits alternatives Append("b") and Append("aa")
RunContext charges proposal cost
RunContext records proposal batch
RunContext applies both proposals
evaluator independently scores both candidates by text length
evidence store stores ScalarEvidence by reference
graph records AssessmentId -> EvidenceRef
KeepBest observes both assessments
Engine returns candidate whose artifact is "aaa"
```

### Behavioral Requirements

- `TextLengthEvaluator` receives `ResolvedEvaluationRequest`, not
  `EvaluationRequest`.
- Evaluation request shape is `Independent`.
- Assessment granularity is `Aggregate`.
- Stored graph assessment target is independent candidate.
- `ctx.assessment_evidence(assessment_id)` returns the stored scalar evidence.
- `PopulationUpdated` events are emitted for observations.
- Best candidate after one iteration is the mutation with score `3.0`.

### Required Tests

`crates/leaven/tests/scalar_keep_best.rs` or equivalent must assert:

- end-to-end engine result best artifact is `"aaa"`,
- callback sees monotonic event order,
- graph can retrieve assessment metadata and evidence reference,
- `KeepBest` best matches returned `RunResult.best`.

Unit tests:

- `crates/leaven-evidence/tests/scalar.rs`
- `crates/leaven-preference/tests/scalar.rs`
- `crates/leaven-population/tests/keep_best.rs`

### Verification

```bash
cargo run -p p1_keep_best
cargo nextest run -p leaven --test scalar_keep_best
cargo nextest run -p leaven-evidence --test scalar
cargo nextest run -p leaven-preference --test scalar
cargo nextest run -p leaven-population --test keep_best
```

## P2: Pairwise Tournament

### Purpose

P2 proves pairwise evidence, pairwise evaluation requests, evaluator registry
dispatch, and a population that owns fitted preference state.

It must not fake pairwise comparison as scalar scoring.

### Required Evidence Types

`leaven-evidence/src/pairwise.rs` owns:

```rust
pub enum PairwiseJudgment {
    Left,
    Right,
    Tie,
}

pub struct PairwiseJudgmentEvidence {
    judgment: PairwiseJudgment,
    confidence: Option<FiniteF64>,
    rationale: Option<String>,
}
```

Effective public API:

```rust
impl PairwiseJudgmentEvidence {
    pub fn new(judgment: PairwiseJudgment) -> Self;
    pub fn with_confidence(
        judgment: PairwiseJudgment,
        confidence: FiniteF64,
    ) -> Self;
    pub fn with_rationale(
        judgment: PairwiseJudgment,
        rationale: impl Into<String>,
    ) -> Self;
    pub fn judgment(&self) -> PairwiseJudgment;
    pub fn confidence(&self) -> Option<FiniteF64>;
    pub fn rationale(&self) -> Option<&str>;
}

impl leaven_core::Evidence for PairwiseJudgmentEvidence {}
```

Requirements:

- Confidence is finite by construction.
- `Tie` is first-class and not represented as missing evidence.
- Rationale is human/debug context only; algorithms cannot require it.

### Required Evaluator Registry Surface

`leaven-kernel`:

```rust
impl EvaluatorId {
    pub const PAIRWISE_JUDGE: Self = Self::new_const("pairwise_judge");
}
```

`leaven-engine`:

```rust
pub trait DynEvaluator<P: OptimizationProblem>: Send + Sync {
    fn id(&self) -> EvaluatorId;
    fn fingerprint(&self) -> Fingerprint;
    fn cache_policy(&self, request: &ResolvedEvaluationRequest) -> CachePolicy;
    fn evaluate_boxed<'a>(
        &'a self,
        request: ResolvedEvaluationRequest,
        ctx: EvaluationContext<'a, P>,
    ) -> LocalBoxFuture<'a, Result<Metered<Vec<Assessment<P>>>, EvaluationError>>;
}
```

`EngineBuilder` accepts named evaluators:

```rust
pub fn evaluator<E>(self, evaluator: E) -> Self
where
    E: Evaluator<P> + 'static;
```

`RunContext` supports registry dispatch:

```rust
pub async fn evaluate(
    &mut self,
    evaluator: EvaluatorId,
    request: EvaluationRequest,
) -> Result<EvaluationReport, RunContextError>;
```

Requirements:

- `evaluate(id, request)` uses the same path as `evaluate_with`: resolution,
  cache policy, budget, evidence store, graph records, and events.
- Missing evaluator ID returns a typed error and records `RunEvent::Error`.
- `evaluate_with` remains valid for stage-owned static evaluators.

### Required Tournament Types

`leaven-population/src/tournament.rs` owns:

```rust
pub struct BradleyTerryFit {
    learning_rate: FiniteF64,
    abilities: BTreeMap<CandidateId, FiniteF64>,
}

pub struct TournamentPopulation {
    id: PopulationId,
    fit: BradleyTerryFit,
    observations: usize,
}
```

Effective public API:

```rust
impl BradleyTerryFit {
    pub fn new(learning_rate: FiniteF64) -> Self;
    pub fn ability(&self, candidate: CandidateId) -> FiniteF64;
    pub fn observe_pairwise(
        &mut self,
        left: CandidateId,
        right: CandidateId,
        judgment: PairwiseJudgment,
    );
    pub fn best(&self) -> Option<CandidateId>;
}

impl TournamentPopulation {
    pub fn new(fit: BradleyTerryFit) -> Self;
    pub fn id(&self) -> PopulationId;
    pub fn observe_pairwise(
        &mut self,
        left: CandidateId,
        right: CandidateId,
        assessment: AssessmentId,
        evidence: PairwiseJudgmentEvidence,
    ) -> Vec<PopulationEvent>;
    pub fn best(&self) -> Option<CandidateId>;
}
```

Fitting requirements:

- The model starts candidates at ability `0.0`.
- `Left` increases left ability and decreases right ability.
- `Right` increases right ability and decreases left ability.
- `Tie` moves both abilities toward each other or leaves equal abilities equal.
- Updates are deterministic.
- Abilities remain finite.
- The fitted model is owned by `TournamentPopulation`, not by
  `PreferenceRelation`.

### Required Example

Package:

```text
examples/p2_pairwise_tournament
```

The example flow is:

```text
seed candidate A
create candidate B
register deterministic pairwise judge under EvaluatorId::PAIRWISE_JUDGE
issue EvaluationRequest::Pairwise { left: A, right: B, order: Ordered }
judge returns one Assessment::Pairwise with PairwiseJudgmentEvidence
evidence store stores pairwise evidence by reference
TournamentPopulation observes the assessment
population best is the judged winner
print winner, judgment, ability scores
```

### Behavioral Requirements

- `EvaluationRequest::Pairwise` produces exactly one assessment.
- Request order is preserved for `PairOrder::Ordered`.
- The cache does not pool `(A, B)` with `(B, A)` when order is ordered.
- `PairOrder::Unordered` may pool both orderings only if evaluator policy says
  it is deterministic and symmetric.
- `Assessment::Pairwise` target contains both candidate IDs.
- Tournament observation reads evidence from the evidence store or receives it
  explicitly from the optimizer; it does not pretend graph records contain
  evidence values.

### Required Tests

- `crates/leaven-evidence/tests/pairwise.rs`
- `crates/leaven-population/tests/tournament.rs`
- `crates/leaven-engine/tests/evaluator_registry.rs`
- example command below

Property tests:

- generated finite learning rates and judgments never produce non-finite
  abilities,
- reversing an ordered request changes the cache key,
- a sequence where candidate X always beats Y never ranks Y above X.

### Verification

```bash
cargo run -p p2_pairwise_tournament
cargo nextest run -p leaven-evidence --test pairwise
cargo nextest run -p leaven-population --test tournament
cargo nextest run -p leaven-engine --test evaluator_registry
```

## P3: GEPA Parity Over An Edit Surface

### Purpose

P3 proves GEPA is one optimizer built from swappable strategy values over an
explicit `EditSurface`. It must not bake GEPA concepts into the engine or the
artifact trait.

### Required Casewise Evidence

`leaven-evidence/src/casewise.rs` owns:

```rust
pub struct CaseOutcome<E: Evidence> {
    case: CaseId,
    evidence: E,
}

pub struct CasewiseEvidence<E: Evidence> {
    outcomes: Vec<CaseOutcome<E>>,
}
```

Effective public API:

```rust
impl<E: Evidence> CasewiseEvidence<E> {
    pub fn new(outcomes: Vec<CaseOutcome<E>>) -> Self;
    pub fn outcomes(&self) -> &[CaseOutcome<E>];
    pub fn get(&self, case: CaseId) -> Option<&E>;
}

impl<E: Evidence> Evidence for CasewiseEvidence<E> {}
```

Requirements:

- Sparse case coverage is allowed.
- Missing case evidence is represented by absence, not zero.
- Duplicate case IDs are rejected or canonicalized deterministically; the
  chosen policy must be tested.

### Required Pareto Frontier

`leaven-population` owns Pareto state:

```rust
pub struct ParetoFrontier;
pub struct ParetoFrontierBuilder;
```

Minimum API:

```rust
impl ParetoFrontier {
    pub fn by_case() -> ParetoFrontierBuilder;
    pub fn observe_casewise_scalar(
        &mut self,
        candidate: CandidateId,
        assessment: AssessmentId,
        evidence: &CasewiseEvidence<ScalarEvidence>,
    ) -> Vec<PopulationEvent>;
    pub fn best(&self) -> Option<CandidateId>;
    pub fn contains(&self, candidate: CandidateId) -> bool;
}

impl ParetoFrontierBuilder {
    pub fn partition_filter(self, filter: impl Into<PartitionFilter>) -> Self;
    pub fn build(self) -> ParetoFrontier;
}
```

Requirements:

- A candidate that is strictly worse on every observed case is not admitted.
- A candidate that improves at least one case and regresses no observed case is
  admitted.
- If two candidates are incomparable, both remain in the frontier.
- Partition filters exclude observations before frontier update, not after.

### Required GEPA Module Shape

`leaven-gepa` should stop being a unit-struct skeleton and own real modules:

```text
crates/leaven-gepa/src/
  lib.rs
  optimizer.rs
  parent_selector.rs
  part_selector.rs
  proposer.rs
  acceptance.rs
  validation.rs
```

Minimum public shape:

```rust
pub struct Gepa<P, S, Pop>
where
    P: OptimizationProblem,
    S: EditSurface<P::Artifact>,
{
    surface: S,
    population: Pop,
    // parent-selector/proposer/acceptance policy fields
}

pub trait ParentSelector<P: OptimizationProblem, Pop> {
    type Selection;
    fn select(&mut self, population: &Pop, graph: RunGraphView<'_, P>) -> Self::Selection;
}

pub trait PartSelector<A: Artifact, S: EditSurface<A>> {
    fn select_part(&mut self, artifact: &A, surface: &S) -> Result<S::PartId, SurfaceError>;
}
```

Requirements:

- `Population` and `ParentSelector` are separate.
- GEPA owns the chosen `EditSurface`.
- GEPA proposers may emit surface-native edits.
- GEPA lowers surface edits through `S::change_part(...)` before recording
  `ProposalEffect::Change`.
- The engine does not know part selectors, GEPA acceptance policies, reflective mutation, or
  Pareto-frequency weighting.
- P3 uses deterministic fake reflection; no LLM/provider dependency is allowed.

### Required Example

Package:

```text
examples/p3_gepa_parity
```

The example defines local:

```rust
struct PartMapArtifact(BTreeMap<String, String>);
struct PartMapSurface;
enum PartMapEdit { Replace(String) }
```

The example flow is:

```text
seed artifact with multiple parts
GEPA selects candidate
GEPA selects one part through PartMapSurface
deterministic reflective proposer creates surface edit
GEPA lowers surface edit to artifact-native change
RunContext records and applies proposal
evaluator returns CasewiseEvidence<ScalarEvidence>
ParetoFrontier observes casewise evidence
StrictImprovement gate accepts candidate
best candidate reflects improved part
```

### Behavioral Requirements

- `AssessmentGranularity::PerCase` is requested.
- Per-case evidence is not collapsed into aggregate score before frontier
  update.
- Surface fingerprint participates in cache identity where surface-derived
  evaluation/rendering is cached.
- Validation/test partitions are hidden from proposer-facing views.
- Protected validation cases are not used to generate reflective mutation.
- `PartMapSurface::change_part` is pure: it returns a change without mutating
  the artifact.

### Required Tests

- `crates/leaven-evidence/tests/casewise.rs`
- `crates/leaven-population/tests/pareto_frontier.rs`
- `crates/leaven-gepa/tests/gepa_smoke.rs`
- example command below

Property tests:

- surface lowering followed by artifact apply changes only the selected part,
- frontier admission is order-independent for the same set of observations,
- hidden partitions never appear in proposer-visible graph views.

### Verification

```bash
cargo run -p p3_gepa_parity
cargo nextest run -p leaven-evidence --test casewise
cargo nextest run -p leaven-population --test pareto_frontier
cargo nextest run -p leaven-gepa --test gepa_smoke
```

## P4: Meta-Harness Lite

### Purpose

P4 proves side-effectful workspace materialization, backend-neutral paths,
explicit cleanup, agentic fresh authoring, and large evidence stored by
reference.

### Required Hard Cutover

The public name is `Materializer`, not `WorkspaceRenderer`.

Remove:

```rust
WorkspaceRenderer
ArtifactWorkspaceRenderer
HistoryWorkspaceRenderer
SurfaceWorkspaceRenderer
```

Replace with:

```rust
Materializer
ArtifactMaterializer
HistoryMaterializer
SurfaceMaterializer
```

No compatibility aliases.

### Required Workspace Types

`leaven-workspace` owns:

```rust
pub struct WorkspacePath(/* backend-neutral relative path */);
pub struct Workspace;
pub trait WorkspaceFactory;
pub trait WorkspaceBackend;
```

`WorkspacePath` requirements:

- Accepts relative UTF-8 workspace paths.
- Rejects empty paths unless explicitly representing workspace root.
- Rejects absolute host paths.
- Rejects parent traversal (`..`) that escapes the workspace.
- Uses `/` as public separator.
- Does not expose host `PathBuf` as the public address type.

`Workspace` requirements:

- `cleanup(self)` is explicit and async.
- Drop may mark abandoned local resources but cannot be the authoritative
  cleanup path.
- `local_mount()` is optional and must not be required by examples.

### Required Materializer Trait

`leaven-engine` or `leaven-render`, depending on final crate placement, owns:

```rust
pub trait Materializer<P: OptimizationProblem, T>: Send + Sync {
    async fn materialize(
        &self,
        value: &T,
        workspace: &mut WorkspaceView<'_>,
        ctx: RenderContext<'_, P>,
    ) -> Result<Metered<MaterializeReport>, RenderError>;
}

pub struct MaterializeReport {
    pub files_written: usize,
    pub bytes_written: u64,
    pub truncations: Vec<TruncationNote>,
}
```

Requirements:

- Materialization charges cost through the render/materialize stage.
- Materializers receive actor-scoped graph views.
- Materializers write only through `WorkspacePath`.
- Materializer composition is by fields, not a global registry.

### Required Evidence Shape

If P4 needs reusable command/agent trajectory evidence, `leaven-evidence` owns:

```rust
pub struct CommandEvidence;
pub struct CommandRecord;
pub struct AgentTrajectoryEvidence;
```

Minimum behavior:

- command records include command, exit status, stdout/stderr refs or bounded
  inline snippets, and duration,
- large outputs are stored by reference, not copied into graph records,
- evidence remains opaque to `leaven-engine`.

### Required Example

Package:

```text
examples/p4_meta_harness_lite
```

The example flow is:

```text
create seed harness artifact
allocate local workspace
materialize artifact under WorkspacePath("artifact/")
materialize selected graph history under WorkspacePath("history/")
materialize recent evidence under WorkspacePath("evidence/")
deterministic fake agent reads workspace
agent proposer returns ProposalEffect::Create with CausalInputs::None
RunContext records and applies fresh candidate
repo-task evaluator runs in isolated workspace
evidence store stores command/trajectory evidence by reference
optimizer updates population/frontier
workspace cleanup is awaited
best candidate is returned
```

### Behavioral Requirements

- Fresh agent-authored artifacts use `ProposalEffect::Create`.
- Fresh authoring without causal predecessor uses `CausalInputs::None`.
- Historical influence is represented with `InfoRef`, not fake causal parents.
- Workspace paths in the example are backend-neutral.
- Cleanup is explicit and verified.
- Materialized held-out/test evidence follows trust policy and cannot leak to
  proposer/agent when hidden.
- Evaluation output may be large; graph stores references, not blobs.

### Required Tests

- `crates/leaven-workspace/tests/workspace_path.rs`
- `crates/leaven-engine/tests/materializer_contract.rs`
- example command below

Property tests:

- generated workspace paths never escape root,
- materializer writes are deterministic for the same graph view and inputs,
- cleanup is called exactly once on successful example path,
- `Create + None + informed_by(history)` creates no causal parent edges.

### Verification

```bash
cargo run -p p4_meta_harness_lite
cargo nextest run -p leaven-workspace --test workspace_path
cargo nextest run -p leaven-engine --test materializer_contract
```

## Milestone Gate

Add and maintain these commands:

```bash
just milestone-p0
just milestone-p1
just milestone-p2
just milestone-p3
just milestone-p4
just milestone-p5
just milestone-examples
```

`just milestone-examples` runs all milestone package binaries in order,
including the P5 paper-reproduction pressure test. It is a behavior gate, not a
replacement for tests.

Completion gate:

```bash
just check
```

## Definition Of Done

The milestone examples work is complete when all of these are true:

- `examples/p0_graph_skeleton` performs real graph mutations and lineage
  assertions.
- `examples/p1_keep_best` performs the full scalar keep-best loop.
- `examples/p2_pairwise_tournament` performs real pairwise evaluation and
  fitted tournament population update.
- `examples/p3_gepa_parity` performs a GEPA step through an explicit
  `EditSurface` and casewise frontier update.
- `examples/p4_meta_harness_lite` performs materialized workspace agentic
  fresh authoring with explicit cleanup.
- Every new public type has a contract test or example-backed scenario test.
- Every behavior-bearing crate/file is covered by the coverage gate.
- The docs name the runnable milestone commands.
- `just milestone-examples`, `just test`, and `just check` pass.

## Deferral Rule

If a requirement in this spec proves wrong while implementing, update this
spec in the same change as the code. Do not leave behavior implicit in tests,
comments, examples, or implementation details.
