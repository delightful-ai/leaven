# Leaven GEPA Optimizer Surface

Status: planning spec.
Date: 2026-05-09.

This spec defines the product-grade GEPA surface Leaven should expose next.
It is subordinate to:

- `docs/specs/initial_library.md`
- `docs/specs/guiding_principles.md`
- `docs/specs/leaven_v0_2_1b_corrected_crate_topology_lib_rs.md`
- `docs/specs/milestone_examples_behavioral_contract.md`
- `docs/specs/agentic_stage_runtime.md`
- `docs/specs/agentic_skill_optimization_primitives.md`
- `docs/testing/README.md`

When this document conflicts with those specs, preserve the current Leaven
architecture:

```text
Engine runs Optimizer.
Optimizer owns algorithm rhythm.
RunContext provides shared services.
RunGraph records truth.
Population owns live archive/frontier state.
CandidateSelector chooses what to try next.
EditSurface exposes chosen artifact parts.
GEPA remains one optimizer, not the engine.
```

## 1. Problem

Leaven currently has GEPA-shaped primitives and a P3 parity proof, but it does
not yet have an off-the-shelf optimizer library surface.

Today, a user can build a one-step GEPA-like flow by manually writing an
`Optimizer<P>` that:

1. evaluates a seed candidate,
2. updates a population,
3. selects a parent,
4. selects a surface part,
5. asks a proposer for an edit,
6. lowers the edit through `EditSurface`,
7. records and applies a proposal through `RunContext`,
8. evaluates the child,
9. gates it,
10. updates population state.

That proves the substrate. It is not the library product.

The next Leaven GEPA milestone is a reusable `leaven-gepa` optimizer that
owns that rhythm and exposes a short, typed entrypoint for ordinary users.

## 2. Upstream Comparator

Python GEPA is useful because its integration burden is small:

```text
candidate: dict[str, str] or str
adapter.evaluate(batch, candidate, capture_traces)
adapter.make_reflective_dataset(candidate, eval_batch, components)
optional adapter.propose_new_texts(...)
```

Its public library provides:

- `gepa.optimize(...)` over adapter-backed `dict[str, str]` candidates.
- `gepa.optimize_anything(...)` over `str`, `dict[str, str]`, or seedless
  candidates.
- single-task, multi-task, and generalization modes.
- ASI capture from evaluator returns, `oa.log`, and optional stdout/stderr.
- candidate selectors, component selectors, minibatch samplers, acceptance
  criteria, stop conditions, callbacks, run dirs, caching, result snapshots,
  Pareto frontier modes, optional merge, optional refiner, and parallel
  proposals.

Leaven should match the product usefulness, not the Python API shape.

Hard cutover rule:

```text
Do not implement Python GEPA adapter compatibility.
Do not flatten Leaven artifacts into dict[str, str] as the primary contract.
Do not introduce a generic string-map candidate layer below typed artifacts.
```

The Rust-native replacement for Python's candidate map is:

```text
P::Artifact                typed candidate state
S: EditSurface<P::Artifact> chosen optimizable projection
S::PartId                  named component/module/field/file/etc
S::View<'a>                borrowed part view
S::Edit                    surface-native replacement/edit
P::Artifact::Change        artifact-native change after lowering
```

## 3. Product Goal

The end-user path should look like this:

```rust
let result = leaven::optimize::<MyProblem>()
    .seed(seed_artifact)
    .cases(train_cases)
    .holdout(validation_cases)
    .evaluator(my_evaluator)
    .using(
        Gepa::builder()
            .surface(my_surface)
            .reflection_lm(reflection_lm)
            .population(ParetoFrontier::by_case().build())
            .build()
    )
    .budget(Budget::metric_calls(300))
    .run()
    .await?;

let best = result.best_candidate().expect("at least seed candidate");
```

The GEPA-customizer path should look like this:

```rust
let gepa = Gepa::builder()
    .surface(SkillDirByFrontmatterId)
    .candidate_selector(ParetoFrequencyWeighted::default())
    .part_selector(InvokedAndFailingPart::default())
    .batch_sampler(EpochShuffled::new(4))
    .proposer(ReflectiveMutation::with_lm(reflection_lm))
    .gate(StrictImprovement)
    .validation(FullValidation::new(PartitionId::from("TRAIN")))
    .population(
        ParetoFrontier::by_case()
            .partition_filter(BTreeSet::from([PartitionId::from("TRAIN")]))
            .build()
    )
    .merge(SystemAwareMerge::adaptive())
    .build();
```

The optimizer-author path stays unchanged:

```rust
impl Optimizer<MyProblem> for MyOptimizer {
    async fn step(
        &mut self,
        ctx: &mut RunContext<'_, MyProblem>,
    ) -> Result<StepStatus, OptimizerError> {
        // own the paper-specific rhythm
    }

    fn best_candidate(
        &self,
        graph: RunGraphView<'_, MyProblem>,
    ) -> Option<CandidateId> {
        self.population.best(graph)
    }
}
```

GEPA must be a reusable optimizer value. It must not require users to copy the
P3 example's local optimizer loop.

## 4. Crate Graph

### 4.1 Existing Ownership

Keep the current crate ownership:

| Crate | Owns | Must not know |
| --- | --- | --- |
| `leaven-core` | artifact/problem/proposal/evaluation/evidence algebra | graph, engine, GEPA, surface, LLMs |
| `leaven-surface` | `EditSurface`, `Part`, surface errors/fingerprints | graph, GEPA, stores, workspaces |
| `leaven-engine` | `RunContext`, `RunGraph`, budget, cache, stage traits, engine loop | GEPA policy, concrete populations, concrete LLM SDKs |
| `leaven-evidence` | scalar/casewise/attribution/pairwise evidence shapes | graph mutation, GEPA rhythm |
| `leaven-population` | `KeepBest`, `ParetoFrontier`, `TournamentPopulation`, population events | GEPA selectors, graph mutation internals |
| `leaven-render` | renderers/materializers over typed values | optimizer rhythm, GEPA policy |
| `leaven-lm` | provider-neutral LM request/response trait vocabulary | GEPA, engine graph |
| `leaven-lm-*` | concrete LM adapters | GEPA internals |
| `leaven-gepa` | GEPA optimizer, GEPA strategies, GEPA request/result types | concrete provider adapters, concrete workspace backends |
| `leaven` | umbrella import surface | implementation logic |

### 4.2 Required Dependency Direction

`leaven-gepa` may depend on:

```text
leaven-kernel
leaven-core
leaven-surface
leaven-engine
leaven-evidence
leaven-preference
leaven-population
leaven-render
leaven-lm
```

`leaven-gepa` must not depend on:

```text
leaven-lm-openai
leaven-lm-anthropic
leaven-lm-local
leaven-agent
leaven-agentic
leaven-workspace-*
leaven-artifact-*
leaven-dsrs
leaven-cuda
leaven-python
```

Domain adapters such as DSRs, CUDA, Python, skill banks, git artifacts, and
agentic workflows provide artifacts, surfaces, evaluators, renderers, and
optional convenience constructors. They do not own GEPA's rhythm.

### 4.3 Module Graph In `leaven-gepa`

The planned `leaven-gepa/src/lib.rs` map should be:

```text
batch.rs              BatchSampler, EpochShuffled, FixedMinibatch
candidate_selector.rs CandidateSelector, ParetoFrequencyWeighted, SelectBestCandidate, UniformFrontier, TopK
gate.rs               Gate, StrictImprovement, ImprovementOrEqual, NoRegression
gepa.rs               Gepa, GepaBuilder, GepaConfig, optimizer impl, private step state
merge.rs              MergeScheduler, SystemAwareMerge, GepaMerge
part_selector.rs      PartSelector, RoundRobinPart, InvokedAndFailingPart
proposal.rs           GepaMutationRequest, GepaProposal, SurfaceEdit
reflection.rs         ReflectiveMutation, reflection prompt construction, ASI rendering
result.rs             GepaResult, candidate summaries, frontier summaries
validation.rs         ValidationPolicy, FullValidation, MinibatchThenValidation
```

