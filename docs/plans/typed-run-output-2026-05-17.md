# Typed Run Output for Leaven Scoring: Plan

## Goal

Make Leaven's ordinary `leaven-run::optimize(...)` scoring path preserve typed runner outputs through scoring, while still lowering generated outputs deliberately into text/blob evidence for reports and GEPA reflection. The immediate pressure is faithful DSPy-style GEPA: a typed module prediction must reach typed metric code intact instead of being flattened into `String` before scoring; DSRs supplies the Rust typed substrate that exposes this gap.

## Background

### User intent and governing decision

- The user wants the Leaven shape to remain faithful to DSPy's GEPA programming model rather than forcing typed module predictions through an early string/JSON flattening layer.
- The plan should improve Leaven generically, not add a DSRs-only workaround, because DSRs exposed a real limitation in `leaven-run`'s current runner/scorer abstraction.
- The current direction is: preserve typed runner output until scoring, then render it explicitly into Leaven's existing generated-output evidence/reflection/report path.

### Current `leaven-run` runner/scorer boundary

- `RunOutput` is currently public and string-only: `RunOutput { output: String, cost: Cost }`, with `RunOutput::new(output: impl Into<String>)`: `crates/leaven-run/src/evidence.rs:7-29`.
- `ScoreContext` passes `output: RunOutput` to scorer closures, so scorers currently see only the string output plus cost: `crates/leaven-run/src/evidence.rs:228-240`.
- `OptimizeBuilder` stores `Runner<A, I> = Fn(A, RunCase<I>) -> Future<RunOutput>` and `Scorer<A, I, T> = Fn(ScoreContext<A, I, T>) -> Future<Result<Score, ScoreError>>`: `crates/leaven-run/src/builder.rs:41-44`.
- `.runner(...)` fixes the runner return to `RunOutput`: `crates/leaven-run/src/builder.rs:179-187`; `.score(...)` fixes scorer context to `ScoreContext<A, I, T>`: `crates/leaven-run/src/builder.rs:191-199`.
- `RunProblem<A, I, T>` fixes evidence to `CasewiseEvidence<CaseAssessmentEvidence>`: `crates/leaven-run/src/builder.rs:53-66`.
- `ScoringEvaluator` repeats the same string-shaped private aliases and stores runner/scorer closures plus cases, fingerprint, cache policy, and parallelism: `crates/leaven-run/src/evaluator.rs:19-35`.
- In `evaluate_job`, runner output is cloned into `ScoreContext`, score/cost are computed, and `output.output` is hard-lowered into `OutputRecord::inline(...)`: `crates/leaven-run/src/evaluator.rs:222-264`.

### Evidence, reports, and GEPA reflection are already text/blob lowered

- `CaseAssessmentEvidence` carries `score: ScalarEvidence`, `output: OutputRecord`, and `feedback: String`: `crates/leaven-evidence/src/feedback.rs:10-35`.
- `OutputRecord` is inline text or blob reference, not an arbitrary typed value: `crates/leaven-evidence/src/command.rs:11-38`.
- `leaven-run` reports flatten `OutputRecord` back to report text through `run_report::output_record_text(...)`; public `ReportScore.output` is a `String`: `crates/leaven-run/src/run_report.rs:276-294`, `crates/leaven-eval/src/report.rs:10-20`.
- GEPA reflection consumes rendered reflection examples, not typed runner output. `ReflectiveExample.output` is `Option<String>` and `feedback` is `String`: `crates/leaven-gepa/src/reflection.rs:45-58`.
- GEPA builds `ReflectRequest` once through `ReflectiveDatasetBuilder` and passes that request to any `GepaReflector`; reflectors do not re-derive evidence: `crates/leaven-gepa/src/reflection.rs:70-139`, `crates/leaven-gepa/src/proposer.rs:68-79`.
- The default `GepaReflectiveDataset` projects case input separately from evidence and uses `ReflectionProjection` for evidence-to-example lowering: `crates/leaven-gepa/src/reflection.rs:150-319`.
- `CasewiseEvidence<CaseAssessmentEvidence>` already projects output/score/feedback into reflection examples by rendering `OutputRecord` to text: `crates/leaven-gepa/src/reflection.rs:337-357`.
- GEPA scoring uses `GepaScoreEvidence` only for scalar comparison; `CasewiseEvidence<CaseAssessmentEvidence>` is already supported by projecting scalar scores and discarding output/feedback for population comparison: `crates/leaven-gepa/src/optimizer.rs:34-70`, `:889-930`.

