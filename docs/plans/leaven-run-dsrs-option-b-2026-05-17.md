# Leaven Run Option B for DSRs: Plan

> **Closeout note (2026-05-20).** Status: landed as **hard cutover**, with the
> shape simplified from the original plan. Items 1–2 (typed `RunOutput<Out>`
> and `ScoreContext<..., Out>` threaded through builder/evaluator) landed.
> Items 3–4 (explicit renderer seam, renderer fingerprint as a `RuntimeKind`
> slot, renderer mismatch on resume) did **not** land — the implementation
> review (2026-05-19) replaced them with a simpler contract: the scorer is the
> rendering boundary via `Score::with_output(...)` /
> `Score::with_text_output(...)`, missing output is a hard
> `MissingReportableOutput` evaluation failure with runner+scorer cost
> preserved, and there is no separate output-renderer identity in the
> compatibility manifest. Renderer-fingerprint cache invalidation is therefore
> not a current concern: any rendering change is part of scorer identity,
> which is already in the compatibility manifest.
>
> The compatibility schema bumped to `v3` (the v1→v3 jump reflects iterative
> work; v2 never reached `main`) and any older manifest is refused with
> `ResumeCompatibilityError::SchemaMismatch` before runtime work.
>
> Default `Out` landed as `()` (not `String`) per
> `docs/specs/case_visibility_and_target_isolation.md` §6. The String default
> back-compat shim and the `TypeId`-based auto-render for `Out = String` were
> removed. The "runner after score" type-changing ordering is an explicit
> `OptimizeError::InvalidBuilderOrder` refusal before runtime work; a future
> type-state pass could promote it to a compile error.
>
> **DSRs reality check.** As of 2026-05-20 DSRs went Option A
> (`DSRs/crates/dsrs-leaven/src/evaluator.rs` implements
> `leaven_engine::Evaluator<DsrsLeavenProblem>` directly) and does not depend
> on `leaven-run`. The typed-`Out` work in this plan is still valid as a
> leaven-side spec slot (`case_visibility_and_target_isolation.md`); it is
> just not consumed by DSRs today. If DSRs reconsiders, the `.score(...)`
> path is now available.

## Goal

Define the smallest Leaven-side improvement that lets DSRs build DSPy-like `Module` / `Predicted` / `TypedMetric` primitives on top of Leaven while using the ordinary `leaven-run::optimize(...).runner(...).score(...).using(Gepa...)` path. Leaven should not become DSPy or DSRs; it should preserve typed runner output through scoring, then deliberately lower that output into existing Leaven evidence/report/GEPA reflection records.

## Background

### Supersession note

- 2026-05-19 implementation review changed the lowering shape: the public builder does **not** expose an output-renderer API or renderer fingerprint axis. Typed runner output is scorer-local; `.score(...)` supplies reportable generated output through `Score::with_output(...)` / `Score::with_text_output(...)`, and ordinary `String` runner output still defaults into report output.

### Provenance

- Before this plan pass, the workspace was refreshed with `jj git fetch` as requested.

### Scope decision

- Option B means improving `leaven-run`, not adding a DSRs-only custom evaluator. Option A remains technically possible because `leaven_engine::Evaluator<P>` already exists, but it would bypass the public `leaven-run` path and duplicate scoring-evaluator mechanics.
- DSRs owns DSPy-like vocabulary: module, predicted output, typed metric, signature/schema parsing, and adapter ergonomics. Leaven owns artifact/evaluator/case/evidence/optimizer primitives and should expose only the generic seams DSRs needs.
- Do not reintroduce `crates/leaven-dsrs`; repo guidance identifies it as an orphan placeholder/bait. DSRs bridge work belongs in the DSRs root, likely `DSRs/crates/dsrs-leaven`, after Leaven exposes the generic seam.

### Leaven primitives that already exist and should be reused

