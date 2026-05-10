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
Give Leaven a candidate, training work, a scorer, an optimizer, and a budget.
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
score / evaluator
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
    .run()
    .await?;
```

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
| Scoring | `.score(fn)` / `.evaluator(e)` | evaluator id/registry | `Evaluator<P>`, assessments, evidence store | `leaven-run`, `leaven-engine` |
| Feedback/traces | evaluator return type | reflector renderer | `Evidence`, `AttributableEvidence`, renderers | `leaven-evidence`, `leaven-render`, `leaven-gepa` |
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
score closure adapters when they can be expressed without domain loss
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
- evaluator execution logic beyond adapter helpers;
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
score/evaluator
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

The public builder must lower:

```text
.train(cases)       -> TRAIN split, default in-loop feedback/use
.validation(cases)  -> VALIDATION split, held out from proposers by default
.test(cases)        -> TEST split, final-report-only by default
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

## 10. Open Design Pressure

The only unclear part that remains is how rich `.score(fn)` can be while still
being honest.

Simple scalar scoring can be closure-adapted easily. GEPA-quality reflection
often needs traces, per-case feedback, artifacts, and sometimes agent/session
evidence. The public API should therefore support a ladder:

```text
.score(|candidate, case| -> f64)
.score_with_feedback(|candidate, case| -> ScoreAndFeedback)
.evaluator(my_typed_evaluator)
```

Do not collapse all three into one trait if doing so makes the simple case
verbose or the rich case lossy.
