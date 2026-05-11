# Agent Report: Layer 1 Ordinary User Surface

Date: 2026-05-11

Auditor: Codex

Scope:

- `crates/leaven`
- `crates/leaven-run`
- `crates/leaven-lm`
- `crates/leaven-lm-cache`
- `examples`
- `scripts`
- relevant `docs/specs`

Question audited:

Can an ordinary user run a real optimizer over a real LM, agent, or program with
train/validation/test data, a score function with natural-language feedback,
cache policy, and an honest result facade, without learning engine internals or
relying on examples that bypass Leaven?

Answer:

No. Layer 1 has the outline of the desired public builder, but it is not yet an
honest off-the-shelf optimizer surface. The public path can run a deterministic
GEPA-shaped example and can produce report fields, but the hard parts are either
missing, synchronous, fixture-backed, string-shaped, or routed around Leaven's LM
and cache crates. The gap is not one missing adapter. It is a mismatch between
what the ordinary surface appears to prove and what the lower-level code actually
exercises.

The intended ordinary shape is already documented:

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

Spec evidence:

- `docs/specs/gepa_optimizer_surface.md:56-70` shows the ordinary
  `leaven::optimize(...).train(...).validation(...).test(...).runner(...).score(...).using(...).run()`
  path.
- `docs/specs/gepa_public_private_surface.md:51-83` says Layer 1 users should
  touch seed, train/validation/test cases, scoring/evaluator, runner, GEPA,
  budget, and result/report, and should not touch `RunGraph`, `TrustPolicy`,
  `EvaluationRequest`, `Population`, selectors, or `EvidenceStore`.
- `docs/specs/gepa_public_private_surface.md:138-154` shows async scoring over
  an agent trace.
- `docs/specs/gepa_public_private_surface.md:892-908` makes the public scoring
  function the ordinary concept and shows natural-language feedback, metrics,
  trace evidence, and metadata.
- `docs/specs/gepa_public_private_surface.md:1115-1182` says `Score` carries
  primary comparable score, metrics, feedback, structured records, attachments,
  and metadata, with hidden scorer-only data kept out of reflective proposer
  visibility.
- `docs/specs/agentic_library_user_journey.md:16-26` says users define the task
  and optimized artifact while Leaven owns optimization, execution, evidence,
  and recovery infrastructure.

What exists today is weaker.

## Findings

### L1-001: Public Runner And Scorer Are Sync-Only

- `id`: `L1-001`
- `severity`: `blocker`
- `surface`: Layer 1 public optimizer runner/scorer
- `target doc`: `surfaces/layer-1-user/public-api-ledger.md`

Evidence:

- `crates/leaven-run/src/builder.rs:28` defines `Runner<A, C>` as
  `Arc<dyn Fn(&A, &C) -> RunOutput + Send + Sync>`.
- `crates/leaven-run/src/builder.rs:29` defines `Scorer<A, C>` as
  `Arc<dyn for<'a> Fn(ScoreContext<'a, A, C>) -> Score + Send + Sync>`.
- `crates/leaven-run/src/builder.rs:138-150` exposes `.runner(...)` and
  `.score(...)` only as synchronous closures.
- `crates/leaven-run/src/evaluator.rs:97-102` calls the runner and scorer
  synchronously inside an async evaluator.
- `docs/specs/gepa_public_private_surface.md:987-1039` specifies the intended
  `CandidateRunner` and `Scorer` contracts as async and result-bearing.

Promised behavior:

Ordinary users can run real LM programs, model judges, subprocess evaluators, and
agents through the same public builder without learning engine internals. The
spec's canonical runner/scorer contract is async because execution and scoring
may await providers, remote workspaces, tools, or model judges.

Actual behavior:

The public builder only accepts sync closures. The evaluator serially calls
`runner(artifact, case)` and `scorer(ScoreContext { ... })`. There is no async
runner, no async scorer, no `Result`-returning scorer, no scoring-stage cost
return, and no built-in bounded concurrency at the Layer 1 surface.

Why it matters:

Real Layer 1 workloads are naturally async. LM calls, agent turns, subprocess
execution, sandbox lifecycle, model-judge scoring, and external verifiers cannot
be represented honestly by this API. Users are pushed toward hidden runtimes,
`block_on`, process spawning, or non-Leaven escape hatches before they can even
start a normal optimizer run. That means the ordinary surface does not actually
prove the intended LM/agent/program path.

Correction direction:

Hard-cut the product builder to an async runner/scorer lowering path. The
durable contract should be close to the spec:

- a `CandidateRunner` that receives a typed candidate/case context and returns
  `Result<CandidateRun<O>, CandidateRunError<O>>`;
- a `Scorer` that receives a typed `ScoreContext` and returns
  `Result<Metered<Score>, ScoreError>`;
- explicit bounded concurrency for case evaluation;
- sync scalar conveniences only as adapters into the async path, not as a second
  semantic lane.

Do not keep a public old/new split. Layer 1 should teach one real path.

### L1-002: GEPA Reflection Is A Fixed-Edit Fixture, Not A Real LM/Agent Reflector

- `id`: `L1-002`
- `severity`: `blocker`
- `surface`: Layer 1 GEPA reflection and public examples
- `target doc`: `surfaces/layer-1-user/examples-and-end-to-end-proof.md`
- `target doc`: `cross-cutting/lm-and-cache-surface.md`

Evidence:

- `examples/p8_aime_gepa/src/main.rs:91-93` installs
  `ReflectiveMutation::new(AimePromptEdit::ReplaceSystem(OPTIMIZED.to_owned()))`.
- `crates/leaven-gepa/src/proposer.rs:21-28` names `ReflectiveMutation` as a
  deterministic fixture and stores one edit.
- `crates/leaven-gepa/src/proposer.rs:40-47` ignores artifact, surface, and part,
  then returns the stored edit.
- `crates/leaven-gepa/src/proposer.rs:50-56` exposes additional placeholder
  names, `ReflectiveMutationConfig` and `SystemAwareMerge`.
- `crates/leaven-gepa/src/optimizer.rs:560-563` calls
  `self.reflector.propose_edit(&artifact, &self.surface, &part)`.
- `docs/specs/gepa_optimizer_surface.md:322-341` says ordinary GEPA reflection
  should evaluate a feedback minibatch, extract/capture feedback assessment IDs,
  propose edits, record causal and `informed_by` provenance, and then evaluate
  children.
- `docs/specs/gepa_optimizer_surface.md:463-483` lists reflection inputs:
  parent candidate id, selected part, part view, screening assessment IDs,
  casewise evidence, attribution evidence, lineage summary, objective/background,
  command evidence, transcript refs, validation/apply errors, and previous
  candidate summaries.

Promised behavior:

The ordinary GEPA path should be able to swap in an LM or agent reflector that
reads selected trace/evidence/feedback and proposes edits. The public example
should prove real reflection, at least with a mock LM/agent, before live provider
spend.

Actual behavior:

The AIME public example uses a hard-coded edit fixture. The GEPA reflector trait
currently sees only `artifact`, `surface`, and `part`, and the built-in
`ReflectiveMutation` ignores even those. It cannot inspect scoring feedback,
casewise evidence, trace lines, hidden split policy, run history, candidate
lineage, or budget.

Why it matters:

The current path can show "GEPA improved the score" while no reflective mutation
occurred. That is exactly the false positive the review tree was created to
catch: the example exercises builder mechanics but not the library capability
the user actually cares about. A future implementor could see green examples and
still have no working LM/agent reflection seam.

Correction direction:

Reserve `ReflectiveMutation` for a real async reflection stage. Rename or move
the fixed edit fixture to something explicit such as `FixedEditProposer` under
tests/examples. The real reflection request should include:

- selected parent candidate;
- selected surface part and current part view;
- selected scored trace/evidence and casewise feedback;
- optional attribution for the part;
- objective/background prompt text;
- lineage summary and prior candidate summaries;
- explicit budget/cost surface;
- scoped graph/evidence access or pre-lowered evidence payloads;
- causal and `informed_by` provenance output.

This can be implemented through the engine `Proposer<P>` seam or through an
equally honest GEPA reflector trait, but it must be async and evidence-aware.

### L1-003: The Live AIME Solver Bypasses Leaven LM And Cache

- `id`: `L1-003`
- `severity`: `high`
- `surface`: Live LM execution, public example proof, cache policy
- `target doc`: `surfaces/layer-1-user/examples-and-end-to-end-proof.md`
- `target doc`: `cross-cutting/lm-and-cache-surface.md`

Evidence:

- `examples/p8_aime_gepa/README.md:23-33` presents an OpenAI solver path as an
  opt-in runner/provider swap over the same public builder surface.
- `examples/p8_aime_gepa/Cargo.toml:12-16` depends on `leaven`, `futures`,
  `serde`, and `serde_json`, but not `leaven-lm-openai` or `leaven-lm-cache`.