### `leaven-run` local guidance

- `crates/leaven-run/AGENTS.md` says `Score` is ordinary builder evidence, not the universal evidence model, and generated output/feedback/cost must lower into typed evidence/report records rather than becoming scalar-only metadata: `crates/leaven-run/AGENTS.md:57-68`.
- The same guidance says `RunOutput` carries runner output and cost so solver/program LM calls, subprocesses, and agent runtimes can be charged through evaluation reports while generated outputs remain first-class evidence: `crates/leaven-run/AGENTS.md:64-73`.
- Ordinary runners receive `RunCase<I>`, not the durable `Case<I, T>` envelope; target/metadata isolation must stay intact: `crates/leaven-run/AGENTS.md:74-82`.
- `ScoreContext` already has known gaps, including generic output views; do not overstate current scorer context as complete GEPA-style context: `crates/leaven-run/AGENTS.md:78-82`.

### DSRs integration pressure

- Existing DSRs integration plan originally chose a custom evaluator because `leaven-run` was text-first and DSRs metrics need typed module output/prediction context: `docs/plans/dsrs-leaven-integration-2026-05-16.md:108-126`.
- That plan explicitly allowed a narrow generic Leaven change if implementation exposed a real generic seam gap: `docs/plans/dsrs-leaven-integration-2026-05-16.md:166-178`.
- DSRs `Module` returns typed `Predicted<M::Output>` and DSRs `TypedMetric` evaluates `&Example<S>` against `&Predicted<M::Output>`; those values should map to Leaven runner output and scorer context, not to pre-scoring text: DSRs loaded-root refs from scout were `DSRs/crates/dspy-rs/src/core/module.rs:56-80`, `DSRs/crates/dspy-rs/src/evaluate/evaluator.rs:73-82`, `DSRs/crates/dspy-rs/src/core/predicted.rs:169-202`.
- DSRs `Example<S> { input, output }` maps naturally to `leaven_eval::Case<S::Input, S::Output>`; Leaven runner receives only input through `RunCase<I>`, while scorer sees optional target through `ScoreCase<I, T>::target()`: `crates/leaven-eval/src/dataset.rs:12-39`, `crates/leaven-run/src/evidence.rs:133-227`.
- A local compiler experiment in the DSRs implementation worktree showed DSRs can change `Module` and `TypedMetric` from public `async fn` traits to `fn -> impl Future + Send`; existing `async fn` impls still compile, and `dsrs-leaven` can then implement Leaven's `Evaluator` directly. Narrow compile checks passed for `dsrs-core`, `dsrs-evaluate`, `dsrs-predict`, `dsrs-gepa`, `dsrs-data`, and `dsrs-leaven` with `--no-run`.

### Prior art and compatibility anchors

- Public tests use direct `ctx.output.output` access and assert `OutputRecord::inline(...)` / report strings; the current string route is a public contract, not an incidental implementation detail: examples include `crates/leaven-run/tests/scoring_evaluator.rs` and `crates/leaven-run/tests/optimize_builder.rs` per seam scout.
- `docs/plans/2026-05-15-gepa-aime-parity/requirements-summary.md` targets the public high-level builder path `leaven::optimize(...).train(...).validation(...).test(...).runner(...).score(...).using(Gepa...).budget(...).run()` with honest train/validation/test evidence.
- `examples/p8_aime_gepa/AGENTS.md` currently describes `run_solver` returning `RunOutput` with generated answer/cost and `score_answer` receiving hidden target through `ScoreContext`; this example is a key compatibility proof for preserving the string-output path.
- `docs/plans/2026-05-14-gepa-reflection-unification-design.md` chose typed `ReflectiveExample` records over Python GEPA's untyped dicts, kept case input projection out of durable `CaseAssessmentEvidence`, and moved reflection data selection into `ReflectiveDatasetBuilder`.

## Approach

Make `leaven-run`'s ordinary builder output-generic while preserving `String` as the default public contract. A typed runner returns `RunOutput<Out>`, the scorer receives `ScoreContext<..., Out>` so DSPy/DSRs-style typed predictions reach typed metric code intact, and only after scoring does `leaven-run` call an explicit output renderer to lower `Out` into the existing `OutputRecord` stored in `CaseAssessmentEvidence`.

The target programming model is DSPy GEPA's shape, translated into Leaven/Rust types:

```text
student/module prediction -> metric sees typed prediction -> score + feedback -> GEPA reflects over rendered output/feedback
```

For DSRs this means:

```text
Module::call(input) -> Predicted<M::Output>
TypedMetric::evaluate(&Example<S>, &Predicted<M::Output>)
render Predicted<M::Output> -> OutputRecord
leaven_gepa::Gepa consumes CasewiseEvidence<CaseAssessmentEvidence>
```

### Recommended API shape

Use one runner-output type parameter on the ordinary runner/scorer/evaluator path. Name it `Out` to avoid colliding with the existing `OptimizeBuilder` optimizer type parameter.

```rust
pub struct RunOutput<Out = String> {
    pub output: Out,
    pub cost: Cost,
}

pub struct ScoreContext<A, I, T = NoTarget, Out = String> {
    pub artifact: A,
    pub case: ScoreCase<I, T>,
    pub output: RunOutput<Out>,
    pub budget: BudgetSnapshot,
}
```

Keep the string route ergonomic and source-compatible:

- `RunOutput::new(...)` remains the string constructor for `RunOutput<String>`.
- `RunOutput<String>` remains the default in public docs, tests, examples, and preludes.
- Existing scorer code that reads `ctx.output.output` keeps working for the default path.
- `ReportScore.output` remains `String`.
- GEPA `ReflectiveExample.output` remains `Option<String>`.

Do **not** make `RunProblem` output-generic in the durable engine problem type. Keep:

```rust
RunProblem<A, I, T = NoTarget>
```

because durable evidence remains:

```rust
CasewiseEvidence<CaseAssessmentEvidence>
```

Typed output is an evaluator-local/scorer-local value. The durable boundary is still the rendered `OutputRecord` plus score and feedback.

### Builder renderer requirement

Current `OptimizeBuilder<A, I, T, O>` already uses `O` for the optimizer. Introduce the output type as a fifth parameter and name it `Out`:

```rust
OptimizeBuilder<A, I, T, Opt, Out = String>
```

When `.runner(...)` changes the runner output type, it consumes `self` and returns a re-parameterized builder:

```rust
fn runner<NextOut, F, Fut>(self, runner: F) -> OptimizeBuilder<A, I, T, Opt, NextOut>
where
    F: Fn(A, RunCase<I>) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = RunOutput<NextOut>> + Send + 'static,
    NextOut: Clone + Send + Sync + 'static;
```

Choose an explicit runtime renderer requirement for this slice, not a public type-state redesign. The default `String` route keeps the built-in renderer and remains runnable without ceremony. A non-`String` route must install a renderer before execution; otherwise `.run()` fails with `OptimizeError::MissingOutputRenderer`. This keeps the public path source-compatible while still making typed output without durable rendering an explicit error, not silent evidence loss.

### Output rendering seam

Add an explicit output-rendering seam owned by `leaven-run`:

```rust
render_output: Fn(&RunOutput<Out>) -> Result<OutputRecord, OutputRenderError>
```

Define `OutputRenderError` in `leaven-run` alongside `ScoreError`; rendering failure is an evaluation failure and should preserve already-incurred runner/scorer cost.

Expose two builder methods:

```rust
.render_output(renderer)
.render_output_with_fingerprint(fingerprint, renderer)
```

The second is the durable-mode anti-drift path and should be the documented route for custom typed renderers. The first is acceptable for ephemeral/debug runs under the same rules as closure runner/scorer fingerprints.

Default behavior for `String` is exactly today's behavior:

```rust
OutputRecord::inline(output.output.clone())
```

Typed-output users configure a renderer that turns their typed prediction into the text/blob evidence they want reports and GEPA reflection to see. The renderer must see only runner output, not scorer targets; target leakage remains controlled by the existing `RunCase` / `ScoreCase` split.

Recommended evaluation order:

```text
runner returns RunOutput<Out>
scorer receives cloned RunOutput<Out> in ScoreContext
if scoring succeeds:
  renderer lowers original RunOutput<Out> -> OutputRecord
  evaluator stores CaseAssessmentEvidence(score, output_record, feedback)
```

This preserves typed output through scoring and keeps evidence deliberately lowered. The evaluator requires `Out: Clone + Send + Sync + 'static` because it must both pass output to the scorer and retain/render the original output after scoring.

### Compatibility and caching

Rendering changes durable evidence, report output, GEPA reflection text, and evaluator-cache entries. Renderer identity therefore belongs in the evaluator/runtime compatibility layer, not in GEPA.

Add renderer identity beside runner/scorer identity:

- `RuntimeKind::OutputRenderer`
- an output-renderer fingerprint in `ScoringEvaluatorIdentity`
- an output-renderer entry in `RunCompatibilityManifest` / summary
- evaluator fingerprint mixing that includes output-renderer fingerprint

