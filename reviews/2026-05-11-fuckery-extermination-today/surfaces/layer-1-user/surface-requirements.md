# Layer 1 Surface Requirements

Status: canonical Layer 1 audit doc.

This states the exact ordinary-user public contract Layer 1 must satisfy. Names
are contract names unless marked as sketches. Implementation may choose module
placement consistent with repo topology, but the user-visible behavior must not
weaken.

## 1. Ordinary Import Contract

### Required Layer 1 Names

The ordinary prelude should expose the minimum user path:

- `optimize`
- `OptimizeBuilder`
- `OptimizeResult` or the hard-cut replacement `Optimized`
- `OptimizationReport` or the hard-cut replacement report view
- `OptimizeError`
- `Budget`
- `Case<I, T = NoTarget>` or equivalent stable case type
- `NoTarget`
- `RunOutput` only if hard-cut to typed candidate run output; otherwise replace
  with `CandidateRun<O>`
- `CandidateRunner`
- `CandidateRunCtx`
- `CandidateRun<O>`
- `Scorer`
- `ScoreContext`
- `Score`
- `ScoreError`
- `ScoreDirection`
- `MetricValue` / metric axis type
- `Feedback`
- `ScoreAttachment`
- common runtime-role constructors/configs for solver, reflector, scorer/judge,
  and agent roles
- `Gepa` default ordinary constructor when the `gepa` feature is enabled

### Forbidden Layer 1 Prelude Names

The ordinary prelude must not export these as default-facing Layer 1 names:

- `RunGraph`, `RunGraphView`, `RunContext`
- `ReadScope`, `TrustPolicy`
- `EvaluationRequest`, `ResolvedEvaluationRequest`, `EvaluationSet`
- `Assessment`, `AssessmentGranularity`, `AssessmentTarget`
- `Population`, `ParentSelector`, `PartSelector`
- `Proposer`, `Evaluator`, `Renderer`, `Materializer`
- engine `CachePolicy`
- `EvidenceStore`
- `CachedLm`, `InMemoryLmCache`, `LmCacheEntry`, `LmCacheKey`, `LmCacheStore`

Evidence: Layer 1 should not require graph/trust/evaluation/population/store
names (`docs/specs/gepa_public_private_surface.md:69-83`), but the current
prelude exports many of them (`crates/leaven/src/prelude.rs:3-25`) and re-exports
the LM cache prelude under the `lm-cache` feature (`crates/leaven/src/prelude.rs:48-49`).

### Proof

Add a compile/import contract under `crates/leaven/tests`:

- ordinary example compiles with only `use leaven::prelude::*`;
- ordinary example contains no explicit engine imports;
- deny-list assertion prevents ordinary prelude from exposing the forbidden
  names above.

## 2. Builder Contract

### Required Methods

The Layer 1 builder must provide one canonical state machine:

```text
optimize(seed)
.task(task) or explicit single-task/no-dataset input
.train(cases) / .cases(cases)
.validation(cases)
.test(cases)
.runner(runner)
.score(fn)
.evaluator(evaluator)
.using(optimizer)
.budget(budget)
.runtime(role_config) or role-specific runtime/cache methods
.store(store)
.resume(checkpoint)
.on_event(callback)
.run()
```

The spec requires these builder responsibilities and pre-run refusals
(`docs/specs/gepa_public_private_surface.md:641-674`). The current builder has
only `.train`, `.validation`, `.test`, `.runner`, `.score`, `.using`, `.budget`,
`.on_event`, `.store`, and `.run` (`crates/leaven-run/src/builder.rs:92-198`).

### Required Refusals

`OptimizeError` or its hard-cut replacement must distinguish:

- missing optimizer;
- missing scorer/evaluator;
- missing budget unless explicitly unlimited;
- validation/test without train in default GEPA mode;
- both primary scorer and primary evaluator installed;
- runner required but absent;
- duplicate case ids;
- default-disallowed split overlap;
- invalid budget caps;
- resume checkpoint fingerprint mismatch;
- declared attachment persistence unsupported by configured store;
- score error;
- runner error;
- attachment staging error;
- no comparable score when the configured optimizer requires one.