- `examples/p8_aime_gepa/src/main.rs:271-274` switches to `run_openai_solver`
  when `LEAVEN_AIME_LIVE_OPENAI` is present.
- `examples/p8_aime_gepa/src/main.rs:293-301` spawns a Python script with
  `Command::new(...)`.
- `examples/p8_aime_gepa/scripts/openai_solver.py:24-38` reads
  `OPENAI_API_KEY`, builds a raw Responses API request, and posts to
  `https://api.openai.com/v1/responses` through `urllib`.
- `crates/leaven-lm/src/model.rs:9-21` defines the provider-neutral `Lm` trait.
- `crates/leaven-lm-openai/src/client.rs:10-21` defines `OpenAiLm`, a Responses
  API implementation of `Lm`.

Promised behavior:

The live OpenAI path should prove that ordinary users can swap in a real LM
provider through Leaven's provider-neutral LM interface and cache policy, without
rewriting the example outside the library.

Actual behavior:

The live solver path shells out to a Python script that calls OpenAI directly.
It does not exercise `leaven-lm`, `OpenAiLm`, `LmRequest`, `LmResponse`,
`TokenUsage`, `leaven-lm-cache`, cache keys, cache policy, or LM-call budget
integration.

Why it matters:

This is a public proof failure. A user can run a live benchmark and believe
Leaven's LM substrate works, while the code path bypasses that substrate. This
also hides the fact that there is no ordinary way to configure separate solver
LM and reflector LM roles with independent caching and cost accounting.

Correction direction:

The AIME example should route live solver calls through `OpenAiLm` implementing
`Lm`, with `LmRequest` built from the prompt/case and a Leaven-owned cache
configuration. The reflector should also be an LM/agent-backed Leaven component,
not a fixed edit. Python may remain as a dataset materialization helper, but it
must not be the canonical live provider path for the optimizer example.

### L1-004: Cache Policy Leaks As A Public Wrapper And Is Not Wired Through Runs

- `id`: `L1-004`
- `severity`: `high`
- `surface`: LM/runtime/cache ergonomics
- `target doc`: `cross-cutting/lm-and-cache-surface.md`

Evidence:

- `docs/specs/lm_runtime_and_response_cache.md:20-27` teaches public user code
  that manually constructs `OpenAiLm::from_env(...)`, then wraps it in
  `CachedLm::read_write(...)`.
- `crates/leaven-lm-cache/src/cached.rs:6-17` exposes `CachedLm<M, C>` as an LM
  wrapper holding `inner`, `cache`, and `policy`.
- `crates/leaven-lm-cache/src/cached.rs:53-87` implements per-call cache policy
  dispatch manually inside the wrapper.
- `crates/leaven-lm-cache/src/lib.rs:9-18` re-exports `CachedLm` in the public
  crate and prelude.
- `crates/leaven/src/prelude.rs:48-49` re-exports `leaven_lm_cache::prelude::*`
  when `lm-cache` is enabled.
- `crates/leaven-run/src/evaluator.rs:61-63` always returns
  `CachePolicy::Never` for the public scoring evaluator.

Promised behavior:

Ordinary users should configure cache policy as a capability of their LM runtime
or optimization run. The lower-level cache traits and stores should remain
available to power users, but Layer 1 should not require users to think in
wrapper stacks.

Actual behavior:

The public shape teaches users to manually wrap an LM in `CachedLm`. At the same
time, the `leaven-run` scoring evaluator does not expose or honor cache policy
and always disables the engine evaluation cache. There is no integrated Layer 1
place to say "use read/write response cache for solver calls and reflector
calls".

Why it matters:

Caching is central for expensive LM/agent optimization. The current shape makes
response caching an implementation composition detail users must learn, while
evaluation caching is unavailable in the ordinary builder. That creates a
confused mental model: users see cache types, but the main run path does not
actually give them a coherent cache policy story.

Correction direction:

Keep `LmCacheStore`, `LmCachePolicy`, and deterministic key semantics as
advanced pieces. Add a Layer 1 runtime/cache configuration surface that describes
capabilities:

```rust
let runtime = LmRuntime::openai("gpt-4.1-mini")
    .cache(LmCachePolicy::ReadWrite)
    .build_from_env()?;
```

The exact names can change. The important correction is that ordinary examples
configure cache policy on solver/reflector runtime roles, not by manually
stacking wrappers. The public scoring/evaluation path should also expose
evaluation cache policy when the artifact/case identities make reuse sound.