Rules:

- Built-in string renderer has a stable built-in fingerprint and requires no user ceremony.
- Durable custom renderers must provide a fingerprint via `.render_output_with_fingerprint(...)`.
- Ephemeral custom renderers may use an ephemeral renderer fingerprint.
- Missing renderer identity in an old manifest may be interpreted as the built-in string renderer only for compatibility checks; evaluator cache keys should still version/mix the new evaluator fingerprint so old cache entries are not reused unsafely.

### GEPA and reports stay lowered

GEPA should remain typed-output-unaware. It already consumes:

- scalar scores through `GepaScoreEvidence`
- output/feedback for reflection through `CasewiseEvidence<CaseAssessmentEvidence>` → `OutputRecord` → `ReflectiveExample.output: Option<String>`

The plan should not add `Out` to GEPA public types, `ReflectiveExample`, or `CaseAssessmentEvidence`. Typed output exists to make the runner/scorer contract faithful; durable evidence and reflection remain rendered.

## Work Items

### Item 1 — Add generic run output, typed score context, and renderer vocabulary

**Goal:** Make the public runner/scorer evidence types capable of carrying typed output and define the explicit lowering boundary into `OutputRecord`, while preserving the existing string API.

**Done when:**

- `RunOutput` becomes `RunOutput<Out = String>`.
- `RunOutput::new(...)` still constructs string output for `RunOutput<String>`.
- `RunOutput<String>::default()` preserves empty-string behavior.
- Add a typed constructor such as `RunOutput::typed(output)` or an equivalent unambiguous constructor for non-string `Out`.
- `ScoreContext` becomes output-generic with default `String`.
- Define `OutputRenderError` and the internal renderer closure type over `&RunOutput<Out>`.
- Existing tests using `ctx.output.output` still compile unchanged on the default route.
- A typed-output scorer test can name `ScoreContext<A, I, T, TypedPrediction>` and inspect typed fields without rendering first.

**Key files:**

- `crates/leaven-run/src/evidence.rs:7-29`, `:228-240`
- `crates/leaven-run/src/evaluator.rs:19-35`
- `crates/leaven-run/src/lib.rs:19-43`
- `crates/leaven-run/tests/scoring_evaluator.rs`
- `crates/leaven-run/tests/optimize_builder.rs`

**Dependencies:** None.

**Size:** Medium.

### Item 2 — Thread typed output and renderer state through `OptimizeBuilder`

**Goal:** Let the ordinary builder path carry the runner output type from `.runner(...)` into `.score(...)` and require a renderer before typed outputs can reach `.run()`, while keeping durable problem/evidence shape unchanged.

**Done when:**

- `OptimizeBuilder<A, I, T, Opt>` becomes `OptimizeBuilder<A, I, T, Opt, Out = String>` or an equivalent shape that does not confuse `Opt` and `Out`.
- Private `Runner<A, I>` becomes output-generic.
- Private `Scorer<A, I, T>` becomes output-generic.
- `.runner(...)` consumes `self` and returns a re-parameterized builder with `NextOut`.
- The built-in `String` path starts with a built-in renderer installed and remains runnable without new user calls.
- A non-`String` `.runner(...)` without a renderer is refused at `.run()` with `OptimizeError::MissingOutputRenderer`.
- `.render_output(...)` and `.render_output_with_fingerprint(...)` install the output renderer for `Out`.
- `.score(...)` receives `ScoreContext<A, I, T, Out>`.
- `.using(Gepa...)`, budget, storage, callbacks, train/validation/test behavior, and `RunProblem<A, I, T>` do not depend on output type.
- `RunProblem<A, I, T>` remains unchanged and still uses `CasewiseEvidence<CaseAssessmentEvidence>`.

**Key files:**

- `crates/leaven-run/src/builder.rs:41-44`, `:53-66`, `:95-204`, `:376-386`
- `crates/leaven-run/src/evaluator.rs:19-80`
- `crates/leaven-run/src/result.rs`
- `crates/leaven-run/tests/optimize_builder.rs`

**Dependencies:** Item 1.

**Size:** Large.

### Item 3 — Lower typed output in `ScoringEvaluator`

**Goal:** Preserve typed runner output through scoring, then lower it deliberately into `OutputRecord` for durable evidence.

**Done when:**

