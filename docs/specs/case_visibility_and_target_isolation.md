# Case Visibility And Target Isolation

Status: implementation spec.
Date: 2026-05-15.

This spec defines how `leaven-run` and `leaven-eval` preserve case identity,
targets, and metadata without leaking hidden answers into ordinary candidate
execution. It is subordinate to `docs/specs/initial_library.md`,
`docs/specs/guiding_principles.md`, `docs/specs/eval_lowering_detail.md`, and
`docs/specs/gepa_public_private_surface.md`.

## 1. Problem

The current product runner path passes the raw case payload to user runner
closures. If the user payload is shaped like:

```rust
struct AimeCase {
    source_id: String,
    problem: String,
    answer: i64,
    solution: String,
    needs_modular: bool,
}
```

then the runner can read `answer`, `solution`, and provenance fields unless the
example voluntarily ignores them. That is not an enforceable product contract.
For real benchmark and optimizer runs, answer-bearing data must be available to
scoring and reports without becoming runner-visible input.

This is a library boundary problem, not an AIME example problem.

## 2. Vocabulary

`CaseId` is Leaven's stable handle for one evaluation unit. It is used in
datasets, splits, engine case sets, reports, cache keys, and checkpoint/resume
state. `CaseId` is not the domain input, and it is not necessarily the upstream
dataset row label.

`input` is the domain payload the candidate is allowed to run on.

`target` is scorer-visible reference data. It may contain gold answers, expected
outputs, hidden verifier material, rubrics, or reference solutions. It is
optional because not every task is a labeled example.

`metadata` is provenance, audit, report, routing, or stratification data. It is
not ordinary runner input and it is not a score target by default.

`source_id` is a domain/upstream provenance field. It should normally live in
case metadata, while the Leaven `CaseId` provides the typed run identity. A
domain adapter may derive a stable `CaseId` from a `source_id`, but reports must
not require humans to reverse-engineer source rows from positional case ids.

## 3. Canonical Case Shape

The lowered case shape belongs in `leaven-eval`:

```rust
pub struct Case<I, T = NoTarget> {
    pub id: CaseId,
    pub input: I,
    pub target: Option<T>,
    pub metadata: MetadataBag,
}

pub enum NoTarget {}
```

`Case<I, T>` is the durable envelope. Ordinary runners and scorers do not receive
this envelope directly. They receive typed views with narrower visibility.

Builder conveniences may accept plain inputs and synthesize `CaseId`s for toy
workflows, but any product-real dataset path must preserve caller-provided
`CaseId`s and metadata.

## 4. Visibility Contract

The ordinary product path has four visibility tiers:

| Consumer | May see | Must not see by default |
| --- | --- | --- |
| Runner | candidate artifact, case id, case input, budget snapshot, runner-local environment handles | target, metadata, hidden split/report policy, run graph |
| Scorer | candidate artifact, case id, input, runner output, optional target, budget snapshot, selected scorer metadata view | run graph mutation, hidden split policy, unrelated case metadata |
| Optimizer/reflection | candidate ids/artifacts, allowed evaluation feedback, allowed generated output/trace summaries, split-allowed report facts | hidden targets, raw scorer-only metadata, validation/test data outside policy |
| Report/audit | case id, split, score, feedback, output/evidence refs, projected metadata such as source id, cache status, cost | hidden target payloads unless explicitly projected |

Runner invisibility is structural: the runner API must not include a type
parameter or field from which `T` can be read.

Scorer visibility is controlled: the scorer may see `target` because judging
often requires gold answers, reference solutions, rubrics, or hidden verifier
material. Scorer feedback is the only ordinary path by which target-derived
information may become visible to the optimizer.

Metadata visibility is narrower than target visibility. Pure provenance such as
`source_id` should be report-visible. Stratification fields may be visible to
split/sampler policy. Scorer metadata must be explicitly selected by the
builder/domain adapter; a scorer must not automatically receive the full
metadata bag just because it exists.

## 5. Runner View

The runner view is intentionally target-free:

```rust
pub struct RunCaseView<'a, I> {
    id: CaseId,
    input: &'a I,
}

impl<'a, I> RunCaseView<'a, I> {
    pub fn id(&self) -> CaseId;
    pub fn input(&self) -> &'a I;
}

pub struct CandidateRunCtx<'a, P: OptimizationProblem, I> {
    candidate: CandidateView<'a, P::Artifact>,
    case: Option<RunCaseView<'a, I>>,
    budget: BudgetSnapshot,
}
```

There is deliberately no `T` type parameter on `RunCaseView` or
`CandidateRunCtx`. That absence is the enforcement mechanism. A runner that only
receives `CandidateRunCtx<'_, P, I>` cannot read target data through the
ordinary API.

An implementation may offer a convenience closure form:

```rust
Fn(A, I) -> impl Future<Output = RunOutput>
```

or:

```rust
Fn(A, RunCaseInput<I>) -> impl Future<Output = RunOutput>
```

where `RunCaseInput<I>` contains only `case_id` and `input`. It must not contain
target or metadata.

If a domain intentionally wants target-conditioned execution, it is not the
ordinary runner path. It must use a different, explicitly named adapter with a
different proof contract, because that changes benchmark semantics.

## 6. Scorer View

The scorer view carries target, output, and budget:

```rust
pub struct ScoreContext<'a, P: OptimizationProblem, I, T = NoTarget, O = ()> {
    candidate: CandidateView<'a, P::Artifact>,
    case: Option<ScoreCaseView<'a, I, T>>,
    output: Option<&'a O>,
    run_error: Option<RunErrorView<'a, O>>,
    trace: TraceView<'a>,
    budget: BudgetSnapshot,
}

pub struct ScoreCaseView<'a, I, T = NoTarget> {
    id: CaseId,
    input: &'a I,
    target: Option<&'a T>,
    split: Option<SplitRole>,
}
```

The current ordinary scorer case view exposes no metadata projection. Domain
adapters may add a selected, typed scorer metadata projection only when scoring
semantics require it. That projection must not be an empty marker type. It must
name the projected fields and prove their cache/fingerprint effect. Examples:

- a benchmark license field needed to decide whether to skip scoring;
- a rubric version used by a judge;
- a verifier config id that is part of the scoring contract.

If projected metadata can change the numeric score or feedback, it must
participate in evaluator fingerprinting and cache correctness. Until such a
typed projection exists, scorer metadata is a missing feature, not a public
empty view.

## 7. Metadata Policy

Metadata has explicit classes:

```rust
pub enum MetadataUse {
    Provenance,
    Report,
    Stratification,
    Scoring,
    OperatorDebug,
}
```

This enum is design vocabulary; the first implementation may use a simpler
typed policy, but it must preserve the same distinction.

Default rules:

1. Provenance metadata is stored and report-projectable.
2. Report metadata may appear in result summaries and sidecars.
3. Stratification metadata may be used by split builders, samplers, or filters.
4. Scoring metadata is visible only to scorers and participates in fingerprints.
5. Operator-debug metadata is durable but not visible to runner/scorer/optimizer
   unless explicitly projected.

Putting a value in metadata must never make it runner-visible.

If the candidate should use a field to solve the task, the field belongs in
`input`, not metadata.

## 8. Target Policy

`target` is hidden from the runner and optimizer by default. It is visible to
scoring logic because scoring is the boundary that turns hidden references into
score and feedback evidence.

The scorer may use target to produce feedback. That feedback can become
optimizer-visible according to split policy. This is not a leak; it is the
intended reflective-learning channel. The target itself remains hidden.

A target value must be serializable whenever the run is durable. If the target
cannot be serialized, the builder must refuse durable execution or require an
explicit external target reference that can be restored.

Target changes affect dataset/evaluator identity. Reusing cached evaluations
after a target change is invalid unless the cache policy explicitly keys on a
stable target/evaluator fingerprint that changed.

## 9. Cache And Fingerprint Requirements

The engine evaluation cache must never key deterministic reuse only on
`CandidateId`.