Current errors cover only missing budget, missing score, held-out without train,
seed insertion, and optimizer failure (`crates/leaven-run/src/error.rs:5-25`).

### Proof

Add builder scenario tests for each refusal. Existing tests cover missing budget,
explicit unlimited budget, held-out without train, no best, store/callback
dispatch, and missing score in AIME (`crates/leaven-run/tests/optimize_builder.rs:23-158`;
`examples/p8_aime_gepa/src/main.rs:459-475`), but they do not cover single-task,
duplicate ids, evaluator-vs-score conflict, runtime roles, resume, runner errors,
score errors, attachment persistence, or invalid budgets.

## 3. Work Input Contract

### Case Type

Layer 1 must have a stable case shape, owned by the eval/product boundary and
re-exported for ordinary use:

```rust
pub struct Case<I, T = NoTarget> {
    pub id: CaseId,
    pub input: I,
    pub target: Option<T>,
    pub metadata: MetadataBag,
}

pub enum NoTarget {}
```

The spec defines this minimum public case shape and rules
(`docs/specs/gepa_public_private_surface.md:773-820`). Cases are units of work,
not synonymous with datasets, labels, or environments.

### Modes

Layer 1 must support:

- single-task/no-dataset mode;
- train-only multi-task/search mode;
- train+validation/test generalization mode;
- domain task suites that lower to stable case ids and split roles.

Mode inference must be:

```text
no train/validation/test      -> single-task
train only                    -> multi-task/search
train + validation/test       -> generalization
explicit mode override        -> override when needed
```

Evidence: mode inference is specified in
`docs/specs/gepa_public_private_surface.md:722-734`, and `EvaluationSet::Unscoped`
exists for single-task/evaluator-internal work in the original spec
(`docs/specs/initial_library.md:1092-1108`).

### Current Gap

Current `leaven-run` accepts plain split vectors and generates dense positional
case ids (`crates/leaven-run/src/builder.rs:214-222`;
`crates/leaven-run/src/builder.rs:302-356`). That can remain only as an explicit
dense-id convenience.

### Proof

Add law/scenario tests:

- `Dataset` rejects duplicate case ids;
- disjoint splits reject overlap;
- fingerprints are stable and change when content/membership changes;
- public builder lowers train/validation/test into `TRAIN`, `VALIDATION`, and
  `TEST`;
- public builder supports no-dataset single-task;
- default GEPA never feeds test into in-loop feedback;
- reports state whether test was final-report-only.

The eval spec lists these proof obligations (`docs/specs/eval_lowering_detail.md:790-818`).

## 4. Runner Contract

### Type Contract

The canonical runner contract must be async and typed:

```rust
pub trait CandidateRunner<P: OptimizationProblem, I, T = NoTarget, O = ()>:
    Send + Sync + 'static
{
    fn run(
        &self,
        ctx: CandidateRunCtx<'_, P, I, T>,
    ) -> impl Future<Output = Result<CandidateRun<O>, CandidateRunError<O>>> + Send;
}

pub struct CandidateRunCtx<'a, P: OptimizationProblem, I, T = NoTarget> {
    pub candidate: CandidateView<'a, P::Artifact>,
    pub case: Option<CaseView<'a, I, T>>,
    pub budget: BudgetSnapshot,
}

pub struct CandidateRun<O = ()> {
    pub output: O,
    pub trace: TraceBundle,
    pub attachments: Vec<ScoreAttachment>,
    pub cost: Cost,
}
```

Evidence: this is the specified candidate execution contract
(`docs/specs/gepa_public_private_surface.md:987-1012`).

### Rules

- runner output/trace becomes part of `ScoreContext`;
- runner cost is charged before score cost;
- runner failures are not scores by default;
- score-on-error is explicit policy;
- self-contained scoring can omit `.runner(...)`;
- environment handles live in adapters/domain crates, not hidden in `leaven-run`.

