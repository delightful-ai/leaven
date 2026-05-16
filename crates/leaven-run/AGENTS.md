## Boundary
This crate owns the ordinary product-builder path: `optimize(seed)`, train /
validation / test inputs, runner and scoring helpers, default evidence-store
wiring, optimize errors, result facades, and public reports for typical users.

It composes the engine and standard vocabulary. It must not become a shortcut
around engine graph mutation or a home for optimizer strategy state.

## Route Here
- Builder ergonomics belong here: required budget policy, train vs held-out
  validation/test rules, callbacks, store selection, runner/scorer wiring, and
  `Optimized` facades.
- Ordinary durability belongs here: omitted `.store(...)` opens a managed local
  `.leaven/runs/<run-id>/` directory, `.run_dir(path)` is the user-facing
  durable override/resume handle, and `.ephemeral()` is the explicit
  throwaway path.
- `RunProblem`, `RunCase`, `RunOutput`, `Score`, `ScoreCase`, `ScoreContext`,
  `ScoringEvaluator`, `OptimizeStore`, and `IntoOptimizeStore` belong here when
  they serve the high-level optimize workflow.
- Default evidence-only and durable store composition belongs here when it is
  product wiring over store capabilities, not a new backend.
- The current lowering stack is local product glue: builder input vectors become
  `Dataset`/`DatasetSplits`, engine `CaseSet` partitions named `TRAIN`,
  `VALIDATION`, and `TEST`, a `ScoringEvaluator`, an `Engine` with
  validation/test hidden from proposers, final baseline/best evaluations, and a
  concrete `StandardRunSummary`.

## Route Away
- Graph records, checkpoint envelopes, budget ledgers, trust, cache,
  `RunContext`, and stage traits belong in `leaven-engine`.
- Dataset/split/report vocabulary that is reusable below the builder belongs
  in `leaven-eval`.
- Evidence shapes belong in `leaven-evidence`; store capabilities and concrete
  backends belong in `leaven-store` and `leaven-store-*`.
- Optimizer implementations and strategy state belong in optimizer crates.
  This crate accepts an optimizer; it does not own search policy.

## Proof Anchors
- `tests/optimize_builder.rs` proves required budget policy, held-out case
  rejection, callbacks, supplied store capabilities, no-best error mapping,
  default durable local storage, explicit ephemeral mode, and `run_dir` resume.
- `tests/scoring_evaluator.rs` proves runner/scorer evaluation shape,
  per-case granularity requirements, independent-request requirements,
  missing input errors, finite score refusal, and cost reporting.
- `cargo nextest run -p leaven-run` proves the product-builder contract.
- `cargo test -p leaven --test topology_contract` proves this crate still
  composes engine/eval/evidence/store without absorbing their ownership.

## Local Bait
- `optimize` exists in both `leaven-engine` and `leaven-run`. Engine's builder
  configures execution machinery; run's builder is the user-facing product
  workflow. Put ordinary user ergonomics here.
- Do not expose `RunGraph` or private engine records through result facades to
  make tests convenient. Add an engine view/report if the public contract needs
  that fact.
- `Score` is builder evidence for the runner/scorer path, not the universal
  evidence model. Reusable evidence types belong in `leaven-evidence`.
- `Score` is the ordinary user word; assessment/evidence/preference are the
  durable internal truth. Do not make `.score(...)` a scalar-only dead end:
  generated output, feedback, absent scores, failed evidence, and metric axes
  must lower into typed evidence/report records instead of becoming zero or
  string metadata.
- Runner helpers are async and bounded-concurrent. `RunOutput` carries runner
  output and cost so solver/program LM calls, subprocesses, and agent runtimes
  can be charged through evaluation reports while generated outputs remain
  first-class evidence.
  Scoring is also async and fallible: `.score(...)` receives owned
  `ScoreContext` values, returns `Result<Score, ScoreError>`, and may attach
  scorer cost. Treat scalar comparison as the current selection contract, not as
  permission to drop generated outputs, runtime failures, or metered provider
  work.
