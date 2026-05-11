## Boundary
This crate owns the ordinary product-builder path: `optimize(seed)`, train /
validation / test inputs, runner and scoring helpers, default evidence-store
wiring, optimize errors, result facades, and public reports for typical users.

It composes the engine and standard vocabulary. It must not become a shortcut
around engine graph mutation or a home for optimizer strategy state.

## Route Here
- Builder ergonomics belong here: required budget policy, train vs held-out
  validation/test rules, callbacks, store selection, runner/scorer wiring, and
  `OptimizeResult` facades.
- `RunProblem`, `RunOutput`, `Score`, `ScoreContext`, `ScoringEvaluator`,
  `OptimizeStore`, and `IntoOptimizeStore` belong here when they serve the
  high-level optimize workflow.
- Default evidence-only and durable store composition belongs here when it is
  product wiring over store capabilities, not a new backend.
- The current lowering stack is local product glue: builder input vectors become
  `Dataset`/`DatasetSplits`, engine `CaseSet` partitions named `TRAIN`,
  `VALIDATION`, and `TEST`, a `ScoringEvaluator`, an `Engine` with
  validation/test hidden from proposers, final baseline/best evaluations, and a
  flattened `OptimizationReport`.

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
  rejection, callbacks, supplied store capabilities, no-best error mapping, and
  default optimize flow.
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
- Current runner/scorer helpers are sync and scalar-leaning. Treat that as the
  present implementation state, not the final Layer 1 contract for async
  runner/scorer/evaluator roles, cache policy, or richer evidence/reporting.
- The current builder fixes case identity by vector position via
  `CaseId::from_index` and uses literal `TRAIN`/`VALIDATION`/`TEST` partitions.
  That is acceptable for today's proof path, but stable user-provided case IDs,
  duplicate-id rejection, and split-use lowering belong in the next product
  surface rather than more ad hoc positional logic here.
- `ScoringEvaluator::cache_policy` returns `CachePolicy::Never`. Do not teach
  users to get solver/judge/reflector caching by editing this evaluator; Layer 1
  needs role-level runtime/cache policy while LM response cache capabilities
  stay in `leaven-lm-cache`.

## Decision Cards
- when: extending `optimize(seed)` user ergonomics
  do: keep the public path in this crate and lower into engine/eval/store seams deliberately
  preserve: required budget, hidden validation/test defaults, callback/store wiring, and result facades that do not expose raw `RunGraph`
  avoid: putting GEPA strategy knobs, engine graph shortcuts, provider clients, or dataset execution machinery into the builder just because P8 needs them
  verify: run `cargo nextest run -p leaven-run --test optimize_builder`

- when: adding async runner/scorer, rich scoring, stable cases, or single-task mode
  do: hard-cut the builder/evaluator/report path together instead of adding parallel sync-vs-async product APIs
  preserve: one ordinary lowering route into `ScoringEvaluator` or its replacement, with typed errors and metered cost rather than score-zero fallbacks
  avoid: treating `RunOutput { output, trace }` plus scalar `Score` as the final evidence model
  verify: run `cargo nextest run -p leaven-run --test scoring_evaluator --test optimize_builder`, then the affected product example

- when: moving evaluation planning out of this crate
  do: promote reusable dataset/split/request/report vocabulary into `leaven-eval` while keeping actual execution here or in engine
  preserve: `leaven-run` as product-builder composition, not the permanent hidden home for all evaluation lowering
  avoid: duplicating split-use/trust policy inside GEPA or examples
  verify: run `cargo nextest run -p leaven-eval -p leaven-run`
