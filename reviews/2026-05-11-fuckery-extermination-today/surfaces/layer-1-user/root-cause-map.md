# Layer 1 Root-Cause Map

Status: canonical Layer 1 audit doc.

Scope: ordinary users who want to run a real optimizer over an LM, program, or
agent workload without learning engine internals.

## Summary

Layer 1 is not failing because one adapter is missing. It is failing because the
ordinary-user contract has repeatedly been substituted with nearby lower-level
or fixture-shaped proof:

- a builder shell substitutes for a complete optimizer product surface;
- an empty unit-case path substitutes for single-task/no-dataset work;
- sync callbacks substitute for async execution and scoring;
- raw vectors substitute for stable cases and split semantics;
- scalar strings substitute for score/evidence/feedback truth;
- cache wrappers and provider scripts substitute for runtime roles;
- fixed edits substitute for reflective mutation;
- aggregate result fields substitute for graph-backed public reports;
- broad umbrella/prelude exports substitute for layer-specific import surfaces.

The governing specs already state the intended shape: end users should get a
short path and should not have to understand every internal trait
(`docs/specs/initial_library.md:453-468`); ordinary Layer 1 users should touch
seed/program, train or one unscoped task, optional validation/test, scorer,
runner, GEPA, budget, result/report, and should not touch `RunGraph`,
`TrustPolicy`, `EvaluationRequest`, `Population`, selectors, or `EvidenceStore`
(`docs/specs/gepa_public_private_surface.md:51-83`).

## RC-L1-001: Layer Boundaries Collapsed Into The Default Import Story

- severity: high
- surface: `leaven` umbrella crate and ordinary prelude
- ideal contract: Layer 1 imports should teach `optimize`, cases/tasks, runner,
  score/evaluator, optimizer value, budget, runtime/cache roles, and result
  facades. Engine-author and GEPA-customizer machinery remains available through
  explicit advanced/engine/GEPA/cache paths.
- current implementation: `leaven::prelude::*` re-exports `EvaluationRequest`,
  `EvaluationSet`, `Proposal`, engine `CachePolicy`, `Engine`, `Evaluator`,
  `Optimizer`, `Population`, `Proposer`, `RunContext`, `RunGraphView`, and
  `TrustPolicy` beside `optimize`, `RunOutput`, `Score`, and `ScoreContext`
  (`crates/leaven/src/prelude.rs:3-25`). The root umbrella also publicly
  re-exports the same lower-level names (`crates/leaven/src/lib.rs:16-39`), while
  default features enable `std`, `derive`, and `gepa` (`crates/leaven/Cargo.toml:38-42`).
- blocker/gap: ordinary users are told not to learn engine internals, but the
  first import surface puts those internals in their hands as common names.
- user impact: future examples and implementors will keep solving Layer 1 tasks
  by reaching into graph/trust/evaluation machinery instead of improving the
  product builder. This makes the API feel like a bag of traits, not an
  optimizer library.
- correction direction: hard-cut `leaven::prelude::*` to ordinary Layer 1 names.
  Move `RunContext`, `RunGraphView`, `TrustPolicy`, `EvaluationRequest`,
  `Population`, `Proposer`, `Evaluator`, `CachePolicy`, cache stores/keys, and
  GEPA strategy slots to explicit advanced, engine, GEPA, or cache preludes.
- required proof/tests: a public import contract test that compiles a Layer 1
  example using only the ordinary prelude and rejects/default-deny-lists engine
  author names from that prelude. Keep topology tests for crate edges, but add a
  public-maturity gate because topology alignment alone does not prove product
  alignment.

## RC-L1-002: The Builder Shape Exists, But Its State Machine Is Too Narrow

- severity: blocker
- surface: `leaven-run::optimize`
- ideal contract: the builder supports `optimize(seed)`, `.train` or `.cases`,
  single-task/no-dataset mode, optional `.validation`, optional `.test`,
  `.runner`, `.score` or `.evaluator`, `.using`, `.budget`, `.store`,
  `.resume`, `.on_event`, and `.run`, with pre-run typed refusals for missing or
  contradictory inputs (`docs/specs/gepa_public_private_surface.md:641-674`).