`lib.rs` remains a map only.

## 5. Core API Shape

### 5.1 `Gepa`

Canonical shape:

```rust
pub struct Gepa<P, S, Pop = ParetoFrontier>
where
    P: OptimizationProblem,
    S: EditSurface<P::Artifact>,
{
    surface: S,
    population: Pop,
    candidate_selector: Box<dyn CandidateSelector<P>>,
    part_selector: Box<dyn PartSelector<P, S>>,
    batch_sampler: Box<dyn BatchSampler<P>>,
    proposer: Box<dyn GepaProposer<P, S>>,
    gate: Box<dyn Gate<P>>,
    validation: Box<dyn ValidationPolicy<P>>,
    merge: Option<SystemAwareMerge<P, S>>,
    config: GepaConfig,
    state: GepaState,
}
```

The concrete code may keep some slots generic during implementation if object
safety or associated types force it. The public builder should hide that for
ordinary use.

`S` stays static. Its `PartId`, `Edit`, and `fingerprint` are part of
proposal typing, attribution typing, and surface-edit lowering.

### 5.2 Builder Requirements

`Gepa::builder()` must support:

```text
surface(S)
population(Pop)
candidate_selector(...)
part_selector(...)
batch_sampler(...)
proposer(...)
reflection_lm(...)
gate(...)
validation(...)
merge(...)
max_metric_calls(...)
max_iterations(...)
seed(u64)
proposal_count(usize)
track_best_outputs(bool)
track_candidate_history(bool)
```

The builder must reject incomplete configurations before the run starts:

- no surface,
- no proposer and no reflection LM default path,
- validation policy references a missing partition,
- batch sampler cannot draw from the configured search set,
- `skip_perfect_score` without a perfect-score definition,
- merge enabled without enough candidate lineage/support requirements.

### 5.3 Optimizer Implementation

`Gepa<P, S, Pop>` implements `leaven_engine::Optimizer<P>`.

It must use only `RunContext` for graph mutations:

```text
insert seed         -> RunContext::insert_seed, or engine builder pre-seeding
produce proposals   -> RunContext::record_proposal_batch / propose
apply proposals     -> RunContext::apply_batch / apply_proposal
evaluate candidates -> RunContext::evaluate / evaluate_with
emit population     -> RunContext::emit(PopulationUpdated)
charge budget       -> stage contexts or RunContext::charge
```

It must not mutate `RunGraph` storage directly.

## 6. GEPA Step Contract

One ordinary reflective mutation iteration is:

```text
1. Ensure seeds are inserted and population has observed seed baseline.
2. Build a population view from the current graph.
3. CandidateSelector chooses parent candidate(s).
4. PartSelector chooses one or more surface parts on the parent artifact.
5. BatchSampler chooses a feedback minibatch from the search partition.
6. GEPA evaluates the parent on the minibatch with per-case granularity.
7. GEPA extracts/captures feedback assessment IDs.
8. GepaProposer proposes one or more edits/native proposals.
9. GEPA lowers surface edits through EditSurface::change_part.
10. GEPA records a ProposalBatch with typed causal and informed_by provenance.
11. GEPA applies the batch through RunContext.
12. GEPA evaluates children on the same minibatch.
13. Gate decides which children deserve validation/admission.
14. ValidationPolicy chooses validation/search request for admitted children.
15. GEPA evaluates admitted children as required.
16. Population observes candidate/assessment IDs explicitly.
17. CandidateSelector observes selection outcome.
18. GEPA emits iteration events and either continues or returns Done.
```

Required invariants:

- per-case evidence reaches `ParetoFrontier` before any aggregate-only
  best-candidate decision;
- validation/test partitions can be evaluated but must not be visible to the
  proposer when trust policy hides them;
- proposer feedback uses `InfoRef::Assessment`, `InfoRef::Candidate`, or
  external refs, not stringly metadata keys;
- every accepted child has causal lineage through `CausalInputs`;
- every reflection/proposal also records what it read through `informed_by`;
- gate rejection does not erase graph truth about the proposal, apply attempt,
  or screening assessment;
