# GEPA Public And Private Surface

Status: planning surface contract.
Date: 2026-05-10.

This spec defines the user-facing GEPA surface and the private/lowered contracts
that support it. It is the coordination document for the current GEPA/eval
specs:

- `docs/specs/gepa_optimizer_surface.md`
- `docs/specs/eval_lowering_detail.md`
- `docs/specs/eval_nomenclature.md`

It is subordinate to:

- `docs/specs/initial_library.md`
- `docs/specs/guiding_principles.md`
- `docs/specs/leaven_v0_2_1b_corrected_crate_topology_lib_rs.md`

## 1. Design Correction

The public GEPA story must not be:

```text
Build an evaluation spec, dataset splits, split usage rules, visibility policy,
actor scopes, request templates, evidence visibility, and a graph report.
```

That language is implementation-facing. It may describe real machinery, but it
is not how a user thinks when they want to run an optimizer.

The public GEPA story is:

```text
Give Leaven a candidate, training work, a scoring function, an optimizer, and a budget.
Optionally give it validation/test work and swap GEPA strategies.
```

The hard rule:

```text
Ordinary GEPA users should not learn actors, graph scopes, evaluation request
templates, split permissions, visibility policy, or run graph internals.
```

Those concepts still exist. They live behind the builder or in the optimizer
author layer.

## 2. The Three User Layers

### 2.1 Layer 1: Run GEPA

This layer is for users who want Leaven to optimize a program or artifact.

They should touch:

```text
seed / program
train cases or one unscoped task
optional validation cases
optional test cases
scoring function / evaluator
runner/executor when the artifact cannot be evaluated directly by the score
Gepa
budget
result / report
```

They should not touch:

```text
RunGraph
Actor
ReadScope
TrustPolicy
EvaluationRequest
ResolvedEvaluationRequest
SplitUse
Population
ParentSelector
PartSelector
EvidenceStore
```

Canonical shape:

```rust
let result = leaven::optimize(seed_program)
    .train(train_cases)
    .validation(dev_cases)
    .test(test_cases)
    .score(score_fn)
    .using(Gepa::default().with_reflection_lm(lm))
    .budget(Budget::metric_calls(300))
    .run()
    .await?;

let best = result.best().expect("seed candidate exists");
```

Single-task search should feel just as native:

```rust
let result = leaven::optimize(seed_kernel)
    .score(bench)
    .using(Gepa::default().single_task())
    .budget(Budget::wall_time(minutes(30)))
    .run()
    .await?;
```

Multi-task search should be train-only by default:

```rust
let result = leaven::optimize(seed_program)
    .train(tasks)
    .score(task_scorer)
    .using(Gepa::default().multi_task())
    .budget(Budget::metric_calls(500))
    .run()
    .await?;
```

Generalization should be train/validation/test by ordinary ML words:

```rust
let result = leaven::optimize(seed_prompt)
    .train(train)
    .validation(dev)
    .test(test)
    .score(metric)
    .using(Gepa::default().generalization())
    .budget(Budget::metric_calls(500))
    .run()
    .await?;
```

Run-then-score task suites should feel like an eval framework, not like engine
plumbing:

```rust
let result = leaven::optimize(seed_agent)
    .train(train_tasks)
    .validation(dev_tasks)
    .runner(agent_runner)
    .score(|ctx| async move {
        let target = ctx.case().and_then(|case| case.target());
        judge_agent_trace(ctx.trace(), ctx.output(), target).await
    })
    .using(Gepa::default().with_reflection_lm(lm))
    .budget(Budget::usd(50.0))
    .run()
    .await?;
```

Domain adapters may package `.runner(...)`, `.score(...)`, surfaces, and case
presentation defaults into one helper, but the public primitive remains
runner-plus-score.

### 2.2 Layer 2: Customize GEPA

This layer is for users who want GEPA, but not the default GEPA.

They should touch recognizable algorithm knobs:

```text
surface
parent selector
part selector
batch sampler
reflector / proposer
acceptance
population / frontier
validation cadence
merge
stopping
```

Example:

```rust
let gepa = Gepa::builder()
    .surface(SkillDirByFrontmatterId)
    .parent_selector(ParetoFrequencyWeighted::default())
    .part_selector(InvokedAndFailingPart::default())
    .batch_sampler(EpochShuffled::new(4))
    .reflector(ReflectiveMutation::with_lm(lm))
    .acceptance(StrictImprovement)
    .population(ParetoFrontier::by_case())
    .validation(FullValidation::every(10))
    .merge(SystemAwareMerge::adaptive())
    .build();
```

This layer may expose strategy traits. It should still not force users to build
engine trust/read scopes or evaluation request templates.

### 2.3 Layer 3: Author Optimizers

This layer is for users building their own optimizer.

They should touch the real machinery:

```rust
impl Optimizer<MyProblem> for MyOptimizer {
    async fn step(
        &mut self,
        ctx: &mut RunContext<'_, MyProblem>,
    ) -> Result<StepStatus, OptimizerError> {
        // choose candidates, build evaluation requests, observe evidence,
        // update private strategy state, and decide whether to continue
    }
}
```

At this layer, the following are legitimate public concepts:

```text
RunContext
RunGraphView
EvaluationRequest
EvaluationSet
Assessment
Evidence
Population
PreferenceRelation
BudgetHandle
TrustPolicy / ReadScope when building secure agentic flows
```

The design goal is not to hide power-user machinery. It is to keep it out of
the first mile.

## 3. Parent Selection Versus Part Selection

GEPA has two different selection questions.

```text
Parent selection: Which candidate should GEPA mutate next?
Part selection:   Where inside that candidate should GEPA edit?
```

Example:

```text
Candidate A: prompt { system, rubric, examples }
Candidate B: prompt { system, rubric, examples }
Candidate C: prompt { system, rubric, examples }
```

Parent selection chooses `A`, `B`, or `C`.

Part selection then chooses `system`, `rubric`, or `examples` inside the chosen
candidate.

They are orthogonal because good search policies can disagree:

- mutate the most promising parent, but edit the part most associated with
  failures;
- mutate a rarely explored frontier parent, but cycle parts round-robin;
- mutate the current best parent, but edit only the part touched by a failing
  trace;
- mutate two parents for merge, where part selection becomes a merge-region or
  conflict-region choice.

Public names should therefore be:

```text
parent_selector
part_selector
```

`candidate_selector` is acceptable internally, but "parent selector" is clearer
for GEPA users because it names the role of the selected candidate in the next
proposal.

## 4. Interactable GEPA Map

| GEPA aspect | User-visible API | Customizer API | Lowered/private contract | Owner |
| --- | --- | --- | --- | --- |
| Candidate/program | `optimize(seed)` / `.seed(seed)` | artifact type | `Artifact`, `CandidateId`, graph insertion | `leaven-core`, `leaven-engine`, domain crates |
| Editable view | default/derived surface | `.surface(surface)` | `EditSurface`, part ids, surface fingerprint | `leaven-surface`, artifact/domain crates |
| Parent choice | hidden default | `.parent_selector(...)` | selector reads population + graph view | `leaven-gepa` |
| Part choice | hidden default | `.part_selector(...)` | selector reads selected artifact through surface | `leaven-gepa`, `leaven-surface` |
| Training work | `.train(cases)` / `.cases(cases)` | sampler/filter policy | `CaseSet`, `EvaluationSet::Partition(TRAIN)` | `leaven-run`, `leaven-eval`, `leaven-engine` |
| Validation work | `.validation(cases)` | validation cadence/policy | held-out partition + run policy | `leaven-run`, `leaven-eval`, `leaven-gepa` |
| Test work | `.test(cases)` | final-test policy | final-report-only partition by default | `leaven-run`, `leaven-eval` |
| Candidate execution | `.runner(r)` / domain default | runner policy | evaluator adapter creates output/trace before score | `leaven-run`, domain crates |
| Scoring | `.score(fn)` / `.evaluator(e)` | evaluator id/registry | `Evaluator<P>`, assessments, evidence store | `leaven-run`, `leaven-engine` |
| Feedback/traces | `Score` feedback/attachments | reflector renderer | `Evidence`, `AttributableEvidence`, renderers | `leaven-evidence`, `leaven-render`, `leaven-gepa` |
| Reflection | `.with_reflection_lm(lm)` | `.reflector(...)` / `.proposer(...)` | proposer stage, LM request/response, rendered feedback | `leaven-gepa`, `leaven-lm`, `leaven-render` |
| Batch/minibatch | default by mode | `.batch_sampler(...)` | sampled evaluation requests | `leaven-gepa`, `leaven-eval` |
| Acceptance | hidden default | `.acceptance(...)` | acceptance/preference relation over evidence | `leaven-gepa`, `leaven-preference` |
| Population/frontier | default by mode | `.population(...)` | optimizer-owned archive/frontier state | `leaven-gepa`, `leaven-population` |
| Merge | off by default | `.merge(...)` | proposal effect with multi-parent provenance | `leaven-gepa`, `leaven-core` |
| Budget | `.budget(...)` | stopper/budget policy | `BudgetLedger`, stage charges | `leaven-kernel`, `leaven-engine` |
| Events | `.on_event(...)` | callbacks | `RunEvent`, scoped graph views | `leaven-run`, `leaven-engine` |
| Persistence | `.store(...)` / `.resume(...)` | checkpoint policy | run graph + optimizer state snapshots | `leaven-run`, `leaven-engine`, `leaven-store` |
| Report | `result.report()` | report options | graph-backed summaries, no copied truth | `leaven-run`, `leaven-eval`, `leaven-gepa` |