- current implementation: `optimize(seed)` initializes empty train/validation/test
  vectors, a no-op default runner, no scorer, no optimizer, no budget, and no
  store (`crates/leaven-run/src/builder.rs:54-71`). The only type-fixing method
  for non-unit work after `optimize(seed)` is `.train(Vec<C>)`
  (`crates/leaven-run/src/builder.rs:92-114`). The default `C = ()` path can
  still accept `.score(...)`, `.using(...)`, and `.run()`, but `run()` lowers
  zero train/validation/test cases into `Dataset::from_ordered(Vec::new())` and
  an empty `CaseSet`, not into an unscoped or singleton task
  (`crates/leaven-run/src/builder.rs:208-222`). The implemented public methods
  are `.validation`, `.test`, `.runner`, `.score`, `.using`, `.budget`,
  `.on_event`, `.store`, and `.run`
  (`crates/leaven-run/src/builder.rs:117-198`). There is no `.cases`,
  `.task`, `.evaluator`, `.resume`, runtime/cache role configuration, or typed
  single-task entry.
- blocker/gap: spec mode inference says no train/validation/test means
  single-task (`docs/specs/gepa_public_private_surface.md:722-734`), and the core
  design includes `EvaluationSet::Unscoped` for single-task/evaluator-internal
  work (`docs/specs/initial_library.md:1092-1108`). The public builder cannot
  express that ordinary mode. An empty case set is not the same contract as
  "evaluate this one unscoped task."
- user impact: a user with a single benchmark, live environment, human eval, or
  agent task must fake a one-item training set or leave the public API. This is a
  product-contract failure, not a convenience gap.
- correction direction: add a first-class single-task/no-dataset work path and
  stabilize the builder as one public state machine. Do not keep old and new
  ordinary paths in parallel.
- required proof/tests: Layer 1 scenario tests for single-task optimization,
  train-only multi-task optimization, train+validation+test generalization, and
  pre-run typed refusals for missing scorer/evaluator, missing budget, held-out
  cases without train in default GEPA mode, duplicate case ids, invalid budgets,
  and resume fingerprint mismatch. Missing optimizer must be proven either as an
  intentional typestate compile-fail contract or as a typed pre-run refusal; do
  not leave it as an accidental method-resolution error.

## RC-L1-003: Execution And Scoring Are Sync Closures Instead Of A Real Run Contract

- severity: blocker
- surface: runner/scorer API and scoring evaluator adapter
- ideal contract: candidate execution and scoring are async, result-bearing,
  metered, concurrency-safe contracts. The spec's `CandidateRunner` returns a
  `Future<Output = Result<CandidateRun<O>, CandidateRunError<O>>>`, and the
  public `Scorer` returns a `Future<Output = Result<Metered<Score>, ScoreError>>`
  (`docs/specs/gepa_public_private_surface.md:987-1039`).
- current implementation: `Runner<A, C>` is a synchronous
  `Fn(&A, &C) -> RunOutput`, and `Scorer<A, C>` is a synchronous
  `Fn(ScoreContext) -> Score` (`crates/leaven-run/src/builder.rs:28-29`;
  `crates/leaven-run/src/evaluator.rs:15-16`). `.runner(...)` and `.score(...)`
  accept only those sync closures (`crates/leaven-run/src/builder.rs:136-153`).
  The evaluator calls both closures inline while already inside an async
  evaluator (`crates/leaven-run/src/evaluator.rs:65-102`).
- blocker/gap: the original Rust library standard says async by default
  (`docs/specs/initial_library.md:511-527`), and evaluators are expected to cover
  deterministic functions, LM judges, human judges, subprocess runners, agentic
  sandboxes, compiler/profiler harnesses, pairwise judges, and listwise rankers
  (`docs/specs/initial_library.md:2128-2165`).
- user impact: real LM calls, model judges, subprocesses, sandboxes, and agents
  force hidden runtimes, blocking, shelling out, or bypassing Leaven before the
  user has even expressed the normal workload.
- correction direction: make the async `CandidateRunner` plus async `Scorer` the
  only semantic Layer 1 lowering path. Sync/scalar conveniences may exist only as
  adapters into the same path, not as a second product contract.
