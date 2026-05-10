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
- `docs/specs/agentic_task_execution_substrate.md`
- `docs/specs/agentic_library_user_journey.md`
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
    .validation_cases(validation_cases)
    .test_cases(test_cases)
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
    .data_policy(GepaDataPolicy::default_generalization())
    .validation(FullValidation::new(PartitionId::from("VALIDATION")))
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

### 4.1 Existing And Planned Ownership

Keep the current crate ownership and add `leaven-eval` as the shared eval
substrate crate:

| Crate | Owns | Must not know |
| --- | --- | --- |
| `leaven-core` | artifact/problem/proposal/evaluation/evidence algebra | graph, engine, GEPA, surface, LLMs |
| `leaven-surface` | `EditSurface`, `Part`, surface errors/fingerprints | graph, GEPA, stores, workspaces |
| `leaven-engine` | `RunContext`, `RunGraph`, budget, cache, stage traits, engine loop | GEPA policy, concrete populations, concrete LLM SDKs |
| `leaven-eval` | eval protocols, optional case catalogs, train/validation/test split manifests, leakage policies, casewise result adapters, eval reports | graph mutation, GEPA rhythm, concrete agent/provider/workspace backends, environment execution |
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
leaven-eval
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

`leaven-eval` is a warm product-support crate, not cold core. Its initial
dependency direction should be:

```text
leaven-eval -> leaven-kernel
leaven-eval -> leaven-core
leaven-eval -> leaven-evidence
leaven-eval -> leaven-engine   # only for evaluator/helper adapters
```

It must not depend on GEPA, concrete LM/provider crates, concrete workspace
backends, DSRs, or agentic domain crates. Agentic and LM-program adapters may
depend on `leaven-eval` to lower their domain case suites into the common eval
suite shape.

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
data_policy.rs        GepaDataPolicy, split-use rules, leakage defaults
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
data_policy(...)
```

The builder must reject incomplete configurations before the run starts:

- no surface,
- no proposer and no reflection LM default path,
- validation policy references a missing partition,
- train/search partition is empty when the sampler needs cases,
- validation/test partitions overlap train unless the split policy explicitly
  permits overlap,
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
5. BatchSampler chooses a feedback minibatch from the train/search partition.
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

## 7. Evaluation Protocol Semantics

Evals are first-class library infrastructure. GEPA is one consumer of that
infrastructure; LM-program optimizers, future DSRs adapters, Harbor/AISI-like
agentic suites, pairwise optimizers, and non-GEPA optimizers must be able to
reuse the same split and report semantics.

Layering rule:

```text
Evaluation protocol  = what is measured, when, by whom, and how results count.
Dataset/case catalog = optional source of examples, tasks, prompts, fixtures, ids.
Environment          = optional execution substrate for an evaluator or agent.
```

These are related but not interchangeable.

- A scalar reward evaluator may have no dataset and no environment.
- A prompt benchmark has a dataset but may not need a workspace environment.
- An agentic SWE task has a dataset-like task catalog and an environment.
- A live web/human eval may have an environment and no fixed case catalog.
- A pairwise preference optimizer has an eval protocol even when the "cases"
  are candidate pairs produced online.

`leaven-eval` should live at the evaluation plan/report layer. It may provide
optional case-catalog helpers because train/validation/test split semantics are
reusable, but it must not become the environment crate or a second engine.
Workspaces stay in `leaven-workspace`; agent sessions stay in `leaven-agent`;
agentic task semantics stay in `leaven-agentic`.

For maintainability and `scatter.md`, keep the protocol concept local to
`leaven-eval` as a module/type, not as a separate crate:

```text
leaven-core    owns cold request algebra: EvaluationRequest, EvaluationSet.
leaven-engine  owns execution capability: Evaluator, registry, cache, RunContext.
leaven-eval    owns product eval plans/reports: split use, leakage, report axes.
domain crates  own domain cases/environments and lower into leaven-eval plans.
optimizers     own search rhythm and strategy state.
```

That means an `EvalProtocol`/`EvalPlan` is declarative data: request shape,
granularity, split-use policy, report axes, leakage summary, and optional case
catalog identity. It is not a trait that runs evaluations and it is not an
environment abstraction.

### 7.1 Common Eval Surface

`leaven-eval` should introduce a reusable eval surface over the cold
`EvaluationSet` algebra and engine evaluator trait:

```rust
pub struct EvalProtocol {
    pub id: EvalProtocolId,
    pub request_shape: EvalRequestShape,
    pub granularity: AssessmentGranularity,
    pub split_uses: SplitUsePolicy,
    pub report_axes: Vec<ScoreAxis>,
    pub metadata: MetadataBag,
}