## 5. What We Need To Add

### 5.1 New `leaven-run` Crate

Add `crates/leaven-run` as the product-builder crate.

Why it exists:

```text
The public builder needs to compose engine + eval + store + optimizer values.
Putting that in leaven-engine makes engine depend on product policy.
Putting that in leaven makes the umbrella crate an implementation bucket.
Putting that in leaven-gepa makes ordinary run ergonomics GEPA-specific.
```

`leaven-run` owns:

```text
OptimizeBuilder
RunInput / RunDataset lowering
train/validation/test builder methods
runner adapters
scorer closure adapters when they can be expressed without domain loss
evaluator installation helpers
default inline evidence store selection
lowering from product inputs into Engine, CaseSet, TrustPolicy, and reports
RunOutput / Optimized result facade
```

`leaven-run` may depend on:

```text
leaven-kernel
leaven-core
leaven-engine
leaven-eval
leaven-store
leaven-store-inline
```

It may optionally depend on optimizer crates through features only for
convenience constructors. Its core builder must stay generic over
`O: Optimizer<P>`.

`leaven-run` must not own:

- optimizer strategy state;
- GEPA parent/part selectors;
- candidate execution or evaluator logic beyond adapter helpers;
- domain case semantics;
- workspace or agent runtime protocols.

The umbrella crate may re-export this as the public entrypoint:

```rust
pub use leaven_run::optimize;
```

The lower-level engine builder remains available through `leaven::engine` or
the `leaven-engine` crate directly.

### 5.2 `leaven-gepa`

`leaven-gepa` owns GEPA algorithm configuration and optimizer implementation:

```text
Gepa
GepaBuilder
parent selector traits and defaults
part selector traits and defaults
batch samplers
reflective mutation / proposer adapters
acceptance policies
GEPA validation cadence
GEPA merge scheduling
GEPA result summaries
```

`leaven-gepa` should not own train/validation/test builder methods. It should
consume the lowered run context produced by `leaven-run` and the engine.

### 5.3 `leaven-eval`

`leaven-eval` is not the public GEPA front door.

It owns lowered product data:

```text
Dataset
DatasetSplits
SplitRole
SplitUse
FinalTestPolicy
EvaluationReport
report summaries
fingerprints
```

It must not own:

- public `optimize(...).train(...).score(...)` builder verbs;
- `Evaluator<P>`;
- `RunContext`;
- graph mutation;
- actor/read-scope enforcement;
- GEPA strategy state;
- environments or workspace lifecycles.

Prefer names that make the lowered nature obvious:

```text
Dataset
DatasetSplits
SplitUse
FinalTestPolicy
EvaluationReport
```

Avoid first-class public names like `EvaluationSpec` and `VisibilityPolicy` in
the GEPA run path. If a lowered configuration object is still needed, prefer
`EvaluationPlan` and keep it behind builder/adaptor APIs.