- `Artifact` already represents the optimized domain value and explicitly allows prompt modules, struct trees, repositories, skill directories, kernels, and other opaque programs: `crates/leaven-core/src/artifact.rs:6-40`.
- `OptimizationProblem` already bundles artifact, case, evidence, and proposal annotations for engine-level optimizers/evaluators: `crates/leaven-core/src/problem.rs:1-40`.
- `Evaluator<P>` already exists as the engine boundary for resolved evaluation requests to metered assessment evidence, with evaluator fingerprint and cache policy as part of the contract: `crates/leaven-engine/src/stage/evaluator.rs:1-35`.
- `Case<I, T>`, `Dataset`, train/validation/test splits, and optional targets already model DSRs-style examples: `crates/leaven-eval/src/dataset.rs:14-56`.
- `Score` already carries scalar value, natural-language feedback, and scorer cost: `crates/leaven-run/src/evidence.rs:31-58`.
- `CaseAssessmentEvidence` already stores scalar score, generated output, and feedback: `crates/leaven-evidence/src/feedback.rs:9-42`.
- `OutputRecord` already represents lowered generated output as inline text or blob ref: `crates/leaven-evidence/src/command.rs:10-39`.
- `leaven_gepa::Gepa` already consumes scalar casewise scores through `GepaScoreEvidence`, including `CasewiseEvidence<CaseAssessmentEvidence>`: `crates/leaven-gepa/src/optimizer.rs:33-68`.
- GEPA reflection already consumes rendered examples through `ReflectiveExample { input, output: Option<String>, score, feedback }` and the `ReflectiveDatasetBuilder` seam: `crates/leaven-gepa/src/reflection.rs:40-189`.

### Current `leaven-run` gap

- `RunOutput` is public but string-only: `RunOutput { output: String, cost: Cost }`; constructor and cost attachment live at `crates/leaven-run/src/evidence.rs:7-29`.
- `ScoreContext` receives `output: RunOutput`, so scorer closures can only see string output today: `crates/leaven-run/src/evidence.rs:225-240`.
- `OptimizeBuilder` stores `Runner<A, I> -> RunOutput` and `Scorer<A, I, T> -> ScoreContext<A, I, T>`; `.runner(...)` and `.score(...)` are fixed to those string-shaped aliases: `crates/leaven-run/src/builder.rs:41-44`, `:178-199`.
- `ScoringEvaluator` duplicates the same runner/scorer aliases and hard-lowers output after scoring via `OutputRecord::inline(output.output)`: `crates/leaven-run/src/evaluator.rs:18-24`, `:222-260`.
- There is no existing `Metric` trait or `.metric(...)` builder method in the Leaven crates. The current metric-like surface is `.score(...)` plus `Score`.

### GEPA does not need typed-output awareness

- GEPA candidate comparison uses `GepaScoreEvidence::scalar_casewise()` and only needs scalar case scores: `crates/leaven-gepa/src/optimizer.rs:33-68`.
- GEPA reflection sees rendered text through `ReflectiveExample.output: Option<String>` and default projection from `CasewiseEvidence<CaseAssessmentEvidence>`: `crates/leaven-gepa/src/reflection.rs:40-58`, `:337-350`.
- Therefore Option B should keep GEPA typed-output-unaware. Typed output is for the runner/scorer metric path; evidence/reflection/reporting stay rendered through `OutputRecord`.

### DSRs adapter pressure

- DSRs `Module` returns typed `Predicted<M::Output>`; DSRs `TypedMetric` evaluates an example and typed prediction. Loaded-root scout refs: `DSRs/crates/dspy-rs/src/core/module.rs:57-78`, `DSRs/crates/dspy-rs/src/core/predicted.rs:169-202`, `DSRs/crates/dspy-rs/src/evaluate/evaluator.rs:73-85`.
- DSRs `Example<S>` maps naturally to `leaven_eval::Case<S::Input, S::Output>`; Leaven runner receives target-free `RunCase<I>`, while scorer sees optional target through `ScoreCase<I, T>::target()`: `crates/leaven-run/src/evidence.rs:125-165`, `:183-223`.
- Existing DSRs integration planning chose a custom evaluator because `leaven-run` was text-first, but explicitly allowed a narrow generic Leaven change if implementation exposed a real generic seam gap: `docs/plans/dsrs-leaven-integration-2026-05-16.md:108-121`.
- The prior typed-output plan already identifies that gap and proposes preserving typed runner output through scoring, then rendering output into existing durable evidence: `docs/plans/typed-run-output-2026-05-17.md`.

### Compatibility and proof anchors