pub struct EvalSuite<C = EvalCase> {
    pub protocol: EvalProtocol,
    pub catalog: Option<CaseCatalog<C>>,
    pub splits: Option<SplitManifest>,
    pub fingerprint: Fingerprint,
    pub metadata: MetadataBag,
}

pub struct CaseCatalog<C = EvalCase> {
    pub cases: BTreeMap<CaseId, C>,
    pub fingerprint: Fingerprint,
    pub metadata: MetadataBag,
}

pub struct SplitManifest {
    pub version: CaseSetVersion,
    pub roles: BTreeMap<PartitionId, SplitRole>,
    pub cases: BTreeMap<PartitionId, Vec<CaseId>>,
    pub policy: SplitPolicy,
}

pub enum SplitRole {
    Train,
    Validation,
    Test,
    Probe,
    Search,
    ReportOnly,
    Custom(String),
}

pub enum SplitPolicy {
    DisjointRequired,
    OverlapAllowed { reason: String },
}
```

The exact type names may change during implementation, but the product promise
must not:

- eval protocols are valid without an attached dataset or environment;
- case catalogs are reusable data sources, not the eval itself;
- environments are referenced by evaluator/domain config and represented in
  evidence, not owned by `leaven-eval`;
- train/search cases are the default feedback source for proposal and
  minibatch screening;
- validation/dev cases are for model selection, checkpoint selection, and
  generalization reporting;
- test cases are final-report-only by default and are not used for proposer
  feedback, candidate admission, or frontier selection;
- split membership is fingerprinted and versioned, so a run report can state
  exactly what data it optimized on and what data it held out;
- dynamic evaluation sets resolve through `RunContext` before evaluator calls,
  preserving the existing `ResolvedEvaluationSet` boundary.

### 7.2 Split Use Policy

GEPA should own a small `GepaDataPolicy` over `PartitionId`s. `leaven-eval`
maps user-facing `EvalSuite` splits into that policy, but `leaven-gepa` should
not need to know a concrete suite type to run.

Default GEPA split use:

| Split role | Proposer feedback | Batch sampler | Gate/admission | Population default | Final report |
| --- | --- | --- | --- | --- | --- |
| Train/Search | yes | yes | yes | yes | yes |
| Validation | no case content; scores only if policy exposes them | no | optional | no by default | yes |
| Test | no | no | no | no | yes, post-loop only |
| Probe | only through explicit `EvalHandle` permission | no by default | no by default | no by default | optional |

This preserves a hard leakage boundary:

```text
proposer-visible data <= train/search evidence + exposed aggregate summaries
selector-visible data <= population policy + exposed scores
evaluator-visible data <= requested resolved cases
test data <= final/reporting evaluators unless explicitly overridden
```

Any override that lets validation influence selection or exposes validation
scores to selectors must be explicit in run config and recorded in the run
summary. Any override that exposes test content or test traces to proposers is
not a GEPA product default.

### 7.3 Optimizer Fit Matrix

The shared eval layer must fit every optimizer surface already sketched, not
just GEPA:

| Optimizer surface | Needs from `leaven-eval` | Must remain optimizer-owned |
| --- | --- | --- |
| GEPA | train/search minibatches, validation/test policy, per-case reports | candidate/part selection, reflection, merge, GEPA data policy |
| MIPRO | train/eval trials, metric axes, bootstrap/eval protocol reports | surrogate model, acquisition, proposal search |
| TextGrad | feedback aggregation by case/part, reportable held-out evals | gradient/critique propagation and update rule |
| Trace-style optimizers | trace/evidence protocol and split-aware reports | trace capture strategy and credit assignment |
| Pairwise tournament | pairwise request protocol, selection/eval split reports | fitted preference model and pair selector |
| Agentic/Harbor/AISI | task split manifests, scorer-visible hidden targets, final test reports | workspace/runtime execution, task presentation, transcript parsing |
| Future DSRs/LM programs | typed case catalogs, closure/program evaluator adapters, train/val/test reports | DSRs program artifact semantics and module surfaces |

This keeps the layer honest: `leaven-eval` standardizes how evaluations are
declared, split, executed through evaluators, and reported. Optimizers still
own their search rhythm and strategy state.

### 7.4 LM Program Cases

For LM programs, including a future DSRs adapter, `leaven-eval` should provide
plain typed case vocabulary and closure/evaluator adapters:

```rust
pub struct LmCase<I = serde_json::Value, O = serde_json::Value> {
    pub id: CaseId,
    pub input: I,
    pub expected: Option<O>,
    pub metadata: MetadataBag,
}