### 5.4 `leaven-engine`

`leaven-engine` remains the execution substrate:

```text
Engine
EngineBuilder
RunContext
RunGraph
Evaluator
EvaluationRequest
CaseSet
TrustPolicy
BudgetLedger
callbacks
checkpointing
```

It must not depend on `leaven-eval`, `leaven-run`, or `leaven-gepa`.

### 5.5 Domain Adapter Crates

Domain crates own domain truth:

```text
leaven-dsrs       LM-program artifacts, module surfaces, DSRS evaluators
leaven-agentic    task-suite cases, hidden targets, workspace/session evidence
artifact crates   artifact-specific surfaces and helpers
```

They may provide convenience conversions into `leaven-run`/`leaven-eval`
inputs, but common crates must not learn domain internals.

## 6. Public Versus Private Contracts

### 6.1 Public Ordinary Contract

A type is ordinary-public only if a user running default GEPA plausibly needs to
name it.

Ordinary-public:

```text
Artifact or seed value
Gepa
Budget
scorer/evaluator
runner when not domain-derived
train/validation/test
RunOutput / Optimized result
```

Not ordinary-public:

```text
EvaluationRequest
ResolvedEvaluationRequest
TrustPolicy
ReadScope
Actor
SplitUse
Population
ParentSelector
PartSelector
EvidenceStore
```

Those can still be public Rust APIs. They are not the default story.

### 6.2 GEPA Customizer Contract

GEPA customizer traits must be small and swappable:

```text
ParentSelector
PartSelector
BatchSampler
Reflector / Proposer
Acceptance
MergeScheduler
ValidationPolicy
```

Each trait must correspond to one load-bearing choice in the GEPA loop.
Changing one must not require forking the engine or reimplementing GEPA.

Minimum strategy contracts:

| Slot | Input | Output | Must Not |
| --- | --- | --- | --- |
| `ParentSelector` | population state + scoped graph view + optional search state | parent candidate id(s) or typed "no parent" decision | mutate graph, run evaluators, inspect forbidden splits |
| `PartSelector` | selected artifact + surface + optional attributed evidence | surface part id(s) or typed surface/selection error | lower edits, mutate artifact, call LMs |
| `BatchSampler` | split/case view + sampling cursor + budget hint | nonempty case batch or typed "no cases" decision | bypass split policy, duplicate cases unless policy allows |
| `Reflector` / `Proposer` | parent, selected part, rendered score feedback, objective/background | surface edit(s) or native proposal(s) with causal inputs | apply proposals directly, write graph, hide parse errors as empty output |
| `Acceptance` | parent/child comparable score summaries + configured metric axes | accept/reject/defer decision with reason | update population, request hidden test evidence |
| `ValidationPolicy` | accepted candidate ids + validation cadence state | evaluation request intent or skip decision | execute evaluation, read test split under default policy |
| `Population` | candidate ids + assessment ids + graph view | updated private frontier/best state | own graph truth, persist evidence payload copies |
| `MergeScheduler` | frontier/lineage summaries + graph view | merge parent set and merge intent or skip decision | manufacture candidates without proposal provenance |
| `Stopper` | iteration, budget snapshot, optimizer state summary | continue/done reason | mutate graph or optimizer state |

Every slot may own private state. Every private state that affects future
decisions must either be derivable from graph truth or included in the optimizer
checkpoint schema.

### 6.3 Optimizer Author Contract

Optimizer authors keep the full substrate:

```text
Optimizer<P>
RunContext<'_, P>
EvaluationRequest
EvaluationSet
Assessment<P>
Evidence
Population
PreferenceRelation
Budget
RunEvent
```

This is where Leaven remains a power-user library.

Layer 3 contracts:

- `Optimizer<P>::initialize` runs once after seed insertion and before the first
  step. It may initialize private state and observe seed evidence if available.
- `Optimizer<P>::step` is the only optimizer loop hook. It mutates public run
  truth only through `RunContext`; it must not hold `RunContext` or graph views
  across calls.
- `StepStatus::Continue` means the engine may call `step` again.
  `StepStatus::Done` means no more optimizer work remains; final validation/test
  may still run through product policy.