Evidence: runner rules are specified in
`docs/specs/gepa_public_private_surface.md:1014-1028`.

### Current Gap

Current runner is a sync `Fn(&A, &C) -> RunOutput`
(`crates/leaven-run/src/builder.rs:28-29`) and current `RunOutput` is only
`String + Vec<String>` (`crates/leaven-run/src/evidence.rs:3-20`).

### Proof

Add tests for async runner, metered cost, typed output, trace bundle,
attachments, runner failure, score-on-error, bounded concurrency, and no hidden
environment abstraction in `leaven-run`.

## 5. Scoring Contract

### Public Verb

The public verb is `.score(...)`, not `.reward(...)` and not
`.score_with_feedback(...)`. Reward signals are a score source, not the public
API concept. The spec explicitly states `.score(fn)` is the ordinary path and a
separate feedback method should not exist (`docs/specs/gepa_public_private_surface.md:1232-1245`).

### ScoreContext

`ScoreContext` must be an accessor-based typed view, not public fields and not a
graph handle:

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
```

Evidence: `ScoreContext` contract and forbidden internals are specified in
`docs/specs/gepa_public_private_surface.md:930-985`.

### Scorer

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

Allowed ergonomic inputs lower to the same contract:

```text
Fn(ScoreContext) -> impl IntoScore
Fn(ScoreContext) -> Result<impl IntoScore, ScoreError>
async Fn(ScoreContext) -> impl IntoScore
async Fn(ScoreContext) -> Result<impl IntoScore, ScoreError>
```

Evidence: scorer contract and overloads are specified in
`docs/specs/gepa_public_private_surface.md:1029-1049`.

### Score

`Score` must support:

- primary comparable score;
- named metrics with direction/role;
- natural-language feedback;
- structured feedback records;
- file, directory, image, transcript, log, JSON, and workspace attachments;
- evidence refs;
- metadata;
- unscored diagnostics when policy allows.

Evidence: `Score` contents and attachment rules are specified in
`docs/specs/gepa_public_private_surface.md:1115-1182`.

### Current Gap

Current `Score` is `f64 + String + Vec<(String, String)>`, and current
`ScoreContext` is public fields for artifact, case, output only
(`crates/leaven-run/src/evidence.rs:23-54`). Current evaluator flattens
structured feedback into trace strings (`crates/leaven-run/src/evaluator.rs:103-116`).

### Proof

Add tests for:

- scalar/bool lifting into rich score;
- non-finite score refusal;
- score error recorded as failure, not zero;
- metered scoring cost;
- multiple metric axes with direction;
- natural-language and structured feedback preservation;
- attachment staging and missing attachment errors;
- hidden targets visible only to scorer/evaluator;
- no `RunGraph`, `TrustPolicy`, or `EvaluationRequest` access from
  `ScoreContext`.

## 6. Runtime And Cache Role Contract

Layer 1 must configure runtime roles, not manual wrapper stacks. Required roles:

- solver/program runner;
- reflector/proposer;
- scorer/model judge;
- agent runtime.

Each role needs:

- provider/runtime value;
- cache policy;
- budget/cost policy;
- fingerprint identity for resume/cache safety;
- reportable cost/cache summary.

Current LM pieces are real but low-level: `Lm` is provider-neutral and async
(`crates/leaven-lm/src/model.rs:9-22`), `OpenAiLm` implements OpenAI Responses
API lowering (`crates/leaven-lm-openai/src/client.rs:10-37`), and `CachedLm`
wraps an LM/cache/policy (`crates/leaven-lm-cache/src/cached.rs:6-17`). The spec
currently teaches wrapper stacking (`docs/specs/lm_runtime_and_response_cache.md:15-31`),
which is acceptable for advanced cache docs but not for the canonical ordinary
example.

Proof:

- `leaven-lm-cache` policy/key tests stay as lower-level law tests;
- Layer 1 scenario proves a cached mocked solver LM, cached mocked reflector LM,
  and cached mocked judge/scorer LM;
- OpenAI mapping tests need no live credentials;
- live example depends on Leaven LM/provider crates rather than Python provider
  bypass.

## 7. GEPA Reflection Requirement Visible To Layer 1

Layer 1 does not need to customize GEPA internals, but the ordinary proof must
exercise real reflection.

Required reflector behavior:

- consumes selected parent candidate;
- consumes selected part and part view;
- consumes feedback minibatch identity;
- consumes assessment ids and selected evidence/trace views;
- preserves hidden validation/test policy;
- uses solver/reflector runtime roles when LM/agent-backed;
- returns edits/proposals with causal and `informed_by` provenance;
- is async-capable.

Evidence: GEPA reflective mutation iteration and inputs are specified in
`docs/specs/gepa_optimizer_surface.md:322-357` and
`docs/specs/gepa_optimizer_surface.md:463-483`.

Current gap: `SurfaceProposer` sees only artifact, surface, and part
(`crates/leaven-gepa/src/proposer.rs:6-19`), and `ReflectiveMutation` returns a
stored edit (`crates/leaven-gepa/src/proposer.rs:21-47`).

Proof:

- mock-LM reflector test consumes casewise feedback and produces an edit;
- invalid reflector output becomes typed proposal error;
- validation/test content is hidden from reflector by default;
- fixed edit helper exists only under a fixture/demo/test-support path.

## 8. Result Contract

The result facade must expose run truth without requiring ordinary users to
learn `RunGraph`.

Required public shape:

```rust
pub struct Optimized<P: OptimizationProblem, S = StandardRunSummary> {
    pub run_id: RunId,
    pub best: Option<CandidateId>,
    pub stop: StopReason,
    pub budget: BudgetSnapshot,
    pub summary: S,
}
```

Required accessors:

```text
best_id() -> Option<CandidateId>
best() -> Option<&P::Artifact> when an in-memory graph snapshot is owned
report() -> &EvaluationReport
gepa() -> Option<&GepaSummary>
events() -> public event summaries
graph()/into_graph() only on explicit advanced result type or feature
```

Evidence: result contract is specified in
`docs/specs/gepa_public_private_surface.md:1184-1228`.

Current gap: `OptimizeResult` requires a best candidate id and cloned artifacts
(`crates/leaven-run/src/result.rs:6-18`); `OptimizationReport` is aggregate
floats plus event strings (`crates/leaven-run/src/result.rs:35-61`); missing
train averages and empty averages become `0.0`
(`crates/leaven-run/src/builder.rs:452-457`;
`crates/leaven-run/src/result.rs:64-71`).

Proof:

- no comparable score returns `best = None` unless policy explicitly says seed
  wins;
- stop reason distinguishes optimizer done, budget exhausted, user/callback stop,
  and error abort;
- report cites assessment ids, evidence refs, metric summaries, attachments, and
  split roles;
- missing/failed evidence is absent/error, not `0.0`;
- test results are marked final-report-only unless policy allows in-loop use;
- events are public summaries, not strings or mutable graph access.

## 9. Canonical Product Proof

The canonical Layer 1 example must prove the whole surface:

```rust
let result = leaven::optimize(seed_program)
    .train(train_cases)
    .validation(dev_cases)
    .test(test_cases)
    .runner(program_runner)
    .score(score_fn)
    .using(Gepa::default().with_reflection_lm(reflection_lm))
    .budget(Budget::metric_calls(300))
    .run()
    .await?;
```

This shape is already the spec product goal
(`docs/specs/gepa_optimizer_surface.md:54-70`). The current `p8_aime_gepa`
example uses a similar shell (`examples/p8_aime_gepa/src/main.rs:75-99`), but it
must not remain canonical until it uses real async runner/scorer, stable cases,
rich score/evidence, Leaven LM/runtime/cache roles, evidence-aware reflection,
and graph-backed result reporting.

Acceptance proof:

- `just milestone-p8` runs the canonical mock proof;
- a live-provider smoke swaps only provider/runtime construction;
- `just check` is the final completion gate (`docs/testing/README.md:7-17`);
- no product proof may rely on provider shell-out or fixed edit reflection.