- Compatibility manifests currently track runner, scorer, and evaluator fingerprints, but not output-renderer identity: `crates/leaven-run/src/compatibility.rs:20-39`, `:62-152`.
- Existing compatibility tests cover scorer mismatch, dataset/split mismatch, and missing durable runner/scorer fingerprints: `crates/leaven-run/tests/optimize_builder.rs:650-769`.
- Existing scoring evaluator tests cover string output evidence and target visibility invariants: `crates/leaven-run/tests/scoring_evaluator.rs:156-309`.
- P8/AIME parity prior art requires the high-level builder path `leaven::optimize(...).runner(...).score(...).using(Gepa...).run()`, not a proxy-only custom evaluator: `docs/plans/2026-05-15-gepa-aime-parity/requirements-summary.md:8-23`.

## Approach

Option B is a targeted `leaven-run` generic-output change. It does not add DSRs concepts to Leaven and it does not make Leaven imitate DSPy's public vocabulary. It makes the ordinary Leaven builder path preserve typed runner output through scoring, then explicitly render/lower that output into existing Leaven evidence.

Today the public path is:

```text
optimize(seed)
  -> .train/.validation/.test(Case<I, T>)
  -> .runner(A, RunCase<I>) -> RunOutput<String>
  -> .score(ScoreContext<A, I, T>) -> Score
  -> ScoringEvaluator
  -> CasewiseEvidence<CaseAssessmentEvidence>
  -> reports + GEPA reflection
```

The corrected Option B path is:

```text
optimize(seed)
  -> .train/.validation/.test(Case<I, T>)
  -> .runner(A, RunCase<I>) -> RunOutput<Out>
  -> .score(ScoreContext<A, I, T, Out>) -> Score
  -> render RunOutput<Out> into OutputRecord
  -> CasewiseEvidence<CaseAssessmentEvidence>
  -> reports + GEPA reflection
```

For DSRs this gives the adapter shape DSRs needs without forcing it into a custom Leaven evaluator:

```text
DSRs Module::call(input) -> Predicted<M::Output>
Leaven runner returns RunOutput<Predicted<M::Output>>
DSRs TypedMetric evaluates ScoreCase<I, T> + Predicted<M::Output>
Leaven renderer lowers Predicted<M::Output> -> OutputRecord
Leaven GEPA consumes CasewiseEvidence<CaseAssessmentEvidence>
```

### Chosen API direction

Use one output type parameter on the ordinary runner/scorer/evaluator path:

```rust
RunOutput<Out = String>
ScoreContext<A, I, T = NoTarget, Out = String>
OptimizeBuilder<A, I, T, Opt, Out = String>
ScoringEvaluator<A, I, T = NoTarget, Out = String>
```

Preserve the existing string route:

- `RunOutput::new(...)` remains the string constructor.
- `RunOutput<String>::default()` remains empty string plus zero cost.
- Existing scorer code using `ctx.output.output` continues to compile on the default route.
- Existing reports and GEPA reflection remain rendered string consumers.
- `ReportScore.output` remains `String`.

Do not add output type parameters to:

- `RunProblem<A, I, T>`
- `CaseAssessmentEvidence`
- `ReflectiveExample`
- `Gepa`
- report types

Typed output is evaluator/scorer-local. The durable Leaven boundary remains rendered `OutputRecord` plus scalar score and feedback.

### Renderer requirement

Add an explicit output-rendering seam owned by `leaven-run`:

```rust
Fn(&RunOutput<Out>) -> Result<OutputRecord, OutputRenderError>
```

Expose a builder-level renderer API with both ephemeral and durable forms. The implementation may use two methods (`.render_output(...)` and `.render_output_with_fingerprint(...)`) or one method plus a fingerprint configuration, but the public contract must distinguish durable fingerprinted renderers from ephemeral closure renderers.

Rules:

- The initial/default `String` builder has a built-in renderer installed and produces today's behavior: `OutputRecord::inline(output.output.clone())`.
- A builder may run only if it has a renderer for its current `Out` type.
- Missing renderer is a builder/run-configuration error: `OptimizeError::MissingOutputRenderer` before engine execution begins.
- A renderer that is present but fails during evaluation maps to `EvaluationError::with_cost_source(...)`, preserving incurred runner/scorer cost and using `OutputRenderError` as the source.
- Durable custom renderers must provide a stable fingerprint.
- Ephemeral custom renderers may receive an ephemeral runtime fingerprint under the same rules as closure runner/scorer fingerprints.
- Avoid silent `Display` or `Debug` stringification.
- Renderer is bound to the current `Out` type. If `.runner(...)` changes `Out`, any previously installed non-default renderer for the old output type must not carry across.
- Renderer receives only runner output, not target-bearing case data; target leakage remains controlled by `RunCase` / `ScoreCase`.