### L1-005: Score And RunOutput Are Too Thin For Real Feedback Evidence

- `id`: `L1-005`
- `severity`: `high`
- `surface`: Scoring, natural-language feedback, traces, and result evidence
- `target doc`: `surfaces/layer-1-user/evaluation-datasets-results.md`

Evidence:

- `crates/leaven-run/src/evidence.rs:3-10` defines `RunOutput` as an output
  string plus `Vec<String>` trace lines.
- `crates/leaven-run/src/evidence.rs:23-32` defines `Score` as one `f64`, one
  feedback string, and `Vec<(String, String)>` structured feedback.
- `crates/leaven-run/src/evidence.rs:46-53` exposes `ScoreContext` as public
  fields: `artifact`, `case`, and `output`.
- `crates/leaven-run/src/evaluator.rs:106-115` clones trace lines, flattens
  structured feedback into strings, and creates `ScoredFeedbackEvidence`.
- `docs/specs/gepa_public_private_surface.md:930-985` says `ScoreContext` should
  be a typed view with candidate, case, output, run error, trace, history, and
  budget accessors, without exposing graph internals.
- `docs/specs/gepa_public_private_surface.md:1115-1155` says `Score` carries
  primary score, metrics, natural-language feedback, structured feedback, file
  and directory attachments, evidence refs, and metadata.

Promised behavior:

Users should be able to score any trace, including full history or rich
agent/program evidence, and return natural-language feedback plus structured
artifacts that reflection can use. A score may include metrics, attachments,
transcripts, logs, JSON, and metadata, not just one float and one string.

Actual behavior:

Layer 1 scores are scalar-only with one feedback string. Structured feedback is
not structured durable evidence; it is appended to trace text. `RunOutput` has no
typed output, attachments, errors, cost, transcript refs, or workspace evidence.
`ScoreContext` is not a graph-backed public view and has no history, budget,
split, hidden target, or run-error access.

Why it matters:

The optimizer cannot honestly use rich feedback. Model-judge rationales,
compiler diagnostics, agent transcripts, file artifacts, screenshots, verifier
logs, and multi-metric outcomes either disappear, become plain strings, or live
outside Leaven. This blocks exactly the "score function with natural-language
feedback" ordinary-user promise that drove the public surface design.

Correction direction:

Implement the rich score contract from the spec. Minimum direction:

- `Score` supports a primary comparable score, named metrics with directions,
  feedback entries, attachments, and metadata.
- `ScoreContext` becomes an accessor-based typed view with candidate, case,
  output, run error, trace, history, and budget.
- runner output can carry typed output, trace bundle, attachments, and cost.
- scoring functions can return `Metered<Score>` so scorer LM/tool cost charges
  exactly once.
- attachments are staged into evidence/artifact storage before reports cite
  them.

Do not add a parallel `.score_with_feedback(...)` API. The ordinary `.score(...)`
path should become the rich path.

### L1-006: Train/Validation/Test Lowering Uses Positional Case IDs And Drops Dataset Identity

- `id`: `L1-006`
- `severity`: `medium`
- `surface`: Dataset/split semantics and result investigation
- `target doc`: `surfaces/layer-1-user/evaluation-datasets-results.md`

Evidence:

- `crates/leaven-run/src/builder.rs:214-216` combines train, validation, and
  test vectors, then builds a `Dataset::from_ordered(...)` and generated splits.
- `crates/leaven-run/src/builder.rs:302-319` builds `CaseSet` partitions using
  dense positional `CaseId::from_index(...)`.
- `crates/leaven-eval/src/dataset.rs:44-47` supports `Dataset::from_ordered`,
  which generates dense ordered ids.
- `crates/leaven-eval/src/dataset.rs:95-100` has an explicit-id builder path
  that can reject duplicate case ids.
- `examples/p8_aime_gepa/scripts/materialize_hf_aime.py:19-25` materializes
  AIME cases as `problem`, `answer`, `solution`, and `needs_modular`, with no
  stable dataset case id.
- `docs/specs/gepa_public_private_surface.md:650-652` says `.train`,
  `.validation`, and `.test` must reject duplicate case ids and
  default-disallowed overlap.
- `docs/specs/eval_lowering_detail.md:230-238` says a dataset case is a unit of
  work, not necessarily a labeled example, and targets are optional.

Promised behavior:

Layer 1 should support ordinary train/validation/test semantics with stable case
identity, duplicate detection, split roles, and reportable case-level outcomes.
Cases may be labeled or unlabeled, but they should still be durable units of
work.

Actual behavior:

The current builder accepts plain `Vec<C>` and assigns positional dense ids
after concatenating splits. This is convenient for toy data but loses external
dataset identity. The AIME materializer also drops source ids. The explicit
`DatasetBuilder::case(id, case)` path exists in `leaven-eval`, but it is not the
ordinary `leaven-run` public path.

Why it matters:

A future implementor cannot reliably inspect "which exact AIME problem improved
or regressed" across reruns if the case id is just its current vector position.
Duplicate cases across splits are not naturally refused by user-supplied ids.
Resume, cache reuse, report comparison, and paper reproduction all become weaker
because the public builder hides identity loss under an ergonomic vector API.

Correction direction:

Add a Layer 1 case/suite input path with stable ids and optional targets. Plain
`Vec<C>` can remain only as an explicit dense-id convenience. The primary public
path for real examples should accept `Case`, `LmCase`, `CaseSuite`, or an
equivalent user-id-preserving type, then lower into `Dataset`,
`DatasetSplits`, and engine `CaseSet` without throwing away source identity.

### L1-007: The Ordinary Prelude Exports Engine Internals Next To Layer 1 Types

- `id`: `L1-007`
- `severity`: `medium`
- `surface`: Umbrella import experience and ordinary-user mental model
- `target doc`: `surfaces/layer-1-user/public-api-ledger.md`

Evidence:

- `crates/leaven/src/prelude.rs:8-12` re-exports `CachePolicy`, `Engine`,
  `Evaluator`, `Materializer`, `Optimizer`, `Population`, `Proposer`,
  `RunContext`, `RunGraphView`, `Stopper`, and `TrustPolicy`.
- `crates/leaven/src/prelude.rs:21-24` re-exports Layer 1 types such as
  `OptimizationReport`, `OptimizeResult`, `RunOutput`, `Score`,
  `ScoreContext`, and `optimize` in the same prelude.
- `docs/specs/gepa_public_private_surface.md:69-83` says ordinary Layer 1 users
  should not touch `RunGraph`, `Actor`, `ReadScope`, `TrustPolicy`,
  `EvaluationRequest`, selectors, `Population`, or `EvidenceStore`.
- `docs/specs/gepa_public_private_surface.md:877-890` says docs should teach
  Layer 1 before internal types and keep examples short enough that users can
  see how to run GEPA without learning the engine.

Promised behavior:

The ordinary import experience should guide users toward `optimize`, cases,
runner/scorer, GEPA, budget, and result/report. Engine internals remain
available for optimizer authors but should not be taught as the common path.

Actual behavior:

`leaven::prelude::*` imports ordinary product-builder types and engine-author
types together. A user following the prelude has `RunContext`, `RunGraphView`,
`TrustPolicy`, `Evaluator`, `Proposer`, and `Population` in scope before they
know whether those concepts are meant for them.

Why it matters:

This is a naming and routing leak. The public docs say ordinary users should not
learn the engine, but the first import surface presents engine concepts as
common imports. That makes future examples more likely to accidentally depend on
internals and makes the API feel like a bag of traits instead of an optimizer
library.

Correction direction:

Split the import surface:

- an ordinary prelude with `optimize`, `OptimizeBuilder`, `OptimizeResult`,
  `OptimizationReport`, `RunOutput`, `Score`, `ScoreContext`, `Budget`, cases,
  and common GEPA/runtime constructors;
- an advanced/engine prelude for `Engine`, `RunContext`, `RunGraphView`,
  `Evaluator`, `Proposer`, `TrustPolicy`, `Population`, and lower-level
  optimizer-author machinery.

Do not remove advanced access. Just stop teaching it as Layer 1.

### L1-008: The Result Facade Is A Thin Snapshot, Not The Honest Report Surface

- `id`: `L1-008`
- `severity`: `medium`
- `surface`: Result/report facade
- `target doc`: `surfaces/layer-1-user/evaluation-datasets-results.md`

Evidence:

- `crates/leaven-run/src/result.rs:6-18` defines `OptimizeResult<A>` with
  `run_id`, `best`, `best_artifact`, `seed_artifact`, and `report`.
- `crates/leaven-run/src/result.rs:35-61` defines `OptimizationReport` with
  dataset/split fingerprints, budget/cost, aggregate train/validation/test
  scores, one `EvaluationReport`, and event names as strings.