- Ordinary runners receive `RunCase<I>`, not the durable `Case<I, T>` envelope.
  That is the structural target/metadata isolation boundary: do not add target,
  metadata, or raw case envelope access to the ordinary runner signature.
- `ScoreContext` exposes a budget snapshot and `ScoreCase<I, T>` with case id,
  input, optional target, and an empty scorer metadata projection. It still does
  not expose the full spec target: score-on-error, generic output views, score
  history, or selected scorer metadata projection. Do not describe this slice as
  complete GEPA-style scorer context until those gaps are closed.
- The builder's canonical `.train` / `.validation` / `.test` path now accepts
  `leaven_eval::Case<I, T>` envelopes and preserves caller-provided case IDs in
  datasets, scoring evidence, and reports. `.train_inputs`,
  `.validation_inputs`, and `.test_inputs` are input-only toy conveniences that
  lower to `Case<I, NoTarget>` with dense generated IDs. The builder installs
  engine `CaseSet` entries with the case envelope IDs, so resolved-set IDs,
  assessment targets, scoring evidence, and reports cite the same product case
  identity.
- Durable local runs write a product-layer compatibility manifest before work
  and compare it before resume. Arbitrary closure runners/scorers are not
  introspectable; `.runner_fingerprint(...)` and `.scorer_fingerprint(...)`
  are the current explicit declarations for durable mode. Ephemeral runs may
  omit them. Cache, budget, LM-role, and optimizer compatibility still contain
  narrow placeholders except where engine optimizer checkpoint validation
  already owns the concrete refusal.
- `OptimizeBuilder` defaults evaluation caching to automatic deterministic
  candidate/case caching for ordinary durable runs. Explicit
  `.evaluation_cache_policy(CachePolicy::Never)` is the throwaway/debug path.
  Do not teach users to get solver/judge/reflector response caching by editing
  `ScoringEvaluator`; role-level LM response cache capabilities stay in
  `leaven-lm-cache`.
- Single-task/no-dataset optimization is a missing Layer 1 product mode, not a
  reason to fake a one-row training set. If a task has no dataset, model that
  as an explicit product mode and report shape.
- Domain environments belong in domain adapters or workspace/runtime seams.
  Do not add a hidden `.environment(...)` abstraction here that absorbs Python,
  CUDA, LM, or agent execution details.

## Decision Cards
- when: extending `optimize(seed)` user ergonomics
  do: keep the public path in this crate and lower into engine/eval/store seams deliberately
  preserve: required budget, durable-by-default local run storage, explicit ephemeral opt-out, hidden validation/test defaults, callback/store wiring, and result facades that do not expose raw `RunGraph`
  avoid: putting GEPA strategy knobs, engine graph shortcuts, provider clients, or dataset execution machinery into the builder just because P8 needs them
  verify: run `cargo nextest run -p leaven-run --test optimize_builder`

- when: adding scorer async/failure support, rich scoring, stable cases, or single-task mode
  do: hard-cut the builder/evaluator/report path together instead of adding parallel simple-vs-rich product APIs
  preserve: one ordinary lowering route into `ScoringEvaluator` or its replacement, with typed errors and metered cost rather than score-zero fallbacks
  avoid: treating scalar-only `Score` as the final evidence model or smuggling generated output through trace/report strings
  verify: run `cargo nextest run -p leaven-run --test scoring_evaluator --test optimize_builder`, then the affected product example

- when: moving evaluation planning out of this crate
  do: promote reusable dataset/split/request/report vocabulary into `leaven-eval` while keeping actual execution here or in engine
  preserve: `leaven-run` as product-builder composition, not the permanent hidden home for all evaluation lowering
  avoid: duplicating split-use/trust policy inside GEPA or examples
  verify: run `cargo nextest run -p leaven-eval -p leaven-run`