### Evaluator data flow

Evaluation should proceed in this order:

```text
RunCase<I> built from Case<I, T>
runner(artifact, RunCase<I>) -> RunOutput<Out>
scorer(ScoreContext<A, I, T, Out>) -> Score
renderer(&RunOutput<Out>) -> OutputRecord
CaseAssessmentEvidence::new(score, output_record, feedback)
```

`Out: Clone + Send + Sync + 'static` is acceptable for this slice because the evaluator must pass output into the scorer and then render the original output after scoring. DSRs can wrap heavy typed predictions in `Arc` if cloning the prediction is expensive.

Rendering happens after the scorer succeeds. Rendering failure is an evaluation failure and must preserve incurred runner/scorer cost in the error cost.

### Compatibility and cache identity

Rendering changes durable evidence, report output, GEPA reflection text, and evaluator cache identity. Renderer identity therefore belongs beside runner/scorer identity in `leaven-run`, not in GEPA.

Add renderer identity to:

- `RuntimeKind`, as `OutputRenderer` or an equivalent runtime slot.
- `ScoringEvaluatorIdentity`.
- evaluator fingerprint mixing.
- `RunCompatibilityManifest`.
- `RunCompatibilitySummary`.

Compatibility rules:

- Built-in string renderer has a stable built-in fingerprint and requires no user ceremony.
- Durable custom renderer without a fingerprint is refused.
- Renderer fingerprint mismatch refuses resume.
- Bump the compatibility manifest schema when adding the renderer field. Prefer a hard schema cutover for durable resume rather than backfilling v1 manifests; if implementation intentionally accepts a v1 manifest, it must be limited to the built-in string renderer and still produce a distinct evaluator fingerprint so old cache entries cannot be reused unsafely.

### Metric surface

Do not add `.metric(...)` as a core primitive in this slice. Leaven already has the engine-level `Evaluator<P>` and the high-level `.score(...) -> Score` closure. DSRs can adapt `TypedMetric` through `.score(...)` once `ScoreContext` carries typed output.

If later useful, `.metric(...)` can be an ergonomic adapter layered over `.score(...)`; it should not become a new cold-core primitive and it should not block Option B.

## Work Items

### Item 1 — Generic runner output and score context

**Goal:** Let `leaven-run` represent typed runner output in public evidence types while preserving the default string API.

**Done when:**

- `RunOutput` becomes `RunOutput<Out = String>`.
- `RunOutput::new(...)` still constructs string output, preferably from `impl RunOutput<String>` or an equivalent unambiguous string-only constructor home.
- `RunOutput<String>::default()` preserves current empty-string behavior.
- There is an explicit typed constructor for non-string outputs, such as `RunOutput::typed(output)`.
- `ScoreContext` becomes output-generic with default `String`.
- Existing string-output tests still compile with `ctx.output.output`.
- A narrow unit test can construct `ScoreContext<..., TypedOutput>` directly and inspect typed fields; the end-to-end typed scorer proof waits for Item 5.

**Key files:**

- `crates/leaven-run/src/evidence.rs:7-29`, `:225-240`
- `crates/leaven-run/src/lib.rs`
- `crates/leaven-run/tests/scoring_evaluator.rs:156-309`

**Dependencies:** None.

**Size:** Medium.

### Item 2 — Thread output type through builder

**Goal:** Make `.runner(...)` establish the output type and `.score(...)` receive the same typed output, while keeping durable problem/evidence shape unchanged.

**Done when:**

- `OptimizeBuilder<A, I, T, Opt>` gains an `Out = String` type parameter.
- Private `Runner` and `Scorer` aliases become output-generic.
- `.runner(...)` consumes `self` and returns a builder parameterized by `NextOut`.
- `.runner(...)` clears or rebuilds renderer state for `NextOut`; an old renderer for the previous `Out` cannot leak into the new output type.
- Only the default `String` path starts with the built-in renderer installed.
- `.score(...)` receives `ScoreContext<A, I, T, Out>`.
- `.using(...)`, `.budget(...)`, `.train(...)`, `.validation(...)`, `.test(...)`, callbacks, store, and `RunProblem<A, I, T>` remain output-type independent.
- Existing builder tests using string outputs remain source-compatible.