- `best_candidate(graph)` is a pure read over optimizer state and graph view. It
  must not evaluate, mutate, or charge budget.
- `Evaluator<P>` receives a `ResolvedEvaluationRequest` plus
  `EvaluationContext`; it returns `Metered<Vec<Assessment<P>>>`. Returned
  assessment shape must match the request shape, and metered cost must include
  all evaluator-owned work.
- `Evaluator::fingerprint` and `cache_policy` are part of the cache contract. If
  scorer logic, runner logic, hidden targets, model judge prompt, or environment
  setup changes, the fingerprint must change.
- Known optimizer/evaluator failures must use typed errors at their capability
  boundary. Generic message variants are only for genuinely unclassified edges.

### 6.4 Private/Lowered Contract

The lowered run contract is allowed to be more precise than the public surface:

```text
builder train/validation/test inputs
  -> Dataset + DatasetSplits
  -> engine CaseSet
  -> TrustPolicy / ReadScope
  -> EvaluationRequest values
  -> graph assessments and evidence refs
  -> EvaluationReport + GEPA result summary
```

The user should usually see only the left and right ends of that chain.

## 7. Topology Invariants

The cohesive dependency direction is:

```text
leaven-core       -> leaven-kernel
leaven-surface    -> leaven-core, leaven-kernel
leaven-evidence   -> leaven-core, leaven-kernel
leaven-engine     -> leaven-core, leaven-kernel, leaven-store
leaven-eval       -> leaven-core, leaven-kernel, leaven-evidence
leaven-gepa       -> leaven-core, leaven-engine, leaven-eval,
                     leaven-evidence, leaven-population, leaven-preference,
                     leaven-render, leaven-surface, leaven-lm
leaven-run        -> leaven-core, leaven-engine, leaven-eval,
                     leaven-kernel, leaven-store, leaven-store-inline
domain adapters   -> leaven-core, leaven-surface, leaven-engine as needed,
                     leaven-eval/leaven-run for convenience adapters
leaven            -> re-exports only
```

Forbidden edges:

```text
leaven-core    -> leaven-eval / leaven-engine / leaven-gepa / leaven-run
leaven-engine  -> leaven-eval / leaven-gepa / leaven-run
leaven-eval    -> leaven-engine / leaven-gepa / leaven-run / domain crates
leaven-gepa    -> concrete LM providers / concrete workspace backends
leaven-run     -> concrete LM providers / concrete workspace backends / domain crates
leaven         -> implementation logic
```

This preserves the original topology:

- cold algebra stays below everything;
- engine executes but does not know product policy;
- eval product data is reusable and non-executing;
- GEPA owns GEPA rhythm and strategy slots;
- run builders compose products without polluting engine or umbrella crates.

## 8. Required Behavior

### 8.1 Builder Lowering

Layer 1 builder methods must form a small state machine. `run()` may start only
when all required public inputs are present and every contradiction has already
been rejected.

Public methods:

| Method | Contract |
| --- | --- |
| `optimize(seed)` | Inserts a seed candidate before optimizer initialization. Seedless search is not implied by `None`; expose an explicit seedless constructor only when a generator/objective contract exists. |
| `.train(cases)` / `.cases(cases)` | Installs the train/search work set. May be called once unless an explicit append method exists. Reject duplicate case ids. |
| `.validation(cases)` | Installs held-out validation work. Requires train cases in default GEPA mode. Reject duplicate case ids and default-disallowed overlap. |
| `.test(cases)` | Installs final-report work. Requires train cases in default GEPA mode. Test is final-report-only unless policy explicitly says otherwise. |
| `.runner(runner)` | Installs candidate execution before score judging. Optional when the scoring function is self-contained or a domain adapter supplies a runner. |
| `.score(fn)` | Installs the primary scorer adapter under the primary evaluator id. Mutually exclusive with a primary `.evaluator(...)`. |
| `.evaluator(e)` | Installs a typed engine evaluator. Ordinary path uses it as the primary evaluator; optimizer-author paths may install named auxiliary evaluators through the engine. |
| `.using(optimizer)` | Supplies the optimizer value. Required for `leaven-run` core builder; umbrella convenience constructors may prefill `Gepa::default()` but must make that visible in docs. |
| `.budget(budget)` | Supplies run limits. Product builders must require an explicit budget or explicit `Budget::unlimited()`; engine builders may keep `Budget::unlimited()` as their default. |
| `.store(store)` | Supplies durable blob/evidence/checkpoint storage. If omitted, the product builder may use inline storage only for non-resumable runs and must still stage attachments durably for the result lifetime. |
| `.resume(checkpoint)` | Restores graph truth plus optimizer private state. Requires matching optimizer/evaluator/score/runner fingerprints or fails before continuing. |
| `.on_event(callback)` | Registers public run events. Layer 1 callbacks receive summaries and ids, not mutable graph access. |
| `.run()` | Freezes builder inputs, lowers them once, initializes the optimizer, executes until stop/error/budget, optionally runs final validation/test, and returns `Optimized`. |