For deterministic scorer caching, the cache key or evaluator fingerprint must
cover:

- scorer implementation/config fingerprint;
- runner implementation/config fingerprint when a runner is used;
- case-set version and resolved case ids;
- candidate cache identities;
- input identity/content;
- target identity/content when target can affect scoring;
- projected scoring metadata identity/content when metadata can affect scoring.

Pure provenance metadata such as `source_id` should not affect score cache
identity unless user scoring code reads it. It still belongs in report
projection and durable audit records.

Default remains no-cache. Deterministic cache requires an explicit policy and
safe candidate/cache identities.

## 10. Reports

Reports should carry enough case reference data to audit a run without exposing
hidden targets:

```rust
pub struct ReportScore {
    pub case_id: CaseId,
    pub source: Option<CaseSourceRef>,
    pub split: Option<SplitRole>,
    pub score: f64,
    pub feedback: String,
    pub output: OutputRecord,
    pub cache: CacheStatus,
    pub cost: Cost,
    pub metadata: ReportMetadataView,
}
```

`CaseSourceRef` is the report-visible provenance handle. For AIME it should hold
the upstream `source_id`.

Report metadata projection must be explicit. Hidden targets and unprojected
metadata do not appear in ordinary reports.

## 11. AIME Lowering

AIME should lower to:

```rust
pub struct AimeInput {
    pub problem: String,
}

pub struct AimeTarget {
    pub answer: i64,
    pub solution: String,
}

Case {
    id: stable_case_id_from_source_id(...),
    input: AimeInput { problem },
    target: Some(AimeTarget { answer, solution }),
    metadata: metadata! {
        "source_id" => source_id,
        "needs_modular" => needs_modular,
    },
}
```

The solver runner sees `AimeInput` only. It cannot see `answer`, `solution`,
`source_id`, or `needs_modular` through the ordinary runner API.

The scorer sees `AimeTarget` and may use `solution` to generate feedback. The
final report projects `source_id` and any requested audit tags.

If a run intentionally gives the solver a tag such as `needs_modular`, that tag
must be moved into `AimeInput`; doing so changes the task definition and must be
visible in the dataset/version fingerprint.

## 12. Implementation Cutover

The cutover should be hard:

1. Change `leaven-run` runner/scorer generics from raw `C` to `Case<I, T>` plus
   target-free runner views and target-aware scorer views.
2. Keep `leaven-eval::Case` as the durable envelope.
3. Update `OptimizeBuilder` train/validation/test inputs to accept case records
   with stable ids and metadata, while retaining explicit convenience methods for
   plain input-only toy cases if desired.
4. Stop constructing reports from positional `CaseId::from_index` when caller
   supplied stable ids.
5. Remove or rewrite examples that pass answer-bearing case structs to runners.
6. Preserve hard separation in type signatures; do not rely on documentation or
   user discipline to hide target fields.

## 13. Proof Requirements

Minimum proof set:

1. `leaven-eval` example/law: duplicate `CaseId`s are rejected and metadata is
   preserved in the dataset.
2. `leaven-run` scenario: runner receives only input and case id for a
   `Case<Input, Target>`; scorer receives target.
3. `leaven-run` scenario: report includes projected `source_id` without exposing
   target payload.
4. `leaven-run` cache example: default scorer remains no-cache.
5. `leaven-run` deterministic-cache scenario, when cache policy lands: target or
   projected scoring metadata changes invalidate deterministic reuse.
6. Compile-fail test, preferably with `trybuild`: a runner closure that attempts
   to receive `Case<I, T>` or access target through `CandidateRunCtx` does not
   typecheck on the ordinary API.
7. P8 scenario: AIME runner cannot observe answer/solution/source metadata; AIME
   scorer and report still can.

Narrow verification commands for the implementation slice:

```bash
cargo test -p leaven-eval
cargo test -p leaven-run --test scoring_evaluator --test optimize_builder
cargo test -p leaven --test topology_contract
```

Run `just check` before claiming the full behavior complete.
