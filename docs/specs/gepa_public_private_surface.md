# GEPA Public And Private Surface

Status: planning surface contract.
Date: 2026-05-10.

Supersession note, 2026-05-16: this document remains the Layer 1/Layer 2/Layer
3 audience-separation contract, but GEPA parity semantics and GEPA-specific
library API choices now live in `docs/specs/gepa_reference_behavior.md`. If this
document's examples imply train-only parent selection, generic population-backed
GEPA defaults, stale reflection request names, or a validation cadence that
conflicts with real GEPA, follow the reference behavior document.

This spec defines the user-facing GEPA surface and the private/lowered contracts
that support it. It is the coordination document for the current GEPA/eval
specs:

- `docs/specs/gepa_optimizer_surface.md`
- `docs/specs/eval_lowering_detail.md`
- `docs/specs/eval_nomenclature.md`
- `docs/specs/case_visibility_and_target_isolation.md`

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
CandidateSelector
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

let best = result.best();
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

Examples that use `Gepa::default()` rely on a derivable edit surface. The
surface resolution order is:

```text
explicit GepaBuilder::surface(...)
domain adapter supplied surface
artifact-provided DefaultEditSurface<A>
```

If no surface is available, the builder must reject before the run starts. It
must not silently invent a string or part decomposition.

### 2.2 Layer 2: Customize GEPA

This layer is for users who want GEPA, but not the default GEPA.

They should touch recognizable algorithm knobs:

```text
surface
candidate selector
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
    .candidate_selector(ParetoFrequencyWeighted::default())
    .part_selector(InvokedAndFailingPart::default())
    .batch_sampler(EpochShuffled::new(4))
    .reflector(LmBackedReflector::with_default_renderer(lm, "gpt-4.1-mini"))
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

## 3. Candidate Selection Versus Part Selection

GEPA has two different selection questions.

```text
Candidate selection: Which candidate should GEPA mutate next?
Part selection:      Where inside that candidate should GEPA edit?
```

Example:

```text
Candidate A: prompt { system, rubric, examples }
Candidate B: prompt { system, rubric, examples }
Candidate C: prompt { system, rubric, examples }
```

Candidate selection chooses `A`, `B`, or `C`.

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
candidate_selector
part_selector
```

`candidate_selector` is acceptable internally, but "candidate selector" is clearer
for GEPA users because it names the role of the selected candidate in the next
proposal.

## 4. Interactable GEPA Map

| GEPA aspect | User-visible API | Customizer API | Lowered/private contract | Owner |
| --- | --- | --- | --- | --- |
| Candidate/program | `optimize(seed)` / `.seed(seed)` | artifact type | `Artifact`, `CandidateId`, graph insertion | `leaven-core`, `leaven-engine`, domain crates |
| Editable view | default/derived surface | `.surface(surface)` | `EditSurface`, part ids, surface fingerprint | `leaven-surface`, artifact/domain crates |
| Candidate choice | hidden default | `.candidate_selector(...)` | selector reads population + graph view | `leaven-gepa` |
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
| Persistence | `.run_dir(...)` / `.store(...)` / `.resume(...)` | checkpoint policy | run graph + optimizer state snapshots | `leaven-run`, `leaven-engine`, `leaven-store`, `leaven-store-file` |
| Report | `result.report()` | report options | graph-backed report construction over eval/gepa schemas | `leaven-run`, `leaven-eval`, `leaven-gepa` |

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
leaven-store-file
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
candidate selector traits and defaults
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
scoring function / evaluator
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
CandidateSelector
PartSelector
EvidenceStore
```

Those can still be public Rust APIs. They are not the default story.

### 6.2 GEPA Customizer Contract

GEPA customizer traits must be small and swappable:

```text
CandidateSelector
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
| `CandidateSelector` | population state + scoped graph view + optional search state | candidate id(s) or typed "no parent" decision | mutate graph, run evaluators, inspect forbidden splits |
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
leaven-eval       -> leaven-core, leaven-kernel
leaven-gepa       -> leaven-core, leaven-engine,
                     leaven-evidence, leaven-kernel, leaven-lm,
                     leaven-population, leaven-preference,
                     leaven-render, leaven-surface