`leaven-run` must reject before execution:

- missing optimizer;
- missing scorer/evaluator;
- missing budget unless explicitly unlimited;
- validation/test without train in default GEPA mode;
- both primary scorer and primary evaluator installed;
- runner required by scorer/domain adapter but absent;
- case id duplication or default-disallowed split overlap;
- resume checkpoint fingerprint mismatch;
- store configuration that cannot persist declared attachments.

The public builder must lower:

```text
.train(cases)       -> TRAIN split, default in-loop feedback/use
.validation(cases)  -> VALIDATION split, held out from proposers by default
.test(cases)        -> TEST split, final-report-only by default
.runner(runner)     -> candidate execution adapter when needed
.score(fn)          -> evaluator adapter
.using(optimizer)   -> engine optimizer value
.budget(budget)     -> engine budget ledger
.on_event(callback) -> engine callback
.store(store)       -> engine/run persistence and evidence storage
```

It must not require the user to construct these directly in Layer 1:

```text
CaseSet
SplitUse
TrustPolicy
EvidenceStore
EvaluationRequest
```

### 8.2 Default Modes

Mode inference must be boring:

```text
no train/validation/test      -> single-task
train only                    -> multi-task/search
train + validation/test       -> generalization
explicit `.single_task()` etc -> overrides inference when needed
```

The user should not feel like they are abusing the API by choosing any of the
three original modes.

### 8.3 Default GEPA Policy

Default GEPA behavior:

```text
train/search evidence can drive proposer feedback, parent selection,
part selection, acceptance/admission, and population updates.

validation evidence can drive reports and explicit validation cadence.
It does not feed reflective proposers by default.

test evidence is final-report-only by default.
```

The public explanation should be "validation and test are held out", not
"actors cannot see split evidence".

### 8.4 Swappability

Every load-bearing GEPA decision remains a trait slot:

```text
which parent to mutate
which part to edit
which cases to evaluate
how to reflect/propose
what counts as acceptance
how population/frontier state updates
when to validate
when to merge
when to stop
```

The ordinary builder supplies defaults. It does not remove the slots.

## 9. Documentation Rules

When revising the companion specs:

1. Teach Layer 1 before internal types.
2. Put actor/trust/visibility language only in lowered/engine sections.
3. Use `parent_selector` in GEPA-facing docs and reserve
   `candidate_selector` for lower-level/general optimizer internals.
4. Describe train/validation/test by user intent first, then by partition and
   policy lowering.
5. Avoid using `EvaluationSpec` as a public front-door concept. If the lowered
   object exists, keep it behind product builders.
6. Keep examples short enough that a user can see how to run GEPA without
   learning the engine.

## 10. Scoring Contract

The ordinary public concept is a scoring function. It is not an evaluation spec,
split policy, graph request, or GEPA feedback hook.

Canonical shape:

```rust
let result = leaven::optimize(seed_program)
    .train(train_cases)
    .score(|ctx| async move {
        Score::new(0.82)
            .metric("exact_match", MetricValue::maximize(1.0))
            .metric("latency_ms", MetricValue::minimize(184.0))
            .feedback("The final answer is correct, but retrieval used the wrong source.")
            .attach_file("trace", ctx.trace_file())
            .attach_dir("workspace", ctx.workspace_dir())
            .metadata("judge", "gpt-5.2")
    })
    .using(Gepa::default())
    .budget(Budget::metric_calls(300))
    .run()
    .await?;
```