**Key files:**

- `crates/leaven-run/src/builder.rs:41-44`, `:53-66`, `:178-214`
- `crates/leaven-run/tests/optimize_builder.rs`

**Dependencies:** Item 1.

**Size:** Large.

### Item 3 — Add explicit output rendering

**Goal:** Deliberately lower typed output into `OutputRecord` after scoring.

**Done when:**

- `OutputRenderError` exists and is public from `leaven-run`.
- `OutputRenderError` is a minimal `Error + Send + Sync + 'static` type with message/source support; cost stays on the enclosing `EvaluationError`, not inside the render error.
- Internal renderer closure type accepts `&RunOutput<Out>` and returns `OutputRecord` or `OutputRenderError`.
- Built-in string renderer produces today's `OutputRecord::inline(output.output.clone())` behavior.
- The builder exposes a renderer API with ephemeral and durable/fingerprinted forms; exact method spelling is implementation-owned.
- A builder without a renderer for its current `Out` fails with `OptimizeError::MissingOutputRenderer` before engine execution.
- Renderer receives runner output only, not target-bearing case data.
- Rendering happens after scorer succeeds.
- Rendering failure maps to `EvaluationError::with_cost_source(...)` with `OutputRenderError` as the source and incurred runner/scorer cost preserved.

**Key files:**

- `crates/leaven-run/src/evidence.rs`
- `crates/leaven-run/src/error.rs`
- `crates/leaven-run/src/builder.rs`
- `crates/leaven-run/src/evaluator.rs:222-260`
- `crates/leaven-run/tests/scoring_evaluator.rs`
- `crates/leaven-run/tests/optimize_builder.rs`

**Dependencies:** Items 1–2.

**Size:** Medium.

### Item 4 — Add renderer fingerprinting to compatibility

**Goal:** Prevent resume/cache reuse when output-rendering behavior changes.

**Done when:**

- Runtime compatibility includes output renderer identity.
- `RuntimeKind` includes `OutputRenderer` or an equivalent slot.
- `ScoringEvaluatorIdentity` includes renderer fingerprint.
- Evaluator fingerprint changes when renderer fingerprint changes.
- `RunCompatibilityManifest` stores renderer fingerprint and uses a bumped manifest schema.
- `RunCompatibilitySummary` exposes renderer fingerprint.
- Durable custom renderer without fingerprint is refused.
- Built-in string renderer requires no user ceremony.
- Tests prove renderer mismatch refuses resume.
- Tests prove a v1 manifest is either refused by schema or accepted only for the built-in string renderer without unsafe evaluator-cache reuse.

**Key files:**

- `crates/leaven-run/src/compatibility.rs:20-39`, `:62-152`
- `crates/leaven-run/src/builder.rs:336-386`
- `crates/leaven-run/src/result.rs`
- `crates/leaven-run/tests/optimize_builder.rs:650-769`
- `crates/leaven-run/tests/scoring_evaluator.rs`

**Dependencies:** Items 2–3.

**Size:** Medium.

### Item 5 — Prove the public Option B path

**Goal:** Demonstrate DSPy-like typed prediction scoring through ordinary `leaven-run`, without a custom evaluator and without depending on DSRs code.

**Done when:**

- A focused test defines a typed prediction struct with output plus metadata.
- Runner returns `RunOutput<TypedPrediction>`.
- Scorer receives `ScoreContext<..., TypedPrediction>` and scores typed fields directly.
- Renderer lowers typed prediction to `OutputRecord`.
- `CaseAssessmentEvidence` remains unchanged.
- `ReportScore.output` remains `String` and `run_report` still lowers `OutputRecord` to report text.
- The chain uses `.using(Gepa...)`.
- The GEPA proof uses deterministic local pieces: a minimal artifact plus surface, a fixed/deterministic GEPA reflector, a narrow iteration/budget, and no live provider calls.
- GEPA public types receive no runner-output type parameter.
- GEPA reflection receives rendered output text, not typed prediction internals; the assertion should observe `ReflectiveExample.output` or the nearest stable reflection boundary.
- Existing string P8/AIME behavior remains unchanged; if `.runner(...)` type changes require source edits in P8, include those compatibility edits in this item.
- No DSRs crate is required for the Leaven proof.

**Key files:**