- required proof/tests: scenario tests that run an async mocked LM/program
  runner, async model-judge scorer, metered scorer cost, runner failure,
  score-on-error policy, and bounded concurrent case evaluation without
  `block_on` inside user callbacks.

## RC-L1-004: Score And Feedback Collapse Into Scalars And Strings

- severity: high
- surface: `RunOutput`, `Score`, `ScoreContext`, report evidence projection
- ideal contract: `.score(...)` is the ordinary public concept. It should receive
  a typed `ScoreContext` view with candidate, optional case, output, run error,
  trace, bounded history, and budget accessors, and return a rich `Score` with a
  primary comparable score, metrics, natural-language feedback, structured
  feedback, attachments, metadata, and typed errors
  (`docs/specs/gepa_public_private_surface.md:894-985`;
  `docs/specs/gepa_public_private_surface.md:1029-1081`;
  `docs/specs/gepa_public_private_surface.md:1115-1182`).
- current implementation: `RunOutput` is just `output: String` plus
  `trace: Vec<String>` (`crates/leaven-run/src/evidence.rs:3-20`). `Score` is
  `value: f64`, one feedback string, and `Vec<(String, String)>`
  (`crates/leaven-run/src/evidence.rs:23-43`). `ScoreContext` exposes public
  fields for artifact, case, and output only (`crates/leaven-run/src/evidence.rs:46-54`).
  The evaluator appends structured feedback to trace strings and records
  `ScoredFeedbackEvidence` (`crates/leaven-run/src/evaluator.rs:103-116`).
- blocker/gap: the eval lowering spec says score normalization must preserve
  primary score, metrics, feedback refs, attachments, metadata, and unscored
  diagnostics until a report projection is explicitly chosen
  (`docs/specs/eval_lowering_detail.md:312-342`).
- user impact: model-judge rationale, compiler diagnostics, agent transcripts,
  files, screenshots, JSON, verifier logs, and rich trace evidence either become
  plain text or live outside Leaven. Reflection cannot learn from evidence that
  never entered the graph as typed evidence.
- correction direction: hard-cut `.score(...)` to the rich `Score` contract. Keep
  scalar and bool returns as `IntoScore` conveniences that normalize into the
  same evidence path. Do not add a separate `.score_with_feedback(...)`
  (`docs/specs/gepa_public_private_surface.md:1232-1245`).
- required proof/tests: law/example tests for finite score validation, unscored
  diagnostics, metered score cost, attachment staging failures, metadata not
  becoming optimizer decision axes, score errors not becoming `0.0`, and reports
  citing staged evidence refs rather than runtime paths.

## RC-L1-005: Dataset Semantics Are Positional Instead Of User-Stable

- severity: high
- surface: train/validation/test lowering and report investigation
- ideal contract: cases are work items, not necessarily labels or datasets.
  Public cases have stable ids, optional targets, metadata, disjoint split
  defaults, duplicate rejection, and final-test-only default semantics
  (`docs/specs/gepa_public_private_surface.md:773-820`).
- current implementation: `leaven-eval` already has `Case`, `NoTarget`,
  `Dataset::builder().case(...)`, and duplicate-id rejection
  (`crates/leaven-eval/src/dataset.rs:9-24`;
  `crates/leaven-eval/src/dataset.rs:95-100`), but Layer 1 does not re-export or
  use that path. `run()` concatenates split vectors, builds
  `Dataset::from_ordered(...)`, derives splits from lengths, and builds a
  `CaseSet` (`crates/leaven-run/src/builder.rs:214-222`). Case ids are dense
  positions generated by `CaseId::from_index` (`crates/leaven-run/src/builder.rs:302-356`).
- blocker/gap: product builders accepting `.train`, `.validation`, `.test`,
  `.cases`, or `.evaluation_suite` must construct stable dataset/split identity,
  reject duplicate ids, default to disjoint splits, fingerprint inputs, and lower
  split-use intent into engine trust policy (`docs/specs/eval_lowering_detail.md:650-673`).
- user impact: reports cannot reliably answer which original case improved,
  regressed, duplicated, or crossed splits. Reproduction, cache identity, paper
  benchmark comparison, and debugging all become position-dependent.
- correction direction: introduce the public `Case<I, T = NoTarget>` /
  case-suite path and make it the real example path. Raw `Vec<C>` can remain only
  as an explicit dense-id convenience that advertises its identity tradeoff.