Scalar scores are lifted into `Score`:

```rust
.score(|ctx| ctx.output().passed() as i32 as f64)
```

Typed evaluators remain the power-user escape hatch:

```rust
.evaluator(my_typed_evaluator)
```

`ScoreContext` is the public trace/state object. It may expose:

```rust
pub struct ScoreContext<'a, P: OptimizationProblem, C = ()> {
    pub candidate: CandidateView<'a, P::Artifact>,
    pub case: Option<CaseView<'a, C>>,
    pub output: Option<OutputView<'a>>,
    pub run_error: Option<RunErrorView<'a>>,
    pub trace: TraceView<'a>,
    pub history: ScoreHistoryView<'a, P>,
}

pub struct CaseView<'a, C> {
    pub id: CaseId,
    pub input: &'a C,
    pub target: Option<TargetView<'a>>,
    pub split: Option<SplitRole>,
    pub metadata: &'a MetadataBag,
}
```

`candidate` is always present. `case` is `None` for single-task search or any
online evaluation that has no stable dataset case. `target` is optional because a
case is a unit of work, not necessarily a labeled example.

`output` is present when the builder/domain adapter runs the candidate before
calling the scoring function. It may be absent for scoring functions that own the
whole execution themselves, such as black-box benchmarks or external harnesses.
`run_error` is present only when a runner failed and the configured policy allows
score-on-error. `trace` may be empty, but it is always addressable so code does
not branch on whether tracing was enabled.

`history` is read-only and bounded by the configured score-history policy. It
may include previous scores for this candidate, this case, and historical best
evaluations. It must not expose mutable graph state.

`ScoreContext` must not expose `RunGraph`, `Actor`, `ReadScope`, `TrustPolicy`, or
evaluation request templates in Layer 1.

When `.runner(...)` is installed, it adapts to this candidate-execution
contract:

```rust
pub trait CandidateRunner<P: OptimizationProblem, C>: Send + Sync + 'static {
    fn run(
        &self,
        ctx: CandidateRunCtx<'_, P, C>,
    ) -> impl Future<Output = Result<CandidateRun, CandidateRunError>> + Send;
}

pub struct CandidateRunCtx<'a, P: OptimizationProblem, C> {
    pub candidate: CandidateView<'a, P::Artifact>,
    pub case: Option<CaseView<'a, C>>,
}

pub struct CandidateRun {
    pub output: Option<OutputValue>,
    pub trace: TraceBundle,
    pub attachments: Vec<ScoreAttachment>,
    pub cost: Cost,
}
```

Runner rules:

1. Runner output/trace becomes part of `ScoreContext`.
2. Runner cost is charged before score cost.
3. Runner failures are not scores. By default, they record execution error
   evidence and follow the failure policy without calling the scoring function.
4. A domain adapter may enable score-on-error. In that mode, the scoring function
   receives `run_error` plus any partial trace/output so it can return a real
   score for compiler errors, verifier failures, or agent crashes.
5. A self-contained scoring function may omit `.runner(...)`; then it owns
   execution and must attach any produced trace/evidence itself.

Every public scoring function is adapted to this canonical contract:

```rust
pub trait Scorer<P: OptimizationProblem, C>: Send + Sync + 'static {
    fn call(
        &self,
        ctx: ScoreContext<'_, P, C>,
    ) -> impl Future<Output = Result<Score, ScoreError>> + Send;
}
```

Builder overloads may accept simpler closures, but they all lower to that shape:

```text
Fn(ScoreContext<'_, P, C>) -> impl IntoScore
Fn(ScoreContext<'_, P, C>) -> Result<impl IntoScore, ScoreError>
async Fn(ScoreContext<'_, P, C>) -> impl IntoScore
async Fn(ScoreContext<'_, P, C>) -> Result<impl IntoScore, ScoreError>
```

Scalar convenience forms may exist only if they are unambiguous. They still
normalize as if the user had received a full `ScoreContext`.

`IntoScore` must support:

```text
Score                 rich score
FiniteF64 / f64        primary higher-is-better score after finite validation
bool                   1.0 for true, 0.0 for false
```