pub struct CaseOutcome<E> {
    pub case: CaseId,
    pub score: ScalarEvidence,
    pub feedback: Option<E>,
    pub evidence: Vec<EvidenceRef>,
}
```

The LM-program adapter's job is:

```text
EvalSuite<LmCase> or EvalProtocol + candidate artifact + evaluator closure/program runner
-> Evaluator<P>
-> per-case Assessment<P> records with structured evidence
```

DSRs should plug in here as a domain adapter. It should not own the common
train/validation/test semantics and it should not force GEPA to know DSRs
program types.

### 7.5 Agentic Harbor/AISI-Like Cases

The current `leaven-agentic::CaseSuite` proves the shape: cases may contain
text/messages, hidden targets, workspace files, setup requirements, and
workspace capability requirements. That should remain the agentic domain
vocabulary, but the split/reporting semantics should converge with
`leaven-eval`.

Agentic adapters should lower:

```text
AgentCase/HarborTask/AISI-like task suite
-> EvalSuite<AgentCaseLike> for split/report protocol
-> AgenticEvaluator
-> casewise assessments + transcript/command/workspace evidence
```

Required semantics:

- case input files and prompts may be candidate-visible when the case role
  permits it;
- hidden targets, graders, reference outputs, and test traces are
  scorer-visible only;
- each case run records workspace allocation, setup, agent session, command
  outputs, parse/scoring outcomes, cleanup status, and cost;
- the evaluator chooses workspace isolation granularity, commonly one fresh
  workspace per candidate-case pair for mutable agent tasks;
- split policy decides which assessments can feed the frontier, not the
  evaluator implementation;
- Harbor/AISI-like suites can use the same post-loop test report surface as
  prompt/program suites.

`leaven-agentic` may keep rich domain case records. `leaven-eval` should own
only the shared eval-suite contract, split manifest, leakage policy, helper
adapters, and report summaries that are useful outside agentic tasks too.

`leaven-eval` must not own environment execution. An agentic evaluator may
record environment identity, workspace config fingerprints, setup results, and
cleanup status as evidence/report fields, but the actual environment lifecycle
belongs to `leaven-workspace` and `leaven-agentic`.

### 7.6 Eval Reports

The product result should include a graph-backed eval report, not just a GEPA
candidate summary:

```rust
pub struct EvalRunReport {
    pub suite_fingerprint: Fingerprint,
    pub split_manifest: SplitManifest,
    pub train: Option<SplitReport>,
    pub validation: Option<SplitReport>,
    pub test: Option<SplitReport>,
    pub policy: SplitUseSummary,
}
```

Reports must answer:

```text
what cases existed?
was there a case catalog at all?
which environment, if any, produced the assessment evidence?
which cases were used for proposal feedback?
which cases were used for selection/admission?
which cases were held out until final reporting?
which evaluator/scorer produced each assessment?
which candidate won under which split policy?
```

## 8. Modes

Leaven GEPA must feel native in the three upstream `optimize_anything` modes.

### 8.1 Single-Task Search

Shape:

```text
seed artifact + evaluator + no train/validation case set
```

Implementation:

- use `EvaluationSet::Unscoped`, or a singleton generated case set;
- default population should be `KeepBest` unless the user requests a frontier;
- default batch sampler returns the unscoped/single case every iteration;
- result still records proposal/evaluation/cost lineage normally.

### 8.2 Multi-Task Search

Shape:

```text
seed artifact + train cases + no holdout
```

Implementation:

- map provided cases to `PartitionId::TRAIN`;
- record an `EvalSuite`/split manifest when the product builder receives
  concrete cases rather than an already-registered case set;
- default population is `ParetoFrontier::by_case()` for GEPA;
- validation policy defaults to evaluating on the same train/search partition;
- trust policy does not invent hidden validation data.

### 8.3 Generalization

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
- optional test partition is evaluated after the optimization loop against the
  selected candidates/frontier and remains outside admission by default.

## 9. Surface And Proposal Contract

### 9.1 Surface Edit

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

### 9.2 GEPA Proposal

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

### 9.3 Merge Canonicalization

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

## 10. Reflection And ASI

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

## 11. Result Contract

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
    pub eval_report: Option<EvalRunReport>,
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

## 12. Off-The-Shelf Entry Points

### 12.1 Generic Leaven Entry

The preferred user entry remains:

```rust
leaven::optimize::<P>()
    .seed(...)
    .cases(...)
    .validation_cases(...)
    .test_cases(...)
    .evaluator(...)
    .using(Gepa::builder().surface(...).reflection_lm(...).build())
    .run()
    .await