- required proof/tests: law tests for duplicate case rejection, stable
  fingerprints, split overlap refusal, missing case refusal, and scenario tests
  proving builder `.train/.validation/.test` yields stable `TRAIN`,
  `VALIDATION`, and `TEST` partitions with hidden validation/test content.

## RC-L1-006: Runtime And Cache Are Pieces, Not An Ordinary Role Story

- severity: high
- surface: LM/runtime/cache ergonomics
- ideal contract: ordinary runs can configure solver/program LM, reflector LM,
  scorer/model judge, and agent runtime roles with independent provider, cache,
  and budget policy. Low-level cache stores and keys are advanced pieces.
- current implementation: `leaven-lm` has the provider-neutral async `Lm` trait
  (`crates/leaven-lm/src/model.rs:9-22`), `leaven-lm-openai` implements
  `OpenAiLm` (`crates/leaven-lm-openai/src/client.rs:10-37`), and
  `leaven-lm-cache` exposes `CachedLm` plus cache policy/store/key types
  (`crates/leaven-lm-cache/src/lib.rs:9-19`). The LM spec itself teaches ordinary
  code to construct `OpenAiLm::from_env(...)` and wrap it with
  `CachedLm::read_write(...)` (`docs/specs/lm_runtime_and_response_cache.md:15-31`).
  Meanwhile the Layer 1 scoring evaluator hard-codes `CachePolicy::Never`
  (`crates/leaven-run/src/evaluator.rs:61-63`). `OpenAiLm::from_env` accepts a
  `default_model` for "fingerprint stability" but ignores it, while each
  `LmRequest` carries its own model (`crates/leaven-lm-openai/src/client.rs:27-37`;
  `crates/leaven-lm-openai/src/client.rs:44-47`).
- blocker/gap: the LM response cache and engine evaluation cache are deliberately
  separate (`docs/specs/lm_runtime_and_response_cache.md:54-57`), but Layer 1 has
  no role-based place to configure either for solver, reflector, or scorer work.
  It also has no ordinary runtime identity contract that says which provider,
  model, cache policy, and budget/cost policy belong to each role.
- user impact: users see cache implementation types but cannot configure the run
  they actually asked for. Expensive LM/agent optimizer runs re-spend by default
  or bypass Leaven to manage caching externally.
- correction direction: add an ordinary runtime/cache role surface and keep
  `CachedLm`, cache stores, cache keys, and backend selection in explicit
  advanced/cache APIs. Do not teach wrapper stacking in the canonical Layer 1
  example.
- required proof/tests: cache-policy contract tests remain in `leaven-lm-cache`
  (`docs/specs/lm_runtime_and_response_cache.md:237-252`), plus Layer 1 scenario
  tests proving solver, reflector, and scorer/model-judge roles can use cached
  mocked LM calls, report hit/miss/cost summaries, and swap to OpenAI by provider
  construction rather than example architecture changes. Provider-runtime tests
  must prove model identity/fingerprint behavior instead of accepting an ignored
  "default model" argument as ordinary semantics.

## RC-L1-007: The Canonical Example Proves A Proxy

- severity: blocker
- surface: `examples/p8_aime_gepa`
- ideal contract: product examples prove the public surface they claim. For GEPA,
  the example must use an actual reflector/proposer that consumes selected
  feedback/evidence/trace context and must route live LM calls through Leaven LM
  and cache surfaces.
- current implementation: the AIME example uses the desired builder shell
  (`examples/p8_aime_gepa/src/main.rs:75-99`) but installs
  `ReflectiveMutation::new(AimePromptEdit::ReplaceSystem(...))`
  (`examples/p8_aime_gepa/src/main.rs:81-94`). `ReflectiveMutation` is documented
  as a deterministic fixture, stores one edit, ignores artifact/surface/part, and
  returns the stored edit (`crates/leaven-gepa/src/proposer.rs:21-47`). Live
  OpenAI solver mode switches on `LEAVEN_AIME_LIVE_OPENAI` and shells out to a
  Python script (`examples/p8_aime_gepa/src/main.rs:271-301`), whose script calls
  the OpenAI Responses API directly with `urllib`
  (`examples/p8_aime_gepa/scripts/openai_solver.py:24-45`). The example depends on
  `leaven` with `std` and `gepa`, not `leaven-lm-openai` or `leaven-lm-cache`
  (`examples/p8_aime_gepa/Cargo.toml:12-16`).