- population events are opinions, not graph truth.

## 7. Modes

Leaven GEPA must feel native in the three upstream `optimize_anything` modes.

### 7.1 Single-Task Search

Shape:

```text
seed artifact + evaluator + no train/validation case set
```

Implementation:

- use `EvaluationSet::Unscoped`, or a singleton generated case set;
- default population should be `KeepBest` unless the user requests a frontier;
- default batch sampler returns the unscoped/single case every iteration;
- result still records proposal/evaluation/cost lineage normally.

### 7.2 Multi-Task Search

Shape:

```text
seed artifact + train cases + no holdout
```

Implementation:

- map provided cases to `PartitionId::TRAIN`;
- default population is `ParetoFrontier::by_case()` for GEPA;
- validation policy defaults to evaluating on the same train/search partition;
- trust policy does not invent hidden validation data.

### 7.3 Generalization

Shape:

```text
seed artifact + train cases + validation cases
```

Implementation:

- train partition feeds reflection/proposal/minibatch feedback;
- validation partition may feed reporting/final selection depending on policy;
- trust policy hides validation/test from proposer-facing views by default;
- population partition filter should default to search/train unless configured
  otherwise.

## 8. Surface And Proposal Contract

### 8.1 Surface Edit

```rust
pub struct SurfaceEdit<S, A>
where
    A: Artifact,
    S: EditSurface<A>,
{
    pub part: S::PartId,
    pub edit: S::Edit,
}
```

Surface edits are GEPA's default proposal payload. Artifact-native proposals
remain allowed for specialized proposers.

### 8.2 GEPA Proposal

```rust
pub enum GepaProposal<P, S>
where
    P: OptimizationProblem,
    S: EditSurface<P::Artifact>,
{
    SurfaceEdit {
        target: CandidateId,
        edit: SurfaceEdit<S, P::Artifact>,
        annotations: P::ProposalAnnotations,
        informed_by: Vec<InfoRef>,
    },
    Native(Proposal<P>),
}
```

GEPA lowers `SurfaceEdit` into `Proposal::mutate(...)` or
`Proposal::merge(...)` by calling:

```rust
surface.change_part(parent_artifact, part, edit)
```

### 8.3 Merge Canonicalization

GEPA merge reads two parent artifacts through the same surface. It picks one
parent as the apply target and lowers imported content into that target's
native `Change`.

Graph lineage records:

```text
effect: Change { target: left, change }
causal: Pair(left, right)
informed_by: [Candidate(left), Candidate(right), ...]
```

There is no magical two-artifact `apply_change`.

## 9. Reflection And ASI

Python GEPA's `make_reflective_dataset` becomes a renderer/proposer concern,
not a universal adapter contract.

Leaven needs a standard reflective mutation proposer:

```rust
pub struct ReflectiveMutation<Lm, R = DefaultReflectionRenderer> {
    lm: Lm,
    renderer: R,
    config: ReflectiveMutationConfig,
}
```

The renderer consumes:

```text
parent candidate id
selected surface part
surface part view
screening/minibatch assessment IDs
casewise evidence
optional attribution evidence for the selected part
lineage summary
objective/background prompt text
```

It produces `leaven_lm::Messages` or another provider-neutral LM input.

ASI sources:

- evaluator evidence fields,
- casewise scalar outcomes,
- attribution evidence keyed by `S::PartId`,
- command/stdout/stderr evidence when the evaluator records it,
- transcript refs from agentic evaluators,
- validation/apply errors,
- previous successful candidate summaries.

Do not add a global `oa.log` equivalent in `leaven-gepa`. Logging capture is
stage/evaluator evidence policy. A closure-based evaluator helper may provide
ergonomic stdout/log capture later, but that belongs as a helper over
`Evaluator<P>`, not as GEPA core behavior.

## 10. Result Contract

`GepaResult` should be a typed view over graph truth plus optimizer state, not
a second source of truth.

Minimum public shape:

```rust
pub struct GepaResult {
    pub best: Option<CandidateId>,
    pub seed: CandidateId,
    pub population_id: PopulationId,
    pub iterations: u64,
    pub total_cost: Cost,
    pub candidates: Vec<GepaCandidateSummary>,
    pub parents: Vec<GepaLineageSummary>,
    pub frontier: GepaFrontierSummary,
    pub rejected: Vec<GepaRejectionSummary>,
}
```

Convenience methods:

```rust
impl GepaResult {
    pub fn best_candidate<P>(&self, graph: RunGraphView<'_, P>) -> Option<CandidateId>
    where
        P: OptimizationProblem;

    pub fn best_artifact<'a, P>(&self, graph: RunGraphView<'a, P>) -> Option<&'a P::Artifact>
    where
        P: OptimizationProblem;

    pub fn candidate_tree_dot<P>(&self, graph: RunGraphView<'_, P>) -> String
    where
        P: OptimizationProblem;
}
```

The umbrella `leaven::optimize(...).run()` result remains the engine-level
run result. GEPA-specific summaries should be exposed through optimizer
reports or graph renderers without duplicating artifacts/evidence.

## 11. Off-The-Shelf Entry Points

### 11.1 Generic Leaven Entry

The preferred user entry remains:

```rust
leaven::optimize::<P>()
    .seed(...)
    .cases(...)
    .evaluator(...)
    .using(Gepa::builder().surface(...).reflection_lm(...).build())
    .run()
    .await
```

This keeps GEPA as one optimizer under the engine.

### 11.2 Convenience `leaven_gepa::optimize`

`leaven-gepa` may also provide a thin convenience wrapper:

```rust
pub fn optimize<P, S>(
    seed: P::Artifact,
    surface: S,
) -> GepaRunBuilder<P, S>
where
    P: OptimizationProblem,
    S: EditSurface<P::Artifact>;
```

This wrapper must lower to the same engine builder. It must not own a second
engine path.

### 11.3 Closure Evaluator Helper

A later ergonomic helper may adapt closures into `Evaluator<P>`:

```rust
gepa::closure_evaluator(|artifact, case| async move {
    Ok(CaseFeedback {
        score,
        side_info,
    })
})
```

This is a helper, not the core evaluator trait. It should live where the
closure helper belongs after trait bounds are clear, likely `leaven-std` or
`leaven-gepa::helpers`.

## 12. Stop, Budget, Cache, And Checkpoint

Required standard controls:

```text
max_iterations
max_metric_calls
max_lm_calls
max_cost
stop file callback through engine stop policy if already available
external stop callbacks through engine callback/stopper surface
```

Budget rules:

- evaluator calls charge through `RunContext::evaluate`;
- reflection LM calls charge as proposer cost;
- merge evaluation charges through the same evaluation path;
- proposal generation must fail before graph mutation if budget is exhausted.

Caching rules:

- evaluator cache remains engine-owned;
- cache keys use evaluator fingerprint, resolved evaluation set, request shape,
  and artifact cache identities;
- surface fingerprint participates only when rendering/evaluation actually
  depends on the surface projection.

Checkpoint rules:

- graph truth is enough to reconstruct public history;
- GEPA private state must implement `CheckpointableOptimizer` before
  long-running/resumable runs are considered product-ready;
- checkpointed private state includes RNG, batch sampler cursor, selector
  selection stats, merge scheduler state, and population state if not fully
  derivable from graph events.

## 13. Trust Requirements

Default GEPA generalization mode must hide holdout/test data from proposer
views.

Actor access:

| Actor | May read | Must not read |
| --- | --- | --- |
| Candidate selector | population view, scoped graph summary | hidden case content, evaluator internals |
| Part selector | parent artifact, visible evidence/attribution | hidden validation/test cases |
| Reflective proposer | selected part, visible feedback, visible lineage | hidden validation/test case content |
| Evaluator | requested candidate(s), resolved requested cases | proposer private state |
| Callback | event payload plus configured callback scope | data hidden from callback scope |

Trust enforcement must happen through `RunGraphView`/context scopes, not only
through convention.

## 14. Concrete Requirements

### 14.1 Functional Requirements

The first product-grade GEPA implementation must:

1. implement `Optimizer<P>` for `Gepa<P, S, Pop>`;
2. support seed insertion and seed baseline evaluation;
3. support train-only, unscoped, and train+validation runs;
4. select candidates through a swappable `CandidateSelector`;
5. select parts through a swappable `PartSelector`;
6. sample minibatches through a swappable `BatchSampler`;
7. propose surface edits through `ReflectiveMutation`;
8. lower surface edits into typed artifact changes;
9. record proposal batches with typed causal and informational provenance;
10. apply proposals through `RunContext`;
11. screen children on minibatches;
12. gate children through a swappable `Gate`;
13. validate/admit through a swappable `ValidationPolicy`;
14. update population explicitly;
15. return best candidate through population state;
16. expose a result summary;
17. respect budget and trust scopes.

### 14.2 Default Policies

Defaults:

```text
population:           ParetoFrontier::by_case().build()
candidate selector:   ParetoFrequencyWeighted
part selector:        RoundRobinPart
batch sampler:        EpochShuffled { minibatch_size: 3 }
gate:                 StrictImprovement
validation:           MinibatchThenValidation or FullValidation over search partition
merge:                disabled
proposal count:       1
track best outputs:   true where evidence/output shape supports it
```

Single-task defaults may use `KeepBest` instead of `ParetoFrontier` when no
case axis exists.

### 14.3 Error Requirements

GEPA must return typed errors for:

- no seed and no seedless generator configured;
- no candidate selected from population;
- selected candidate missing from graph;
- surface has no parts;
- part disappeared between selection and lowering;
- proposer output references unknown part;
- proposer output is invalid for the surface;
- evaluator does not support requested granularity;
- expected casewise evidence missing;
- gate cannot compare requested evidence shape;
- validation policy requests forbidden partition;
- trust policy denies proposer read;
- budget exhausted before proposal/evaluation mutation.

Do not use `OptimizerError::Message` for known public failures once the error
shape is known.

## 15. Tests And Acceptance

Each test must name a claim and live at the lowest clean layer.

### 15.1 `leaven-gepa` Law/Example Tests

Required tests:

- builder rejects missing surface/proposer/evaluator-required config;
- `RoundRobinPart` cycles deterministically over a stable surface;
- `ParetoFrequencyWeighted` samples only population/frontier members;
- surface edit lowering changes only the selected part for a generated
  `PartMapArtifact`;
- gate policies implement strict/equal/no-regression laws;
- batch sampler is deterministic under a seed;
- reflective proposer turns casewise feedback into a surface edit using
  `leaven-lm-mock`;
- invalid proposer output becomes typed proposal error, not panic;
- merge canonicalizes to one target while preserving pair causal lineage;
- private checkpoint state captures/restores RNG and sampler cursor.

### 15.2 Umbrella Scenario Tests

Required tests under `crates/leaven/tests/`:

- single-task GEPA optimizes a one-string artifact with `KeepBest`;
- multi-task GEPA improves a two-part artifact through `ParetoFrontier`;
- generalization GEPA hides validation from proposer and still reports
  validation evaluation;
- rejected candidates remain visible in graph but absent from population;
- result best candidate matches population best;
- callback/event order includes proposal, apply, evaluation, population, and
  optimization end events.

### 15.3 Example Packages

Existing:

```text
examples/p3_gepa_parity
```

New product examples:

```text
examples/p8_gepa_prompt_optimizer
examples/p9_gepa_skill_surface_smoke
```

`p8` should be the minimal off-the-shelf prompt optimizer. `p9` should prove
GEPA over a folder/skill-like surface with mock LM/runtime only.

### 15.4 Verification Commands

During implementation:

```bash
cargo nextest run -p leaven-gepa
cargo nextest run -p leaven --test gepa_parity
cargo run -p p3_gepa_parity
```

Completion:

```bash
just check
```

## 16. Implementation Milestones

### Milestone A: Real GEPA Loop, Deterministic Proposer

Goal: replace example-local GEPA rhythm with reusable `Gepa<P, S, Pop>`.

Scope:

- `Gepa` implements `Optimizer<P>`;
- deterministic proposer returns configured surface edit;
- builder supports surface/population/selector/part/gate/batch/validation;
- P3 example becomes thin setup code using `Gepa` directly.

Exit tests:

```bash
cargo nextest run -p leaven-gepa
cargo nextest run -p leaven --test gepa_parity
cargo run -p p3_gepa_parity
```

### Milestone B: Mock-LM Reflective Mutation

Goal: prove reflection loop without provider network calls.

Scope:

- `ReflectiveMutation` uses `leaven-lm` trait vocabulary;
- `leaven-lm-mock` drives deterministic proposal text;
- standard reflection renderer consumes casewise evidence and part view;
- typed parse/validation errors become proposer feedback.

Exit tests:

```bash
cargo nextest run -p leaven-gepa reflective_mutation
cargo run -p p8_gepa_prompt_optimizer
```

### Milestone C: Product Entry Builder

Goal: make the short user path work.

Scope:

- engine builder accepts seed/cases/evaluator/optimizer ergonomically;
- `leaven_gepa::optimize(seed, surface)` wrapper lowers to engine builder;
- single-task, multi-task, and generalization modes are explicit;
- result summary available.

Exit tests:

```bash
cargo nextest run -p leaven --test gepa_product_surface
```

### Milestone D: Merge, Cache, Checkpoint

Goal: parity with upstream's useful long-run features.

Scope:

- `SystemAwareMerge`;
- merge scheduler state;
- cache-aware full/minibatch evaluation path;
- `CheckpointableOptimizer` for GEPA private state.

Exit tests:

```bash
cargo nextest run -p leaven-gepa merge checkpoint
```

### Milestone E: Agentic/Skill Surface Proof

Goal: prove Leaven's typed surface beats Python's string-map adapter.

Scope:

- skill/folder artifact surface;
- mock agentic evaluator;
- trace attribution feeds `InvokedAndFailingPart`;
- validation partition hidden from proposer;
- no concrete provider/network dependency.

Exit tests:

```bash
cargo run -p p9_gepa_skill_surface_smoke
cargo nextest run -p leaven-agentic-skill
```

## 17. Non-Goals

Do not do these in the GEPA surface work:

- Python GEPA API compatibility.
- A second engine loop inside `leaven-gepa`.
- `dict[str, str]` as the core candidate representation.
- GEPA-specific fields on `Artifact`.
- GEPA-specific hooks inside `leaven-engine`.
- Concrete OpenAI/Anthropic dependencies in `leaven-gepa`.
- Hidden validation leakage for convenience.
- Compatibility aliases for old names.
- Public test holes.

## 18. Open Decisions

1. Whether the default product entrypoint lives only on `leaven::optimize` or
   also as `leaven_gepa::optimize`.
2. Whether `CandidateSelector` moves from `leaven-gepa` into
   `leaven-population` once non-GEPA optimizers reuse it.
3. Whether closure evaluator helpers live in `leaven-std` or `leaven-gepa`.
4. Whether `GepaResult` is a concrete struct or a renderer over graph +
   optimizer state.
5. Whether single-task GEPA defaults to `KeepBest` or a degenerate
   single-axis `ParetoFrontier`.

These are implementation-shaping decisions, not blockers for Milestone A.

## 19. First Implementation Slice

The first coherent slice should be:

```text
Implement reusable deterministic Gepa<P, S, Pop> as Optimizer<P>.
```

It should modify:

```text
crates/leaven-gepa/src/gepa.rs
crates/leaven-gepa/src/optimizer.rs
crates/leaven-gepa/src/proposal.rs
crates/leaven-gepa/src/batch.rs
crates/leaven-gepa/src/validation.rs
crates/leaven-gepa/src/lib.rs
crates/leaven-gepa/tests/gepa_smoke.rs
crates/leaven/tests/gepa_parity.rs
examples/p3_gepa_parity/src/main.rs
```

It should not touch:

```text
leaven-core
leaven-engine graph internals
concrete LM provider crates
DSRs
```

That slice makes Leaven, not DSRs, the optimizer library under test.