```

This keeps GEPA as one optimizer under the engine.

### 12.2 Convenience `leaven_gepa::optimize`

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

### 12.3 Closure Evaluator Helper

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
`leaven-eval`.

## 13. Stop, Budget, Cache, And Checkpoint

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

## 14. Trust Requirements

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

## 15. Concrete Requirements

### 15.1 Functional Requirements

The first product-grade GEPA implementation must:

1. implement `Optimizer<P>` for `Gepa<P, S, Pop>`;
2. support seed insertion and seed baseline evaluation;
3. support train-only, unscoped, and train+validation runs;
4. support explicit test partitions as final-report-only by default;
5. record split manifests/fingerprints for product-builder case inputs;
6. select candidates through a swappable `CandidateSelector`;
7. select parts through a swappable `PartSelector`;
8. sample minibatches through a swappable `BatchSampler`;
9. propose surface edits through `ReflectiveMutation`;
10. lower surface edits into typed artifact changes;
11. record proposal batches with typed causal and informational provenance;
12. apply proposals through `RunContext`;
13. screen children on minibatches;
14. gate children through a swappable `Gate`;
15. validate/admit through a swappable `ValidationPolicy`;
16. update population explicitly;
17. return best candidate through population state;
18. expose a result summary with train/validation/test report slots;
19. respect budget and trust scopes.

### 15.2 Default Policies

Defaults:

```text
population:           ParetoFrontier::by_case().build()
candidate selector:   ParetoFrequencyWeighted
part selector:        RoundRobinPart
batch sampler:        EpochShuffled { minibatch_size: 3 }
gate:                 StrictImprovement
validation:           MinibatchThenValidation or FullValidation over search partition
data policy:          train/search for feedback, validation for optional selection/reporting, test for final report only
merge:                disabled
proposal count:       1
track best outputs:   true where evidence/output shape supports it
```

Single-task defaults may use `KeepBest` instead of `ParetoFrontier` when no
case axis exists.

### 15.3 Error Requirements

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
- split manifest references unknown cases;
- required train/search split is empty;
- disjoint split policy is violated;
- validation policy requests forbidden partition;
- test partition is requested for proposer feedback or admission under default policy;
- trust policy denies proposer read;
- budget exhausted before proposal/evaluation mutation.

Do not use `OptimizerError::Message` for known public failures once the error
shape is known.

## 16. Tests And Acceptance

Each test must name a claim and live at the lowest clean layer.

### 16.1 `leaven-gepa` Law/Example Tests

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
- data policy refuses test partition as proposer feedback/admission by default;
- private checkpoint state captures/restores RNG and sampler cursor.

### 16.2 Umbrella Scenario Tests

Required tests under `crates/leaven/tests/`:

- single-task GEPA optimizes a one-string artifact with `KeepBest`;
- multi-task GEPA improves a two-part artifact through `ParetoFrontier`;
- generalization GEPA hides validation from proposer and still reports
  validation evaluation;
- test cases run only in post-loop final reporting under default policy;
- split manifest fingerprint changes when case membership changes;
- rejected candidates remain visible in graph but absent from population;
- result best candidate matches population best;
- callback/event order includes proposal, apply, evaluation, population, and
  optimization end events.

### 16.3 Example Packages

Existing:

```text
examples/p3_gepa_parity
```

New product examples:

```text
examples/p8_gepa_prompt_optimizer
examples/p9_gepa_skill_surface_smoke
examples/p10_eval_suite_train_val_test
```

`p8` should be the minimal off-the-shelf prompt optimizer. `p9` should prove
GEPA over a folder/skill-like surface with mock LM/runtime only.

### 16.4 Verification Commands

During implementation:

```bash
cargo nextest run -p leaven-gepa
cargo nextest run -p leaven-eval
cargo nextest run -p leaven --test gepa_parity
cargo run -p p3_gepa_parity
```

Completion:

```bash
just check
```

## 17. Implementation Milestones

### Milestone 0: Shared Eval Suite Substrate

Goal: make train/validation/test semantics reusable before GEPA product
ergonomics depend on them.

Scope:

- scaffold `leaven-eval` with `EvalProtocol`/`EvalPlan`, `EvalSuite`,
  `CaseCatalog`, `SplitManifest`, `SplitRole`, `SplitPolicy`, and
  `EvalRunReport`;
- map product-builder `.cases`, `.validation_cases`, and `.test_cases` into
  stable partitions;
- add leakage-policy helpers that hide validation/test content from proposers
  and mark test final-report-only by default;
- provide one closure evaluator helper for typed LM-program-style cases;
- prove an eval protocol can run without a case catalog and without an
  environment;
- add one adapter path from `leaven-agentic::CaseSuite` into the shared split
  manifest without moving agentic case internals into `leaven-eval`.

Exit tests:

```bash
cargo nextest run -p leaven-eval
cargo nextest run -p leaven-agentic case_suite
cargo run -p p10_eval_suite_train_val_test
```

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
- builder exposes `.cases`, `.validation_cases`, `.test_cases`, and
  `.eval_suite`;
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
- test partition final-report-only by default;
- shared eval report summarizes train/validation/test outcomes;
- no concrete provider/network dependency.

Exit tests:

```bash
cargo run -p p9_gepa_skill_surface_smoke
cargo nextest run -p leaven-agentic-skill
```

## 18. Non-Goals

Do not do these in the GEPA surface work:

- Python GEPA API compatibility.
- A second engine loop inside `leaven-gepa`.
- `dict[str, str]` as the core candidate representation.
- GEPA-specific fields on `Artifact`.
- GEPA-specific hooks inside `leaven-engine`.
- Concrete OpenAI/Anthropic dependencies in `leaven-gepa`.
- Hidden validation leakage for convenience.
- Test-set feedback in the optimization loop by default.
- Moving rich agentic case/workspace semantics into `leaven-eval`.
- Compatibility aliases for old names.
- Public test holes.

## 19. Open Decisions

1. Whether the default product entrypoint lives only on `leaven::optimize` or
   also as `leaven_gepa::optimize`.
2. Whether `CandidateSelector` moves from `leaven-gepa` into
   `leaven-population` once non-GEPA optimizers reuse it.
3. Whether closure evaluator helpers live in `leaven-eval`, `leaven-std`, or
   `leaven-gepa`. This spec prefers `leaven-eval` once that crate exists.
4. Whether `GepaResult` is a concrete struct or a renderer over graph +
   optimizer state.
5. Whether single-task GEPA defaults to `KeepBest` or a degenerate
   single-axis `ParetoFrontier`.
6. Whether the crate is named `leaven-eval` or `leaven-evals`; this spec uses
   singular because the crate owns evaluation infrastructure, not a benchmark
   catalog.
7. Whether validation scores may influence default selection. The conservative
   default is report-only unless the user selects a validation-aware policy.

These are implementation-shaping decisions, not blockers for Milestone 0/A.

## 20. First Implementation Slice

The first coherent slice should be:

```text
Scaffold shared eval-suite semantics, then implement reusable deterministic
Gepa<P, S, Pop> as Optimizer<P>.
```

It should modify:

```text
crates/leaven-eval/src/lib.rs
crates/leaven-eval/src/protocol.rs
crates/leaven-eval/src/suite.rs
crates/leaven-eval/src/split.rs
crates/leaven-eval/src/report.rs
crates/leaven-eval/tests/split_policy.rs
crates/leaven-gepa/src/gepa.rs
crates/leaven-gepa/src/optimizer.rs
crates/leaven-gepa/src/data_policy.rs
crates/leaven-gepa/src/proposal.rs
crates/leaven-gepa/src/batch.rs
crates/leaven-gepa/src/validation.rs
crates/leaven-gepa/src/lib.rs
crates/leaven-gepa/tests/gepa_smoke.rs
crates/leaven/tests/eval_suite_surface.rs
crates/leaven/tests/gepa_parity.rs
examples/p10_eval_suite_train_val_test/src/main.rs
examples/p3_gepa_parity/src/main.rs
```

It should not touch:

```text
leaven-core evaluation algebra except for missing public constants/errors
leaven-engine graph internals except product-builder wiring if needed
concrete LM provider crates
DSRs
```

That slice makes Leaven, not DSRs, the optimizer library under test.