- blocker/gap: the GEPA spec requires reflection to capture feedback assessment
  IDs, propose edits, record causal and `informed_by` provenance, and preserve
  train/validation/test policy (`docs/specs/gepa_optimizer_surface.md:322-357`).
  Reflector inputs include selected part, assessment IDs, casewise evidence,
  attribution, objective/background, transcript refs, validation/apply errors,
  and candidate summaries (`docs/specs/gepa_optimizer_surface.md:463-483`).
- user impact: the example can show scores moving from 0 to 1
  (`examples/p8_aime_gepa/src/main.rs:398-409`) while no real reflection,
  provider-neutral LM, response cache, or runtime-role policy has been proven.
- correction direction: quarantine fixed edits as fixtures and reserve
  `ReflectiveMutation` for real async evidence-aware reflection. Route AIME mock
  and live solver/reflector work through the same Leaven runtime/LM/cache
  surfaces.
- required proof/tests: the canonical product proof must include a mock-LM or
  mock-agent reflector that reads scored feedback and produces an edit, plus a
  live-provider smoke whose only change from mock is provider construction. The
  coverage gate currently runs `p8_aime_gepa` (`scripts/coverage-gate.py:13-24`);
  that gate must stop ratifying proxy proof as product proof.

## RC-L1-008: Result Reporting Copies A Thin Snapshot Instead Of Exposing Run Truth

- severity: high
- surface: `OptimizeResult` / `OptimizationReport`
- ideal contract: the ordinary completed-run handle exposes optional best,
  stop reason, budget, graph-backed report, public events, optional GEPA summary,
  split/case outcomes, cost/cache summaries, evidence refs, and final-test
  semantics without requiring users to learn `RunGraph`
  (`docs/specs/gepa_public_private_surface.md:1184-1228`).
- current implementation: the engine already returns `RunResult { best:
  Option<CandidateId> }` and emits `StopReason` in events
  (`crates/leaven-engine/src/engine.rs:117-184`;
  `crates/leaven-engine/src/engine.rs:306-309`;
  `crates/leaven-engine/src/events.rs:16-23`). `OptimizeStore` can hold optional
  checkpoint persistence (`crates/leaven-run/src/store.rs:10-17`;
  `crates/leaven-run/src/store.rs:47-75`), but the Layer 1 builder has no
  `.resume(...)` method. `OptimizeResult` requires a `CandidateId` best, cloned
  best artifact, seed artifact, and `OptimizationReport`
  (`crates/leaven-run/src/result.rs:6-18`). `OptimizationReport` carries dataset
  and split fingerprints, aggregate train/validation/test scores, one
  `EvaluationReport`, and event names as strings (`crates/leaven-run/src/result.rs:35-61`).
  Report construction uses `.unwrap_or(0.0)` for missing train averages, clones
  the best artifact, and collects event names as strings
  (`crates/leaven-run/src/builder.rs:439-491`). Empty averages return `0.0`
  (`crates/leaven-run/src/result.rs:64-71`).
- blocker/gap: the original engine/report principle is that reports point at the
  graph and do not duplicate it (`docs/specs/initial_library.md:1988-1997`), and
  eval reports should cite graph refs/evidence refs and state final-test-only
  semantics (`docs/specs/eval_lowering_detail.md:780-789`). Layer 1 is also
  dropping engine-owned stop/best optionality and durable resume truth at the
  product facade.
- user impact: a run can complete without telling an ordinary user why it
  stopped, whether evidence was missing, which cases changed, whether test
  affected selection, what was cached, what proposals were tried, or where the
  supporting evidence lives.
- correction direction: make the result facade graph-backed and truth-preserving.
  Missing/failed scores must remain absent/errors, never numeric zero.
- required proof/tests: result scenario tests for optional best/no comparable
  score, stop reason, budget exhaustion, final-test-only reporting, missing
  evidence, public event summaries, case-level deltas, cache/cost summaries, and
  no ordinary `RunGraph` requirement.