- `ScoringEvaluator` is output-generic and stores an output renderer for `RunOutput<Out>`.
- `evaluate_job` passes `RunOutput<Out>` to `ScoreContext<A, I, T, Out>` before rendering.
- `Out: Clone + Send + Sync + 'static` or a better ownership pattern is explicitly required and tested.
- The default string renderer produces the same `OutputRecord::inline(...)` as today.
- A custom typed renderer can lower a typed prediction to `OutputRecord`.
- Renderer receives only runner output, not scorer targets or full case envelopes.
- Rendering failure becomes an evaluation failure with already-incurred runner/scorer costs preserved in the error cost.
- Successful evidence remains `CaseAssessmentEvidence::new(score, output_record, feedback)`.

**Key files:**

- `crates/leaven-run/src/evaluator.rs:19-35`, `:222-264`
- `crates/leaven-run/src/evidence.rs`
- `crates/leaven-evidence/src/command.rs:11-38`
- `crates/leaven-evidence/src/feedback.rs:10-35`
- `crates/leaven-run/tests/scoring_evaluator.rs`

**Dependencies:** Items 1–2.

**Size:** Medium.

### Item 4 — Add renderer fingerprinting to durable compatibility and evaluator cache identity

**Goal:** Prevent durable resume/evaluator-cache reuse when rendered output behavior changes.

**Done when:**

- Add an output-renderer identity/fingerprint alongside runner and scorer identity.
- `RuntimeKind` includes `OutputRenderer` or equivalent classification.
- `ScoringEvaluatorIdentity` includes output-renderer fingerprint.
- Evaluator fingerprint changes when output-renderer fingerprint changes.
- `RunCompatibilityManifest` stores output-renderer fingerprint and reports it in summaries.
- Durable custom renderer without fingerprint is refused.
- Built-in string renderer requires no user ceremony.
- Tests prove renderer mismatch refuses resume.
- Tests prove changed renderer changes evaluator fingerprint/cache identity.

**Key files:**

- `crates/leaven-run/src/compatibility.rs`
- `crates/leaven-run/src/builder.rs:360-386`
- `crates/leaven-run/src/result.rs`
- `crates/leaven-run/tests/optimize_builder.rs:650-760`
- `crates/leaven-run/tests/scoring_evaluator.rs`

**Dependencies:** Items 2–3.

**Size:** Medium.

### Item 5 — Prove lowered reports and GEPA reflection stay typed-output-unaware

**Goal:** Confirm that reports and GEPA consume only rendered `OutputRecord` text while the scorer receives typed output.

**Done when:**

- `run_report::report_scores` remains based on `OutputRecord`.
- `ReportScore.output` remains `String`.
- `ReflectionProjection for CasewiseEvidence<CaseAssessmentEvidence>` remains based on `output_record_text`.
- No GEPA public type gains a runner-output type parameter.
- Tests prove a typed runner output is rendered into report text.
- Tests prove GEPA reflective examples include the rendered output text, not the typed prediction object.

**Key files:**

- `crates/leaven-run/src/run_report.rs:276-294`
- `crates/leaven-eval/src/report.rs:10-20`
- `crates/leaven-gepa/src/reflection.rs:45-58`, `:337-357`
- `crates/leaven-gepa/src/optimizer.rs:34-70`, `:889-930`
- `crates/leaven-run/tests/scoring_evaluator.rs`
- Existing or new `crates/leaven-gepa/tests/*`

**Dependencies:** Items 1–4.

**Size:** Small.

### Item 6 — Prove the public `runner -> score -> using(Gepa)` typed prediction path

**Goal:** Demonstrate the corrected DSPy-like programming model through the ordinary public path, not a custom evaluator.

**Done when:**

- A public-path test or small example defines a typed prediction struct with typed answer plus metadata.
- Runner returns `RunOutput<TypedPrediction>`.
- Scorer receives `ScoreContext<..., TypedPrediction>` and scores typed fields directly.
- Renderer lowers the typed prediction into `OutputRecord`.
- The chain uses `.using(Gepa...)` rather than a custom evaluator-only path.
- GEPA reflection receives rendered output text, not the typed prediction.
- Existing string P8/AIME behavior continues unchanged.
- Prefer adding a focused new test/example over mutating P8 unless P8 needs API updates for compatibility; P8 remains the string-output public AIME proof unless deliberately upgraded later.

**Key files:**

- `crates/leaven-run/tests/optimize_builder.rs`
- `crates/leaven-run/tests/scoring_evaluator.rs`
- `crates/leaven-gepa/tests/*`
- Optional new focused example under `examples/` if a test alone is too hidden
- `examples/p8_aime_gepa/src/main.rs` only if public API compatibility forces updates
- `examples/p8_aime_gepa/AGENTS.md` and `README.md` only if P8 behavior changes