Do not support `Option<Score>` as a public return. Use `Score::unscored(...)`
for diagnostics without a comparable value, so absence is explicit.

Errors are not scores. A `ScoreError` means the scoring function failed to
produce an assessment. The adapter must record the failure as evaluation error
evidence and then follow the configured failure policy. It must not silently
turn score errors, panics, non-finite scores, missing attachments, or invalid
metric directions into score `0.0`.

`Score` carries:

```text
primary comparable score
named metrics with direction/role
natural-language feedback
structured feedback records
file, directory, image, transcript, log, JSON, and workspace attachments
metadata that records context
```

Type sketch:

```rust
pub struct Score {
    pub primary: Option<ComparableScore>,
    pub metrics: MetricSet,
    pub feedback: Vec<Feedback>,
    pub attachments: Vec<ScoreAttachment>,
    pub metadata: MetadataBag,
}

pub struct ComparableScore {
    pub value: FiniteF64,
    pub direction: ScoreDirection,
}

pub enum Feedback {
    Text(String),
    Structured(serde_json::Value),
}

pub enum ScoreAttachment {
    File { name: String, path: PathBuf },
    Directory { name: String, path: PathBuf },
    Evidence { name: String, evidence: EvidenceRef },
}
```

Attachments are staged into the evidence/artifact store and become durable
references. Runtime paths are never the durable score payload.

The invariants:

1. A score may contain arbitrary feedback evidence.
2. An optimizer may rank, admit, or update population state only from declared
   comparable score axes.
3. Metadata records context; it does not drive optimizer decisions unless the
   user promotes it to a metric.
4. In-loop GEPA scores must provide at least one comparable score axis unless
   the configured optimizer policy explicitly supports unscored feedback-only
   observations.
5. A case may have no target/reference. Fixed gold answers, hidden verifier
   targets, LLM judges, human judgments, environment reward signals, and open-ended
   task scoring are all score sources; they are not dataset requirements.
6. Scoring functions may see scorer-only data such as hidden targets when the
   builder/domain adapter provides it. Reflective proposers may only see the
   feedback/evidence allowed by split policy.

## 11. Result Contract

`Optimized` / `RunOutput` is the ordinary user's completed-run handle. It is not
a duplicate run graph and it must not copy evidence payloads into a second truth.

Minimum public shape:

```rust
pub struct Optimized<P: OptimizationProblem, S = StandardRunSummary> {
    pub run_id: RunId,
    pub best: Option<CandidateId>,
    pub stop: StopReason,
    pub budget: BudgetSnapshot,
    pub summary: S,
}
```

Required methods:

```text
best_id() -> Option<CandidateId>
best() -> Option<&P::Artifact> when the result owns an in-memory graph snapshot
report() -> &EvaluationReport
gepa() -> Option<&GepaResult> when the optimizer supplied a GEPA summary
events() -> public event summaries, not mutable graph access
graph() / into_graph() only on an explicitly advanced result type or feature
```

Result invariants:

1. `best` comes from `Optimizer::best_candidate` after the optimizer stops and
   after any product-policy final evaluation that can affect public best.
2. `report()` is graph-backed. It cites assessment ids, evidence refs, metric
   summaries, and attachment refs; it does not copy blobs or hidden targets.
3. Test split outputs are marked final-report-only unless policy explicitly
   allowed in-loop use.
4. If no candidate has a comparable score, `best` is `None`; the seed does not
   win by default unless it has admissible evidence or the optimizer declares a
   seed-as-best policy.
5. `Optimized` records whether the run stopped because the optimizer was done,
   budget was exhausted, a callback stopped it, or an error aborted it.
6. Public result accessors must not require users to learn `RunGraph` for the
   ordinary best/report path.

## 12. Open Design Pressure

The remaining implementation pressure is not whether rich scores exist; they
must. The pressure is how much ergonomic lifting `leaven-run` can provide before
callers should install a typed `Evaluator<P>`.

The hard line:

```text
.score(fn)       ordinary user path, scalar or rich
.evaluator(e)     lower-level engine/evidence adapter path
```

Do not add a separate public `.score_with_feedback(...)` step. It reintroduces a
concept users should not need and makes the scalar-to-rich transition look like a
different API instead of the same scoring function returning more information.