- `crates/leaven-run/src/builder.rs:439-491` builds the report by walking
  `engine.view()`, computing latest train averages, cloning the best artifact,
  and collecting `event_name(...)` strings.
- `crates/leaven-run/src/builder.rs:637-645` projects per-case reports into
  score, feedback string, and trace strings.
- `docs/specs/gepa_public_private_surface.md:1184-1228` says the ordinary result
  should expose stop reason, budget, report, optional GEPA summary, public event
  summaries, and graph access only on an explicit advanced result type or
  feature.
- `docs/specs/agentic_library_user_journey.md:229-243` says the final result
  should answer what the best candidate is, why it was selected, which cases
  improved or regressed, what changed, what the agent did, what proposer tried,
  cost, and resume/reproduce status.

Promised behavior:

The result facade should be honest enough that ordinary users can inspect the
best candidate, why it won, split outcomes, case-level improvements/regressions,
cost, provider/runtime evidence, proposal attempts, and resume/reproduce status
without learning `RunGraph`.

Actual behavior:

The current facade reports a cloned best artifact, aggregate scores, split
summaries, and event names. It does not expose stop reason, GEPA summary,
candidate lineage, changed artifact parts/files, proposal attempts, repair
feedback, cache hits/misses, provider transcript refs, attachment refs, or
resume status. Events are strings rather than public event summaries.

Why it matters:

The run may complete, but the result does not give a future user or implementor
enough information to understand whether the optimizer worked. It also risks
becoming a second shallow truth copied out of the graph instead of a clear
graph-backed report view. This is especially weak for LM/agent optimization,
where the "why" lives in traces, feedback, proposals, and cost records.

Correction direction:

Keep the result facade graph-backed and ordinary-user-oriented. It should expose
best id/artifact, stop reason, budget snapshot, evaluation report, optional GEPA
summary, public event summaries, split/case deltas, cost/cache summaries, and
refs to staged evidence/attachments. Graph access should remain an explicit
advanced result mode, not required for ordinary best/report inspection.

## Non-Finding

### NF-L1-001: P0-P7 Are Milestone Proofs, Not The Main Layer 1 Public Example

- `id`: `NF-L1-001`
- `surface`: examples routing

Evidence:

- `docs/testing/README.md:36-39` says milestone examples are workspace packages
  and must be run through `just milestone-*` recipes.
- `docs/specs/milestone_examples_behavioral_contract.md:31-39` says each
  milestone is both a runnable package and a library-contract proof.
- `docs/specs/milestone_examples_behavioral_contract.md:1090-1104` defines the
  milestone completion set for P0-P4 and related gates.
- `examples/p8_aime_gepa/README.md:1-9` explicitly labels P8 as the AIME GEPA
  public API example.

Reason:

I did not treat `examples/p0_graph_skeleton` through
`examples/p7_self_optimization_kernel` as the denominator for the ordinary
Layer 1 user surface. They are useful behavior gates and implementation
pressure tests, but `p8_aime_gepa` is the explicit high-level public API example
for this audit question. The Layer 1 findings therefore attach primarily to
`leaven-run`, the umbrella imports, LM/cache surfaces, and `p8_aime_gepa`.

## Implementation Guidance For The Next Pass

The smallest honest correction is not to patch the AIME example around the
current API. The public API has to become capable enough that the example can be
thin.

Recommended order:

1. Cut `leaven-run` runner/scorer to the async rich-score path.
2. Add stable case/suite input so train/validation/test reports preserve case
   identity.
3. Add an ordinary LM runtime/cache configuration surface for solver and
   reflector roles.
4. Replace fixed `ReflectiveMutation` in ordinary examples with a real mock-LM
   or mock-agent reflector that consumes scored feedback.
5. Route live AIME solver and reflector through `leaven-lm-openai` and Leaven
   cache policy, not a Python provider script.
6. Expand `OptimizeResult`/`OptimizationReport` into graph-backed public report
   views that answer the user-facing questions without exposing `RunGraph`.
7. Split ordinary and advanced preludes so examples teach the public surface
   before internal machinery.

Hard cutover rule:

Do not keep compatibility shims for the current sync runner/scorer or fixed
reflection names in the ordinary path. If a compatibility helper is temporarily
useful for tests, name it as scaffolding and keep it out of the public Layer 1
examples.

## Files I Did Not Change

This report is documentation only. I did not edit product code, existing audit
ledgers, or other report paths. I did not run broad verification.