**Dependencies:** Items 1–5.

**Size:** Medium.

### Item 7 — Update the DSRs/DSPy bridge plan to consume the public typed path

**Goal:** Align the DSRs bridge pressure with the new Leaven public path: DSRs is the Rust typed substrate, while the governing model is DSPy-style typed module prediction plus typed metric plus GEPA reflection over rendered text.

**Done when:**

- The DSRs integration plan no longer says `leaven-run` must be avoided because output is string-first.
- Bridge sketch maps:
  - `dsrs_core::Example<S>` → `leaven_eval::Case<S::Input, S::Output>`
  - `Module::call(...) -> Predicted<M::Output>` → `RunOutput<Predicted<M::Output>>` or an owned/shared wrapper chosen by implementation
  - `TypedMetric::evaluate(&Example<S>, &Predicted<M::Output>)` → `.score(...)`
  - output renderer → `OutputRecord` text/blob for reports and GEPA
- GEPA remains typed-output-unaware in the bridge design.
- The DSRs Send-future trait cutover remains a DSRs-side prerequisite for any direct Leaven evaluator/stage implementation, but the public `leaven-run` path is the preferred route once typed outputs land.

**Key files:**

- `docs/plans/dsrs-leaven-integration-2026-05-16.md:108-178`, `:220-255`
- `docs/plans/typed-run-output-2026-05-17.md`
- DSRs split-crate equivalents of `DSRs/crates/dspy-rs/src/core/module.rs`, `core/predicted.rs`, and `evaluate/evaluator.rs`
- Future `DSRs/crates/dsrs-leaven/*` after the split crate branch lands

**Dependencies:** Items 1–6.

**Size:** Medium.

### Item 8 — Documentation and verification cleanup

**Goal:** Make the new public contract clear and prevent future regressions back to string-only scoring.

**Done when:**

- `crates/leaven-run/AGENTS.md` mentions typed `RunOutput<Out>` and deliberate rendering into evidence.
- Public docs/examples still present string output as the default route.
- The plan records that typed outputs are not durable report payloads in this slice; only rendered `OutputRecord` is durable.
- Verification commands are recorded:
  - `cargo nextest run -p leaven-run --test scoring_evaluator --test optimize_builder`
  - relevant GEPA reflection tests
  - `just milestone-p8` if P8 example behavior changes
  - `just check` before completion

**Key files:**

- `docs/plans/typed-run-output-2026-05-17.md`
- `crates/leaven-run/AGENTS.md:57-82`
- `examples/p8_aime_gepa/README.md`
- `examples/p8_aime_gepa/AGENTS.md`

**Dependencies:** Items 1–7. Docs may be updated in the same commits as earlier items when a local contract changes; this item is the final consistency pass.

**Size:** Small.

## Open Questions

None blocking. The plan chooses:

- `RunOutput<Out = String>` and `ScoreContext<..., Out = String>` rather than a separate typed-output type.
- `RunProblem<A, I, T>` remains unchanged; typed output is evaluator/scorer-local and durable evidence stays `CasewiseEvidence<CaseAssessmentEvidence>`.
- Output rendering is explicit and fingerprinted; `.render_output_with_fingerprint(...)` is the durable custom-renderer path.
- Typed outputs are not durable report payloads in this slice; reports and GEPA consume rendered `OutputRecord` text.

Implementation decision: use `OptimizeError::MissingOutputRenderer` for typed-output builders that reach `.run()` without a renderer. A future type-state refinement is allowed only if it preserves the existing string route and keeps the builder surface readable.

## References

- `crates/leaven-run/src/evidence.rs`
- `crates/leaven-run/src/evaluator.rs`
- `crates/leaven-run/src/builder.rs`
- `crates/leaven-run/AGENTS.md`
- `crates/leaven-evidence/src/feedback.rs`
- `crates/leaven-evidence/src/command.rs`
- `crates/leaven-gepa/src/reflection.rs`
- `crates/leaven-gepa/src/proposer.rs`
- `crates/leaven-gepa/src/optimizer.rs`
- `docs/plans/dsrs-leaven-integration-2026-05-16.md`
- `docs/plans/2026-05-15-gepa-aime-parity/requirements-summary.md`
- `docs/plans/2026-05-14-gepa-reflection-unification-design.md`
- `examples/p8_aime_gepa/AGENTS.md`