leaven-run        -> leaven-core, leaven-engine, leaven-eval,
                     leaven-evidence, leaven-kernel, leaven-store,
                     leaven-store-file, leaven-store-inline
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
| `.run_dir(path)` | Uses a durable local run directory as both store override and resume handle. If the directory already has a latest checkpoint, the builder restores graph truth, budget, cache index, and optimizer continuation before continuing. |
| `.ephemeral()` | Explicit throwaway mode. It uses inline evidence and no checkpoint persistence; this is the only ordinary spelling for non-resumable runs. |
| `.store(store)` | Supplies a low-level `OptimizeStore`: `OptimizeStore::evidence(evidence_store)` for evidence-only plumbing, or `OptimizeStore::durable(evidence_store, run_persistence)` when checkpoint persistence is also configured. This is an advanced override; omitted `.store(...)` uses the Leaven-managed durable local run directory, not inline storage. |
| `.resume(checkpoint)` | Reserved public shape for direct checkpoint/run-id resume. The implemented ordinary handle is currently `.run_dir(existing_dir)`, which restores the latest checkpoint in that directory. |
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
- non-finite, negative, or otherwise invalid budget caps;
- resume checkpoint fingerprint mismatch;
- store configuration that cannot persist declared attachments.

Minimum `OptimizeBuildError` decision classes:

```rust
pub enum OptimizeBuildError {
    MissingOptimizer,
    MissingScorerOrEvaluator,
    MissingBudget,
    HeldOutWithoutTrain,
    ConflictingPrimaryEvaluation,
    RunnerRequired,
    DuplicateCaseId { split: SplitRole, id: CaseId },
    SplitOverlap { id: CaseId, first: SplitRole, second: SplitRole },
    InvalidBudget(BudgetError),
    ResumeFingerprintMismatch { expected: Fingerprint, actual: Fingerprint },
    StoreCannotPersistAttachments,
}
```

The exact enum may grow as implementation discovers new refusals. It must not
collapse these decision classes into a string error; callers need to know
whether to add config, change input data, repair storage, or abandon resume.

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
.store(store)       -> evidence storage; durable form also wires engine/run persistence
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
train/search evidence can drive proposer feedback, candidate selection,
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

### 8.5 Case And Split Contract

Layer 1 cases are work items. They are not synonymous with datasets, labels, or
environments.

Minimum public case shape, defined once in `leaven-eval` and re-exported by
`leaven-run`/`leaven`:

```rust
pub struct Case<I, T = NoTarget> {
    pub id: CaseId,
    pub input: I,
    pub target: Option<T>,
    pub metadata: MetadataBag,
}

pub enum NoTarget {}
```

Builder conveniences may accept plain `I` values and synthesize stable case ids,
but the lowered form is always an ordered set of `Case<I, T>` values assigned to
one split. Domain adapters may wrap richer task/environment records, but they
must still lower to case ids plus split roles before reaching the engine.

Case rules:

1. `input` is the thing the runner may use to execute the candidate.
2. `target` is optional and scorer-visible by default. It is not
   runner-visible, and it is not proposer-visible unless split policy explicitly
   allows target-derived feedback.
3. `metadata` is report/debug/provenance/stratification context. It is not
   runner-visible. It affects scoring only when a builder/domain adapter
   explicitly projects selected metadata into the scorer view.
4. Case order is preserved for deterministic samplers, but samplers may choose a
   different evaluation order.
5. Case ids are unique across all default splits. If overlap is desired, the user
   must opt into an overlap policy that says which split role each use receives.
6. Empty train is valid only for explicit single-task mode. Empty validation/test
   sets are no-ops and should be rejected only if the caller explicitly required
   them.

Default split semantics:

```text
train       in-loop search work; may update population and proposer feedback
validation held-out model-selection work; may affect selected best only by policy
test        final-report work; never affects optimizer state by default
```

The report may show test scores for all candidates selected for final testing,
but test scores must not retroactively mutate population state or reflection
history under the default policy.

### 8.6 Scoring Call Cardinality

The ordinary `.score(...)` path is cardinal and case-oriented:

```text
for each candidate selected by GEPA
  for each selected case, or once if no case set exists
    run candidate when a runner is installed
    call scorer with typed output/trace/error context
    normalize Score into assessment/evidence
```

`leaven-run` may batch work internally for efficiency, but the public semantics
are as if each candidate/case pair produced an independent `Score`. A scorer
must not rely on call order. If the user needs true batch, pairwise, listwise, or
tournament semantics, they should install a typed `.evaluator(...)` or write an
optimizer-stage evaluator instead of forcing that through `.score(...)`.

### 8.7 Budget Contract

`Budget` is a set of caps over cost axes, not a stopper and not just a number.

Layer 1 constructors should include the common caps:

```rust
Budget::metric_calls(500)
Budget::usd(50.0)
Budget::wall_time(Duration::from_secs(1800))
Budget::new().metric_calls(500).usd(50.0).tokens(2_000_000)
Budget::unlimited()
```

These are required product constructors for the `leaven-run` slice, not aliases
that already exist in the current kernel. The implementation slice that exposes
these examples must expand `Budget` to cap the first-class `Cost` axes it
advertises, including LLM calls, token counts, seconds/wall time, and explicit
money axes such as USD.

Budget rules:

1. Product builders require either a finite budget or explicit
   `Budget::unlimited()`.
2. Every cap is finite and non-negative at construction time.
3. Every stage that spends money, time, tokens, calls, or user-defined units
   charges the central budget ledger through the engine.
4. Charging is monotone. Refunds or compensating adjustments must be explicit
   events, not negative costs.
5. A run stops when any hard cap is exceeded or a stopper decides to stop from a
   budget snapshot.
6. `BudgetExceeded` reports the axis, cap, already-spent amount, attempted
   charge, and stage id.
7. Budget bookkeeping is independent of stopping strategy: GEPA may stop because
   it is done even with budget remaining, and budget may stop a run in the middle
   of runner, scorer, reflector, or evaluator work.

## 9. Documentation Rules

When revising the companion specs:

1. Teach Layer 1 before internal types.
2. Put actor/trust/visibility language only in lowered/engine sections.
3. Use `candidate_selector` in GEPA-facing docs and reserve
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
            .attach_evidence("trace", ctx.trace().evidence_ref())
            .metadata("judge", "gpt-5.2")
    })
    .using(Gepa::default())
    .budget(Budget::metric_calls(300))
    .run()
    .await?;
```

Scalar scores are lifted into `Score`:

```rust
.score(|ctx| {
    if ctx.output().is_some_and(|output| output.passed()) { 1.0 } else { 0.0 }
})
```

Typed evaluators remain the power-user escape hatch:

```rust
.evaluator(my_typed_evaluator)
```

`ScoreContext` is the public trace/state object. It is a typed view, not a graph
handle. Public examples may use method syntax (`ctx.output()`, `ctx.trace()`),
and implementations should prefer accessors over public fields so the view can
remain graph-backed.

```rust
pub struct ScoreContext<'a, P: OptimizationProblem, I = (), T = NoTarget, O = ()> {
    /* private fields */
}

impl<'a, P, I, T, O> ScoreContext<'a, P, I, T, O>
where
    P: OptimizationProblem,
{
    pub fn candidate(&self) -> CandidateView<'a, P::Artifact>;
    pub fn case(&self) -> Option<CaseView<'a, I, T>>;
    pub fn output(&self) -> Option<&'a O>;
    pub fn run_error(&self) -> Option<RunErrorView<'a, O>>;
    pub fn trace(&self) -> TraceView<'a>;
    pub fn history(&self) -> ScoreHistoryView<'a, P>;
    pub fn budget(&self) -> BudgetSnapshot;
}

pub struct CaseView<'a, I, T = NoTarget> {
    pub id: CaseId,
    pub input: &'a I,
    pub target: Option<&'a T>,
    pub split: Option<SplitRole>,
    pub metadata: &'a MetadataBag,
}