- `crates/leaven-run/tests/optimize_builder.rs`
- `crates/leaven-run/tests/scoring_evaluator.rs`
- `crates/leaven-run/src/run_report.rs`
- `crates/leaven-eval/src/report.rs`
- `crates/leaven-gepa/src/reflection.rs:40-58`, `:337-350`
- `crates/leaven-gepa/src/optimizer.rs:33-68`
- Optional focused `crates/leaven-gepa/tests/*` if the reflection assertion belongs there.

**Dependencies:** Items 1–4.

**Size:** Medium.

### Item 6 — Update Leaven-side docs and DSRs bridge handoff

**Goal:** Keep Leaven's public contract honest and record the downstream DSRs handoff without turning this Leaven work item into a DSRs implementation plan.

**Done when:**

- `crates/leaven-run/AGENTS.md` mentions typed `RunOutput<Out>` and explicit rendering into `OutputRecord`.
- Public docs continue presenting `String` as the default route.
- Docs explicitly state typed outputs are not durable report payloads in this slice.
- A follow-up note or minimal edit to `docs/plans/dsrs-leaven-integration-2026-05-16.md` records that Option B supersedes the earlier "avoid `leaven-run` because it is string-first" rationale once the Leaven work lands; it does not specify DSRs implementation internals.
- The plan records that `.metric(...)` is deferred ergonomic sugar, not required Option B infrastructure.

**Key files:**

- `docs/plans/leaven-run-dsrs-option-b-2026-05-17.md`
- `docs/plans/dsrs-leaven-integration-2026-05-16.md`
- `crates/leaven-run/AGENTS.md:57-82`

**Dependencies:** API decision from Items 1–4; may be finalized after Item 5.

**Size:** Small.

## Risks and Migration Notes

- `.runner(...)` becomes type-changing. Start by updating/adding tests that prove the default `String` route remains source-compatible, then thread the generic output type through the builder.
- Missing renderer and failing renderer are different failures: missing renderer is a pre-engine `OptimizeError`; failing renderer is an evaluator-side `EvaluationError` with `OutputRenderError` as source.
- Renderer output affects durable evidence, reports, reflection text, and evaluator cache identity; compatibility changes must land with renderer support, not as a later cleanup.
- Typed `Out` is not durable in this slice. Only rendered `OutputRecord` is stored or reported.
- DSRs can use `Arc<Predicted<_>>` or another owned wrapper if `Out: Clone` is too expensive for large prediction metadata.

## Verification Order

1. `RunOutput<Out>` and `ScoreContext<..., Out>` compile with default string compatibility.
2. Builder/evaluator thread `Out` through `.runner(...)` and `.score(...)`.
3. Renderer vocabulary, missing-renderer refusal, and render-failure mapping work.
4. Renderer fingerprinting changes compatibility manifests and evaluator identity safely.
5. Public typed-output builder + GEPA proof passes without DSRs code.
6. Documentation and DSRs bridge handoff are updated.
7. Narrow gates:
   - `cargo nextest run -p leaven-run --test scoring_evaluator --test optimize_builder`
   - relevant GEPA reflection tests if touched
   - `just milestone-p8` if P8 behavior changes
8. Completion gate: `just check`.

## Resolved Decisions

No blocking questions remain for this plan. Chosen decisions:

- Reuse Leaven's existing `Evaluator<P>`; do not create another evaluator primitive.
- Keep `.score(...)` as the core scoring closure and add `.metric(...)` only as optional ergonomic adapter if it proves useful during implementation.
- Preserve `String` as the default route and avoid breaking existing `RunOutput::new(...)`, `ctx.output.output`, reports, and examples.
- Do not add typed output to `RunProblem`, `CaseAssessmentEvidence`, `ReflectiveExample`, or GEPA public types.

## References

- `crates/leaven-run/src/evidence.rs`
- `crates/leaven-run/src/builder.rs`
- `crates/leaven-run/src/evaluator.rs`
- `crates/leaven-run/src/compatibility.rs`
- `crates/leaven-run/tests/scoring_evaluator.rs`
- `crates/leaven-run/tests/optimize_builder.rs`
- `crates/leaven-gepa/src/optimizer.rs`
- `crates/leaven-gepa/src/reflection.rs`
- `docs/plans/typed-run-output-2026-05-17.md`
- `docs/plans/dsrs-leaven-integration-2026-05-16.md`
- `docs/plans/2026-05-15-gepa-aime-parity/requirements-summary.md`