impl<'a, I, T> CaseView<'a, I, T> {
    pub fn input(&self) -> &'a I;
    pub fn target(&self) -> Option<&'a T>;
    pub fn split(&self) -> Option<SplitRole>;
    pub fn metadata(&self) -> &'a MetadataBag;
}
```

The scorer contract is async and fallible so model judges can await provider
calls without lifetime gymnastics. Do not simplify that by replacing the typed
view with a scalar-only, sync-only, or infallible scorer path.

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

`budget` is the point-in-time budget snapshot visible to the scoring function.
Scorers may use it to choose a cheap judge, skip optional checks, or include
remaining-budget facts in feedback, but all actual charging still goes through
engine budget accounting.

Runner output/trace/cost becomes part of `ScoreContext`. Scorer output must
lower score, feedback, structured fields, attachments, and cost into typed
evidence. Scorer failures are `ScoreError`s; they are not zero scores. If a
failure incurred provider/runtime cost, that cost must still be charged by the
engine before the evaluation error returns.

`ScoreContext` must not expose `RunGraph`, `Actor`, `ReadScope`, `TrustPolicy`, or
evaluation request templates in Layer 1.

When `.runner(...)` is installed, it adapts to this candidate-execution
contract:

```rust
pub trait CandidateRunner<P: OptimizationProblem, I, O = ()>:
    Send + Sync + 'static
{
    fn run(
        &self,
        ctx: CandidateRunCtx<'_, P, I>,
    ) -> impl Future<Output = Result<CandidateRun<O>, CandidateRunError<O>>> + Send;
}

pub struct CandidateRunCtx<'a, P: OptimizationProblem, I> {
    pub candidate: CandidateView<'a, P::Artifact>,
    pub case: Option<RunCaseView<'a, I>>,
    pub budget: BudgetSnapshot,
}

pub struct RunCaseView<'a, I> {
    pub id: CaseId,
    pub input: &'a I,
}

pub struct CandidateRun<O = ()> {
    pub output: O,
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
6. Generic runners capture their own environment handles. Workspace, sandbox,
   process, and agent runners live in adapter crates such as `leaven-agentic` or
   domain crates; `leaven-run` must not grow a hidden environment abstraction.
7. Runner case views are target-free and metadata-free. Hidden answers,
   reference solutions, source ids, and stratification tags must not be reachable
   through the ordinary runner type signature.

Every public scoring function is adapted to this canonical contract:

```rust
pub trait Scorer<P: OptimizationProblem, I, T = NoTarget, O = ()>:
    Send + Sync + 'static
{
    fn call(
        &self,
        ctx: ScoreContext<'_, P, I, T, O>,
    ) -> impl Future<Output = Result<Metered<Score>, ScoreError>> + Send;
}
```

Builder overloads may accept simpler closures, but they all lower to that shape:

```text
Fn(ScoreContext<'_, P, I, T, O>) -> impl IntoScore
Fn(ScoreContext<'_, P, I, T, O>) -> Result<impl IntoScore, ScoreError>
async Fn(ScoreContext<'_, P, I, T, O>) -> impl IntoScore
async Fn(ScoreContext<'_, P, I, T, O>) -> Result<impl IntoScore, ScoreError>
```

The generic names in examples may be elided, but the contract must stay typed:
runner output is not a stringly `serde_json::Value` unless the user's runner
chooses that as `O`.

Scalar convenience forms may exist only if they are unambiguous. They still
normalize as if the user had received a full `ScoreContext`.

`IntoScore` must support:

```text
Score                 rich score
Metered<Score>        rich score plus scoring-stage cost
FiniteF64 / f64        primary higher-is-better score after finite validation
bool                   1.0 for true, 0.0 for false
```

Implementation status: the current `leaven-run` scorer slice has only the
owned builder lowering needed by the P8 AIME example:
`ScoreContext<A, C> { artifact, case, output, budget }`, async/fallible scorer
closures, scorer attachments, and scorer cost. That is a useful cutover from the
old sync/scalar path, but it is not the full Layer 1 scoring contract above. Do
not treat the current owned struct as permission to drop `case = None`,
`run_error`, `trace` accessors, `history()`, `Metered<Score>`, or generic runner
output from the public design.

Do not support `Option<Score>` as a public return. Use `Score::unscored(...)`
for diagnostics without a comparable value, so absence is explicit.

Plain scalar/bool/`Score` returns are metered as zero additional scoring cost.
If a scoring function calls an LLM judge, subprocess, external verifier, or
human-review system, it must return `Metered<Score>` with the cost it incurred.
The adapter charges that cost exactly once after successful scoring. Score
errors may also carry `BudgetExceeded` when scoring stopped before producing a
score.

Errors are not scores. A `ScoreError` means the scoring function failed to
produce an assessment. The adapter must record the failure as evaluation error
evidence and then follow the configured failure policy. It must not silently
turn score errors, panics, non-finite scores, missing attachments, or invalid
metric directions into score `0.0`.

Minimum scoring/runner error decision classes:

```rust
pub enum CandidateRunError<O = ()> {
    Failed { source: BoxError, partial: Option<PartialCandidateRun<O>> },
    TimedOut { partial: Option<PartialCandidateRun<O>> },
    BudgetExceeded(BudgetExceeded),
    PanicCaptured { message: String, partial: Option<PartialCandidateRun<O>> },
    InvalidOutput { reason: String },
}

pub struct PartialCandidateRun<O = ()> {
    pub output: Option<O>,
    pub trace: TraceBundle,
    pub attachments: Vec<ScoreAttachment>,
    pub cost: Cost,
}

pub enum ScoreError {
    InvalidScore(InvalidScore),
    Attachment(AttachmentStageError),
    TimedOut,
    BudgetExceeded(BudgetExceeded),
    PanicCaptured { message: String },
    Failed { source: BoxError },
}
```

These errors describe refusal points. Failure policy decides whether to continue,
retry, mark the candidate/case failed, or abort the run. The scorer itself does
not decide optimizer admission by failing.

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

Attachment rules:

1. File and directory attachments are captured before runner/scorer temporary
   resources are released.
2. Missing paths, unreadable paths, unsupported symlinks, or paths outside the
   allowed workspace become `AttachmentStageError`; they are not ignored.
3. Directory staging must be deterministic: traversal order, path normalization,
   and ignored file rules are part of the store fingerprint.
4. Reports cite staged attachment refs, never host-local runtime paths.

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

Minimum ordinary public shape:

```rust
pub struct Optimized<A> {
    pub run_id: RunId,
    pub best: Option<BestCandidate<A>>,
    pub seed_artifact: A,
    pub stop: OptimizationStopReason,
    pub budget: BudgetSnapshot,
    pub summary: StandardRunSummary,
    pub events: Vec<RunEventSummary>,
}

pub struct BestCandidate<A> {
    pub id: CandidateId,
    pub artifact: A,
}
```

`StandardRunSummary` is the concrete ordinary run summary. It carries
storage/resumability, optimizer cost, final-report cost, train/validation/test
score summaries, and the graph-backed `EvaluationReport`. It is not a generic
parameter on `Optimized` yet. Add a GEPA-specific result/report surface only
when the GEPA summary is behavior-bearing enough to justify a public route.

Required methods:

```text
best_id() -> Option<CandidateId>
best() -> Option<&A> when the result owns an in-memory artifact
report() -> &EvaluationReport
summary() -> &StandardRunSummary
events() -> public event summaries, not mutable graph access
graph() / into_graph() only on an explicitly advanced result type or feature
```

Result invariants:

1. `best` comes from `Optimizer::best_candidate` after the optimizer stops and
   after any explicit validation policy that is allowed to affect model
   selection.
2. `report()` is graph-backed. It cites assessment ids, evidence refs, metric
   summaries, and attachment refs; it does not copy blobs or hidden targets.
3. Test split outputs are marked final-report-only unless policy explicitly
   allowed in-loop use. Under the default policy, test results never change
   `best`.
4. If no candidate has a comparable score, `best` is `None`; the seed does not
   win by default unless it has admissible evidence or the optimizer declares a
   seed-as-best policy.
5. `Optimized` records whether the run stopped because the optimizer was done,
   budget was exhausted, a callback stopped it, or an error aborted it.
6. Public result accessors must not require users to learn `RunGraph` for the
   ordinary best/report path.
7. `BestCandidate<A>` bundles the durable candidate id with the owned artifact;
   do not model those as two parallel optional fields.

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
